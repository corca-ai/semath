import { readFile, stat, writeFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
  type SemathQuery,
  type SemathWorkerRequest,
  type UpdateResult,
} from "../packages/protocol/src/index";
import {
  SemathWorkerHost,
  type SemathWorkerOperations,
} from "../packages/worker/src/host";
import {
  buildPerformanceDocuments,
  editPerformanceDocument,
  semanticallyEditPerformanceDocument,
  type PerformanceFixtureDocument,
} from "./performance-fixtures";
import { shouldEnforceTiming } from "./performance-budget-policy";

const DOCUMENT_COUNT = positiveInteger("SEMATH_BUDGET_DOCUMENTS", 60);
const STABLE_HOST_GATE = process.env.SEMATH_BUDGET_STABLE === "1";
const TIMING_GATE = shouldEnforceTiming(process.env, DOCUMENT_COUNT);
const DELTA_RUNS = positiveInteger(
  "SEMATH_BUDGET_DELTA_RUNS",
  DOCUMENT_COUNT >= 500 ? 10 : 30,
);
// Cold WASM initialization is sensitive to shared-runner CPU allocation. Keep
// the absolute gate well below the previous 5s ceiling while leaving the rapid
// edit/query budgets strict enough to catch interactive regressions.
const COLD_BUDGET_MS = DOCUMENT_COUNT >= 500 ? 5_000 : 2_500;
// Hosted runners occasionally lose a single edit to a 40–60ms scheduler
// pause. Keep the p95 gate below one 60Hz frame plus that observed jitter,
// while the dedicated syntax/query measurements remain visible for diagnosis.
const DELTA_P95_BUDGET_MS = STABLE_HOST_GATE ? (DOCUMENT_COUNT >= 500 ? 50 : 25) : 75;
const SEMANTIC_DELTA_BUDGET_MS = 50;
const QUERY_P95_BUDGET_MS = 8;
const RETAINED_RSS_BUDGET_BYTES = (DOCUMENT_COUNT >= 500 ? 192 : 112) * 1024 * 1024;
const MAX_AFFECTED_DOCUMENTS = 2;
const MAX_TRANSFER_BYTES = 16 * 1024;
const MAX_LAW_RULES_PER_DOCUMENT = 20;

const sources = buildPerformanceDocuments(DOCUMENT_COUNT);
const main = {
  content: sources.map((source) => `\\input{${source.path}}`).join("\n"),
  documentVersion: 1,
  fileId: "main",
  language: "latex" as const,
  path: "main.tex",
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const wasm = await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url));
const wasmArtifactBytes = (await stat(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)))
  .size;
const rssBefore = residentBytes();
const coldStarted = performance.now();

const syntax = new LatexSyntaxService();
const syntaxColdStarted = performance.now();
syntax.reset({ documents: [main, ...sources] });
const syntaxColdMs = performance.now() - syntaxColdStarted;
const adapterColdStarted = performance.now();
const documents = [main, ...sources].map((source) => {
  const fileSyntax = syntax.getFile(source.fileId);
  if (!fileSyntax) throw new Error(`missing syntax for ${source.fileId}`);
  return adaptWasmtexDocument({
    content: source.content,
    language: source.language ?? "latex",
    syntax: fileSyntax,
  });
});
const adapterColdMs = performance.now() - adapterColdStarted;
const snapshot: ProjectSnapshot = {
  documents,
  epoch: "quality-budget",
  inventoryVersion: 1,
  mainFileId: main.fileId,
  projectId: "quality-budget",
  protocolVersion: SEMATH_PROTOCOL_VERSION,
};
const initialTransferBytes = encodedLength(snapshot);

const worker = createWorkerHost(async () => {
  await init({ module_or_path: wasm });
  return operations(new SemathEngine());
});
const engineColdStarted = performance.now();
const initial = await worker.request<UpdateResult>({
  id: worker.nextId(),
  kind: "reset",
  snapshot,
});
const engineColdMs = performance.now() - engineColdStarted;
const coldMs = performance.now() - coldStarted;
assertCounters(initial, DOCUMENT_COUNT + 1);

const deltaDurations: number[] = [];
const syntaxDurations: number[] = [];
const queryDurations = new Map<SemathQuery["kind"], number[]>();
let peakRss = residentBytes();
let maxAffected = 0;
let maxTransferBytes = 0;
let inventoryVersion = snapshot.inventoryVersion;
let currentSource: PerformanceFixtureDocument = sources[0]!;
let current = documents[1]!;

