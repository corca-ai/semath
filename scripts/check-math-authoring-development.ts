import { readFile, writeFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import {
  evaluateMathAuthoringDevelopment,
  observeAuthoredMathAuthoringContext,
  parseMathAuthoringDevelopmentFixture,
  type MathAuthoringDevelopmentCase,
  type MathAuthoringDevelopmentDocument,
  type MathAuthoringDevelopmentObservation,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";

const fixture = parseMathAuthoringDevelopmentFixture(
  JSON.parse(
    await readFile(
      "fixtures/development/math-authoring-context-v1.json",
      "utf8",
    ),
  ),
);
await init({
  module_or_path: await readFile(
    new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
  ),
});

const observations = fixture.cases.map(runCase);
if (process.env.SEMATH_MATH_AUTHORING_REPORT) {
  await writeFile(
    process.env.SEMATH_MATH_AUTHORING_REPORT,
    `${JSON.stringify({ observations }, null, 2)}\n`,
  );
}
if (process.env.SEMATH_MATH_AUTHORING_ALLOW_FAILURES === "1") {
  console.log(`math-authoring development observations: ${observations.length}`);
} else {
  const summary = evaluateMathAuthoringDevelopment(fixture, observations);
  console.log(
    `math-authoring development gate OK: ${summary.cases} cases; ${summary.coveredFeatures.length} safety features`,
  );
}

function runCase(
  item: MathAuthoringDevelopmentCase,
): MathAuthoringDevelopmentObservation {
  const epoch = `math-authoring-development/${item.id}`;
  const engine = new SemathEngine();
  const initial = project(item.documents, item.mainFileId, epoch, 1);
  reset(engine, initial);
  let documents = item.documents;
  let documentVersion = 1;
  let inventoryVersion = 1;
  let analysisGeneration = 0;
  let staleRevisionRejected = false;
  if (item.kind === "revision") {
    documents = item.revisedDocuments;
    documentVersion = 2;
    inventoryVersion = 2;
    analysisGeneration = 1;
    const revised = project(documents, item.mainFileId, epoch, documentVersion);
    engine.applyChanges(
      encode({
        analysisGeneration,
        changes: revised.documents.map((document) => ({
          document,
          kind: "upsert" as const,
        })),
        epoch,
        inventoryVersion,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      }),
    );
    try {
      query(
        engine,
        item,
        documents,
        epoch,
        inventoryVersion,
        analysisGeneration,
        item.staleDocumentVersion,
        "semanticView",
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!/document|version|stale/iu.test(message)) {
        throw new Error(`${item.id}: unexpected stale-revision error: ${message}`);
      }
      staleRevisionRejected = true;
    }
  }
  const semantic = query(
    engine,
    item,
    documents,
    epoch,
    inventoryVersion,
    analysisGeneration,
    documentVersion,
    "semanticView",
  );
  const definition = query(
    engine,
    item,
    documents,
    epoch,
    inventoryVersion,
    analysisGeneration,
    documentVersion,
    "definition",
  );
  engine.free();
  if (semantic.value.kind !== "semanticView") {
    throw new Error(`${item.id}: semantic view is unavailable`);
  }
  const definitionAuthorized =
    definition.value.kind === "locations" &&
    definition.value.authorization.status === "authorized";
  return {
    caseId: item.id,
    context: observeAuthoredMathAuthoringContext(
      item.cursor.fileId,
      semantic.value.view.authoringContext,
      { documents, id: item.id },
    ),
    definitionAuthorized,
    staleRevisionRejected,
  };
}

function project(
  sources: readonly MathAuthoringDevelopmentDocument[],
  mainFileId: string,
  epoch: string,
  documentVersion: number,
): ProjectSnapshot {
  const syntax = new LatexSyntaxService();
  const input = sources.map((source) => ({
    ...source,
    documentVersion,
    language: languageOf(source.path),
  }));
  syntax.reset({ documents: input });
  const documents: ProjectDocument[] = input.map((source) => {
    const parsed = syntax.getFile(source.fileId);
    if (!parsed) throw new Error(`${source.fileId}: missing neutral syntax`);
    return adaptWasmtexDocument({
      content: source.content,
      language: source.language,
      syntax: parsed,
    });
  });
  return {
    documents,
    epoch,
    inventoryVersion: documentVersion,
    mainFileId,
    projectId: epoch,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

function reset(engine: SemathEngine, snapshot: ProjectSnapshot): void {
  const { documents, ...metadata } = snapshot;
  engine.beginReset(encode(metadata));
  for (const document of documents) engine.ingestResetDocument(encode(document));
  engine.finishReset();
}

function query(
  engine: SemathEngine,
  item: MathAuthoringDevelopmentCase,
  documents: readonly MathAuthoringDevelopmentDocument[],
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
  documentVersion: number,
  kind: "definition" | "semanticView",
): QueryResult {
  const document = documents.find(
    (candidate) => candidate.fileId === item.cursor.fileId,
  );
  if (!document) throw new Error(`${item.id}: missing cursor document`);
  const offset = anchorOffset(document.content, item.cursor);
  const envelope: QueryEnvelope = {
    analysisGeneration,
    documentVersion,
    epoch,
    inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: { fileId: document.fileId, kind, offset },
  };
  return decode(engine.query(encode(envelope)));
}

function anchorOffset(
  content: string,
  anchor: MathAuthoringDevelopmentCase["cursor"],
): number {
  let start = -1;
  let from = 0;
  for (let index = 0; index <= (anchor.occurrence ?? 0); index += 1) {
    start = content.indexOf(anchor.needle, from);
    if (start < 0) throw new Error(`missing cursor anchor ${anchor.needle}`);
    from = start + anchor.needle.length;
  }
  return (
    start +
    (anchor.selection?.offset ?? Math.max(0, anchor.needle.length - 1))
  );
}

function encode(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value));
}

function decode(value: Uint8Array): QueryResult {
  return JSON.parse(new TextDecoder().decode(value)) as QueryResult;
}

function languageOf(path: string): "latex" | "markdown" {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
