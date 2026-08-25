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
        authority: hypothesisIsEstablishmentGrade(hypothesis)
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
        hypothesisContradictsFormula(hypothesis, selectedFormula)
      );
    });
  const meaningGrounded = Boolean(
    selectedFormula &&
      meaning?.evidence.some(
        (evidence) =>
          evidence.role === "supporting" &&
          interpretationEvidenceIsGrounded(evidence) &&
          evidenceOwnsFormula(evidence, selectedFormula),
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

function hypothesisIsEstablishmentGrade(
  hypothesis: FormulaRelationHypothesis,
): boolean {
  const relationRoles = hypothesis.relation.roles;
  const bindings = hypothesis.bindings;
  const roleIds = relationRoles.map((role) => role.role);
  const bindingParameters = bindings.map((binding) => binding.parameter);
  return (
    (hypothesis.support === "explicit" || hypothesis.support === "derived") &&
    hypothesis.missingDiscriminatorIds.length === 0 &&
    relationRoles.length > 0 &&
    bindings.length === relationRoles.length &&
    new Set(roleIds).size === roleIds.length &&
    new Set(bindingParameters).size === bindingParameters.length &&
    relationRoles.every((role) =>
      bindings.some(
        (binding) =>
          binding.parameter === role.role && binding.symbol === role.symbol,
      ),
    ) &&
    bindings.every(
      (binding) =>
        (binding.proof === "typed" || binding.proof === "derived") &&
        bindingEvidenceIsGrounded(hypothesis, binding.evidence),
    ) &&
    hypothesis.conditions.every((condition) => condition.status === "verified")
  );
}

function bindingEvidenceIsGrounded(
  hypothesis: FormulaRelationHypothesis,
  bindingEvidence: MathInterpretationEvidenceInfo["evidence"],
): boolean {
  return (
    bindingEvidence.sourceRanges.length > 0 &&
    bindingEvidence.sourceRanges.every(validSourceRange) &&
    hypothesis.evidence.some(
      (item) =>
        item.role === "supporting" &&
        sameEvidenceRecord(bindingEvidence, item.evidence) &&
        interpretationEvidenceIsGrounded(item) &&
        item.sourceAnchors.every(
          (anchor) =>
            validSourceRange(anchor.location.range) &&
            anchor.documentVersion === hypothesis.formula.documentVersion &&
            anchor.location.fileId === hypothesis.formula.location.fileId &&
            anchor.location.path === hypothesis.formula.location.path &&
            scopeOwns(anchor.scopePath, hypothesis.formula.scopePath),
        ),
    )
  );
}

function sameEvidenceRecord(
  left: MathInterpretationEvidenceInfo["evidence"],
  right: MathInterpretationEvidenceInfo["evidence"],
): boolean {
  return (
    left.kind === right.kind &&
    left.ruleId === right.ruleId &&
    left.strength === right.strength &&
    sameRangeSet(left.sourceRanges, right.sourceRanges)
  );
}

function sameRangeSet(
  left: readonly SourceRange[],
  right: readonly SourceRange[],
): boolean {
  const rangeKey = (range: SourceRange): string =>
    `${range.startOffset}:${range.endOffset}`;
  const leftKeys = left.map(rangeKey).sort();
  const rightKeys = right.map(rangeKey).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every((key, index) => key === rightKeys[index])
  );
}

function scopeOwns(
  owner: readonly number[],
  nested: readonly number[],
): boolean {
  return (
    owner.length <= nested.length &&
    owner.every((value, index) => value === nested[index])
  );
}

function validSourceRange(range: SourceRange): boolean {
  return (
    Number.isInteger(range.startOffset) &&
    Number.isInteger(range.endOffset) &&
    range.startOffset >= 0 &&
    range.endOffset > range.startOffset
  );
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
  return (
    (hypothesis.formula !== undefined &&
      sameFormulaAnchor(hypothesis.formula, formula)) ||
    hypothesis.evidence.some(
      (evidence) =>
        evidence.role === "contradicting" &&
        evidenceOwnsFormula(evidence, formula),
    )
  );
}

function evidenceOwnsFormula(
  evidence: MathInterpretationEvidenceInfo,
  formula: MathFormulaAnchorInfo,
): boolean {
  return evidence.sourceAnchors.some(
    (anchor) =>
      anchor.lifecycle === "current" &&
      anchor.documentVersion === formula.documentVersion &&
      anchor.location.fileId === formula.location.fileId &&
      anchor.location.path === formula.location.path &&
      sameRange(anchor.location.range, formula.location.range) &&
      sameScope(anchor.scopePath, formula.scopePath),
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
    ) &&
    evidence.evidence.sourceRanges.every((range) =>
      evidence.sourceAnchors.some((anchor) =>
        sameRange(anchor.location.range, range),
      ),
    ) &&
    evidence.sourceAnchors.every((anchor) =>
      evidence.evidence.sourceRanges.some((range) =>
        sameRange(anchor.location.range, range),
      ),
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

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return (
    left.startOffset === right.startOffset && left.endOffset === right.endOffset
  );
}

function sameScope(left: readonly number[], right: readonly number[]): boolean {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}
