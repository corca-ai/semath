import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index.ts";
import { firstDifferentialFailure } from "../packages/evaluation/src/differential.ts";
import { planCursorInvariantSurfaces } from "../packages/evaluation/src/cursor-invariants.ts";
import { SEMATH_PROTOCOL_VERSION } from "../packages/protocol/src/index.ts";

const baseSources = [
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
    content: ["Let $A$ be an event. Let $B$ be an event.", "$A \\cap B$."].join(
      "\n",
    ),
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
    content:
      "Without assuming semantic roles, the isolated relation $q = x/y$ is merely algebraic.",
    documentVersion: 1,
    fileId: "unsupported",
    language: "latex",
    path: "unsupported.tex",
  },
];
const cursorSurfaces = planCursorInvariantSurfaces();
const sources = [
  ...baseSources,
  ...cursorSurfaces.map((surface) => ({
    content: surface.content,
    documentVersion: 1,
    fileId: surface.fileId,
    language: "latex",
    path: surface.path,
  })),
];

const snapshot = makeSnapshot(sources, 1);
const probabilityOccurrence = sources[1].content.indexOf("A \\cap");
const baseQueries = [
  query("probability", probabilityOccurrence),
  query("probability", probabilityOccurrence + 1),
  query("unsupported", sources[3].content.indexOf("q =") + 1),
  query("circuits", sources[2].content.indexOf("i_1 +") + 1),
  definitionQuery("probability", probabilityOccurrence),
  definitionQuery("probability", probabilityOccurrence + 1),
];
const cursorRequests = cursorSurfaces.flatMap((surface) =>
  surface.probes.flatMap((probe) =>
    [
      "selection",
      "semanticView",
      "definition",
      "references",
      "prepareRename",
      "rename",
    ].map((kind) => ({
      envelope: cursorQuery(surface.fileId, probe.offset, kind),
      kind,
      probe,
      surface,
    })),
  ),
);
const queries = [
  ...baseQueries,
  ...cursorRequests.map((request) => request.envelope),
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
if (native.status !== 0)
  throw new Error(native.stderr || "native parity fixture failed");
const nativeResults = JSON.parse(native.stdout);

await init({
  module_or_path: await readFile(
    new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
  ),
});
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const engine = new SemathEngine();
const reset = resetEngine(engine, snapshot);
const wasmResults = queries.map((entry) => decode(engine.query(encode(entry))));
assertEquivalent(
  [
    { name: "native", value: nativeResults },
    { name: "wasm", value: wasmResults },
  ],
  "native/WASM query results",
);
assertEquivalent(
  [
    { name: "clean", value: wasmResults[0].value },
    { name: "incremental", value: wasmResults[1].value },
  ],
  "cursor-edge semantic identity",
);
assertCursorInvariants(cursorRequests, wasmResults.slice(baseQueries.length));
if (
  reset.stats.totalDocuments !== sources.length ||
  reset.stats.semanticNodes <= 0
) {
  throw new Error("parity reset did not expose trustworthy analysis counters");
}
const established = wasmResults[0]?.value;
if (
  established?.kind !== "semanticView" ||
  established.view.decision.status !== "established" ||
  established.view.context.relations.length !== 1
) {
  throw new Error(
    `parity meaning-first probability scenario was not established: ${JSON.stringify(established)}`,
  );
}
const refused = wasmResults[2]?.value;
if (
  refused?.kind !== "semanticView" ||
  refused.view.decision.status === "established"
) {
  throw new Error(
    "parity unsupported algebraic scenario was not safely refused",
  );
}

const updatedSource = {
  ...sources[1],
  content: sources[1].content.replace("event.", "Event."),
  documentVersion: 2,
};
const updatedSnapshot = makeSnapshot(
  [sources[0], updatedSource, sources[2], sources[3]],
  2,
);
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
  throw new Error(
    `unexpected incremental affected set: ${update.analyzedFileIds.join(",")}`,
  );
}
const incrementalQuery = {
  ...queries[0],
  analysisGeneration: 1,
  documentVersion: 2,
  inventoryVersion: 2,
};
const incrementalResult = decode(engine.query(encode(incrementalQuery)));

