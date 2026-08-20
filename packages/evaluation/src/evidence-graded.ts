import type {
  MathInterpretationEvidenceReferenceInfo,
  MathInterpretationRequirementInfo,
  MathInterpretationSetInfo,
} from "../../protocol/src/index";

export interface EvidenceGradedObservation {
  readonly caseId: string;
  readonly interpretations?: MathInterpretationSetInfo;
}

export interface EvidenceGradedFacetReport {
  readonly cases: number;
  readonly contradictionCases: number;
  readonly domainContextCases: number;
  readonly exactAnchorCases: number;
  readonly failures: readonly string[];
  readonly missingDiscriminatorCases: number;
  readonly multipleHypothesisCases: number;
  readonly naturalLanguageCases: number;
  readonly openWorldCases: number;
  readonly orderingCases: number;
  readonly reviewedConventionCases: number;
  readonly supportingEvidenceCases: number;
  readonly withHypotheses: number;
}

export function summarizeEvidenceGradedHypotheses(
  observations: readonly EvidenceGradedObservation[],
): EvidenceGradedFacetReport {
  let contradictionCases = 0;
  let domainContextCases = 0;
  let exactAnchorCases = 0;
  let missingDiscriminatorCases = 0;
  let multipleHypothesisCases = 0;
  let naturalLanguageCases = 0;
  let openWorldCases = 0;
  let orderingCases = 0;
  let reviewedConventionCases = 0;
  let supportingEvidenceCases = 0;
  let withHypotheses = 0;
  const failures: string[] = [];

  for (const observation of observations) {
    const interpretations = observation.interpretations;
    if (!interpretations) {
      failures.push(`${observation.caseId}: missing protocol-16 interpretations`);
      continue;
    }
    const hypotheses = interpretations.hypotheses;
    if (hypotheses.length > 0) withHypotheses += 1;
    if (hypotheses.length > 1) multipleHypothesisCases += 1;
    if (interpretations.missingDiscriminators.length > 0) {
      missingDiscriminatorCases += 1;
    }
    if (interpretations.exhaustiveness === "bounded-open-world") {
      openWorldCases += 1;
    } else {
      failures.push(`${observation.caseId}: candidate set is not open-world`);
    }

    const evidence = hypotheses.flatMap((hypothesis) => hypothesis.evidence);
    if (evidence.some((item) => item.role === "supporting")) {
      supportingEvidenceCases += 1;
    }
    if (evidence.some((item) => item.role === "contradicting")) {
      contradictionCases += 1;
    }
    if (evidence.some((item) => item.provenance === "natural-language-extraction")) {
      naturalLanguageCases += 1;
    }
    if (evidence.some((item) => item.provenance === "domain-context")) {
      domainContextCases += 1;
    }
    if (hypotheses.some((hypothesis) => hypothesis.kind === "reviewed-convention")) {
      reviewedConventionCases += 1;
    }

    if (hasExactInterpretationAnchors(interpretations)) exactAnchorCases += 1;
    else {
      failures.push(
        `${observation.caseId}: incomplete interpretation evidence anchor`,
      );
    }

    if (hasDeterministicEvidenceOrdering(interpretations)) orderingCases += 1;
    else failures.push(`${observation.caseId}: invalid evidence ordering`);

    const discriminatorIds = new Set(
      interpretations.missingDiscriminators.map(requirementId),
    );
    for (const hypothesis of hypotheses) {
      if (
        (hypothesis.kind === "reviewed-convention" ||
          hypothesis.kind === "scoped-domain" ||
          hypothesis.kind === "structural-alternative") &&
        (hypothesis.support === "explicit" || hypothesis.support === "derived")
      ) {
        failures.push(
          `${observation.caseId}: ${hypothesis.kind} acquired ${hypothesis.support} authority`,
        );
      }
      if (
        hypothesis.support === "contradicted" &&
        !hypothesis.evidence.some((item) => item.role === "contradicting")
      ) {
        failures.push(
          `${observation.caseId}: contradicted hypothesis lacks contradictory evidence`,
        );
      }
      for (const id of hypothesis.missingDiscriminatorIds) {
        if (!discriminatorIds.has(id)) {
          failures.push(
            `${observation.caseId}: hypothesis references unknown discriminator ${id}`,
          );
        }
      }
    }
  }

  return {
    cases: observations.length,
    contradictionCases,
    domainContextCases,
    exactAnchorCases,
    failures,
    missingDiscriminatorCases,
    multipleHypothesisCases,
    naturalLanguageCases,
    openWorldCases,
    orderingCases,
    reviewedConventionCases,
    supportingEvidenceCases,
    withHypotheses,
  };
}

