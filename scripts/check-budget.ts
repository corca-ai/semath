import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService, type LatexDocumentInput } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
  type SemathWorkerRequest,
  type UpdateResult,
} from "../packages/protocol/src/index";
import {
  SemathWorkerHost,
  type SemathWorkerOperations,
} from "../packages/worker/src/host";

const DOCUMENT_COUNT = positiveInteger("SEMATH_BUDGET_DOCUMENTS", 60);
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
const DELTA_P95_BUDGET_MS = 75;
const QUERY_P95_BUDGET_MS = 3;
const RETAINED_RSS_BUDGET_BYTES = (DOCUMENT_COUNT >= 500 ? 192 : 112) * 1024 * 1024;
const MAX_AFFECTED_DOCUMENTS = 2;
const MAX_TRANSFER_BYTES = 16 * 1024;

const sources = Array.from({ length: DOCUMENT_COUNT }, (_, index) => ({
  content: [
    `Let p${index} denote probability and let A${index} and B${index} be events.`,
    `$p${index} = \\frac{\\mathbb{P}(A${index} \\cap B${index})}{\\mathbb{P}(B${index})}$`,
  ].join("\n"),
  documentVersion: 1,
  fileId: `section-${index}`,
  language: "latex" as const,
  path: `section-${index}.tex`,
}));
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
const rssBefore = process.memoryUsage().rss;
const coldStarted = performance.now();

const syntax = new LatexSyntaxService();
syntax.reset({ documents: [main, ...sources] });
const documents = [main, ...sources].map((source) => {
  const fileSyntax = syntax.getFile(source.fileId);
  if (!fileSyntax) throw new Error(`missing syntax for ${source.fileId}`);
  return adaptWasmtexDocument({
    content: source.content,
    language: source.language,
    syntax: fileSyntax,
  });
});
const snapshot: ProjectSnapshot = {
  documents,
  epoch: "quality-budget",
  inventoryVersion: 1,
  mainFileId: main.fileId,
  projectId: "quality-budget",
  protocolVersion: SEMATH_PROTOCOL_VERSION,
};

const worker = createWorkerHost(async () => {
  await init({ module_or_path: wasm });
  return operations(new SemathEngine());
});
const initial = await worker.request<UpdateResult>({
  id: worker.nextId(),
  kind: "reset",
  snapshot,
});
const coldMs = performance.now() - coldStarted;
assertCounters(initial, DOCUMENT_COUNT + 1);

const deltaDurations: number[] = [];
const syntaxDurations: number[] = [];
const queryDurations: number[] = [];
let peakRss = process.memoryUsage().rss;
let maxAffected = 0;
let maxTransferBytes = 0;
let inventoryVersion = snapshot.inventoryVersion;
let currentSource: LatexDocumentInput = sources[0]!;
let current = documents[1]!;

for (let run = 0; run < DELTA_RUNS; run += 1) {
  inventoryVersion += 1;
  currentSource = {
    ...currentSource,
    content: `${sources[0]!.content}\n% delta ${run}`,
    documentVersion: currentSource.documentVersion + 1,
  };

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
  peakRss = Math.max(peakRss, process.memoryUsage().rss);
  maxAffected = Math.max(maxAffected, update.analyzedFileIds.length);
  if (update.analyzedFileIds.length > MAX_AFFECTED_DOCUMENTS) {
    throw new Error(
      `budget delta analyzed ${update.analyzedFileIds.length} documents; expected at most ${MAX_AFFECTED_DOCUMENTS}`,
    );
  }
  if (!update.analyzedFileIds.includes(current.fileId) || !update.analyzedFileIds.includes("main")) {
    throw new Error("budget affected closure omitted the changed file or its dependent main file");
  }
  assertCounters(update, update.analyzedFileIds.length);

  const query: QueryEnvelope = {
    analysisGeneration: run + 1,
    documentVersion: current.documentVersion,
    epoch: snapshot.epoch,
    inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: {
      fileId: current.fileId,
      kind: "semanticView",
      offset: current.content.indexOf("$p") + 1,
    },
  };
  const queryStarted = performance.now();
  await worker.request<QueryResult>({
    envelope: query,
    id: worker.nextId(),
    kind: "query",
    priority: "cursor",
  });
  queryDurations.push(performance.now() - queryStarted);
}

