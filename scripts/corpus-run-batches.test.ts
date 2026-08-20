import { describe, expect, test } from "bun:test";
import {
  observeQualityRun,
  type PlannedQualityCase,
  type QualityRunPlan,
} from "../packages/evaluation/src/index";
import {
  SEMATH_PROTOCOL_VERSION,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import {
  flattenCorpusRunValues,
  planCorpusRunBatches,
  qualityRunPlanForBatch,
} from "./corpus-run-batches";

describe("corpus run batches", () => {
  test("isolates suite documents without changing global case order", () => {
    const plan = qualityPlan([
      ["mechanics", "base-a"],
      ["mechanics-diversity", "base-b"],
      ["mechanics", "meta-a"],
      ["mechanics-diversity", "meta-b"],
    ]);

    const batches = planCorpusRunBatches(plan);

    expect(batches.map((batch) => batch.suiteId)).toEqual([
      "mechanics",
      "mechanics-diversity",
    ]);
    expect(batches.map((batch) => batch.indices)).toEqual([[0, 2], [1, 3]]);
    expect(batches[0]?.snapshot.documents.map((document) => document.fileId)).toEqual([
      "mechanics/base-a/main",
      "mechanics/meta-a/main",
    ]);
    expect(batches[1]?.snapshot.documents.map((document) => document.fileId)).toEqual([
      "mechanics-diversity/base-b/main",
      "mechanics-diversity/meta-b/main",
    ]);
    expect(plan.snapshot.documents).toHaveLength(4);

    const batchObservations = batches.map((batch) =>
      observeQualityRun(
        qualityRunPlanForBatch(plan, batch),
        batch.indices.map((index) => queryResult(plan.planned[index]!.case.id)),
      )
    );
    const observations = flattenCorpusRunValues(
      plan.planned.length,
      batches,
      batchObservations,
    );
    expect(observations.map((observation) => observation.caseId)).toEqual([
      "base-a",
      "base-b",
      "meta-a",
      "meta-b",
    ]);
  });

  test("rejects missing batch results instead of shifting later observations", () => {
    const plan = qualityPlan([
      ["suite-a", "case-a"],
      ["suite-a", "case-a2"],
      ["suite-b", "case-b"],
      ["suite-b", "case-b2"],
    ]);
    const batches = planCorpusRunBatches(plan);

    expect(() =>
      flattenCorpusRunValues(plan.planned.length, batches, [
        ["case-a"],
        ["case-b", "case-b2"],
      ])
    ).toThrow("suite-a: quality run returned 1/2 results");
  });

  test("refuses isolation that would change project-wide engine semantics", () => {
    const singletonSuites = qualityPlan([
      ["suite-a", "case-a"],
      ["suite-b", "case-b"],
    ]);
    expect(() => planCorpusRunBatches(singletonSuites)).toThrow(
      "suite-a: cannot isolate a singleton suite without changing project semantics",
    );

    const mainFilePlan = qualityPlan([
      ["suite-a", "case-a"],
      ["suite-a", "case-a2"],
    ]);
    expect(() =>
      planCorpusRunBatches({
        ...mainFilePlan,
        snapshot: { ...mainFilePlan.snapshot, mainFileId: "suite-a/case-a/main" },
      })
    ).toThrow("quality corpus batching requires a project without a main file");
  });
});

function qualityPlan(entries: readonly (readonly [string, string])[]): QualityRunPlan {
  const planned: PlannedQualityCase[] = entries.map(([suiteId, caseId]) => ({
    case: {
      cursor: { fileId: "main", needle: "x" },
      diversity: {
        batch: "test",
        mutationFamily: "test",
        projectTopology: "single-file",
        proseFamily: "test",
        semanticSkeleton: "test",
        syntaxStructure: "test",
      },
      documents: [{ content: "x", fileId: "main", path: "main.tex" }],
      expectation: "refused",
      id: caseId,
      refusalCategory: "insufficient-evidence",
      variationTags: [],
    },
    suiteId,
  }));
  return {
    planned,
    queries: entries.map(([suiteId, caseId]) => query(`${suiteId}/${caseId}/main`)),
    snapshot: {
      documents: entries.map(([suiteId, caseId]) =>
        document(`${suiteId}/${caseId}/main`)
      ),
      epoch: "quality-corpus",
      inventoryVersion: 1,
      projectId: "quality-corpus",
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    },
  };
}

function query(fileId: string): QueryEnvelope {
  return {
    analysisGeneration: 0,
    documentVersion: 1,
    epoch: "quality-corpus",
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    query: { fileId, kind: "semanticView", offset: 0 },
  };
}

function document(fileId: string): ProjectDocument {
  return {
    blocks: [],
    content: "x",
    declarations: [],
    documentVersion: 1,
    fileId,
    includes: [],
    language: "latex",
    macros: [],
    mathRoots: [],
    nodes: [],
    path: `${fileId}.tex`,
    proseAnnotations: [],
    schemaVersion: 8,
    scopes: [],
    visibleProse: [],
  };
}

function queryResult(epoch: string): QueryResult {
  return {
    analysisGeneration: 0,
    documentVersion: 1,
    epoch,
    inventoryVersion: 1,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
    value: { diagnostics: [], kind: "diagnostics" },
  };
}