export function evidenceGradedBreadthFailures(
  report: EvidenceGradedFacetReport,
): readonly string[] {
  const failures: string[] = [];
  for (const [facet, count] of [
    ["contradiction", report.contradictionCases],
    ["domain context", report.domainContextCases],
    ["missing discriminator", report.missingDiscriminatorCases],
    ["multiple hypothesis", report.multipleHypothesisCases],
    ["natural-language provenance", report.naturalLanguageCases],
    ["reviewed convention", report.reviewedConventionCases],
    ["supporting evidence", report.supportingEvidenceCases],
  ] as const) {
    if (count === 0) failures.push(`evidence facets: missing ${facet} coverage`);
  }
  for (const [facet, count] of [
    ["exact anchor", report.exactAnchorCases],
    ["open-world", report.openWorldCases],
    ["ordering", report.orderingCases],
  ] as const) {
    if (count !== report.cases) {
      failures.push(
        `evidence facets: ${facet} coverage ${count}/${report.cases}`,
      );
    }
  }
  return failures;
}

function hasExactInterpretationAnchors(
  interpretations: MathInterpretationSetInfo,
): boolean {
  return (
    interpretations.hypotheses.every(
      (hypothesis) =>
        hasExactHypothesisAnchor(hypothesis) &&
        hypothesis.evidence.every(hasExactEvidenceReference) &&
        hypothesis.orderingReasons.every((reason) =>
          reason.evidence.every(hasExactEvidenceReference),
        ),
    ) &&
    interpretations.analysisLimits.every((limit) =>
      limit.evidence.every(hasExactEvidenceReference),
    ) &&
    interpretations.missingDiscriminators.every(hasExactRequirementAnchors)
  );
}

function hasExactHypothesisAnchor(
  hypothesis: MathInterpretationSetInfo["hypotheses"][number],
): boolean {
  return (
    hypothesis.location.fileId.length > 0 &&
    hypothesis.location.path.length > 0 &&
    hypothesis.location.range.startOffset < hypothesis.location.range.endOffset &&
    hypothesis.documentVersion > 0 &&
    Number.isInteger(hypothesis.documentVersion) &&
    hypothesis.scopePath.every((part) => Number.isInteger(part) && part >= 0)
  );
}

function hasExactEvidenceReference(
  reference: MathInterpretationEvidenceReferenceInfo,
): boolean {
  const ranges = reference.evidence.sourceRanges;
  if (ranges.length === 0 || ranges.length !== reference.sourceAnchors.length) return false;
  return reference.sourceAnchors.every((anchor, index) => {
    const range = ranges[index];
    return (
      range !== undefined &&
      anchor.location.fileId.length > 0 &&
      anchor.location.path.length > 0 &&
      anchor.location.range.startOffset === range.startOffset &&
      anchor.location.range.endOffset === range.endOffset &&
      anchor.location.range.startOffset < anchor.location.range.endOffset &&
      anchor.documentVersion > 0 &&
      Number.isInteger(anchor.documentVersion) &&
      anchor.scopePath.every((part) => Number.isInteger(part) && part >= 0)
    );
  });
}

function hasExactRequirementAnchors(
  requirement: MathInterpretationRequirementInfo,
): boolean {
  switch (requirement.kind) {
    case "declaration":
    case "role-declaration":
      return requirement.evidence.every(hasExactEvidenceReference);
    case "condition":
      return requirement.condition.evidence.every(hasExactEvidenceReference);
    case "disambiguation":
      return (
        requirement.evidence.every(hasExactEvidenceReference) &&
        requirement.alternatives.every(
          (alternative) =>
            alternative.evidence.every(hasExactEvidenceReference) &&
            (alternative.relevance?.evidence.every(hasExactEvidenceReference) ??
              true),
        )
      );
  }
}

function hasDeterministicEvidenceOrdering(
  interpretations: MathInterpretationSetInfo,
): boolean {
  return interpretations.hypotheses.every(
    (hypothesis, index) =>
      hypothesis.rank === index &&
      hypothesis.orderingReasons.length > 0 &&
      hypothesis.orderingReasons.every(
        (reason) =>
          reason.kind === "stable-source-order" || reason.evidence.length > 0,
      ),
  );
}

function requirementId(requirement: MathInterpretationRequirementInfo): string {
  return requirement.requirementId;
}
