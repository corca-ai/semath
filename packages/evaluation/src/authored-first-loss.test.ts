import { describe, expect, test } from "bun:test";
import type { RecognitionFrontierSignals } from "./recognition-frontier";
import { classifyAuthoredFirstLoss } from "./authored-first-loss";

const signals: RecognitionFrontierSignals = {
  decision: "partial",
  discourseEvidence: true,
  engineLimited: false,
  identityResolved: true,
  sourceGroundedConflict: false,
  structuralCandidates: true,
  syntaxAvailable: true,
  typeOrConditionEvidence: true,
};

describe("authored first-loss localization", () => {
  test("keeps passing probes out of the loss distribution", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "established",
        expectedRelationsMatched: true,
        identityMatches: true,
        probePassed: true,
        relationSources: [],
      }),
    ).toEqual({ basis: "all reviewed public surfaces match", stage: null });
  });

  test("distinguishes local recognition from propagation loss", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityMatches: true,
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: true,
            relationId: "field:law",
            signals: { ...signals, decision: "established" },
          },
        ],
      }).stage,
    ).toBe("propagation");
  });

  test("maps the existing frontier instead of inventing runtime stages", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityMatches: true,
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: false,
            relationId: "field:law",
            signals: { ...signals, structuralCandidates: false },
          },
        ],
      }).stage,
    ).toBe("pack-unification");
  });

  test("reports scope identity before downstream propagation", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityMatches: false,
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: true,
            relationId: "field:law",
            signals: { ...signals, decision: "established" },
          },
        ],
      }).stage,
    ).toBe("identity");
  });

  test("reports missing neutral source structure before identity", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityMatches: false,
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: false,
            relationId: "field:law",
            signals: { ...signals, syntaxAvailable: false },
          },
        ],
      }).stage,
    ).toBe("neutral-syntax");
  });

  test("keeps unsafe certainty at the decision boundary", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "unsupported",
        expectedRelationsMatched: true,
        identityMatches: true,
        probePassed: false,
        relationSources: [],
      }).stage,
    ).toBe("decision");
  });
});
