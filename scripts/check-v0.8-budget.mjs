import { readFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";

const COLD_START_BUDGET_MS = 1_500;
const WARM_P95_BUDGET_MS = 50;
const RESPONSE_BUDGET_BYTES = 256 * 1024;
const WARM_RUNS = 20;

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const encode = (value) => encoder.encode(JSON.stringify(value));
const decode = (value) => JSON.parse(decoder.decode(value));

function scenario(name, language, content, offset, documents = []) {
  const epoch = `budget:${name}`;
  return {
    name,
    snapshot: {
      protocolVersion: 1,
      epoch,
      inventoryVersion: 1,
      projectId: "budget",
      mainFileId: "main",
      documents: [
        {
          fileId: "main",
          path: language === "latex" ? "main.tex" : "main.md",
          language,
          content,
          documentVersion: 1,
        },
        ...documents,
      ],
    },
    query: {
      protocolVersion: 1,
      epoch,
      inventoryVersion: 1,
      documentVersion: 1,
      analysisGeneration: 1,
      query: { kind: "inspection", fileId: "main", offset },
    },
  };
}

const markdown = "Let $x$ denote the input.\nUse $x + 1$.";
const latex = "\\section{Model}\nLet $p$ denote a probability distribution.\n\\[p(x) = p(x)\\]";
const unfinished = "Draft an unfinished expression: $x = {";
const deepExpression = `$${"f(".repeat(180)}x${")".repeat(180)}$`;
const conflicting = [
  "Let $p$ denote a probability distribution.",
  "$p$ is a random variable.",
  "Inspect $p$.",
].join("\n");
const referenceUses = Array.from({ length: 48 }, (_, index) => `Use ${index}: $x$.`).join(
  "\n",
);
const multiFileMain = "Let $x$ denote the shared input.\nInspect $x$.";

const scenarios = [
  scenario("markdown", "markdown", markdown, markdown.lastIndexOf("x")),
  scenario("latex", "latex", latex, latex.lastIndexOf("p")),
  scenario("unfinished", "markdown", unfinished, unfinished.indexOf("x")),
  scenario("deep", "markdown", deepExpression, deepExpression.indexOf("x")),
  scenario("conflicting-claims", "markdown", conflicting, conflicting.lastIndexOf("p")),
  scenario("multi-file-references", "markdown", multiFileMain, multiFileMain.lastIndexOf("x"), [
    {
      fileId: "uses",
      path: "uses.md",
      language: "markdown",
      content: referenceUses,
      documentVersion: 1,
    },
  ]),
];

const wasmBytes = await readFile(
  new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
);
const coldStart = performance.now();
await init({ module_or_path: wasmBytes });
const coldStartMs = performance.now() - coldStart;

const engine = new SemathEngine();
const durations = [];
let maximumResponseBytes = 0;
const truncation = new Map();

for (const entry of scenarios) {
  decode(engine.resetProject(encode(entry.snapshot)));
  for (let run = 0; run < WARM_RUNS; run += 1) {
    const started = performance.now();
    const rawResult = engine.query(encode(entry.query));
    durations.push(performance.now() - started);
    maximumResponseBytes = Math.max(maximumResponseBytes, rawResult.byteLength);
    const result = decode(rawResult);
    if (run === 0) {
      truncation.set(entry.name, result.value?.inspection?.truncated === true);
    }
  }
}
engine.free();

durations.sort((left, right) => left - right);
const p95Index = Math.min(durations.length - 1, Math.ceil(durations.length * 0.95) - 1);
const warmP95Ms = durations[p95Index];

if (!truncation.get("deep") && !truncation.get("multi-file-references")) {
  throw new Error("the calibration corpus did not exercise a bounded/truncated inspection");
}
if (coldStartMs > COLD_START_BUDGET_MS) {
  throw new Error(`WASM cold start ${coldStartMs.toFixed(2)}ms exceeded ${COLD_START_BUDGET_MS}ms`);
}
if (warmP95Ms > WARM_P95_BUDGET_MS) {
  throw new Error(`inspection p95 ${warmP95Ms.toFixed(2)}ms exceeded ${WARM_P95_BUDGET_MS}ms`);
}
if (maximumResponseBytes > RESPONSE_BUDGET_BYTES) {
  throw new Error(
    `inspection response ${maximumResponseBytes} bytes exceeded ${RESPONSE_BUDGET_BYTES} bytes`,
  );
}

console.log(
  `v0.8 budget OK: cold=${coldStartMs.toFixed(2)}ms warm-p95=${warmP95Ms.toFixed(2)}ms max-response=${maximumResponseBytes}B scenarios=${scenarios.length}`,
);
