import { describe, expect, test } from "bun:test";
import type { RecognitionFrontierSignals } from "./recognition-frontier";
import {
  classifyAuthoredFirstLoss,
  summarizeAuthoredFirstLoss,
} from "./authored-first-loss";

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
        identityFailures: [],
        probePassed: true,
        relationSources: [],
      }),
    ).toEqual({
      basis: "all reviewed public surfaces match",
      reason: "passed",
      stage: null,
    });
  });

  test("distinguishes local recognition from propagation loss", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityFailures: [],
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: true,
            rangeMatched: true,
            relationId: "field:law",
            relationPresent: true,
            rolesMatched: true,
            signals: { ...signals, decision: "established" },
          },
        ],
      }),
    ).toMatchObject({
      reason: "propagation-boundary-loss",
      stage: "propagation",
    });
  });

  test("maps public frontier signals to an actionable reason", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityFailures: [],
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: false,
            rangeMatched: false,
            relationId: "field:law",
            relationPresent: false,
            rolesMatched: false,
            signals: { ...signals, structuralCandidates: false },
          },
        ],
      }),
    ).toMatchObject({
      reason: "structural-dispatch-miss",
      stage: "pack-unification",
    });
  });

  test("reports navigation identity before downstream propagation", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityFailures: [
          { area: "references", basis: "references availability differs" },
        ],
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: true,
            rangeMatched: true,
            relationId: "field:law",
            relationPresent: true,
            rolesMatched: true,
            signals: { ...signals, decision: "established" },
          },
        ],
      }),
    ).toMatchObject({
      reason: "navigation-projection-mismatch",
      stage: "identity",
    });
  });

  test("reports missing neutral source structure before identity", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: signals,
        expectedDecision: "established",
        expectedRelationsMatched: false,
        identityFailures: [
          { area: "cursor-symbol", basis: "symbol null; expected x" },
        ],
        probePassed: false,
        relationSources: [
          {
            localRelationMatched: false,
            rangeMatched: false,
            relationId: "field:law",
            relationPresent: false,
            rolesMatched: false,
            signals: { ...signals, syntaxAvailable: false },
          },
        ],
      }),
    ).toMatchObject({
      reason: "neutral-syntax-unavailable",
      stage: "neutral-syntax",
    });
  });

  test("keeps unsafe certainty at the decision boundary", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "unsupported",
        expectedRelationsMatched: true,
        identityFailures: [],
        probePassed: false,
        relationSources: [],
      }),
    ).toMatchObject({ reason: "unsafe-decision", stage: "decision" });
  });

  test("keeps cursor-entity certainty separate from the selected formula", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "established",
        expectedFormulaDecision: "partial",
        expectedRelationsMatched: true,
        formulaDecision: "partial",
        formulaLocationMatched: true,
        identityFailures: [],
        probePassed: true,
        relationSources: [],
      }),
    ).toMatchObject({ reason: "passed", stage: null });

    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "established",
        expectedFormulaDecision: "partial",
        expectedRelationsMatched: true,
        formulaDecision: "established",
        formulaLocationMatched: true,
        identityFailures: [],
        probePassed: false,
        relationSources: [],
      }),
    ).toMatchObject({
      decisionDomain: "selected-formula",
      reason: "unsafe-decision",
      stage: "decision",
    });
  });

  test("localizes a wrong selected formula before its decision", () => {
    expect(
      classifyAuthoredFirstLoss({
        cursorSignals: { ...signals, decision: "established" },
        expectedDecision: "established",
        expectedFormulaDecision: "ambiguous",
        expectedRelationsMatched: true,
        formulaDecision: "ambiguous",
        formulaLocationMatched: false,
        identityFailures: [],
        probePassed: false,
        relationSources: [],
      }),
    ).toMatchObject({
      decisionDomain: "selected-formula",
      reason: "formula-selection-mismatch",
      stage: "identity",
    });
  });

  test("separates cursor, navigation, and edit projection reasons", () => {
    const base = {
      cursorSignals: signals,
      expectedDecision: "established" as const,
      expectedRelationsMatched: true,
      probePassed: false,
      relationSources: [],
    };
    expect(
      classifyAuthoredFirstLoss({
        ...base,
        identityFailures: [
          { area: "cursor-symbol", basis: "wrong symbol" },
        ],
      }).reason,
    ).toBe("cursor-occurrence-mismatch");
    expect(
      classifyAuthoredFirstLoss({
        ...base,
        identityFailures: [
          { area: "definition", basis: "missing definition" },
        ],
      }).reason,
    ).toBe("navigation-projection-mismatch");
    expect(
      classifyAuthoredFirstLoss({
        ...base,
        identityFailures: [
          { area: "prepare-rename", basis: "rename unavailable" },
        ],
      }).reason,
    ).toBe("edit-projection-mismatch");
  });

  test("builds deterministic matrices without counting passing probes", () => {
    const atlas = summarizeAuthoredFirstLoss([
      {
        basis: "ok",
        caseId: "pass",
        expectedDecision: "established",
        family: "derivation-chain",
        field: "calculus-analysis",
        reason: "passed",
        split: "development",
        stage: null,
      },
      {
        basis: "missing event",
        caseId: "failure-b",
        expectedDecision: "established",
        family: "discourse-reference",
        field: "probability",
        reason: "discourse-evidence-missing",
        split: "development",
        stage: "attachment",
      },
      {
        basis: "wrong cursor",
        caseId: "failure-a",
        expectedDecision: "partial",
        family: "scope-comparison",
        field: "calculus-analysis",
        reason: "cursor-occurrence-mismatch",
        split: "holdout",
        stage: "identity",
      },
    ]);
    expect(atlas).toEqual({
      failed: 2,
      passed: 1,
      byDecision: [
        { key: "established", count: 1 },
        { key: "partial", count: 1 },
      ],
      byFamily: [
        { key: "discourse-reference", count: 1 },
        { key: "scope-comparison", count: 1 },
      ],
      byField: [
        { key: "calculus-analysis", count: 1 },
        { key: "probability", count: 1 },
      ],
      byReason: [
        { key: "cursor-occurrence-mismatch", count: 1 },
        { key: "discourse-evidence-missing", count: 1 },
      ],
      bySplit: [
        { key: "development", count: 1 },
        { key: "holdout", count: 1 },
      ],
      byStage: [
        { key: "attachment", count: 1 },
        { key: "identity", count: 1 },
      ],
      total: 3,
    });
  });
});