const clean = new SemathEngine();
resetEngine(clean, updatedSnapshot);
const cleanResult = decode(engineQuery(clean, incrementalQuery));
assertEquivalent(
  [
    { name: "clean", value: cleanResult.value },
    { name: "incremental", value: incrementalResult.value },
  ],
  "incremental/clean semantic result",
);
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
  return {
    ...query(fileId, offset),
    query: { fileId, kind: "definition", offset },
  };
}

function cursorQuery(fileId, offset, kind) {
  return {
    ...query(fileId, offset),
    query:
      kind === "rename"
        ? { fileId, kind, newName: "j", offset }
        : { fileId, kind, offset },
  };
}

function engineQuery(target, value) {
  return target.query(encode(value));
}

function encode(value) {
  return encoder.encode(JSON.stringify(value));
}

function resetEngine(target, snapshot) {
  const { documents, ...metadata } = snapshot;
  target.beginReset(encode(metadata));
  for (const document of documents)
    target.ingestResetDocument(encode(document));
  return decode(target.finishReset());
}

function decode(value) {
  return JSON.parse(decoder.decode(value));
}

function assertEquivalent(stages, label) {
  const failure = firstDifferentialFailure(stages);
  if (failure) {
    throw new Error(
      `${label} mismatch at ${failure.stage}:${failure.path}\nexpected=${JSON.stringify(failure.expected)}\nactual=${JSON.stringify(failure.actual)}`,
    );
  }
}

function assertCursorInvariants(requests, results) {
  const semanticRanges = new Map();
  for (const [index, request] of requests.entries()) {
    if (request.kind !== "semanticView") continue;
    const value = results[index]?.value;
    if (value?.kind === "semanticView" && value.view.symbol) {
      semanticRanges.set(
        `${request.surface.id}/${request.probe.id}`,
        value.view.symbol.location.range,
      );
    }
  }
  const grouped = new Map();
  for (const [index, request] of requests.entries()) {
    const key = `${request.surface.id}/${request.kind}/${request.probe.identity}`;
    const values = grouped.get(key) ?? [];
    values.push({ request, result: results[index] });
    grouped.set(key, values);
  }
  for (const [key, entries] of grouped) {
    if (entries[0].request.kind === "selection") {
      for (const { request, result } of entries) {
        const expected = semanticRanges.get(
          `${request.surface.id}/${request.probe.id}`,
        );
        const value = result?.value;
        if (
          !expected ||
          value?.kind !== "selection" ||
          !value.ranges.some(
            (range) =>
              range.startOffset === expected.startOffset &&
              range.endOffset === expected.endOffset,
          )
        ) {
          throw new Error(
            `${key}/${request.probe.id}: selection does not include its semantic occurrence`,
          );
        }
      }
      continue;
    }
    if (entries[0].request.kind === "semanticView") {
      const identities = entries.map(({ request, result }) => {
        const value = result?.value;
        const symbol =
          value?.kind === "semanticView" ? value.view.symbol : undefined;
        if (
          symbol?.sourceNotation !== request.probe.expectedSourceNotation ||
          symbol.symbol !== request.probe.expectedSymbol
        ) {
          throw new Error(
            `${key}/${request.probe.id}: expected ${request.probe.expectedSourceNotation}/${request.probe.expectedSymbol}, ` +
              `observed ${symbol?.sourceNotation ?? "none"}/${symbol?.symbol ?? "none"}`,
          );
        }
        return {
          entityId: symbol.entityId ?? null,
          occurrenceId: symbol.occurrenceId,
          sourceNotation: symbol.sourceNotation,
          symbol: symbol.symbol,
        };
      });
      for (const identity of identities.slice(1)) {
        assertEquivalent(
          [
            { name: "clean", value: identities[0] },
            { name: "incremental", value: identity },
          ],
          `${key} semantic occurrence`,
        );
      }
      continue;
    }
    const values = entries.map(({ result }) => result?.value);
    for (const value of values.slice(1)) {
      assertEquivalent(
        [
          { name: "clean", value: values[0] },
          { name: "incremental", value },
        ],
        `${key} navigation result`,
      );
    }
  }
}
