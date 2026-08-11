import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import type { SemanticViewInfo } from "../../../packages/protocol/src/index";
import {
  classifyRecognitionFrontier,
  frontierSignals,
  parseRecognitionFrontier,
  scoreRecognitionFrontier,
  type RecognitionFrontierSignals,
} from "./recognition-frontier";

const signals: RecognitionFrontierSignals = {
  canonicalAvailable: true,
  decision: "partial",
  discourseEvidence: true,
  engineLimited: false,
  identityResolved: true,
  sourceGroundedConflict: false,
  structuralCandidates: true,
  syntaxAvailable: true,
  typeOrConditionEvidence: false,
};

describe("recognition frontier", () => {
  test("freezes diverse document-shaped holdouts", () => {
    const fixture = parseRecognitionFrontier(
      JSON.parse(
        readFileSync(
          new URL(
            "../../../fixtures/challenge/recognition-frontier-v1.json",
            import.meta.url,
          ),
          "utf8",
        ),
      ),
    );
    expect(fixture.cases).toHaveLength(32);
    expect(new Set(fixture.cases.map((item) => item.family)).size).toBe(8);
    expect(
      new Set(fixture.cases.flatMap((item) => item.variationTags)).size,
    ).toBeGreaterThan(40);
  });

  test("reports the first lost stage in dependency order", () => {
    expect(
      classifyRecognitionFrontier({ ...signals, syntaxAvailable: false }),
    ).toBe("syntax-unavailable");
    expect(
      classifyRecognitionFrontier({ ...signals, canonicalAvailable: false }),
    ).toBe("canonical-unsupported");
    expect(
      classifyRecognitionFrontier({ ...signals, discourseEvidence: false }),
    ).toBe("discourse-evidence-missing");
    expect(
      classifyRecognitionFrontier({ ...signals, identityResolved: false }),
    ).toBe("identity-scope-unresolved");
    expect(
      classifyRecognitionFrontier({ ...signals, structuralCandidates: false }),
    ).toBe("structural-candidate-missing");
    expect(classifyRecognitionFrontier(signals)).toBe(
      "type-condition-evidence-missing",
    );
  });

  test("public decisions outrank downstream loss signals", () => {
    expect(
      classifyRecognitionFrontier({ ...signals, decision: "established" }),
    ).toBe("established");
    expect(
      classifyRecognitionFrontier({ ...signals, decision: "ambiguous" }),
    ).toBe("genuine-ambiguity");
    expect(
      classifyRecognitionFrontier({
        ...signals,
        decision: "conflicting",
        sourceGroundedConflict: true,
      }),
    ).toBe("demonstrated-conflict");
  });

  test("derives evidence signals without a second matcher", () => {
    const view = semanticView();
    expect(frontierSignals(view, true)).toEqual({
      canonicalAvailable: true,
      decision: "partial",
      discourseEvidence: true,
      engineLimited: false,
      identityResolved: true,
      sourceGroundedConflict: false,
      structuralCandidates: true,
      syntaxAvailable: true,
      typeOrConditionEvidence: true,
    });
  });

  test("weights false certainty above missed coverage", () => {
    const cases = Array.from({ length: 24 }, (_, index) => ({
      baseline: { decision: "unsupported", stage: "canonical-unsupported" },
      cursor: { fileId: "main", needle: `x_${index}` },
      documents: [
        { content: `$x_${index}$`, fileId: "main", path: "main.tex" },
      ],
      family: `family-${Math.floor(index / 4)}`,
      id: `case-${index}`,
      target: {
        decision: index === 0 ? "ambiguous" : "established",
        relationId: null,
        stage: index === 0 ? "genuine-ambiguity" : "established",
      },
      variationTags: ["authored"],
    }));
    const frontier = parseRecognitionFrontier({
      baseline: { commit: "abc", note: "frozen", protocolVersion: 11 },
      cases,
      schemaVersion: 1,
    });
    const observations = cases.map((item, index) => ({
      caseId: item.id,
      decision: (index === 0 ? "established" : "unsupported") as
        | "established"
        | "unsupported",
      relationId: null,
      signals,
      stage: (index === 0
        ? "established"
        : "canonical-unsupported") as
        | "established"
        | "canonical-unsupported",
    }));
    const score = scoreRecognitionFrontier(frontier, observations);
    expect(score.risk).toEqual({
      falseConflict: 0,
      falseEstablishment: 1,
      missedCoverage: 23,
      total: 54,
    });
    expect(score.firstFailure).toStartWith("case-0:");
  });
});

function semanticView(): SemanticViewInfo {
  const occurrenceId = { documentVersion: 1, fileId: "main", localId: 1 };
  const entityId = {
    anchor: occurrenceId,
    componentId: "whole",
    kind: "symbol",
    scopePath: [0],
  };
  const evidence = {
    kind: "prose" as const,
    ruleId: "test",
    sourceRanges: [{ endOffset: 2, startOffset: 1 }],
    strength: "strong" as const,
  };
  return {
    context: {
      candidates: [
        {
          candidateId: "application",
          family: "application",
          interpretation: "application",
          range: { endOffset: 2, startOffset: 1 },
          rejectingClaimIds: [],
          status: "supported",
          supportingClaimIds: ["claim"],
        },
      ],
      claims: [
        {
          claimId: "claim",
          conflicts: [],
          evidence: [evidence],
          predicate: "role",
          status: "certain",
          value: "state",
        },
      ],
      concepts: [],
      entityId,
      quantities: [],
      relations: [],
      truncated: false,
    },
    decision: {
      facts: [],
      meaning: { label: "State", relationId: null },
      reasons: [],
      requirements: [
        {
          evidence: [evidence],
          label: "Declare a compatible shape",
          requirementId: "shape",
          subjects: ["x"],
        },
      ],
      status: "partial",
    },
    declarations: [],
    diagnostics: [],
    domains: [],
    symbol: {
      definitions: [],
      diagnostics: [],
      entityId,
      location: {
        fileId: "main",
        path: "main.tex",
        range: { endOffset: 2, startOffset: 1 },
      },
      notation: [],
      occurrenceId,
      shapes: [],
      sourceNotation: "x",
      symbol: "x",
      truncated: false,
    },
    truncated: false,
  };
}
