import { readFile } from "node:fs/promises";
import init, { SemathEngine } from "../lib/wasm/semath_wasm.js";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  observeSemanticSafetyCase,
  semanticSafetyCursorOffset,
  type PlannedSemanticSafetyCase,
  type SemanticSafetyObservation,
  type SemanticSafetySpec,
  type SemanticSafetySurfaceResults,
} from "../packages/evaluation/src/semantic-safety";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryResult,
  type SemathQuery,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";

interface VersionedSource {
  readonly content: string;
  readonly documentVersion: number;
  readonly fileId: string;
  readonly path: string;
}

export interface SemanticSafetyLifecycleResult {
  readonly comparedTransitions: number;
  readonly contractFailures: readonly string[];
  readonly failures: readonly string[];
  readonly observations: readonly SemanticSafetyObservation[];
  readonly safetyFailures: readonly string[];
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export async function runSemanticSafetyLifecycle(
  spec: SemanticSafetySpec,
  plan: readonly PlannedSemanticSafetyCase[],
): Promise<SemanticSafetyLifecycleResult> {
  await init({
    module_or_path: await readFile(
      new URL("../lib/wasm/semath_wasm_bg.wasm", import.meta.url),
    ),
  });
  const contractFailures: string[] = [];
  const safetyFailures: string[] = [];
  const observations: SemanticSafetyObservation[] = [];
  let comparedTransitions = 0;
  for (const sourceCase of spec.cases) {
    for (const transition of sourceCase.transitions) {
      for (const transform of ["identity", ...sourceCase.transforms] as const) {
        const before = findPlanCase(
          plan,
          sourceCase.id,
          transition.fromProbeId,
          transform,
        );
        const after = findPlanCase(
          plan,
          sourceCase.id,
          transition.toProbeId,
          transform,
        );
        const label = `${sourceCase.id}/${transition.fromProbeId}->${transition.toProbeId}@${transform}`;
        const incremental = new SemathEngine();
        const beforeSources = new Map(
          before.documents.map((document) => [
            document.fileId,
            { ...document, documentVersion: 1 },
          ]),
        );
        const beforeSyntax = syntaxFrom(beforeSources);
        resetEngine(
          incremental,
          snapshotFrom(beforeSources, beforeSyntax, `${spec.id}-lifecycle`, 1),
        );
        const beforeResults = querySurfaces(
          incremental,
          before,
          beforeSources,
          `${spec.id}-lifecycle`,
          1,
          0,
        );
        const beforeObservation = observeSemanticSafetyCase(before, beforeResults);

        const { changes, sources: afterSources, syntax: afterSyntax } = planChanges(
          beforeSources,
          after.documents,
        );
        applyChanges(
          incremental,
          changes,
          `${spec.id}-lifecycle`,
          2,
          1,
        );
        const incrementalResults = querySurfaces(
          incremental,
          after,
          afterSources,
          `${spec.id}-lifecycle`,
          2,
          1,
        );

        const clean = new SemathEngine();
        resetEngine(
          clean,
          snapshotFrom(afterSources, afterSyntax, `${spec.id}-lifecycle`, 2),
        );
        const cleanResults = querySurfaces(
          clean,
          after,
          afterSources,
          `${spec.id}-lifecycle`,
          2,
          1,
        );
        for (const surface of surfaceNames(after)) {
          const incrementalValue = incrementalResults[surface]!.value;
          const cleanValue = cleanResults[surface]!.value;
          if (JSON.stringify(incrementalValue) !== JSON.stringify(cleanValue)) {
            safetyFailures.push(`${label}: ${surface} clean/incremental values differ`);
          }
        }
        const afterObservation = observeSemanticSafetyCase(
          after,
          incrementalResults,
        );
        if (
          beforeObservation.decision !== "established" ||
          !beforeObservation.proofGrounded
        ) {
          contractFailures.push(
            `${label}: transition source is not source-grounded established meaning for ${transition.relationId}`,
          );
        }
        if (
          afterObservation.meaningRelationId === transition.relationId ||
          afterObservation.relations.some(
            (relation) => relationLeaf(relation.relationId) === transition.relationId,
          )
        ) {
          safetyFailures.push(`${label}: incremental result retained ${transition.relationId}`);
        }
        observations.push(beforeObservation, afterObservation);
        incremental.free();
        clean.free();
        comparedTransitions += 1;
      }
    }
  }
  return {
    comparedTransitions,
    contractFailures,
    failures: [...safetyFailures, ...contractFailures],
    observations,
    safetyFailures,
  };
}

function planChanges(
  before: ReadonlyMap<string, VersionedSource>,
  targetDocuments: PlannedSemanticSafetyCase["documents"],
): {
  readonly changes: ChangeEnvelope["changes"];
  readonly sources: Map<string, VersionedSource>;
  readonly syntax: LatexSyntaxService;
} {
  const sources = new Map<string, VersionedSource>();
  for (const document of targetDocuments) {
    const previous = before.get(document.fileId);
    sources.set(document.fileId, {
      ...document,
      documentVersion:
        previous &&
        previous.content === document.content &&
        previous.path === document.path
          ? previous.documentVersion
          : (previous?.documentVersion ?? 0) + 1,
    });
  }
  const syntax = syntaxFrom(before);
  const removals = [...before.keys()]
    .filter((fileId) => !sources.has(fileId))
    .map((fileId) => {
      syntax.remove(fileId);
      return { fileId, kind: "remove" as const };
    });
  for (const [fileId, source] of sources) {
    const previous = before.get(fileId);
    if (
      !previous ||
      previous.content !== source.content ||
      previous.path !== source.path
    ) {
      syntax.upsert({ ...source, language: "latex" });
    }
  }
  const upserts = syntax.getInvalidatedFiles().flatMap((fileSyntax) => {
    const source = sources.get(fileSyntax.fileId);
    if (!source) return [];
    return [{
      document: adaptWasmtexDocument({
        content: source.content,
        language: "latex",
        syntax: fileSyntax,
      }),
      kind: "upsert" as const,
    }];
  });
  return { changes: [...removals, ...upserts], sources, syntax };
}

function syntaxFrom(
  sources: ReadonlyMap<string, VersionedSource>,
): LatexSyntaxService {
  const syntax = new LatexSyntaxService();
  syntax.reset({
    documents: [...sources.values()].map((source) => ({
      ...source,
      language: "latex" as const,
    })),
  });
  return syntax;
}

function snapshotFrom(
  sources: ReadonlyMap<string, VersionedSource>,
  syntax: LatexSyntaxService,
  epoch: string,
  inventoryVersion: number,
): ProjectSnapshot {
  return {
    documents: [...sources.values()].map((source) =>
      adaptedDocument(source, syntax),
    ),
    epoch,
    inventoryVersion,
    projectId: epoch,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}

function adaptedDocument(
  source: VersionedSource,
  syntax: LatexSyntaxService,
): ProjectDocument {
  const fileSyntax = syntax.getFile(source.fileId);
  if (!fileSyntax) throw new Error(`missing syntax for ${source.fileId}`);
  return adaptWasmtexDocument({
    content: source.content,
    language: "latex",
    syntax: fileSyntax,
  });
}

function resetEngine(engine: SemathEngine, snapshot: ProjectSnapshot): void {
  const { documents, ...metadata } = snapshot;
  engine.beginReset(encode(metadata));
  for (const document of documents) {
    engine.ingestResetDocument(encode(document));
  }
  decode(engine.finishReset());
}

function applyChanges(
  engine: SemathEngine,
  changes: ChangeEnvelope["changes"],
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
): void {
  decode(
    engine.applyChanges(
      encode({
        analysisGeneration,
        changes,
        epoch,
        inventoryVersion,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
      } satisfies ChangeEnvelope),
    ),
  );
}

function querySurfaces(
  engine: SemathEngine,
  item: PlannedSemanticSafetyCase,
  sources: ReadonlyMap<string, VersionedSource>,
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
): SemanticSafetySurfaceResults {
  const semanticView = query(
    engine,
    item,
    item.semanticCursor,
    sources,
    { kind: "semanticView" },
    epoch,
    inventoryVersion,
    analysisGeneration,
  );
  const diagnostics = query(
    engine,
    item,
    item.semanticCursor,
    sources,
    { kind: "diagnostics" },
    epoch,
    inventoryVersion,
    analysisGeneration,
  );
  if (!item.navigationCursor) return { diagnostics, semanticView };
  const cursor = item.navigationCursor;
  return {
    definition: query(
      engine,
      item,
      cursor,
      sources,
      { kind: "definition" },
      epoch,
      inventoryVersion,
      analysisGeneration,
    ),
    diagnostics,
    prepareRename: query(
      engine,
      item,
      cursor,
      sources,
      { kind: "prepareRename" },
      epoch,
      inventoryVersion,
      analysisGeneration,
    ),
    references: query(
      engine,
      item,
      cursor,
      sources,
      { kind: "references" },
      epoch,
      inventoryVersion,
      analysisGeneration,
    ),
    rename: query(
      engine,
      item,
      cursor,
      sources,
      {
        kind: "rename",
        newName:
          item.expected.navigation.mode === "exact"
            ? item.expected.navigation.newName
            : "z",
      },
      epoch,
      inventoryVersion,
      analysisGeneration,
    ),
    semanticView,
  };
}

function query(
  engine: SemathEngine,
  item: PlannedSemanticSafetyCase,
  cursor: PlannedSemanticSafetyCase["semanticCursor"],
  sources: ReadonlyMap<string, VersionedSource>,
  request:
    | {
        readonly kind:
          | "definition"
          | "diagnostics"
          | "prepareRename"
          | "references"
          | "semanticView";
      }
    | { readonly kind: "rename"; readonly newName: string },
  epoch: string,
  inventoryVersion: number,
  analysisGeneration: number,
): QueryResult {
  const source = sources.get(cursor.fileId);
  if (!source) throw new Error(`${item.id}: missing ${cursor.fileId}`);
  const offset = semanticSafetyCursorOffset(item.documents, cursor);
  const semanticQuery: SemathQuery = {
    ...request,
    fileId: cursor.fileId,
    ...(request.kind === "diagnostics" ? {} : { offset }),
  } as SemathQuery;
  return decode(
    engine.query(
      encode({
        analysisGeneration,
        documentVersion: source.documentVersion,
        epoch,
        inventoryVersion,
        protocolVersion: SEMATH_PROTOCOL_VERSION,
        query: semanticQuery,
      }),
    ),
  ) as QueryResult;
}

function surfaceNames(
  item: PlannedSemanticSafetyCase,
): readonly (keyof SemanticSafetySurfaceResults)[] {
  return item.navigationCursor
    ? [
        "semanticView",
        "diagnostics",
        "definition",
        "references",
        "prepareRename",
        "rename",
      ]
    : ["semanticView", "diagnostics"];
}

function findPlanCase(
  plan: readonly PlannedSemanticSafetyCase[],
  sourceCaseId: string,
  probeId: string,
  transform: PlannedSemanticSafetyCase["transform"],
): PlannedSemanticSafetyCase {
  const item = plan.find(
    (candidate) =>
      candidate.sourceCaseId === sourceCaseId &&
      candidate.probeId === probeId &&
      candidate.transform === transform,
  );
  if (!item) throw new Error(`missing lifecycle plan ${sourceCaseId}/${probeId}@${transform}`);
  return item;
}

function relationLeaf(relationId: string): string {
  return relationId.slice(relationId.lastIndexOf(":") + 1);
}

function encode(value: unknown): Uint8Array {
  return encoder.encode(JSON.stringify(value));
}

function decode(value: Uint8Array): unknown {
  return JSON.parse(decoder.decode(value));
}
