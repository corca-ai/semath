import { describe, expect, test } from "bun:test";
import type {
  MathFormulaAnchorInfo,
  MathInterpretationEvidenceInfo,
  MathInterpretationHypothesisInfo,
} from "../../protocol/src/index";
import { observeSelectedFormulaDecision } from "./challenge-observation";

const formula: MathFormulaAnchorInfo = {
  documentVersion: 1,
  location: {
    fileId: "main",
    path: "main.tex",
    range: { endOffset: 20, startOffset: 10 },
  },
  scopePath: [0],
  sourceNotation: "x=y",
};

function evidence(
  role: MathInterpretationEvidenceInfo["role"],
  startOffset = 10,
  endOffset = 20,
): MathInterpretationEvidenceInfo {
  return {
    evidence: {
      kind: "canonical-math",
      ruleId: "test/evidence",
      sourceRanges: [{ endOffset, startOffset }],
      strength: "hard",
    },
    provenance: "typed-structure",
    role,
    sourceAnchors: [
      {
        documentVersion: 1,
        generation: "authored",
        lifecycle: "current",
        location: {
          fileId: "main",
          path: "main.tex",
          range: { endOffset, startOffset },
        },
        scopePath: [0],
      },
    ],
  };
}

function hypothesis(
  relationId: string,
  support: MathInterpretationHypothesisInfo["support"],
  options: {
    evidence?: readonly MathInterpretationEvidenceInfo[];
    formula?: MathFormulaAnchorInfo;
  } = {},
): MathInterpretationHypothesisInfo {
  return {
    bindings: [],
    conditions: [],
    documentVersion: 1,
    evidence: options.evidence ?? [evidence("supporting")],
    ...(options.formula ? { formula: options.formula } : {}),
    hypothesisId: relationId,
    kind: "typed-law",
    label: relationId,
    location: formula.location,
    missingDiscriminatorIds: [],
    orderingReasons: [],
    range: formula.location.range,
    rank: 0,
    relation: {
      conditions: [],
      description: relationId,
      evidence: [],
      range: formula.location.range,
      relationId,
      roles: [],
      title: relationId,
    },
    scopePath: [0],
    support,
  };
}

describe("selected-formula challenge observation", () => {
  test("binds relation support and authority to the exact selected formula", () => {
    const candidate = observeSelectedFormulaDecision({
      authoritativeRelationIds: new Set(),
      disposition: "partial",
      formula,
      hypotheses: [hypothesis("test:law", "supported", { formula })],
    });
    expect(candidate.recognizedRelations).toEqual([
      {
        authority: "candidate",
        formulaAnchor: "selected-formula",
        relationId: "test:law",
        support: "supported",
      },
    ]);
    expect(candidate.decision.meaningRelationId).toBe("test:law");
    expect(candidate.decision.sourceGrounded).toBe(true);

    const authoritative = observeSelectedFormulaDecision({
      authoritativeRelationIds: new Set(["test:law"]),
      disposition: "partial",
      formula,
      hypotheses: [hypothesis("test:law", "supported", { formula })],
    });
    expect(authoritative.recognizedRelations[0]?.authority).toBe(
      "authoritative",
    );

    const wrongAnchor = {
      ...formula,
      location: {
        ...formula.location,
        range: { ...formula.location.range, endOffset: 19 },
      },
    };
    expect(
      observeSelectedFormulaDecision({
        authoritativeRelationIds: new Set(["test:law"]),
        disposition: "partial",
        formula,
        hypotheses: [
          hypothesis("test:law", "supported", { formula: wrongAnchor }),
        ],
      }).recognizedRelations,
    ).toEqual([]);
  });

  test("grounds formula contradictions through overlapping current source anchors", () => {
    const contradicted = hypothesis("test:conflict", "contradicted", {
      evidence: [evidence("contradicting", 12, 18)],
    });
    const grounded = observeSelectedFormulaDecision({
      authoritativeRelationIds: new Set(),
      disposition: "conflicting",
      formula,
      hypotheses: [contradicted],
    });
    expect(grounded.decision).toEqual({
      problemCount: 1,
      reasonKinds: ["source-conflict"],
      sourceGrounded: true,
      status: "conflicting",
    });

    const ungroundedEvidence = {
      ...contradicted.evidence[0]!,
      evidence: {
        ...contradicted.evidence[0]!.evidence,
        sourceRanges: [],
      },
    };
    const ungrounded = observeSelectedFormulaDecision({
      authoritativeRelationIds: new Set(),
      disposition: "conflicting",
      formula,
      hypotheses: [
        hypothesis("test:conflict", "contradicted", {
          evidence: [ungroundedEvidence],
        }),
      ],
    });
    expect(ungrounded.decision.problemCount).toBe(1);
    expect(ungrounded.decision.sourceGrounded).toBe(false);
  });
});
