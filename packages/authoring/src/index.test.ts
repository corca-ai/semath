import { describe, expect, test } from "bun:test";
import {
  checkPackConformance,
  parseCorpus,
  parseQualityManifest,
  type QualityScorecard,
} from "../../evaluation/src/index";
import {
  compareScorecards,
  findCorpusTagProblems,
  findForbiddenRuntimeBranches,
  scaffoldPackWorkspace,
} from "./index";

const pack = {
  laws: [{
    id: "scaled-output",
    roles: [
      { id: "output", shape: "scalar" },
      { id: "coefficient", shape: "scalar" },
      { id: "input", shape: "scalar" },
    ],
    semanticForms: ["output = coefficient input"],
    title: "Scaled output",
  }],
  packId: "sample-field",
  title: "Sample field",
} as const;

describe("pack authoring policies", () => {
  test("scaffolds balanced, varied, strict probe evidence", () => {
    const workspace = scaffoldPackWorkspace(pack);
    expect(workspace.corpus.cases).toHaveLength(10);
    expect(workspace.corpus.cases.filter((item) => item.expectation === "established")).toHaveLength(5);
    expect(workspace.corpus.cases.filter((item) => item.expectation === "refused")).toHaveLength(5);
    expect(new Set(workspace.corpus.cases.map((item) => item.diversity.proseFamily)).size).toBe(5);
    expect(findCorpusTagProblems(workspace.manifest, new Map([[workspace.corpus.domain, workspace.corpus]]))).toEqual([]);
    const manifest = parseQualityManifest(JSON.parse(JSON.stringify(workspace.manifest)));
    const suite = manifest.suites[0]!;
    const corpus = parseCorpus(JSON.parse(JSON.stringify(workspace.corpus)), suite);
    expect(checkPackConformance(
      manifest,
      [{
        activationRules: 1,
        concepts: 3,
        lawIds: ["scaled-output"],
        operators: 0,
        packId: "sample-field",
        quantityKinds: 0,
        roles: 0,
        units: 0,
      }],
      new Map([[suite.id, corpus]]),
    ).failures).toEqual([]);
  });

  test("finds pack-specific runtime decisions but ignores tests and data mentions", () => {
    expect(findForbiddenRuntimeBranches([
      { path: "src/infer.rs", source: 'if pack_id == "sample-field" { specialize(); }' },
      { path: "src/catalog.rs", source: 'const ID: &str = "sample-field";' },
      { path: "src/infer.test.ts", source: 'if (packId === "sample-field") fail();' },
    ], ["sample-field"])).toEqual([
      {
        id: "sample-field",
        line: 1,
        path: "src/infer.rs",
        sourceLine: 'if pack_id == "sample-field" { specialize(); }',
      },
    ]);
  });

  test("compares every safety metric without blending regressions", () => {
    const baseline = scorecard(99, 100);
    const candidate = scorecard(100, 98);
    const comparison = compareScorecards(baseline, candidate);
    expect(comparison.improvements).toContain(
      "sample/scaled-output/recall: 99.0% -> 100.0%",
    );
    expect(comparison.regressions).toContain(
      "sample/scaled-output/precision: 100.0% -> 98.0%",
    );
  });
});

function scorecard(recall: number, precision: number): QualityScorecard {
  const metric = (percent: number) => ({ denominator: 100, numerator: percent, percent });
  return {
    adversarialRefusal: metric(100),
    authoredCases: 10,
    coverage: [],
    diversity: [],
    failures: [],
    generatedCases: 0,
    laws: [{
      evidenceIntegrity: metric(100),
      falsePositives: 0,
      lawId: "scaled-output",
      positives: 5,
      precision: metric(precision),
      recall: metric(recall),
      refusals: 5,
      refusalPreservation: metric(100),
      roleAccuracy: metric(100),
      suiteId: "sample",
    }],
    metamorphic: metric(100),
    refusalCategories: 5,
    schemaVersion: 2,
    variations: [],
  };
}
