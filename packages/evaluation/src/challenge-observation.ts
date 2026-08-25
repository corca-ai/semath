import type {
  MathAuthoringDisposition,
  MathFormulaAnchorInfo,
  MathInterpretationEvidenceInfo,
  MathInterpretationHypothesisInfo,
  RelationInfo,
  SourceRange,
} from "../../protocol/src/index";
import type {
  ChallengeDecisionObservation,
  ChallengeRecognizedRelation,
} from "./challenge";

export interface SelectedFormulaObservationInput {
  readonly authoritativeRelationIds: ReadonlySet<string>;
  readonly disposition: MathAuthoringDisposition | undefined;
  readonly formula: MathFormulaAnchorInfo | undefined;
  readonly hypotheses: readonly MathInterpretationHypothesisInfo[];
}

type FormulaRelationHypothesis = MathInterpretationHypothesisInfo & {
  readonly formula: MathFormulaAnchorInfo;
  readonly relation: RelationInfo;
};

export function observeSelectedFormulaDecision(
  input: SelectedFormulaObservationInput,
): {
  readonly decision: ChallengeDecisionObservation;
  readonly recognizedRelations: readonly ChallengeRecognizedRelation[];
} {
  const selectedFormula = input.formula;
  const anchoredRelations = input.hypotheses.filter(
    (hypothesis): hypothesis is FormulaRelationHypothesis =>
      hypothesis.relation !== undefined &&
      hypothesis.formula !== undefined &&
      selectedFormula !== undefined &&
      sameFormulaAnchor(hypothesis.formula, selectedFormula),
  );
  const recognizedRelations = anchoredRelations
    .map(
      (hypothesis): ChallengeRecognizedRelation => ({
        authority: input.authoritativeRelationIds.has(
          hypothesis.relation.relationId,
        )
          ? "authoritative"
          : "candidate",
        formulaAnchor: "selected-formula",
        relationId: hypothesis.relation.relationId,
        support: hypothesis.support,
      }),
    )
    .sort((left, right) =>
      `${left.relationId}\u0000${left.support}\u0000${left.authority}`.localeCompare(
        `${right.relationId}\u0000${right.support}\u0000${right.authority}`,
      ),
    );
  const meaning = selectFormulaMeaning(input.disposition, anchoredRelations);
  const contradictions = input.hypotheses.filter(
    (hypothesis) =>
      hypothesis.support === "contradicted" &&
      selectedFormula &&
      hypothesisContradictsFormula(hypothesis, selectedFormula),
  );
  const contradictionGrounded =
    contradictions.length > 0 &&
    contradictions.every((hypothesis) => {
      const evidence = hypothesis.evidence.filter(
        (item) => item.role === "contradicting",
      );
      return (
        evidence.length > 0 &&
        evidence.every(interpretationEvidenceIsGrounded) &&
        selectedFormula !== undefined &&
        evidence.some((item) => evidenceAnchorsFormula(item, selectedFormula))
      );
    });
  const meaningGrounded = Boolean(
    meaning?.evidence.some(
      (evidence) =>
        evidence.role === "supporting" &&
        interpretationEvidenceIsGrounded(evidence),
    ),
  );
  return {
    decision: {
      ...(meaning?.label ? { meaningLabel: meaning.label } : {}),
      ...(meaning?.relation
        ? { meaningRelationId: meaning.relation.relationId }
        : {}),
      problemCount: contradictions.length,
      reasonKinds: formulaReasonKinds(input.disposition),
      sourceGrounded:
        input.disposition === "conflicting"
          ? contradictionGrounded
          : meaningGrounded,
      ...(input.disposition ? { status: input.disposition } : {}),
    },
    recognizedRelations,
  };
}

function selectFormulaMeaning(
  disposition: MathAuthoringDisposition | undefined,
  hypotheses: readonly MathInterpretationHypothesisInfo[],
): MathInterpretationHypothesisInfo | undefined {
  if (
    disposition !== "established" &&
    disposition !== "partial" &&
    disposition !== "conventional"
  ) {
    return undefined;
  }
  return hypotheses
    .filter((hypothesis) => hypothesis.support !== "contradicted")
    .sort(
      (left, right) =>
        left.rank - right.rank ||
        left.hypothesisId.localeCompare(right.hypothesisId),
    )[0];
}

function formulaReasonKinds(
  disposition: MathAuthoringDisposition | undefined,
): readonly string[] {
  if (!disposition) return [];
  if (disposition === "established") return ["proof"];
  if (disposition === "conflicting") return ["source-conflict"];
  return ["uncertainty"];
}

function hypothesisContradictsFormula(
  hypothesis: MathInterpretationHypothesisInfo,
  formula: MathFormulaAnchorInfo,
): boolean {
  return hypothesis.evidence.some(
    (evidence) =>
      evidence.role === "contradicting" &&
      evidenceAnchorsFormula(evidence, formula),
  );
}

function evidenceAnchorsFormula(
  evidence: MathInterpretationEvidenceInfo,
  formula: MathFormulaAnchorInfo,
): boolean {
  return evidence.sourceAnchors.some(
    (anchor) =>
      anchor.lifecycle === "current" &&
      anchor.documentVersion === formula.documentVersion &&
      anchor.location.fileId === formula.location.fileId &&
      anchor.location.path === formula.location.path &&
      rangesOverlap(anchor.location.range, formula.location.range),
  );
}

function interpretationEvidenceIsGrounded(
  evidence: MathInterpretationEvidenceInfo,
): boolean {
  return (
    evidence.evidence.sourceRanges.length > 0 &&
    evidence.sourceAnchors.length > 0 &&
    evidence.sourceAnchors.every(
      (anchor) =>
        anchor.lifecycle === "current" &&
        anchor.location.range.endOffset > anchor.location.range.startOffset,
    )
  );
}

function sameFormulaAnchor(
  left: MathFormulaAnchorInfo,
  right: MathFormulaAnchorInfo,
): boolean {
  return (
    left.documentVersion === right.documentVersion &&
    left.location.fileId === right.location.fileId &&
    left.location.path === right.location.path &&
    left.location.range.startOffset === right.location.range.startOffset &&
    left.location.range.endOffset === right.location.range.endOffset &&
    left.sourceNotation === right.sourceNotation &&
    left.scopePath.length === right.scopePath.length &&
    left.scopePath.every((value, index) => value === right.scopePath[index])
  );
}

function rangesOverlap(left: SourceRange, right: SourceRange): boolean {
  return (
    left.startOffset < right.endOffset && right.startOffset < left.endOffset
  );
}
