import { describe, expect, test } from "bun:test";
import {
  parseFreshBlindEvaluation,
  parseFreshBlindLifecycle,
} from "./run-fresh-blind-release";
import {
  projectMathAuthoringContext,
  type MathAuthoringExpectationProbe,
} from "../packages/evaluation/src/index";
import type { MathAuthoringContext } from "../packages/protocol/src/index";

function authoringContext(): MathAuthoringContext {
  return {
    claimEvidence: [],
    conditions: [],
    disposition: "unsupported",
    equationLinks: [],
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    },
    interpretations: {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [],
      missingDiscriminators: [],
      truncated: false,
    },
    notationOccurrences: [],
    requirements: [],
    truncated: false,
  };
}

function expectedProbes(): readonly MathAuthoringExpectationProbe[] {
  return [{
    expected: { authoringContext: projectMathAuthoringContext(authoringContext()) },
    id: "case-1",
  }];
}

function evaluation() {
  return {
    results: [
      {
        batch: {},
        evidenceGraded: {
          cases: 1,
          contradictionCases: 0,
          domainContextCases: 1,
          exactAnchorCases: 0,
          failures: ["facet: wrong source anchor"],
          missingDiscriminatorCases: 1,
          multipleHypothesisCases: 0,
          naturalLanguageCases: 1,
          openWorldCases: 1,
          orderingCases: 1,
          reviewedConventionCases: 0,
          supportingEvidenceCases: 1,
          withHypotheses: 1,
        },
        firstLoss: [],
        firstLossAtlas: { recognition: 1 },
        firstLossCounts: {},
        mathAuthoring: {
          cases: 1,
          exactCases: 1,
          failures: [],
          required: true,
        },
        observations: [
          {
            authoringContext: authoringContext(),
            caseId: "case-1",
            decision: "partial",
            definitions: [],
            diagnostics: [],
            prepareRename: {},
            proofGrounded: false,
            references: [],
            relations: [],
            renameEdits: [],
            symbol: null,
            interpretations: authoringContext().interpretations,
          },
        ],
        score: {
          cases: 1,
          failures: ["case-1: mismatch"],
          passed: 0,
          risk: {
            falseConflict: 0,
            falseEstablishment: 0,
            missedCoverage: 1,
            navigationOrIdentity: 0,
            total: 2,
          },
        },
      },
    ],
  };
}

describe("fresh blind retained report parsers", () => {
  test("retains the full evaluation while exposing authoritative facet failures", () => {
    const raw = evaluation();
    const parsed = parseFreshBlindEvaluation(raw, expectedProbes());
    expect(parsed.raw).toBe(raw);
    expect(parsed.evidenceGradedFailures).toEqual([
      "facet: wrong source anchor",
    ]);
    expect(parsed.mathAuthoringFailures).toEqual([]);
  });

  test("rejects missing facet evidence and malformed score boundaries", () => {
    const missing = evaluation();
    delete (missing.results[0] as Partial<(typeof missing.results)[number]>)
      .evidenceGraded;
    expect(() => parseFreshBlindEvaluation(missing, expectedProbes())).toThrow(
      "unexpected or missing fields",
    );
    const malformed = evaluation();
    const result = malformed.results[0];
    if (!result) throw new Error("test evaluation result is missing");
    expect(() =>
      parseFreshBlindEvaluation({
        ...malformed,
        results: [{ ...result, score: { ...result.score, cases: -1 } }],
      }, expectedProbes()),
    ).toThrow("nonnegative integer");
    expect(() =>
      parseFreshBlindEvaluation({ ...malformed, extra: true }, expectedProbes()),
    ).toThrow("unexpected or missing fields");
    expect(() =>
      parseFreshBlindEvaluation({
        ...malformed,
        results: [
          {
            ...result,
            observations: [{ ...result.observations[0], surprise: true }],
          },
        ],
      }, expectedProbes()),
    ).toThrow("unexpected or missing fields");
  });

  test("requires a strict, nonempty fresh exact-oracle envelope", () => {
    const raw = evaluation();
    const result = raw.results[0];
    if (!result) throw new Error("test evaluation result is missing");
    expect(() =>
      parseFreshBlindEvaluation({
        ...raw,
        results: [{
          ...result,
          mathAuthoring: { ...result.mathAuthoring, required: false },
        }],
      }, expectedProbes()),
    ).toThrow("mathAuthoring.required must be true");
    expect(() =>
      parseFreshBlindEvaluation({
        ...raw,
        results: [{
          ...result,
          mathAuthoring: { ...result.mathAuthoring, surprise: true },
        }],
      }, expectedProbes()),
    ).toThrow("unexpected or missing fields");
    expect(() =>
      parseFreshBlindEvaluation({
        ...raw,
        results: [{
          ...result,
          mathAuthoring: {
            ...result.mathAuthoring,
            cases: 0,
            exactCases: 0,
          },
        }],
      }, expectedProbes()),
    ).toThrow("mathAuthoring.cases must be positive");
  });

  test("recomputes exact authoring results and rejects malformed nested unions", () => {
    const dishonest = evaluation();
    const result = dishonest.results[0];
    if (!result) throw new Error("test evaluation result is missing");
    expect(() =>
      parseFreshBlindEvaluation({
        ...dishonest,
        results: [{
          ...result,
          mathAuthoring: {
            ...result.mathAuthoring,
            exactCases: 0,
            failures: ["trusted without recomputation"],
          },
        }],
      }, expectedProbes()),
    ).toThrow("does not match independent recomputation");

    const malformed = evaluation();
    const observation = malformed.results[0]!.observations[0]!;
    observation.authoringContext.interpretations = {
      ...observation.authoringContext.interpretations,
      hypotheses: [{ support: "invented-authority" }],
    } as unknown as MathAuthoringContext["interpretations"];
    expect(() => parseFreshBlindEvaluation(malformed, expectedProbes())).toThrow();
  });

  test("accepts only the exact lifecycle parity envelope", () => {
    const lifecycle = {
      comparedProbes: 48,
      comparedStages: 96,
      fixtureId: "v0.37",
      fixtureSeal: "a".repeat(64),
      schemaVersion: 1 as const,
    };
    expect(parseFreshBlindLifecycle(lifecycle)).toEqual(lifecycle);
    expect(() =>
      parseFreshBlindLifecycle({ ...lifecycle, extra: true }),
    ).toThrow("unexpected or missing fields");
  });
});
