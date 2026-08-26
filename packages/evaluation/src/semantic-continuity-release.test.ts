import { describe, expect, test } from "bun:test";
import {
  adjudicateSemanticContinuityDecisions,
  selectSemanticContinuityFormulaDecisions,
  semanticContinuityReleaseRegressions,
} from "./semantic-continuity-release";
import type { SemanticContinuityObservation } from "./semantic-continuity";
import type { SemanticContinuityFixture } from "./semantic-continuity";

describe("semantic continuity release policy", () => {
  test("applies only an explicit decision transition", () => {
    const fixture = oneCaseFixture();
    const adjudicated = adjudicateSemanticContinuityDecisions(fixture, [
      { caseId: "case-1", from: "partial", to: "established" },
    ]);
    expect(adjudicated.cases[0]?.target.decision).toBe("established");
    expect(adjudicated.cases[0]?.target.symbol).toBe("x");
    expect(fixture.cases[0]?.target.decision).toBe("partial");
  });

  test("rejects an unknown or stale adjudication", () => {
    const fixture = oneCaseFixture();
    expect(() =>
      adjudicateSemanticContinuityDecisions(fixture, [
        { caseId: "missing", from: "partial", to: "established" },
      ]),
    ).toThrow("unknown");
    expect(() =>
      adjudicateSemanticContinuityDecisions(fixture, [
        { caseId: "case-1", from: "unsupported", to: "established" },
      ]),
    ).toThrow("fixture has partial");
  });

  test("keeps cursor-entity and selected-formula decisions separate", () => {
    const observation = oneObservation({
      decision: "established",
      formulaDecision: "conflicting",
    });
    expect(
      selectSemanticContinuityFormulaDecisions([observation], ["case-1"])[0]
        ?.decision,
    ).toBe("conflicting");
    expect(observation.decision).toBe("established");
  });

  test("fails closed on invalid selected-formula decisions", () => {
    expect(() =>
      selectSemanticContinuityFormulaDecisions(
        [oneObservation({ formulaDecision: null })],
        ["case-1"],
      ),
    ).toThrow("no legacy continuity decision");
    expect(() =>
      selectSemanticContinuityFormulaDecisions(
        [oneObservation()],
        ["missing"],
      ),
    ).toThrow("unknown");
    expect(() =>
      selectSemanticContinuityFormulaDecisions(
        [oneObservation()],
        ["case-1", "case-1"],
      ),
    ).toThrow("duplicate");
  });

  test("allows reviewed misses but rejects every unsafe class", () => {
    const baseline = { cases: 48, maximumRisk: 22, minimumPassed: 37 };
    expect(
      semanticContinuityReleaseRegressions(
        {
          cases: 48,
          failures: ["reviewed coverage miss"],
          families: familyScores(),
          passed: 37,
          risk: {
            falseConflict: 0,
            falseEstablishment: 0,
            missedCoverage: 11,
            navigationOrIdentity: 0,
            total: 22,
          },
        },
        baseline,
      ),
    ).toEqual([]);
    expect(
      semanticContinuityReleaseRegressions(
        {
          cases: 48,
          failures: [],
          families: familyScores(),
          passed: 36,
          risk: {
            falseConflict: 1,
            falseEstablishment: 1,
            missedCoverage: 12,
            navigationOrIdentity: 1,
            total: 58,
          },
        },
        baseline,
      ),
    ).toHaveLength(5);
  });
});

function oneCaseFixture(): SemanticContinuityFixture {
  return {
    baseline: { commit: "abc", note: "frozen", protocolVersion: 11 },
    cases: [
      {
        baseline: { decision: "unsupported", problems: 0 },
        cursor: { fileId: "main", needle: "x", offset: 0 },
        documents: [{ content: "$x$", fileId: "main", path: "main.tex" }],
        family: "notation-identity",
        id: "case-1",
        target: {
          decision: "partial",
          maximumProblems: 0,
          minimumProblems: 0,
          symbol: "x",
        },
        variationTags: ["identity", "exact"],
      },
    ],
    schemaVersion: 1,
  };
}

function oneObservation(
  overrides: Partial<SemanticContinuityObservation> = {},
): SemanticContinuityObservation {
  return {
    caseId: "case-1",
    decision: "partial",
    definitions: [],
    formulaDecision: "partial",
    problems: 0,
    relationIds: [],
    shapeKinds: [],
    symbol: "x",
    ...overrides,
  };
}

function familyScores() {
  return {
    "canonical-structure": { passed: 0, total: 8 },
    "discourse-flow": { passed: 0, total: 8 },
    "lifetime-shadowing": { passed: 0, total: 8 },
    "notation-identity": { passed: 0, total: 8 },
    "safety-retraction": { passed: 0, total: 8 },
    "typed-propagation": { passed: 0, total: 8 },
  };
}
