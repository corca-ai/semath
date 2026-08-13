import { readFile, writeFile } from "node:fs/promises";
import { LatexSyntaxService } from "wasmtex/syntax";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import {
  firstDifferentialFailure,
  type AuthoredScientificProbe,
  type AuthoredScientificSnapshot,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
  type SemathQuery,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { loadFreshBlindEvidence } from "./fresh-blind-evidence";
import { completeLifecycleUpsertIds } from "./fresh-blind-lifecycle-plan";
import { semanticEvaluationCursorOffset } from "./semantic-evaluation-runner";

const path = process.env.SEMATH_FRESH_BLIND_FIXTURE;
if (!path) {
  throw new Error(
    "SEMATH_FRESH_BLIND_FIXTURE must name the sealed fixture explicitly",
  );
}
const evidence = await loadFreshBlindEvidence(path);
await init({
  module_or_path: await readFile(
    new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
  ),
});

let comparedProbes = 0;
let comparedStages = 0;
for (const scenario of evidence.release.fixture.scenarios) {
  if (scenario.snapshots.length < 2) continue;
  const epoch = `fresh-blind-lifecycle/${scenario.id}`;
  const sources = sourceMap(scenario.snapshots[0]!, 1);
  const syntax = new LatexSyntaxService();
  syntax.reset({ documents: [...sources.values()] });
  const incremental = new SemathEngine();
  reset(incremental, project(epoch, sources, syntax, 1));
  compareSnapshot(
    incremental,
    scenario.snapshots[0]!,
    scenario.id,
    evidence.release.fixture.probes,
    sources,
    syntax,
    epoch,
    1,
    0,
  );

  let inventoryVersion = 1;
  let analysisGeneration = 0;
  for (const snapshot of scenario.snapshots.slice(1)) {
    inventoryVersion += 1;
    analysisGeneration += 1;
    const changes = transition(sources, syntax, snapshot);
    incremental.applyChanges(
      encode({
        analysisGeneration,
        changes,
        epoch,
        inventoryVersion,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      }),
    );
    comparedProbes += compareSnapshot(
      incremental,
      snapshot,
      scenario.id,
      evidence.release.fixture.probes,
      sources,
      syntax,
      epoch,
      inventoryVersion,
      analysisGeneration,
    );
    comparedStages += 1;
  }
  incremental.free();
}

const report = {
  comparedProbes,
  comparedStages,
  fixtureId: evidence.release.release.id,
  fixtureSeal: evidence.release.release.seal,
  schemaVersion: 1,
};
if (process.env.SEMATH_FRESH_BLIND_LIFECYCLE_REPORT) {
  await writeFile(
    process.env.SEMATH_FRESH_BLIND_LIFECYCLE_REPORT,
    `${JSON.stringify(report, null, 2)}\n`,
  );
}
console.log(
  `fresh blind lifecycle OK: ${comparedStages} clean/incremental stages, ${comparedProbes} probes`,
);

interface SourceDocument {
  readonly content: string;
  readonly documentVersion: number;
  readonly fileId: string;
  readonly language: "latex" | "markdown";
  readonly path: string;
}

function sourceMap(
  snapshot: AuthoredScientificSnapshot,
  documentVersion: number,
): Map<string, SourceDocument> {
  return new Map(
    snapshot.documents.map((document) => [
      document.fileId,
      {
        ...document,
        documentVersion,
        language: languageOf(document.path),
      },
    ]),
  );
}

function transition(
  sources: Map<string, SourceDocument>,
  syntax: LatexSyntaxService,
  snapshot: AuthoredScientificSnapshot,
) {
  const target = new Map(snapshot.documents.map((document) => [document.fileId, document]));
  const changes: Array<
    | { readonly fileId: string; readonly kind: "remove" }
    | { readonly fileId: string; readonly kind: "path-change"; readonly path: string }
  > = [];
  const explicitlyChanged = new Set<string>();
  const directlyChanged = new Set<string>();
  for (const fileId of sources.keys()) {
    if (target.has(fileId)) continue;
    sources.delete(fileId);
    syntax.remove(fileId);
    explicitlyChanged.add(fileId);
    changes.push({ fileId, kind: "remove" });
  }
  for (const document of snapshot.documents) {
    const previous = sources.get(document.fileId);
    if (previous?.content === document.content && previous.path === document.path) continue;
    if (previous?.content === document.content) {
      const moved = { ...previous, path: document.path };
      sources.set(document.fileId, moved);
      syntax.move(document.fileId, document.path);
      explicitlyChanged.add(document.fileId);
      changes.push({ fileId: document.fileId, kind: "path-change", path: document.path });
      continue;
    }
    const source = {
      ...document,
      documentVersion: (previous?.documentVersion ?? 0) + 1,
      language: languageOf(document.path),
    };
    sources.set(document.fileId, source);
    syntax.upsert(source);
    directlyChanged.add(document.fileId);
  }
  const invalidated = syntax.getInvalidatedFiles();
  const invalidatedById = new Map(invalidated.map((file) => [file.fileId, file]));
  const upserts = completeLifecycleUpsertIds(
    directlyChanged,
    invalidated.map((file) => file.fileId),
  ).flatMap((fileId) => {
    if (explicitlyChanged.has(fileId)) return [];
    const source = sources.get(fileId);
    if (!source) return [];
    const fileSyntax = invalidatedById.get(fileId) ?? syntax.getFile(fileId);
    if (!fileSyntax) throw new Error(`${fileId}: missing wasmtex syntax after upsert`);
    return [
      {
        document: adaptWasmtexDocument({
          content: source.content,
          language: source.language,
          syntax: fileSyntax,
        }),
        kind: "upsert" as const,
      },
    ];
  });
  return [...changes, ...upserts];
}

function compareSnapshot(
  incremental: SemathEngine,
  snapshot: AuthoredScientificSnapshot,
  scenarioId: string,
  probes: readonly AuthoredScientificProbe[],
  sources: Map<string, SourceDocument>,
  syntax: LatexSyntaxService,
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
): number {
  const clean = new SemathEngine();
  const cleanSyntax = new LatexSyntaxService();
  cleanSyntax.reset({ documents: [...sources.values()] });
  reset(clean, project(epoch, sources, cleanSyntax, inventoryVersion));
  const selected = probes.filter(
    (probe) => probe.scenarioId === scenarioId && probe.cursor.snapshotId === snapshot.id,
  );
  for (const probe of selected) {
    const queries = queriesFor(probe, sources);
    for (const query of queries) {
      let incrementalResult: QueryResult;
      let cleanResult: QueryResult;
      try {
        incrementalResult = queryEngine(
          incremental,
          query,
          sources,
          epoch,
          inventoryVersion,
          analysisGeneration,
        );
        cleanResult = queryEngine(
          clean,
          query,
          sources,
          epoch,
          inventoryVersion,
          analysisGeneration,
        );
      } catch (error) {
        throw new Error(
          `${scenarioId}/${snapshot.id}/${probe.id}/${query.kind}: ${error instanceof Error ? error.message : String(error)}`,
          { cause: error },
        );
      }
      const failure = firstDifferentialFailure([
        { name: "clean", value: cleanResult.value },
        { name: "incremental", value: incrementalResult.value },
      ]);
      if (failure) {
        throw new Error(
          `${scenarioId}/${snapshot.id}/${probe.id}/${query.kind}: ` +
            `${failure.stage} diverged at ${failure.path}`,
        );
      }
    }
  }
  clean.free();
  return selected.length;
}

function queriesFor(
  probe: AuthoredScientificProbe,
  sources: Map<string, SourceDocument>,
): readonly SemathQuery[] {
  const document = sources.get(probe.cursor.fileId);
  if (!document) throw new Error(`${probe.id}: missing cursor document`);
  const offset = semanticEvaluationCursorOffset(document.content, probe.cursor);
  const target = { fileId: probe.cursor.fileId, offset };
  return [
    { ...target, kind: "semanticView" },
    { ...target, kind: "definition" },
    { ...target, kind: "references" },
    { ...target, kind: "prepareRename" },
    { ...target, kind: "rename", newName: "z" },
    { fileId: probe.cursor.fileId, kind: "diagnostics" },
  ];
}

function project(
  epoch: string,
  sources: Map<string, SourceDocument>,
  syntax: LatexSyntaxService,
  inventoryVersion: number,
): ProjectSnapshot {
  const documents: ProjectDocument[] = [...sources.values()].map((source) => {
    const parsed = syntax.getFile(source.fileId);
    if (!parsed) throw new Error(`${source.fileId}: missing wasmtex syntax`);
    return adaptWasmtexDocument({
      content: source.content,
      language: source.language,
      syntax: parsed,
    });
  });
  const mainFileId = documents[0]?.fileId;
  if (!mainFileId) throw new Error(`${epoch}: lifecycle snapshot has no documents`);
  return {
    documents,
    epoch,
    inventoryVersion,
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

function queryEngine(
  engine: SemathEngine,
  query: SemathQuery,
  sources: Map<string, SourceDocument>,
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
): QueryResult {
  const document = sources.get(query.fileId);
  if (!document) throw new Error(`${query.fileId}: missing query document`);
  const envelope: QueryEnvelope = {
    analysisGeneration,
    documentVersion: document.documentVersion,
    epoch,
    inventoryVersion,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query,
  };
  return decode(engine.query(encode(envelope)));
}

function encode(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value));
}

function decode<T>(value: Uint8Array): T {
  return JSON.parse(new TextDecoder().decode(value)) as T;
}

function languageOf(path: string): "latex" | "markdown" {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
