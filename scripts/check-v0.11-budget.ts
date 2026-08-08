import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import {
  assertDomainPackResults,
  buildDomainPackFixture,
} from "./v0.11-domain-fixture.mjs";

// Includes first-use compilation of every bounded pack regex on a shared x86 CI runner.
const RESET_BUDGET_MS = 350;
const QUERY_P95_BUDGET_MS = 50;
const RESPONSE_BUDGET_BYTES = 256 * 1024;
const RUNS = 5;

const corpus = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.11/domain-pack-recognition-corpus.json", import.meta.url),
    "utf8",
  ),
);
const { fixture, expectations } = buildDomainPackFixture(corpus);
const bytes = await readFile(
  new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
);
await init({ module_or_path: bytes });
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const engine = new SemathEngine();
const resetStarted = performance.now();
engine.resetProject(encoder.encode(JSON.stringify(fixture.snapshot)));
const resetMs = performance.now() - resetStarted;

const durations: number[] = [];
let largest = 0;
let firstResults: unknown[] = [];
for (let run = 0; run < RUNS; run++) {
  const results = [];
  for (const query of fixture.queries) {
    const started = performance.now();
    const raw = engine.query(encoder.encode(JSON.stringify(query)));
    durations.push(performance.now() - started);
    largest = Math.max(largest, raw.byteLength);
    results.push(JSON.parse(decoder.decode(raw)));
  }
  if (run === 0) firstResults = results;
}
engine.free();
const summary = assertDomainPackResults(firstResults, expectations);
durations.sort((left, right) => left - right);
const p95 = durations[Math.ceil(durations.length * 0.95) - 1]!;
if (resetMs > RESET_BUDGET_MS) {
  throw new Error(
    `v0.11 reset ${resetMs.toFixed(2)}ms exceeded ${RESET_BUDGET_MS}ms`,
  );
}
if (p95 > QUERY_P95_BUDGET_MS) {
  throw new Error(
    `v0.11 query p95 ${p95.toFixed(2)}ms exceeded ${QUERY_P95_BUDGET_MS}ms`,
  );
}
if (largest > RESPONSE_BUDGET_BYTES) {
  throw new Error(
    `v0.11 response ${largest}B exceeded ${RESPONSE_BUDGET_BYTES}B`,
  );
}
console.log(
  `v0.11 budget OK: mixed-catalog-reset=${resetMs.toFixed(2)}ms p95=${p95.toFixed(2)}ms max-response=${largest}B patterns=${summary.recognized}`,
);
