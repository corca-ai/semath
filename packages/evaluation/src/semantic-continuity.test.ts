import { describe, expect, test } from "bun:test";
import {
  parseSemanticContinuityFixture,
  scoreSemanticContinuity,
  type SemanticContinuityObservation,
} from "./semantic-continuity";

describe("semantic continuity holdout", () => {
  test("rejects a suite without the full orthogonal family matrix", () => {
    expect(() =>
      parseSemanticContinuityFixture({
        baseline: { commit: "abc", note: "frozen", protocolVersion: 11 },
        cases: [],
        schemaVersion: 1,
      }),
    ).toThrow("at least 48");
  });

  test("weights unsafe transitions above missed coverage", () => {
    const fixture = parseSemanticContinuityFixture(fixtureValue());
    const observations: SemanticContinuityObservation[] = fixture.cases.map(
      (item) => ({
        caseId: item.id,
        decision: item.id === "lifetime-shadowing-0" ? "conflicting" : "partial",
        definitions: [],
        formulaDecision: null,
        problems: item.id === "lifetime-shadowing-0" ? 1 : 0,
        relationIds: [],
        shapeKinds: [],
        symbol: "x",
      }),
    );
    const score = scoreSemanticContinuity(fixture, observations);
    expect(score.risk.falseConflict).toBe(1);
    expect(score.risk.total).toBeGreaterThan(score.risk.missedCoverage * 2);
  });
});

function fixtureValue(): unknown {
  return {
    baseline: { commit: "abc", note: "frozen", protocolVersion: 11 },
    cases: [
      "lifetime-shadowing",
      "notation-identity",
      "discourse-flow",
      "canonical-structure",
      "typed-propagation",
      "safety-retraction",
    ].flatMap((family) =>
      Array.from({ length: 8 }, (_, index) => ({
        baseline: { decision: "partial", problems: 0 },
        cursor: { fileId: "main", needle: "x", offset: 0 },
        documents: [
          {
            content: `${family} ${index} $x$`,
            fileId: "main",
            path: "main.tex",
          },
        ],
        family,
        id: `${family}-${index}`,
        target: {
          decision: "partial",
          maximumProblems: 0,
          minimumProblems: 0,
          symbol: "x",
        },
        variationTags: [family, `case-${index}`],
      })),
    ),
    schemaVersion: 1,
  };
}
