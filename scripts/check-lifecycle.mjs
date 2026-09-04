import { readFile } from "node:fs/promises";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index.ts";
import {
  firstDifferentialFailure,
  planSemanticLifecycleTraces,
} from "./testing/differential.ts";
import { SEMATH_PROTOCOL_VERSION } from "../packages/protocol/src/index.ts";

await init({
  module_or_path: await readFile(
    new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
  ),
});

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const planned = planSemanticLifecycleTraces(0x5e_21);
const traces = process.env.SEMATH_LIFECYCLE_FULL === "1" ? planned : planned.slice(0, 2);
let comparedStages = 0;

for (const trace of traces) {
  const sources = new Map(
    trace.initialDocuments.map((document) => [
      document.fileId,
      { ...document, documentVersion: 1, language: "latex" },
    ]),
  );
  const syntax = new LatexSyntaxService();
  syntax.reset({ documents: [...sources.values()] });
  let inventoryVersion = 1;
  let analysisGeneration = 0;
  const engine = new SemathEngine();
  resetEngine(engine, snapshotFrom(sources, syntax, inventoryVersion));
  const initialResult = queryEngine(engine, trace.query, sources, inventoryVersion, analysisGeneration);
  assertDecision(
    initialResult,
    trace.initialExpectedDecision,
    `${trace.id}/initial`,
  );
  assertDomains(initialResult, trace.initialExpectedDomains, `${trace.id}/initial`);

  for (const stage of trace.stages) {
    inventoryVersion += 1;
    analysisGeneration += 1;
    const explicitChanges = [];
    for (const change of stage.changes) {
      if (change.kind === "upsert") {
        const previous = sources.get(change.fileId);
        const source = {
          content: change.content ?? previous?.content ?? "",
          documentVersion: (previous?.documentVersion ?? 0) + 1,
          fileId: change.fileId,
          language: "latex",
          path: change.path ?? previous?.path ?? `${change.fileId}.tex`,
        };
        sources.set(change.fileId, source);
        syntax.upsert(source);
      } else if (change.kind === "remove") {
        sources.delete(change.fileId);
        syntax.remove(change.fileId);
        explicitChanges.push({ fileId: change.fileId, kind: "remove" });
      } else {
        const previous = sources.get(change.fileId);
        if (!previous || !change.path) throw new Error(`${trace.id}: invalid path change`);
        const source = { ...previous, path: change.path };
        sources.set(change.fileId, source);
        syntax.move(change.fileId, change.path);
        explicitChanges.push({ fileId: change.fileId, kind: "path-change", path: change.path });
      }
    }
    const changedIds = new Set(explicitChanges.map((change) => change.fileId));
    const upserts = syntax.getInvalidatedFiles().flatMap((fileSyntax) => {
      if (changedIds.has(fileSyntax.fileId)) return [];
      const source = sources.get(fileSyntax.fileId);
      if (!source) return [];
      return [{
        document: adaptWasmtexDocument({
          content: source.content,
          language: "latex",
          syntax: fileSyntax,
        }),
        kind: "upsert",
      }];
    });
    decode(
      engine.applyChanges(
        encode({
          analysisGeneration,
          changes: [...explicitChanges, ...upserts],
          epoch: "semantic-lifecycle",
          inventoryVersion,
          protocolVersion: SEMATH_PROTOCOL_VERSION,
        }),
      ),
    );

    const query = {
      ...trace.query,
      ...(stage.queryNeedle ? { needle: stage.queryNeedle } : {}),
    };
    const incremental = queryEngine(
      engine,
      query,
      sources,
      inventoryVersion,
      analysisGeneration,
    );
    const clean = new SemathEngine();
    const cleanSyntax = new LatexSyntaxService();
    cleanSyntax.reset({ documents: [...sources.values()] });
    resetEngine(clean, snapshotFrom(sources, cleanSyntax, inventoryVersion));
    const rebuilt = queryEngine(
      clean,
      query,
      sources,
      inventoryVersion,
      analysisGeneration,
    );
    const failure = firstDifferentialFailure([
      { name: "clean", value: rebuilt.value },
      { name: "incremental", value: incremental.value },
    ]);
    if (failure) {
      throw new Error(
        `${trace.id}/${stage.id}: ${failure.stage} diverged at ${failure.path}\n` +
          `expected=${JSON.stringify(failure.expected)}\nactual=${JSON.stringify(failure.actual)}`,
      );
    }
    assertDecision(incremental, stage.expectedDecision, `${trace.id}/${stage.id}`);
    assertDomains(incremental, stage.expectedDomains, `${trace.id}/${stage.id}`);
    clean.free();
    comparedStages += 1;
  }
  engine.free();
}

console.log(
  `lifecycle OK: ${traces.length}/${planned.length} traces, ${comparedStages} clean/incremental stages`,
);

function snapshotFrom(sources, syntax, inventoryVersion) {
  return {
    documents: [...sources.values()].map((source) => {
      const fileSyntax = syntax.getFile(source.fileId);
      if (!fileSyntax) throw new Error(`missing syntax for ${source.fileId}`);
      return adaptWasmtexDocument({
        content: source.content,
        language: "latex",
        syntax: fileSyntax,
      });
    }),
    epoch: "semantic-lifecycle",
    inventoryVersion,
    mainFileId: sources.has("main") ? "main" : undefined,
    projectId: "semantic-lifecycle",
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

function queryEngine(engine, target, sources, inventoryVersion, analysisGeneration) {
  const source = sources.get(target.fileId);
  if (!source) throw new Error(`missing query document ${target.fileId}`);
  const first = source.content.indexOf(target.needle);
  if (first < 0 || first !== source.content.lastIndexOf(target.needle)) {
    throw new Error(`query needle must occur exactly once: ${target.needle}`);
  }
  return decode(
    engine.query(
      encode({
        analysisGeneration,
        documentVersion: source.documentVersion,
        epoch: "semantic-lifecycle",
        inventoryVersion,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
        query: { fileId: target.fileId, kind: "semanticView", offset: first },
      }),
    ),
  );
}

function assertDecision(result, expected, label) {
  const value = result?.value;
  const status = value?.kind === "semanticView"
    ? value.view.formulaAnalysis.disposition
    : "missing";
  const matches = expected === "not-established" ? status !== "established" : status === expected;
  if (!matches) throw new Error(`${label}: expected ${expected}, observed ${status}`);
}

function assertDomains(result, expected, label) {
  if (expected === undefined) return;
  const value = result?.value;
  const domains = value?.kind === "semanticView"
    ? value.view.domains.map(({ packId, support }) => ({ packId, support })).slice(0, expected.length)
    : [];
  if (JSON.stringify(domains) !== JSON.stringify(expected)) {
    throw new Error(`${label}: expected domains ${JSON.stringify(expected)}, observed ${JSON.stringify(domains)}`);
  }
}

function resetEngine(engine, snapshot) {
  const { documents, ...metadata } = snapshot;
  engine.beginReset(encode(metadata));
  for (const document of documents) engine.ingestResetDocument(encode(document));
  return decode(engine.finishReset());
}

function encode(value) {
  return encoder.encode(JSON.stringify(value));
}

function decode(value) {
  return JSON.parse(decoder.decode(value));
}
