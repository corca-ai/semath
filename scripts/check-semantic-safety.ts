import { spawnSync } from "node:child_process";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  observeSemanticSafetyCase,
  planSemanticSafetySuite,
  scoreSemanticSafetySuite,
  semanticSafetyCursorOffset,
  type PlannedSemanticSafetyCase,
  type SemanticSafetyObservation,
  type SemanticSafetySurfaceResults,
} from "../packages/evaluation/src/semantic-safety";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
  type SemathQuery,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";
import { loadSemanticSafetySpec } from "./check-semantic-safety-fixture";
import { runSemanticSafetyLifecycle } from "./semantic-safety-lifecycle-runner";

interface PlannedQueries {
  readonly item: PlannedSemanticSafetyCase;
  readonly resultIndexes: {
    readonly definition?: number;
    readonly diagnostics: number;
    readonly prepareRename?: number;
    readonly references?: number;
    readonly rename?: number;
    readonly semanticView: number;
  };
}

const spec = await loadSemanticSafetySpec();
const plan = planSemanticSafetySuite(spec);
const cleanObservations = plan.map(runIsolatedNativeCase);
const lifecycle = await runSemanticSafetyLifecycle(spec, plan);
const lifecycleById = new Map(
  lifecycle.observations.map((observation) => [observation.caseId, observation]),
);
const observations = cleanObservations.map(
  (observation) => lifecycleById.get(observation.caseId) ?? observation,
);

function runIsolatedNativeCase(item: PlannedSemanticSafetyCase): SemanticSafetyObservation {
  const { documents, plannedQueries, queries } = planNativeRun([item]);
  const native = spawnSync(
    "cargo",
    ["run", "--quiet", "--locked", "-p", "semath-native"],
    {
      encoding: "utf8",
      input: JSON.stringify({
        queries,
        snapshot: {
          documents,
          epoch: spec.id,
          inventoryVersion: 1,
          projectId: item.id,
          protocolVersion: SEMATH_PROTOCOL_VERSION,
        },
      }),
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (native.status !== 0) {
    throw new Error(native.stderr || `${item.id}: semantic safety native evaluation failed`);
  }
  const results: unknown = JSON.parse(native.stdout);
  if (!Array.isArray(results) || results.length !== queries.length) {
    throw new Error(
      `${item.id}: semantic safety native evaluation returned ${
        Array.isArray(results) ? results.length : "invalid"
      }/${queries.length} results`,
    );
  }
  const planned = plannedQueries[0];
  if (!planned) throw new Error(`${item.id}: missing planned queries`);
  return observeSemanticSafetyCase(
    planned.item,
    pickResults(results as QueryResult[], planned.resultIndexes),
  );
}
const score = scoreSemanticSafetySuite(spec, plan, observations);
const safetyFailures = [
  ...score.safetyFailures,
  ...lifecycle.safetyFailures,
];
const contractFailures = [
  ...score.contractFailures,
  ...lifecycle.contractFailures,
];
const failures = [...safetyFailures, ...contractFailures];
console.log(
  `semantic safety development gate: ${score.passed}/${score.cases}; ${safetyFailures.length} safety failures, ${contractFailures.length} contract failures; ${lifecycle.comparedTransitions} clean/incremental lifecycle transitions`,
);
if (process.env.SEMATH_SAFETY_REPORT) {
  await Bun.write(
    process.env.SEMATH_SAFETY_REPORT,
    `${JSON.stringify({ ...score, contractFailures, failures, lifecycle, observations, safetyFailures }, null, 2)}\n`,
  );
}
if (failures.length && process.env.SEMATH_SAFETY_ALLOW_FAILURES !== "1") {
  throw new Error(`semantic safety gate failed:\n${failures.join("\n")}`);
}

function planNativeRun(plan: readonly PlannedSemanticSafetyCase[]): {
  readonly documents: ProjectDocument[];
  readonly plannedQueries: PlannedQueries[];
  readonly queries: QueryEnvelope[];
} {
  const documents: ProjectDocument[] = [];
  const queries: QueryEnvelope[] = [];
  const plannedQueries: PlannedQueries[] = [];
  for (const item of plan) {
    const prefix = `${item.id}/`;
    const inputs = item.documents.map((document) => ({
      ...document,
      fileId: prefix + document.fileId,
      path: prefix + document.path,
    }));
    const syntax = new LatexSyntaxService();
    syntax.reset({
      documents: inputs.map((document) => ({
        ...document,
        documentVersion: 1,
      })),
    });
    for (const input of inputs) {
      const snapshot = syntax.getFile(input.fileId);
      if (!snapshot) throw new Error(`${item.id}: missing syntax for ${input.fileId}`);
      documents.push(
        adaptWasmtexDocument({
          content: input.content,
          language: "latex",
          syntax: snapshot,
        }),
      );
    }
    const resultIndexes: Mutable<PlannedQueries["resultIndexes"]> = {
      semanticView: pushQuery(
        item,
        prefix,
        item.semanticCursor,
        { kind: "semanticView" },
        queries,
      ),
      diagnostics: pushQuery(
        item,
        prefix,
        item.semanticCursor,
        { kind: "diagnostics" },
        queries,
      ),
    };
    if (item.navigationCursor) {
      resultIndexes.definition = pushQuery(
        item,
        prefix,
        item.navigationCursor,
        { kind: "definition" },
        queries,
      );
      resultIndexes.references = pushQuery(
        item,
        prefix,
        item.navigationCursor,
        { kind: "references" },
        queries,
      );
      resultIndexes.prepareRename = pushQuery(
        item,
        prefix,
        item.navigationCursor,
        { kind: "prepareRename" },
        queries,
      );
      resultIndexes.rename = pushQuery(
        item,
        prefix,
        item.navigationCursor,
        {
          kind: "rename",
          newName:
            item.expected.navigation.mode === "exact"
              ? item.expected.navigation.newName
              : "z",
        },
        queries,
      );
    }
    plannedQueries.push({ item, resultIndexes });
  }
  return { documents, plannedQueries, queries };
}

function pushQuery(
  item: PlannedSemanticSafetyCase,
  prefix: string,
  cursor: PlannedSemanticSafetyCase["semanticCursor"],
  query:
    | {
        readonly kind:
          | "definition"
          | "diagnostics"
          | "prepareRename"
          | "references"
          | "semanticView";
      }
    | { readonly kind: "rename"; readonly newName: string },
  queries: QueryEnvelope[],
): number {
  const index = queries.length;
  const offset = semanticSafetyCursorOffset(item.documents, cursor);
  queries.push({
    analysisGeneration: 0,
    documentVersion: 1,
    epoch: spec.id,
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: {
      ...query,
      fileId: prefix + cursor.fileId,
      ...(query.kind === "diagnostics" ? {} : { offset }),
    } as SemathQuery,
  });
  return index;
}

function pickResults(
  results: readonly QueryResult[],
  indexes: PlannedQueries["resultIndexes"],
): SemanticSafetySurfaceResults {
  return {
    ...(indexes.definition === undefined
      ? {}
      : { definition: results[indexes.definition]! }),
    diagnostics: results[indexes.diagnostics]!,
    ...(indexes.prepareRename === undefined
      ? {}
      : { prepareRename: results[indexes.prepareRename]! }),
    ...(indexes.references === undefined
      ? {}
      : { references: results[indexes.references]! }),
    ...(indexes.rename === undefined
      ? {}
      : { rename: results[indexes.rename]! }),
    semanticView: results[indexes.semanticView]!,
  };
}

type Mutable<T> = { -readonly [K in keyof T]: T[K] };