if (syntax.getStats().parseCount !== DOCUMENT_COUNT + 1 + DELTA_RUNS) {
  throw new Error("budget syntax parse counter did not advance exactly once per changed document");
}

const incremental = await worker.request<UpdateResult>({
  changes: {
    analysisGeneration: DELTA_RUNS + 1,
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

const clean = new SemathEngine();
const finalDocuments = documents.map((document) =>
  document.fileId === current.fileId ? current : document,
);
const cleanUpdate = decodeUpdate(
  clean.resetProject(
    encoder.encode(
      JSON.stringify({
        ...snapshot,
        documents: finalDocuments,
        inventoryVersion: inventoryVersion + 1,
      }),
    ),
  ),
);
if (
  initial.stats.totalDocuments !== cleanUpdate.stats.totalDocuments ||
  initial.stats.recognizedLaws !== cleanUpdate.stats.recognizedLaws
) {
  throw new Error("budget incremental and clean rebuild summaries diverged");
}
clean.free();
await worker.dispose();

const deltaP95 = percentile(deltaDurations, 0.95);
const deltaMedian = percentile(deltaDurations, 0.5);
const syntaxP95 = percentile(syntaxDurations, 0.95);
const queryP95 = percentile(queryDurations, 0.95);
const peakRssGrowth = Math.max(0, peakRss - rssBefore);
if (coldMs > COLD_BUDGET_MS) {
  throw new Error(`budget cold start ${coldMs.toFixed(2)}ms exceeded ${COLD_BUDGET_MS}ms`);
}
if (deltaP95 > DELTA_P95_BUDGET_MS) {
  throw new Error(`budget delta p95 ${deltaP95.toFixed(2)}ms exceeded ${DELTA_P95_BUDGET_MS}ms`);
}
if (queryP95 > QUERY_P95_BUDGET_MS) {
  throw new Error(`budget query p95 ${queryP95.toFixed(2)}ms exceeded ${QUERY_P95_BUDGET_MS}ms`);
}
if (peakRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(`budget peak RSS growth ${peakRssGrowth}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`);
}
if (maxTransferBytes > MAX_TRANSFER_BYTES) {
  throw new Error(`budget delta transfer ${maxTransferBytes}B exceeded ${MAX_TRANSFER_BYTES}B`);
}
console.log(
  [
    "budget OK:",
    `documents=${DOCUMENT_COUNT + 1}`,
    `cold=${coldMs.toFixed(2)}ms`,
    `syntax-p95=${syntaxP95.toFixed(2)}ms`,
    `delta-median=${deltaMedian.toFixed(2)}ms`,
    `delta-p95=${deltaP95.toFixed(2)}ms`,
    `query-p95=${queryP95.toFixed(2)}ms`,
    `peak-rss-growth=${peakRssGrowth}B`,
    `max-transfer=${maxTransferBytes}B`,
    `max-affected=${maxAffected}`,
  ].join(" "),
);

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
      return decodeUpdate(engine.resetProject(encoder.encode(JSON.stringify(project))));
    },
  };
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

function positiveInteger(name: string, fallback: number): number {
  const source = process.env[name];
  if (source === undefined) return fallback;
  const value = Number(source);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function assertCounters(update: UpdateResult, analyzedDocuments: number) {
  if (update.stats.analyzedDocuments !== analyzedDocuments) {
    throw new Error("budget analyzed-document counter is inconsistent");
  }
  for (const key of ["semanticNodes", "constraints", "lawRulesVisited"] as const) {
    if (update.stats[key] < 0 || !Number.isFinite(update.stats[key])) {
      throw new Error(`budget ${key} counter is invalid`);
    }
  }
}

function percentile(values: readonly number[], fraction: number): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)]!;
}