for (let run = 0; run < DELTA_RUNS; run += 1) {
  inventoryVersion += 1;
  currentSource = editPerformanceDocument(currentSource, run);

  const started = performance.now();
  const syntaxStarted = performance.now();
  const fileSyntax = syntax.upsert(currentSource);
  syntaxDurations.push(performance.now() - syntaxStarted);
  current = adaptWasmtexDocument({
    content: currentSource.content,
    language: "latex",
    syntax: fileSyntax,
  });
  const envelope: ChangeEnvelope = {
    analysisGeneration: run + 1,
    changes: [{ document: current, kind: "upsert" }],
    epoch: snapshot.epoch,
    inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
  maxTransferBytes = Math.max(maxTransferBytes, encodedLength(envelope));
  const update = await worker.request<UpdateResult>({
    changes: envelope,
    id: worker.nextId(),
    kind: "change",
  });
  deltaDurations.push(performance.now() - started);
  peakRss = Math.max(peakRss, residentBytes());
  if (update.analyzedFileIds.length !== 0) {
    throw new Error("budget comment-only delta performed semantic analysis");
  }
  assertCounters(update, 0);

  for (const query of measuredQueries(currentSource)) {
    const envelope: QueryEnvelope = {
      analysisGeneration: run + 1,
      documentVersion: current.documentVersion,
      epoch: snapshot.epoch,
      inventoryVersion,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query,
    };
    const queryStarted = performance.now();
    await worker.request<QueryResult>({
      envelope,
      id: worker.nextId(),
      kind: "query",
      priority: "cursor",
    });
    const durations = queryDurations.get(query.kind) ?? [];
    durations.push(performance.now() - queryStarted);
    queryDurations.set(query.kind, durations);
  }
}

inventoryVersion += 1;
currentSource = semanticallyEditPerformanceDocument(currentSource);
const semanticStarted = performance.now();
const semanticSyntaxStarted = performance.now();
const semanticSyntax = syntax.upsert(currentSource);
syntaxDurations.push(performance.now() - semanticSyntaxStarted);
current = adaptWasmtexDocument({
  content: currentSource.content,
  language: "latex",
  syntax: semanticSyntax,
});
const semanticEnvelope: ChangeEnvelope = {
  analysisGeneration: DELTA_RUNS + 1,
  changes: [{ document: current, kind: "upsert" }],
  epoch: snapshot.epoch,
  inventoryVersion,
  protocolVersion: SEMATH_PROTOCOL_VERSION,
};
maxTransferBytes = Math.max(maxTransferBytes, encodedLength(semanticEnvelope));
const semanticUpdate = await worker.request<UpdateResult>({
  changes: semanticEnvelope,
  id: worker.nextId(),
  kind: "change",
});
const semanticDeltaMs = performance.now() - semanticStarted;
peakRss = Math.max(peakRss, residentBytes());
maxAffected = semanticUpdate.analyzedFileIds.length;
if (maxAffected > MAX_AFFECTED_DOCUMENTS) {
  throw new Error(
    `budget semantic delta analyzed ${maxAffected} documents; expected at most ${MAX_AFFECTED_DOCUMENTS}`,
  );
}
if (
  !semanticUpdate.analyzedFileIds.includes(current.fileId) ||
  !semanticUpdate.analyzedFileIds.includes("main")
) {
  throw new Error("budget affected closure omitted the changed file or its dependent main file");
}
assertCounters(semanticUpdate, semanticUpdate.analyzedFileIds.length);

if (syntax.getStats().parseCount !== DOCUMENT_COUNT + 2 + DELTA_RUNS) {
  throw new Error("budget syntax parse counter did not advance exactly once per changed document");
}

const incremental = await worker.request<UpdateResult>({
  changes: {
    analysisGeneration: DELTA_RUNS + 2,
    changes: [],
    epoch: snapshot.epoch,
    inventoryVersion: inventoryVersion + 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  },
  id: worker.nextId(),
  kind: "change",
});
if (incremental.analyzedFileIds.length !== 0) {
  throw new Error("budget empty delta unexpectedly reanalyzed documents");
}

// The parity rebuild is a separate lifecycle, not a second live editor engine.
// Dispose the incremental worker first so this gate measures the maximum memory
// of either valid lifecycle instead of an artificial overlap of both.
await worker.dispose();
const clean = new SemathEngine();
const finalDocuments = documents.map((document) =>
  document.fileId === current.fileId ? current : document,
);
const cleanUpdate = resetEngine(clean, {
  ...snapshot,
  documents: finalDocuments,
  inventoryVersion: inventoryVersion + 1,
});
if (
  initial.stats.totalDocuments !== cleanUpdate.stats.totalDocuments ||
  initial.stats.recognizedLaws !== cleanUpdate.stats.recognizedLaws
) {
  throw new Error("budget incremental and clean rebuild summaries diverged");
}
clean.free();
const rssAfterDispose = residentBytes();
const retainedRssGrowth = Math.max(0, rssAfterDispose - rssBefore);

const deltaP95 = percentile(deltaDurations, 0.95);
const deltaMedian = percentile(deltaDurations, 0.5);
const syntaxP95 = percentile(syntaxDurations, 0.95);
const queryP95ByKind = Object.fromEntries(
  [...queryDurations].map(([kind, durations]) => [kind, percentile(durations, 0.95)]),
);
const queryP95 = Math.max(...Object.values(queryP95ByKind));
const peakRssGrowth = Math.max(0, Math.max(peakRss, rssAfterDispose) - rssBefore);
const syntaxStats = syntax.getStats() as ReturnType<LatexSyntaxService["getStats"]> & {
  lastInvalidatedDocuments?: number;
  lastTransferBytes?: number;
  notationNodes?: number;
  recoveredNodes?: number;
  snapshotBytes?: number;
};
const report = {
  affectedDocuments: maxAffected,
  adapterColdMs,
  analysis: initial.stats,
  coldMs,
  deltaMedianMs: deltaMedian,
  deltaP95Ms: deltaP95,
  documents: DOCUMENT_COUNT + 1,
  engineColdMs,
  fixtureFamilies: [...new Set(sources.map((source) => source.family))],
  initialTransferBytes,
  peakRssGrowthBytes: peakRssGrowth,
  queryP95ByKind,
  retainedRssGrowthBytes: retainedRssGrowth,
  semanticDeltaMs,
  syntax: {
    bytesPerNode:
      syntaxStats.notationNodes === undefined || syntaxStats.notationNodes === 0
        ? null
        : syntaxStats.snapshotBytes / syntaxStats.notationNodes,
    coldMs: syntaxColdMs,
    deltaP95Ms: syntaxP95,
    documents: syntaxStats.documents,
    lastInvalidatedDocuments: syntaxStats.lastInvalidatedDocuments ?? null,
    lastTransferBytes: syntaxStats.lastTransferBytes ?? null,
    notationNodes: syntaxStats.notationNodes ?? null,
    nodesPerDocument:
      syntaxStats.notationNodes === undefined || syntaxStats.documents === 0
        ? null
        : syntaxStats.notationNodes / syntaxStats.documents,
    parseCount: syntaxStats.parseCount,
    recoveredNodes: syntaxStats.recoveredNodes ?? null,
    snapshotBytes: syntaxStats.snapshotBytes ?? null,
  },
  transferBytes: maxTransferBytes,
  wasmArtifactBytes,
};
console.log(`budget metrics: ${JSON.stringify(report)}`);
const reportPath = process.env.SEMATH_BUDGET_REPORT;
if (reportPath) await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
if (TIMING_GATE && coldMs > COLD_BUDGET_MS) {
  throw new Error(`budget cold start ${coldMs.toFixed(2)}ms exceeded ${COLD_BUDGET_MS}ms`);
}
if (TIMING_GATE && deltaP95 > DELTA_P95_BUDGET_MS) {
  throw new Error(`budget delta p95 ${deltaP95.toFixed(2)}ms exceeded ${DELTA_P95_BUDGET_MS}ms`);
}
if (TIMING_GATE && semanticDeltaMs > SEMANTIC_DELTA_BUDGET_MS) {
  throw new Error(
    `budget semantic delta ${semanticDeltaMs.toFixed(2)}ms exceeded ${SEMANTIC_DELTA_BUDGET_MS}ms`,
  );
}
if (TIMING_GATE && queryP95 > QUERY_P95_BUDGET_MS) {
  throw new Error(`budget query p95 ${queryP95.toFixed(2)}ms exceeded ${QUERY_P95_BUDGET_MS}ms`);
}
if (initial.stats.lawRulesVisited > (DOCUMENT_COUNT + 1) * MAX_LAW_RULES_PER_DOCUMENT) {
  throw new Error(
    `budget law dispatch visited ${initial.stats.lawRulesVisited} rules for ${DOCUMENT_COUNT + 1} documents`,
  );
}
if (peakRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(`budget peak RSS growth ${peakRssGrowth}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`);
}
if (retainedRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(
    `budget retained RSS growth ${retainedRssGrowth}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`,
  );
}
if (
  syntaxStats.lastInvalidatedDocuments !== undefined &&
  syntaxStats.lastInvalidatedDocuments !== 1
) {
  throw new Error(
    `budget leaf edit transferred ${syntaxStats.lastInvalidatedDocuments} syntax documents`,
  );
}
if (maxTransferBytes > MAX_TRANSFER_BYTES) {
  throw new Error(`budget delta transfer ${maxTransferBytes}B exceeded ${MAX_TRANSFER_BYTES}B`);
}
console.log("budget OK");

function operations(engine: SemathEngine): SemathWorkerOperations {
  return {
    apply(changes) {
      return decodeUpdate(engine.applyChanges(encoder.encode(JSON.stringify(changes))));
    },
    dispose() {
      engine.free();
    },
    query(envelope) {
      return decodeQuery(engine.query(encoder.encode(JSON.stringify(envelope))));
    },
    reset(project) {
      return resetEngine(engine, project);
    },
  };
}

function resetEngine(engine: SemathEngine, project: ProjectSnapshot): UpdateResult {
  const { documents, ...metadata } = project;
  engine.beginReset(encoder.encode(JSON.stringify(metadata)));
  for (const document of documents) {
    engine.ingestResetDocument(encoder.encode(JSON.stringify(document)));
  }
  return decodeUpdate(engine.finishReset());
}

function createWorkerHost(createEngine: () => Promise<SemathWorkerOperations>) {
  let requestId = 0;
  const waiting = new Map<
    number,
    { reject: (error: Error) => void; resolve: (value: unknown) => void }
  >();
  const host = new SemathWorkerHost(createEngine, (response) => {
    const waiter = waiting.get(response.id);
    if (!waiter) return;
    waiting.delete(response.id);
    if (response.kind === "result") waiter.resolve(response.result);
    else if (response.kind === "disposed") waiter.resolve(undefined);
    else if (response.kind === "error") waiter.reject(new Error(response.error.message));
    else waiter.reject(new Error(`unexpected worker response: ${response.kind}`));
  });
  return {
    async dispose() {
      const id = ++requestId;
      const disposed = new Promise<void>((resolve, reject) => {
        waiting.set(id, { reject, resolve: () => resolve() });
      });
      host.accept({ id, kind: "dispose" });
      await disposed;
    },
    nextId() {
      return ++requestId;
    },
    request<T>(request: Exclude<SemathWorkerRequest, { kind: "cancel" | "dispose" }>) {
      const response = new Promise<T>((resolve, reject) => {
        waiting.set(request.id, {
          reject,
          resolve: (value) => resolve(value as T),
        });
      });
      host.accept(request);
      return response;
    },
  };
}

function decodeUpdate(bytes: Uint8Array): UpdateResult {
  return JSON.parse(decoder.decode(bytes)) as UpdateResult;
}

function decodeQuery(bytes: Uint8Array): QueryResult {
  return JSON.parse(decoder.decode(bytes)) as QueryResult;
}

function encodedLength(value: unknown): number {
  return encoder.encode(JSON.stringify(value)).byteLength;
}

function measuredQueries(document: PerformanceFixtureDocument): readonly SemathQuery[] {
  const target = { fileId: document.fileId, offset: document.queryOffset };
  return [
    { ...target, kind: "selection" },
    { ...target, kind: "semanticView" },
    { ...target, kind: "definition" },
    { ...target, kind: "references" },
    { ...target, kind: "prepareRename" },
    { ...target, kind: "rename", newName: "renamed" },
  ];
}

function positiveInteger(name: string, fallback: number): number {
  const source = process.env[name];
  if (source === undefined) return fallback;
  const value = Number(source);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function residentBytes(): number {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      return process.memoryUsage().rss;
    } catch (error) {
      if (!(error instanceof Error) || !("errno" in error) || error.errno !== 4) throw error;
    }
  }
  throw new Error("unable to read resident memory after interrupted system calls");
}

function assertCounters(update: UpdateResult, analyzedDocuments: number) {
  if (update.stats.analyzedDocuments !== analyzedDocuments) {
    throw new Error("budget analyzed-document counter is inconsistent");
  }
  for (const key of [
    "semanticNodes",
    "constraints",
    "lawRulesVisited",
    "semanticOccurrences",
    "semanticEntities",
    "semanticClaims",
    "semanticEvidence",
    "semanticDependencyEdges",
    "invalidatedSemanticClaims",
    "semanticCandidates",
  ] as const) {
    if (update.stats[key] < 0 || !Number.isFinite(update.stats[key])) {
      throw new Error(`budget ${key} counter is invalid`);
    }
  }
}

function percentile(values: readonly number[], fraction: number): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)]!;
}
