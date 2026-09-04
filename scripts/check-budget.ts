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
  performanceEntityFanout,
  semanticallyEditPerformanceDocument,
  type PerformanceFixtureDocument,
} from "./performance-fixtures";
import {
  retainedRssBudgetBytes,
  shouldEnforceRetainedRss,
  shouldEnforceTiming,
  timingBudget,
} from "./performance-budget-policy";
import {
  planSemanticEditTrace,
  planSemanticLifecycleTraces,
  shrinkEditTrace,
} from "../packages/evaluation/src/differential";

const DOCUMENT_COUNT = positiveInteger("SEMATH_BUDGET_DOCUMENTS", 60);
const STABLE_HOST_GATE = process.env.SEMATH_BUDGET_STABLE === "1";
const TIMING_GATE = shouldEnforceTiming(process.env, DOCUMENT_COUNT);
const RETAINED_RSS_GATE = shouldEnforceRetainedRss(process.env);
const DELTA_RUNS = positiveInteger(
  "SEMATH_BUDGET_DELTA_RUNS",
  DOCUMENT_COUNT >= 500 ? 10 : 30,
);
const TIMING_BUDGET = timingBudget(DOCUMENT_COUNT, STABLE_HOST_GATE);
const RETAINED_RSS_BUDGET_BYTES = retainedRssBudgetBytes(DOCUMENT_COUNT);
const MAX_AFFECTED_DOCUMENTS = 2;
const MAX_TRANSFER_BYTES = 16 * 1024;
// Dispatch work scales with the reviewed catalog, but must remain far below a
// full scan of all laws. These caps were recorded with the schema-8 catalog and
// guard compiler; future pack growth must improve dispatch before raising them.
const MAX_LAW_RULES_PER_DOCUMENT = 24;
const MAX_EQUIVALENCE_STATES_PER_DOCUMENT = 96;
const MAX_EQUIVALENCE_GUARD_CHECKS_PER_DOCUMENT = 8;

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
const wasmInitStarted = performance.now();
const wasmRuntime = await init({ module_or_path: wasm });
const wasmInitMs = performance.now() - wasmInitStarted;
collectGarbage();
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
let current = documents[2]!;
collectGarbage();
const rssAfterSyntax = residentBytes();

const worker = createWorkerHost(async () => {
  return operations(new SemathEngine());
});
const engineColdStarted = performance.now();
const initial = await worker.request<UpdateResult>({
  id: worker.nextId(),
  kind: "reset",
  snapshot,
});
const engineColdMs = wasmInitMs + performance.now() - engineColdStarted;
const coldMs = wasmInitMs + performance.now() - coldStarted;
// A real worker transfer releases the sender's serialized semantic snapshot.
// The in-process budget host does not, so drop those adapter documents once
// reset has consumed them instead of charging the engine for a test-only copy.
documents.length = 0;
collectGarbage();
const rssAfterEngine = residentBytes();
assertCounters(initial, DOCUMENT_COUNT + 1);

const deltaDurations: number[] = [];
const syntaxDurations: number[] = [];
const queryDurations = new Map<SemathQuery["kind"], number[]>();
const queryResultCounts = new Map<SemathQuery["kind"], number>();
let maxQueryResultBytes = 0;
let peakRss = residentBytes();
let peakRssStage = "initial";
let maxAffected = 0;
let maxTransferBytes = 0;
let inventoryVersion = snapshot.inventoryVersion;
const querySource = sources[0]!;
let currentSource: PerformanceFixtureDocument = sources[1]!;

