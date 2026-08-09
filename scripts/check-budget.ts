import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type ProjectDocument,
  type ProjectSnapshot,
  type UpdateResult,
} from "../packages/protocol/src/index";

const DOCUMENT_COUNT = 60;
const DELTA_RUNS = 30;
const COLD_BUDGET_MS = 5_000;
const DELTA_P95_BUDGET_MS = 500;
const RETAINED_RSS_BUDGET_BYTES = 128 * 1024 * 1024;
const MAX_AFFECTED_DOCUMENTS = 2;

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
const syntax = new LatexSyntaxService();
syntax.reset({ documents: [main, ...sources] });
const documents = [main, ...sources].map((source) => {
  const snapshot = syntax.getFile(source.fileId);
  if (!snapshot) throw new Error(`missing syntax for ${source.fileId}`);
  return adaptWasmtexDocument({
    content: source.content,
    language: source.language,
    syntax: snapshot,
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

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const wasm = await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url));
const rssBefore = process.memoryUsage().rss;
const coldStarted = performance.now();
await init({ module_or_path: wasm });
const engine = new SemathEngine();
const initial = decodeUpdate(engine.resetProject(encoder.encode(JSON.stringify(snapshot))));
const coldMs = performance.now() - coldStarted;
assertCounters(initial, DOCUMENT_COUNT + 1);

const deltaDurations: number[] = [];
let peakRss = process.memoryUsage().rss;
let maxAffected = 0;
let inventoryVersion = snapshot.inventoryVersion;
let current = documents[1]!;
for (let run = 0; run < DELTA_RUNS; run += 1) {
  inventoryVersion += 1;
  current = {
    ...current,
    content: `${sources[0]!.content}\n% delta ${run}`,
    documentVersion: current.documentVersion + 1,
  } satisfies ProjectDocument;
  const envelope: ChangeEnvelope = {
    analysisGeneration: run + 1,
    changes: [{ document: current, kind: "upsert" }],
    epoch: snapshot.epoch,
    inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
  const started = performance.now();
  const update = decodeUpdate(engine.applyChanges(encoder.encode(JSON.stringify(envelope))));
  deltaDurations.push(performance.now() - started);
  peakRss = Math.max(peakRss, process.memoryUsage().rss);
  maxAffected = Math.max(maxAffected, update.analyzedFileIds.length);
  if (update.analyzedFileIds.length > MAX_AFFECTED_DOCUMENTS) {
    throw new Error(
      `budget delta analyzed ${update.analyzedFileIds.length} documents; expected at most ${MAX_AFFECTED_DOCUMENTS}`,
    );
  }
  if (!update.analyzedFileIds.includes(current.fileId) || !update.analyzedFileIds.includes("main")) {
    throw new Error(`budget affected closure omitted the changed file or its dependent main file`);
  }
  assertCounters(update, update.analyzedFileIds.length);
}

const incremental = decodeUpdate(
  engine.applyChanges(
    encoder.encode(
      JSON.stringify({
        analysisGeneration: DELTA_RUNS + 1,
        changes: [],
        epoch: snapshot.epoch,
        inventoryVersion: inventoryVersion + 1,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      } satisfies ChangeEnvelope),
    ),
  ),
);
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
engine.free();
clean.free();

const deltaP95 = percentile(deltaDurations, 0.95);
const deltaMedian = percentile(deltaDurations, 0.5);
const peakRssGrowth = Math.max(0, peakRss - rssBefore);
if (coldMs > COLD_BUDGET_MS) {
  throw new Error(`budget cold start ${coldMs.toFixed(2)}ms exceeded ${COLD_BUDGET_MS}ms`);
}
if (deltaP95 > DELTA_P95_BUDGET_MS) {
  throw new Error(`budget delta p95 ${deltaP95.toFixed(2)}ms exceeded ${DELTA_P95_BUDGET_MS}ms`);
}
if (peakRssGrowth > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(`budget peak RSS growth ${peakRssGrowth}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`);
}
console.log(
  `budget OK: documents=${DOCUMENT_COUNT + 1} cold=${coldMs.toFixed(2)}ms delta-median=${deltaMedian.toFixed(2)}ms delta-p95=${deltaP95.toFixed(2)}ms peak-rss-growth=${peakRssGrowth}B max-affected=${maxAffected}`,
);

function decodeUpdate(bytes: Uint8Array): UpdateResult {
  return JSON.parse(decoder.decode(bytes)) as UpdateResult;
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
