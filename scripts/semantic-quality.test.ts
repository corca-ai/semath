import { describe, expect, test } from "bun:test";
import {
  aggregateSemanticQuality,
  evaluateSemanticQualityBudgets,
  type SemanticQualityObservation,
} from "./semantic-quality";

const observations: SemanticQualityObservation[] = [
  {
    field: "formula",
    domain: "linear-algebra",
    topic: "matrix-products",
    capability: "recognition",
    cases: 2,
    exactCases: 1,
    expectedItems: 2,
    matchedItems: 1,
    actualItems: 2,
    unexpectedItems: 1,
  },
  {
    field: "formula",
    domain: "linear-algebra",
    topic: "matrix-products",
    capability: "refusal",
    cases: 1,
    exactCases: 1,
    expectedItems: 0,
    matchedItems: 0,
    actualItems: 0,
    unexpectedItems: 0,
  },
];

describe("semantic quality scorecards", () => {
  test("aggregates independent counters without averaging percentages", () => {
    expect(aggregateSemanticQuality(observations, ["field", "domain"])).toEqual([
      expect.objectContaining({
        cases: 3,
        exactCases: 2,
        caseAccuracyPercent: 66.7,
        precisionPercent: 50,
        recallPercent: 50,
        unexpectedItems: 1,
      }),
    ]);
  });

  test("evaluates checked budgets and reports every regression", () => {
    const [result] = evaluateSemanticQualityBudgets(observations, [
      {
        id: "strict-linear-algebra",
        selector: { field: "formula", domain: "linear-algebra" },
        minCases: 4,
        minCaseAccuracyPercent: 100,
        minPrecisionPercent: 100,
        minRecallPercent: 100,
        maxUnexpectedItems: 0,
      },
    ]);
    expect(result?.violations).toEqual([
      "cases 3 is below 4",
      "case accuracy 66.7 is below 100",
      "precision 50 is below 100",
      "recall 50 is below 100",
      "unexpected items 1 exceeds 0",
    ]);
  });

  test("rejects invalid observations", () => {
    expect(() =>
      aggregateSemanticQuality([{ ...observations[0]!, exactCases: 3 }], []),
    ).toThrow("exactCases cannot exceed cases");
  });
});