for (let run = 0; run < DELTA_RUNS; run += 1) {
  // Keep the memory sample about one edit/query lifecycle. Without collecting
  // the previous iteration first, Bun's allocator can make this gate count
  // unreachable transient buffers as retained editor state.
  collectGarbage();
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
  if (update.analyzedFileIds.length !== 0) {
    throw new Error("budget comment-only delta performed semantic analysis");
  }
  assertCounters(update, 0);

  for (const query of measuredQueries(querySource)) {
    const envelope: QueryEnvelope = {
      analysisGeneration: run + 1,
      documentVersion: querySource.documentVersion,
      epoch: snapshot.epoch,
      inventoryVersion,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query,
    };
    const queryStarted = performance.now();
    const result = await worker.request<QueryResult>({
      envelope,
      id: worker.nextId(),
      kind: "query",
      priority: "cursor",
    });
    maxQueryResultBytes = Math.max(maxQueryResultBytes, encodedLength(result));
    queryResultCounts.set(
      query.kind,
      Math.max(queryResultCounts.get(query.kind) ?? 0, queryResultCount(result)),
    );
    assertFanoutResult(query, result);
    const durations = queryDurations.get(query.kind) ?? [];
    durations.push(performance.now() - queryStarted);
    queryDurations.set(query.kind, durations);
  }
  const editRss = residentBytes();
  if (editRss > peakRss) {
    peakRss = editRss;
    peakRssStage = `comment-edit-${run + 1}`;
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
const semanticEditRss = residentBytes();
if (semanticEditRss > peakRss) {
  peakRss = semanticEditRss;
  peakRssStage = "semantic-edit";
}
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
collectGarbage();
const rssAfterIncremental = residentBytes();
if (rssAfterIncremental > peakRss) {
  peakRss = rssAfterIncremental;
  peakRssStage = "retained-editor-state";
}
const retainedRssGrowth = Math.max(0, rssAfterIncremental - rssBefore);
const retainedWasmLinearMemoryBytes = wasmRuntime.memory.buffer.byteLength;

// The parity rebuild is a separate lifecycle, not a second live editor engine.
// Dispose the incremental worker first so this gate measures the maximum memory
// of either valid lifecycle instead of an artificial overlap of both.
await worker.dispose();
const clean = new SemathEngine();
const { documents: _discardedDocuments, ...cleanMetadata } = snapshot;
clean.beginReset(
  encoder.encode(
    JSON.stringify({ ...cleanMetadata, inventoryVersion: inventoryVersion + 1 }),
  ),
);
for (const source of [main, ...sources]) {
  const document = (() => {
    if (source.fileId === current.fileId) return current;
    const fileSyntax = syntax.getFile(source.fileId);
    if (!fileSyntax) throw new Error(`missing syntax for ${source.fileId}`);
    return adaptWasmtexDocument({
      content: source.content,
      language: source.language ?? "latex",
      syntax: fileSyntax,
    });
  })();
  clean.ingestResetDocument(encoder.encode(JSON.stringify(document)));
}
const cleanUpdate = decodeUpdate(clean.finishReset());
if (
  initial.stats.totalDocuments !== cleanUpdate.stats.totalDocuments ||
  initial.stats.recognizedLaws !== cleanUpdate.stats.recognizedLaws
) {
  throw new Error("budget incremental and clean rebuild summaries diverged");
}
clean.free();
collectGarbage();
const rssAfterDispose = residentBytes();
if (rssAfterDispose > peakRss) {
  peakRss = rssAfterDispose;
  peakRssStage = "clean-rebuild-disposed";
}
const postDisposeRssGrowth = Math.max(0, rssAfterDispose - rssBefore);

const deltaP95 = percentile(deltaDurations, 0.95);
const deltaMedian = percentile(deltaDurations, 0.5);
const syntaxP95 = percentile(syntaxDurations, 0.95);
const queryP95ByKind = Object.fromEntries(
  [...queryDurations].map(([kind, durations]) => [kind, percentile(durations, 0.95)]),
);
const queryP95 = Math.max(...Object.values(queryP95ByKind));
const shrinkSource = planSemanticEditTrace(0x5e_21);
let failureShrinkEvaluations = 0;
const shrunkFailure = shrinkEditTrace(shrinkSource, (candidate) => {
  failureShrinkEvaluations += 1;
  return candidate.steps.some((step) => step.content?.includes("matrix"));
});
if (failureShrinkEvaluations > shrinkSource.steps.length || shrunkFailure.steps.length !== 1) {
  throw new Error("budget failure shrinking exceeded deterministic linear work");
}
const peakRssGrowth = Math.max(0, Math.max(peakRss, rssAfterDispose) - rssBefore);
const syntaxStats = syntax.getStats() as ReturnType<LatexSyntaxService["getStats"]> & {
  lastInvalidatedDocuments?: number;
  lastTransferBytes?: number;
  notationNodes?: number;
  recoveredNodes?: number;
  snapshotBytes?: number;
};
const report = {
  host: { platform: process.platform, architecture: process.arch, bun: process.versions.bun },
  enforcement: { timing: TIMING_GATE, retainedRss: RETAINED_RSS_GATE },
  retainedWasmLinearMemoryBytes,
  affectedDocuments: maxAffected,
  adapterColdMs,
  analysis: initial.stats,
  coldMs,
  deltaMedianMs: deltaMedian,
  deltaP95Ms: deltaP95,
  documents: DOCUMENT_COUNT + 1,
  engineColdMs,
  failureShrink: {
    evaluations: failureShrinkEvaluations,
    inputSteps: shrinkSource.steps.length,
    outputSteps: shrunkFailure.steps.length,
  },
  fixtureFamilies: [...new Set(sources.map((source) => source.family))],
  initialTransferBytes,
  peakRssGrowthBytes: peakRssGrowth,
  peakRssStage,
  postDisposeRssGrowthBytes: postDisposeRssGrowth,
  queryP95ByKind,
  queryResultBytes: maxQueryResultBytes,
  queryResultCounts: Object.fromEntries(queryResultCounts),
  semanticViewP95Ms: queryP95ByKind.semanticView ?? null,
  lifecycleFamilies: planSemanticLifecycleTraces(0x5e_21).map((trace) => trace.family),
  retainedRssGrowthBytes: retainedRssGrowth,
  rssGrowthByStage: {
    engineBytes: Math.max(0, rssAfterEngine - rssAfterSyntax),
    syntaxBytes: Math.max(0, rssAfterSyntax - rssBefore),
  },
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
  wasmInitMs,
};
console.log(`budget metrics: ${JSON.stringify(report)}`);
const reportPath = process.env.SEMATH_BUDGET_REPORT;
if (reportPath) await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
if (TIMING_GATE && coldMs > TIMING_BUDGET.coldMs) {
  throw new Error(`budget cold start ${coldMs.toFixed(2)}ms exceeded ${TIMING_BUDGET.coldMs}ms`);
}
if (TIMING_GATE && deltaP95 > TIMING_BUDGET.deltaP95Ms) {
  throw new Error(
    `budget delta p95 ${deltaP95.toFixed(2)}ms exceeded ${TIMING_BUDGET.deltaP95Ms}ms`,
  );
}
if (TIMING_GATE && semanticDeltaMs > TIMING_BUDGET.semanticDeltaMs) {
  throw new Error(
    `budget semantic delta ${semanticDeltaMs.toFixed(2)}ms exceeded ${TIMING_BUDGET.semanticDeltaMs}ms`,
  );
}
if (TIMING_GATE && queryP95 > TIMING_BUDGET.queryP95Ms) {
  throw new Error(
    `budget query p95 ${queryP95.toFixed(2)}ms exceeded ${TIMING_BUDGET.queryP95Ms}ms`,
  );
}
if (initial.stats.lawRulesVisited > (DOCUMENT_COUNT + 1) * MAX_LAW_RULES_PER_DOCUMENT) {
  throw new Error(
    `budget law dispatch visited ${initial.stats.lawRulesVisited} rules for ${DOCUMENT_COUNT + 1} documents`,
  );
}
if (
  initial.stats.equivalenceStates >
    (DOCUMENT_COUNT + 1) * MAX_EQUIVALENCE_STATES_PER_DOCUMENT
) {
  throw new Error(
    `budget equivalence compiler visited ${initial.stats.equivalenceStates} states for ${DOCUMENT_COUNT + 1} documents`,
  );
}
if (
  initial.stats.equivalenceGuardChecks >
    (DOCUMENT_COUNT + 1) * MAX_EQUIVALENCE_GUARD_CHECKS_PER_DOCUMENT
) {
  throw new Error(
    `budget equivalence compiler checked ${initial.stats.equivalenceGuardChecks} guards for ${DOCUMENT_COUNT + 1} documents`,
  );
}
if (peakRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  console.warn(
    `budget transient peak RSS growth ${peakRssGrowth}B at ${peakRssStage} exceeded the retained-state budget ${RETAINED_RSS_BUDGET_BYTES}B`,
  );
}
if (RETAINED_RSS_GATE && retainedRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(
    `budget retained RSS growth ${retainedRssGrowth}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`,
  );
}
if (!RETAINED_RSS_GATE && retainedRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  console.warn(
    `budget retained RSS sample ${retainedRssGrowth}B exceeds the Linux x64 reference ${RETAINED_RSS_BUDGET_BYTES}B; this sample is diagnostic only`,
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
    { ...target, kind: "references", includeDeclaration: true },
    { ...target, kind: "prepareRename" },
    { ...target, kind: "rename", newName: "w" },
  ];
}

function queryResultCount(result: QueryResult): number {
  switch (result.value.kind) {
    case "locations":
      return result.value.locations.length;
    case "editProposal":
      return result.value.proposal?.files.reduce(
        (count, file) => count + file.edits.length,
        0,
      ) ?? 0;
    case "renamePreparation":
      return result.value.range ? 1 : 0;
    case "selection":
      return result.value.ranges.length;
    case "semanticView":
      return result.value.view.declarations.length;
    case "diagnostics":
      return result.value.diagnostics.length;
    case "diagnosticExplanation":
      return result.value.diagnostic ? 1 : 0;
  }
}

function assertFanoutResult(query: SemathQuery, result: QueryResult): void {
  if (
    (result.value.kind === "locations" ||
      result.value.kind === "editProposal" ||
      result.value.kind === "renamePreparation") &&
    result.value.authorization.status !== "authorized"
  ) {
    throw new Error(
      `budget ${query.kind} was refused by the bounded entity authority: ${JSON.stringify(result.value.authorization.reason)}`,
    );
  }
  if (query.kind === "references" || query.kind === "rename") {
    const count = queryResultCount(result);
    const expectedFanout = performanceEntityFanout(DOCUMENT_COUNT);
    if (count < expectedFanout) {
      throw new Error(
        `budget ${query.kind} returned ${count} source occurrences; expected at least ${expectedFanout}`,
      );
    }
  }
  if (
    query.kind === "definition" &&
    (result.value.kind !== "locations" || result.value.locations.length !== 1)
  ) {
    throw new Error(
      `budget entity definition did not resolve exactly once: ${JSON.stringify(result.value)}`,
    );
  }
  if (
    query.kind === "prepareRename" &&
    (result.value.kind !== "renamePreparation" || !result.value.range)
  ) {
    throw new Error("budget established entity was not renameable");
  }
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

function collectGarbage(): void {
  // A single full collection can leave finalizer-reachable WASM wrappers for
  // the next cycle. Settle the heap before comparing retained editor state so
  // allocator scheduling is not mistaken for a semantic memory regression.
  for (let pass = 0; pass < 3; pass += 1) Bun.gc(true);
}

function assertCounters(update: UpdateResult, analyzedDocuments: number) {
  if (update.stats.analyzedDocuments !== analyzedDocuments) {
    throw new Error("budget analyzed-document counter is inconsistent");
  }
  for (const key of [
    "semanticNodes",
    "constraints",
    "lawRulesVisited",
    "packFrontierCandidates",
    "packLatentCandidates",
    "packLatentFallbacks",
    "domainHypotheses",
    "domainEvidence",
    "equivalenceStates",
    "equivalenceGuardChecks",
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
