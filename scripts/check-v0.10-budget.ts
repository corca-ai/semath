import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import type { ProjectSnapshot, QueryEnvelope, QueryResult } from "../packages/protocol/src/index";

const WARM_P95_BUDGET_MS = 50;
const RESPONSE_BUDGET_BYTES = 256 * 1024;
const RUNS = 40;
const fixture = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.10/reliable-project-semantics.json", import.meta.url),
    "utf8",
  ),
) as { queries: QueryEnvelope[]; snapshot: ProjectSnapshot };
const bytes = await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url));
await init({ module_or_path: bytes });
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const engine = new SemathEngine();
engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));

const durations: number[] = [];
let largest = 0;
let firstResults: QueryResult[] = [];
for (let run = 0; run < RUNS; run++) {
  const results = [];
  for (const query of fixture.queries) {
    const started = performance.now();
    const raw = engine.query(encoder.encode(JSON.stringify(query)));
    durations.push(performance.now() - started);
    largest = Math.max(largest, raw.byteLength);
    results.push(JSON.parse(decoder.decode(raw)) as QueryResult);
  }
  if (run === 0) firstResults = results;
}
engine.free();

const chapterReferences = firstResults[1]?.value;
const orphanReferences = firstResults[3]?.value;
if (
  chapterReferences?.kind !== "locations" ||
  chapterReferences.locations.some((location) => location.fileId !== "chapter-a") ||
  orphanReferences?.kind !== "locations" ||
  orphanReferences.locations.some((location) => location.fileId !== "orphan")
) {
  throw new Error("v0.10 corpus produced a cross-scope false link");
}
durations.sort((left, right) => left - right);
const p95 = durations[Math.ceil(durations.length * 0.95) - 1]!;
if (p95 > WARM_P95_BUDGET_MS) {
  throw new Error(`v0.10 query p95 ${p95.toFixed(2)}ms exceeded ${WARM_P95_BUDGET_MS}ms`);
}
if (largest > RESPONSE_BUDGET_BYTES) {
  throw new Error(`v0.10 response ${largest}B exceeded ${RESPONSE_BUDGET_BYTES}B`);
}
console.log(
  `v0.10 budget OK: p95=${p95.toFixed(2)}ms max-response=${largest}B queries=${durations.length}`,
);
