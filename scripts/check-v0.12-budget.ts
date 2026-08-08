import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import type {
  ChangeEnvelope,
  ProjectDocument,
  QueryEnvelope,
  QueryResult,
} from "../packages/protocol/src/index";
import corpus from "../fixtures/v0.12/realistic-mixed-project.json";
import {
  assertRealisticProjectResults,
  buildRealisticProjectFixture,
} from "./v0.12-realistic-project-fixture.mjs";

const COLD_BUDGET_MS = 1_000;
const RESET_P95_BUDGET_MS = 500;
const UPDATE_BUDGET_MS = 250;
const QUERY_P95_BUDGET_MS = 50;
const RESPONSE_BUDGET_BYTES = 256 * 1024;
const RETAINED_RSS_BUDGET_BYTES = 96 * 1024 * 1024;
const RESET_RUNS = 10;
const QUERY_RUNS = 40;

const { expectations, fixture } = buildRealisticProjectFixture(corpus);
const bytes = await readFile(
  new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
);
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const retainedStart = process.memoryUsage().rss;
const coldStarted = performance.now();
await init({ module_or_path: bytes });
const engine = new SemathEngine();
engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
const coldMs = performance.now() - coldStarted;

const resetDurations: number[] = [];
for (let run = 0; run < RESET_RUNS; run += 1) {
  const started = performance.now();
  engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
  resetDurations.push(performance.now() - started);
}

const queryDurations: number[] = [];
let largestResponse = 0;
let firstResults: QueryResult[] = [];
for (let run = 0; run < QUERY_RUNS; run += 1) {
  const results: QueryResult[] = [];
  for (const query of fixture.queries) {
    const started = performance.now();
    const raw = engine.query(encoder.encode(JSON.stringify(query)));
    queryDurations.push(performance.now() - started);
    largestResponse = Math.max(largestResponse, raw.byteLength);
    results.push(JSON.parse(decoder.decode(raw)) as QueryResult);
  }
  if (run === 0) firstResults = results;
}
assertRealisticProjectResults(firstResults, expectations);

const mixed = fixture.snapshot.documents.find(
  (document) => document.fileId === "mixed",
) as ProjectDocument;
const updatedMixed = {
  ...mixed,
  content: `${mixed.content}\n% rapid-edit generation 2`,
  documentVersion: 2,
};
const update: ChangeEnvelope = {
  analysisGeneration: 2,
  changes: [{ document: updatedMixed, kind: "upsert" }],
  epoch: fixture.snapshot.epoch,
  inventoryVersion: 2,
  protocolVersion: 1,
};
const updateStarted = performance.now();
engine.applyChanges(encoder.encode(JSON.stringify(update)));
const updateDurations = [performance.now() - updateStarted];
const currentQuery = {
  ...fixture.queries.find(
    (query) => query.query.fileId === "mixed" && query.query.kind === "formulaRecognition",
  )!,
  analysisGeneration: 2,
  documentVersion: 2,
  inventoryVersion: 2,
} satisfies QueryEnvelope;
const currentResult = JSON.parse(
  decoder.decode(engine.query(encoder.encode(JSON.stringify(currentQuery)))),
) as QueryResult;
if (currentResult.analysisGeneration !== 2 || currentResult.documentVersion !== 2) {
  throw new Error("v0.12 update query returned stale generation metadata");
}

for (const envelope of [
  {
    analysisGeneration: 3,
    changes: [
      {
        fileId: "discrete",
        kind: "path-change",
        path: "appendix/renamed-discrete.tex",
      },
    ],
    epoch: fixture.snapshot.epoch,
    inventoryVersion: 3,
    protocolVersion: 1,
  },
  {
    analysisGeneration: 4,
    changes: [{ fileId: "orphan", kind: "remove" }],
    epoch: fixture.snapshot.epoch,
    inventoryVersion: 4,
    protocolVersion: 1,
  },
] satisfies ChangeEnvelope[]) {
  const started = performance.now();
  engine.applyChanges(encoder.encode(JSON.stringify(envelope)));
  updateDurations.push(performance.now() - started);
}

const movedQuery = {
  ...fixture.queries.find((query) => query.query.fileId === "discrete")!,
  analysisGeneration: 4,
  inventoryVersion: 4,
} satisfies QueryEnvelope;
const movedResult = JSON.parse(
  decoder.decode(engine.query(encoder.encode(JSON.stringify(movedQuery)))),
) as QueryResult;
if (movedResult.analysisGeneration !== 4 || movedResult.inventoryVersion !== 4) {
  throw new Error("v0.12 move/remove query returned stale project metadata");
}

engine.free();
const retainedBytes = Math.max(0, process.memoryUsage().rss - retainedStart);
const resetP95 = percentile(resetDurations, 0.95);
const updateP95 = percentile(updateDurations, 0.95);
const queryP95 = percentile(queryDurations, 0.95);
for (const [label, actual, budget] of [
  ["cold start", coldMs, COLD_BUDGET_MS],
  ["reset p95", resetP95, RESET_P95_BUDGET_MS],
  ["update p95", updateP95, UPDATE_BUDGET_MS],
  ["query p95", queryP95, QUERY_P95_BUDGET_MS],
] as const) {
  if (actual > budget) {
    throw new Error(
      `v0.12 ${label} ${actual.toFixed(2)}ms exceeded ${budget}ms`,
    );
  }
}
if (largestResponse > RESPONSE_BUDGET_BYTES) {
  throw new Error(
    `v0.12 response ${largestResponse}B exceeded ${RESPONSE_BUDGET_BYTES}B`,
  );
}
if (retainedBytes > RETAINED_RSS_BUDGET_BYTES) {
  throw new Error(
    `v0.12 retained RSS ${retainedBytes}B exceeded ${RETAINED_RSS_BUDGET_BYTES}B`,
  );
}
console.log(
  `v0.12 budget OK: cold=${coldMs.toFixed(2)}ms reset-p95=${resetP95.toFixed(2)}ms update-p95=${updateP95.toFixed(2)}ms query-p95=${queryP95.toFixed(2)}ms max-response=${largestResponse}B retained-rss=${retainedBytes}B`,
);

function percentile(values: number[], fraction: number) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)]!;
}
