import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index.ts";
import { SEMATH_PROTOCOL_VERSION } from "../packages/protocol/src/index.ts";

const sources = [
  {
    content: [
      "\\newcommand{\\Both}[2]{#1 \\cap #2}",
      "\\input{probability.tex}",
      "\\input{circuits.tex}",
      "\\input{unsupported.tex}",
    ].join("\n"),
    documentVersion: 1,
    fileId: "main",
    language: "latex",
    path: "main.tex",
  },
  {
    content: [
      "Let $A$ be an event. Let $B$ be an event.",
      "$A \\cap B$.",
    ].join("\n"),
    documentVersion: 1,
    fileId: "probability",
    language: "latex",
    path: "probability.tex",
  },
  {
    content: [
      "Let i_1, i_2, and i_3 denote branch currents at the same node.",
      "$i_1 + i_2 = i_3$",
    ].join("\n"),
    documentVersion: 1,
    fileId: "circuits",
    language: "latex",
    path: "circuits.tex",
  },
  {
    content: "Without assuming semantic roles, the isolated relation $q = x/y$ is merely algebraic.",
    documentVersion: 1,
    fileId: "unsupported",
    language: "latex",
    path: "unsupported.tex",
  },
];

const snapshot = makeSnapshot(sources, 1);
const queries = [
  query("probability", sources[1].content.indexOf("A \\cap") + 1),
  query("unsupported", sources[3].content.indexOf("q =") + 1),
  query("circuits", sources[2].content.indexOf("i_1 +") + 1),
  definitionQuery("probability", sources[1].content.indexOf("A \\cap") + 1),
];
const fixture = { queries, snapshot };

const build = spawnSync("cargo", ["build", "--locked", "-p", "semath-native"], {
  encoding: "utf8",
});
if (build.status !== 0) throw new Error(build.stderr || "native build failed");
const native = spawnSync("./target/debug/semath-native", [], {
  encoding: "utf8",
  input: JSON.stringify(fixture),
});
if (native.status !== 0) throw new Error(native.stderr || "native parity fixture failed");
const nativeResults = JSON.parse(native.stdout);

await init({
  module_or_path: await readFile(new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url)),
});
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const engine = new SemathEngine();
const reset = decode(engine.resetProject(encode(snapshot)));
const wasmResults = queries.map((entry) => decode(engine.query(encode(entry))));
assertEqual(nativeResults, wasmResults, "native/WASM query results");
if (reset.stats.totalDocuments !== sources.length || reset.stats.semanticNodes <= 0) {
  throw new Error("parity reset did not expose trustworthy analysis counters");
}
const established = wasmResults[0]?.value;
if (
  established?.kind !== "semanticView" ||
  established.view.status !== "established" ||
  established.view.context.relations.length !== 1
) {
  throw new Error(
    `parity meaning-first probability scenario was not established: ${JSON.stringify(established)}`,
  );
}
const refused = wasmResults[1]?.value;
if (refused?.kind !== "semanticView" || refused.view.status === "established") {
  throw new Error("parity unsupported algebraic scenario was not safely refused");
}

const updatedSource = {
  ...sources[1],
  content: `${sources[1].content}\n% incremental edit`,
  documentVersion: 2,
};
const updatedSnapshot = makeSnapshot([sources[0], updatedSource, sources[2], sources[3]], 2);
const updatedDocument = updatedSnapshot.documents.find(
  (document) => document.fileId === updatedSource.fileId,
);
const update = decode(
  engine.applyChanges(
    encode({
      analysisGeneration: 1,
      changes: [{ document: updatedDocument, kind: "upsert" }],
      epoch: snapshot.epoch,
      inventoryVersion: 2,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    }),
  ),
);
if (update.analyzedFileIds.join(",") !== "main,probability") {
  throw new Error(`unexpected incremental affected set: ${update.analyzedFileIds.join(",")}`);
}
const incrementalQuery = {
  ...queries[0],
  analysisGeneration: 1,
  documentVersion: 2,
  inventoryVersion: 2,
};
const incrementalResult = decode(engine.query(encode(incrementalQuery)));

const clean = new SemathEngine();
clean.resetProject(encode(updatedSnapshot));
const cleanResult = decode(engineQuery(clean, incrementalQuery));
assertEqual(incrementalResult.value, cleanResult.value, "incremental/clean semantic result");
engine.free();
clean.free();
console.log(
  `parity OK: ${queries.length} native/WASM queries, refusal, counters, and incremental closure`,
);

function makeSnapshot(documents, inventoryVersion) {
  const syntax = new LatexSyntaxService();
  syntax.reset({ documents });
  return {
    documents: documents.map((document) => {
      const parsed = syntax.getFile(document.fileId);
      if (!parsed) throw new Error(`missing syntax for ${document.fileId}`);
      return adaptWasmtexDocument({
        content: document.content,
        language: document.language,
        syntax: parsed,
      });
    }),
    epoch: "quality-parity",
    inventoryVersion,
    mainFileId: "main",
    projectId: "quality-parity",
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

function query(fileId, offset) {
  return {
    analysisGeneration: 0,
    documentVersion: 1,
    epoch: "quality-parity",
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: { fileId, kind: "semanticView", offset },
  };
}

function definitionQuery(fileId, offset) {
  return { ...query(fileId, offset), query: { fileId, kind: "definition", offset } };
}

function engineQuery(target, value) {
  return target.query(encode(value));
}

function encode(value) {
  return encoder.encode(JSON.stringify(value));
}

function decode(value) {
  return JSON.parse(decoder.decode(value));
}

function assertEqual(left, right, label) {
  if (JSON.stringify(left) !== JSON.stringify(right)) {
    throw new Error(`${label} mismatch`);
  }
}
