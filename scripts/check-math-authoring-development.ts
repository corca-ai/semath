import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import {
  compileMathAuthoringOracle,
  evaluateMathAuthoringOracle,
  mathAuthoringDiagnosticArtifact,
} from "../packages/evaluation/src/index";
import {
  type ProjectSnapshot,
  type QueryEnvelope,
} from "../packages/protocol/src/index";
import {
  classifyMathAuthoringOracleReport,
  persistMathAuthoringDiagnostic,
  runMathAuthoringOracleWithPorts,
} from "./math-authoring-oracle-runner";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

const sourcePath = "fixtures/challenge/math-authoring-oracle-source-v2.json";
const oraclePath = "fixtures/challenge/math-authoring-oracle-v2.json";
const reviewPath = "fixtures/challenge/math-authoring-oracle-review-v2.json";

const compiled = compileMathAuthoringOracle(
  JSON.parse(await readFile(sourcePath, "utf8")),
  JSON.parse(await readFile(oraclePath, "utf8")),
  JSON.parse(await readFile(reviewPath, "utf8")),
);
if (process.env.SEMATH_AUTHORED_SKIP_BUILD !== "1") buildNative();
await init({
  module_or_path: await readFile("lib/wasm/semath_wasm_bg.wasm"),
});

const observations = runMathAuthoringOracleWithPorts(compiled, {
  createEngine: () => {
    const engine = new SemathEngine();
    return {
      apply: (changes) => decode(engine.applyChanges(encode(changes))),
      free: () => engine.free(),
      query: (query) => decode(engine.query(encode(query))),
      reset: (snapshot) => {
        const { documents, ...metadata } = snapshot;
        engine.beginReset(encode(metadata));
        for (const document of documents) engine.ingestResetDocument(encode(document));
        return decode(engine.finishReset());
      },
    };
  },
  runNative,
});
const artifact = mathAuthoringDiagnosticArtifact(observations);
const artifactPath = await persistMathAuthoringDiagnostic(artifact);
const report = evaluateMathAuthoringOracle(compiled, observations);
if (report.diagnostic.sha256 !== artifact.sha256 ||
  report.diagnostic.artifactId !== artifact.artifactId) {
  throw new Error("math authoring evaluator diagnostic identity mismatch");
}
const gate = classifyMathAuthoringOracleReport(report);
for (const advisory of gate.advisory) console.warn(`math authoring advisory: ${advisory}`);
if (gate.blocking.length) {
  throw new Error(`math authoring public oracle failed:\n${gate.blocking.join("\n")}`);
}
console.log(
  `math authoring public oracle OK: ${compiled.oracle.cases.length} cases; ` +
  `${compiled.oracle.pairs.length} TeX/Markdown pairs; diagnostic ${artifactPath}`,
);

function runNative(
  snapshot: ProjectSnapshot,
  queries: readonly QueryEnvelope[],
  label: string,
): unknown {
  const result = spawnSync("target/debug/semath-native", [], {
    encoding: "utf8",
    input: JSON.stringify({ queries, snapshot }),
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`${label}: ${result.stderr || "native failed"}`);
  return JSON.parse(result.stdout) as unknown;
}

function buildNative(): void {
  const build = spawnSync("cargo", ["build", "--quiet", "--locked", "-p", "semath-native"], { encoding: "utf8" });
  if (build.status !== 0) throw new Error(build.stderr || "failed to build semath-native");
}

function encode(value: unknown): Uint8Array { return encoder.encode(JSON.stringify(value)); }
function decode(value: Uint8Array): unknown { return JSON.parse(decoder.decode(value)); }
