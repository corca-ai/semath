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
    bindings: [
      {
        constraint: { kind: "scalar" },
        evidence: evidence("supporting").evidence,
        parameter: "value",
        proof: "typed",
        symbol: "x",
      },
    ],
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
      roles: [{ label: "Value", role: "value", symbol: "x" }],
      title: relationId,
    },
    scopePath: [0],
    support,
  };
}

describe("selected-formula challenge observation", () => {
  test("binds relation support and authority to the exact selected formula", () => {
    const candidate = observeSelectedFormulaDecision({
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
      disposition: "partial",
      formula,
      hypotheses: [hypothesis("test:law", "explicit", { formula })],
    });
    expect(authoritative.recognizedRelations[0]?.authority).toBe(
      "authoritative",
    );

    const derived = hypothesis("test:law", "derived", { formula });
    expect(
      observeSelectedFormulaDecision({
        disposition: "partial",
        formula,
        hypotheses: [
          {
            ...derived,
            bindings: derived.bindings.map((binding) => ({
              ...binding,
              proof: "derived" as const,
            })),
          },
        ],
      }).recognizedRelations[0]?.authority,
    ).toBe("authoritative");

    const candidateBinding = hypothesis("test:law", "explicit", { formula });
    const bindingLimited = observeSelectedFormulaDecision({
      disposition: "partial",
      formula,
      hypotheses: [
        {
          ...candidateBinding,
          bindings: [
            {
              constraint: { kind: "scalar" },
              evidence: evidence("supporting").evidence,
              parameter: "value",
              proof: "candidate",
              symbol: "x",
            },
          ],
        },
      ],
    });
    expect(bindingLimited.recognizedRelations[0]?.authority).toBe("candidate");

    const assertedBinding = hypothesis("test:law", "explicit", { formula });
    expect(
      observeSelectedFormulaDecision({
        disposition: "partial",
        formula,
        hypotheses: [
          {
            ...assertedBinding,
            bindings: assertedBinding.bindings.map((binding) => ({
              ...binding,
              proof: "asserted" as const,
            })),
          },
        ],
      }).recognizedRelations[0]?.authority,
    ).toBe("candidate");

    const emptyBindings = hypothesis("test:law", "explicit", { formula });
    expect(
      observeSelectedFormulaDecision({
        disposition: "partial",
        formula,
        hypotheses: [{ ...emptyBindings, bindings: [] }],
      }).recognizedRelations[0]?.authority,
    ).toBe("candidate");

    const missingRole = hypothesis("test:law", "explicit", { formula });
    if (!missingRole.relation) throw new Error("test relation must be present");
    expect(
      observeSelectedFormulaDecision({
        disposition: "partial",
        formula,
        hypotheses: [
          {
            ...missingRole,
            relation: {
              ...missingRole.relation,
              roles: [
                ...missingRole.relation.roles,
                { label: "Result", role: "result", symbol: "y" },
              ],
            },
          },
        ],
      }).recognizedRelations[0]?.authority,
    ).toBe("candidate");

    const ungroundedBinding = hypothesis("test:law", "explicit", {
      formula,
    });
    expect(
      observeSelectedFormulaDecision({
        disposition: "partial",
        formula,
        hypotheses: [
          {
            ...ungroundedBinding,
            bindings: ungroundedBinding.bindings.map((binding) => ({
              ...binding,
              evidence: { ...binding.evidence, sourceRanges: [] },
            })),
          },
        ],
      }).recognizedRelations[0]?.authority,
    ).toBe("candidate");

    const requiredCondition = hypothesis("test:law", "explicit", { formula });
    const conditionLimited = observeSelectedFormulaDecision({
      disposition: "partial",
      formula,
      hypotheses: [
        {
          ...requiredCondition,
          conditions: [
            {
              conditionId: "test-condition",
              evidence: [],
              kind: "assumption",
              label: "Test condition",
              status: "required",
              subjects: ["x"],
            },
          ],
        },
      ],
    });
    expect(conditionLimited.recognizedRelations[0]?.authority).toBe(
      "candidate",
    );

    const siblingFormula = {
      ...formula,
      location: {
        ...formula.location,
        fileId: "sibling",
        path: "sibling.tex",
      },
    };
    const siblingAuthority = observeSelectedFormulaDecision({
      disposition: "partial",
      formula,
      hypotheses: [
        hypothesis("test:law", "supported", { formula }),
        hypothesis("test:law", "explicit", { formula: siblingFormula }),
      ],
    });
    expect(siblingAuthority.recognizedRelations[0]?.authority).toBe(
      "candidate",
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
        disposition: "partial",
        formula,
        hypotheses: [
          hypothesis("test:law", "supported", { formula: wrongAnchor }),
        ],
      }).recognizedRelations,
    ).toEqual([]);

    const offFormulaMeaning = observeSelectedFormulaDecision({
      disposition: "partial",
      formula,
      hypotheses: [
        hypothesis("test:law", "supported", {
          evidence: [evidence("supporting", 1, 5)],
          formula,
        }),
      ],
    });
    expect(offFormulaMeaning.decision.meaningRelationId).toBe("test:law");
    expect(offFormulaMeaning.decision.sourceGrounded).toBe(false);
  });

  test("grounds formula contradictions through overlapping current source anchors", () => {
    const contradicted = hypothesis("test:conflict", "contradicted", {
      evidence: [evidence("contradicting")],
    });
    const grounded = observeSelectedFormulaDecision({
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

    const overlapOnly = observeSelectedFormulaDecision({
      disposition: "conflicting",
      formula,
      hypotheses: [
        hypothesis("test:conflict", "contradicted", {
          evidence: [evidence("contradicting", 12, 18)],
        }),
      ],
    });
    expect(overlapOnly.decision.problemCount).toBe(0);
    expect(overlapOnly.decision.sourceGrounded).toBe(false);
  });
});
