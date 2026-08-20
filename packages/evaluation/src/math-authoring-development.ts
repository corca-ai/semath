import type {
  ConventionalCandidateInfo,
  ConventionalRequirementInfo,
  DomainRelevance,
  EntityId,
  Evidence,
  LawBinding,
  LawConditionInfo,
  Location,
  MathAuthoringContext,
  MathFormulaAnchorInfo,
  MathInterpretationAlternativeInfo,
  MathInterpretationConditionInfo,
  MathInterpretationDomainRelevanceInfo,
  MathInterpretationEvidenceInfo,
  MathInterpretationEvidenceReferenceInfo,
  MathInterpretationHypothesisInfo,
  MathInterpretationRequirementInfo,
  RelationInfo,
  SemanticConstraint,
  SourceOccurrenceId,
  SourceRange,
} from "../../protocol/src/index";
import { parseMathInterpretationCandidateCapInfo } from "../../protocol/src/index";

export interface StableRequirementBase {
  readonly group: number;
  readonly kind: MathInterpretationRequirementInfo["kind"];
}

export type StableRequirement =
  | (StableRequirementBase & {
      readonly evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      readonly kind: "declaration";
      readonly occurrenceDocumentVersion: number;
      readonly occurrenceFileId: string;
      readonly occurrenceGroup: number;
      readonly symbol: string;
    })
  | (StableRequirementBase & {
      readonly constraint: SemanticConstraint;
      readonly evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      readonly kind: "role-declaration";
      readonly parameter: string;
      readonly symbol: string;
    })
  | (StableRequirementBase & {
      readonly condition: MathInterpretationConditionInfo;
      readonly kind: "condition";
    })
  | (StableRequirementBase & {
      readonly alternatives: readonly StableMeaningAlternative[];
      readonly evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      readonly kind: "disambiguation";
    });

export interface StableMeaningAlternative
  extends Omit<MathInterpretationAlternativeInfo, "alternativeId"> {
  readonly alternativeGroup: number;
}

export type StableConventionalRequirement =
  | Omit<Extract<ConventionalRequirementInfo, { kind: "role-declaration" }>, "requirementId">
  | Omit<Extract<ConventionalRequirementInfo, { kind: "condition" }>, "requirementId">;

export interface StableConventionalCandidate
  extends Omit<ConventionalCandidateInfo, "candidateId" | "requirements"> {
  readonly candidateGroup: number;
  readonly requirements: readonly StableConventionalRequirement[];
}

export interface StableEquationLink {
  readonly evidence: readonly Evidence[];
  readonly kind: "derived-law" | "shared-entity";
  readonly linkGroup: number;
  readonly sharedEntityGroups: readonly number[];
  readonly source: MathFormulaAnchorInfo;
  readonly target: MathFormulaAnchorInfo;
}

export interface StableApproximation {
  readonly evidence: readonly Evidence[];
  readonly exactness: "approximate";
  readonly relatedFactGroups?: readonly number[];
  readonly relationRange: SourceRange;
}

export interface StableClaimEvidence {
  readonly claim: Location;
  readonly claimGroup: number;
  readonly evidence: readonly Evidence[];
  readonly modality: "asserted" | "cited" | "hedged" | "hypothetical" | "quoted";
  readonly polarity: "negative" | "positive";
  readonly strengthCeiling: "asserted" | "qualified" | "unusable";
  readonly supportingClaimGroups: readonly number[];
  readonly supportingFormulas: readonly MathFormulaAnchorInfo[];
}

export interface StableNotationOccurrence {
  readonly entityAnchorDocumentVersion: number;
  readonly entityAnchorFileId: string;
  readonly entityAnchorOccurrenceGroup: number;
  readonly entityGroup: number;
  readonly entityKind: string;
  readonly entityScopePath: readonly number[];
  readonly location: Location;
  readonly occurrenceDocumentVersion: number;
  readonly occurrenceFileId: string;
  readonly occurrenceGroup: number;
  readonly scopePath: readonly number[];
  readonly sourceNotation: string;
}

export interface StableInterpretationKey {
  readonly documentVersion: number;
  readonly kind: MathInterpretationHypothesisInfo["kind"];
  readonly label: string;
  readonly location: Location;
  readonly scopePath: readonly number[];
}

export interface StableInterpretationHypothesis
  extends Omit<
    MathInterpretationHypothesisInfo,
    "hypothesisId" | "missingDiscriminatorIds" | "orderingReasons"
  > {
  readonly hypothesisGroup: number;
  readonly key: StableInterpretationKey;
  readonly missingDiscriminatorGroups: readonly number[];
  readonly orderingReasons: readonly {
    readonly evidence: readonly MathInterpretationEvidenceReferenceInfo[];
    readonly kind: MathInterpretationHypothesisInfo["orderingReasons"][number]["kind"];
  }[];
}

export interface StableMathAuthoringContext {
  readonly approximation?: StableApproximation;
  readonly claimEvidence: readonly StableClaimEvidence[];
  readonly conditions: readonly LawConditionInfo[];
  readonly conventionalCandidates?: readonly StableConventionalCandidate[];
  readonly disposition: MathAuthoringContext["disposition"];
  readonly equationLinks: readonly StableEquationLink[];
  readonly formula?: MathFormulaAnchorInfo;
  readonly lifecycle: MathAuthoringContext["lifecycle"];
  readonly interpretations: {
    readonly analysisLimits: readonly {
      readonly evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      readonly kind: MathAuthoringContext["interpretations"]["analysisLimits"][number]["kind"];
    }[];
    readonly candidateCap?: MathAuthoringContext["interpretations"]["candidateCap"];
    readonly exhaustiveness: "bounded-open-world";
    readonly hypotheses: readonly StableInterpretationHypothesis[];
    readonly missingDiscriminators: readonly StableRequirement[];
    readonly truncated: boolean;
  };
  readonly notationOccurrences: readonly StableNotationOccurrence[];
  readonly requirements: readonly StableRequirement[];
  readonly truncated: boolean;
}

export interface MathAuthoringExpectationProbe {
  readonly expected: { readonly authoringContext?: StableMathAuthoringContext };
  readonly id: string;
}

export interface MathAuthoringContextObservation {
  readonly authoringContext?: MathAuthoringContext;
  readonly caseId: string;
}

export type MathAuthoringFailureKind =
  | "authority-escalation"
  | "false-conflict"
  | "mismatch"
  | "missing"
  | "unexpected"
  | "unsafe-lifecycle"
  | "wrong-anchor";

export interface MathAuthoringContextFailure {
  readonly actual?: unknown;
  readonly expected?: unknown;
  readonly kind: MathAuthoringFailureKind;
  readonly path: string;
}

export interface MathAuthoringSourceDocument {
  readonly content: string;
  readonly documentVersion: number;
  readonly fileId: string;
  readonly path: string;
}

export interface MathAuthoringDevelopmentReport {
  readonly cases: number;
  readonly exactCases: number;
  readonly failures: readonly string[];
  readonly findings: readonly MathAuthoringContextFailure[];
}

export function mathAuthoringExactRegressions(
  report: Pick<MathAuthoringDevelopmentReport, "cases" | "exactCases" | "failures">,
  requiredCases: number,
): readonly string[] {
  const regressions: string[] = [];
  if (!Number.isInteger(requiredCases) || requiredCases <= 0) {
    throw new Error("requiredCases: expected positive integer");
  }
  if (report.cases !== requiredCases) {
    regressions.push(
      `authoring-context case count ${report.cases} differs from required ${requiredCases}`,
    );
  }
  if (report.exactCases !== report.cases || report.exactCases !== requiredCases) {
    regressions.push(
      `exact authoring context ${report.exactCases}/${report.cases}; required ${requiredCases}/${requiredCases}`,
    );
  }
  regressions.push(
    ...report.failures.map((failure) => `authoring-context safety: ${failure}`),
  );
  return regressions;
}

export const MATH_AUTHORING_DEVELOPMENT_FACETS = [
  "approximation",
  "cap",
  "claim-evidence",
  "clean-incremental",
  "conditions",
  "conventional-candidates",
  "cross-document",
  "equation-links",
  "generated",
  "interpretations",
  "lifecycle",
  "notation",
  "requirements",
  "retraction-transition",
] as const;

export type MathAuthoringDevelopmentFacet =
  (typeof MATH_AUTHORING_DEVELOPMENT_FACETS)[number];

/**
 * Return the non-cursor files that directly ground a selected interpretation.
 *
 * This deliberately inspects the reviewed stable expectation, not engine IDs or
 * the surrounding scenario. A case therefore earns cross-document coverage only
 * when its exact oracle contains a source anchor in another document.
 */
export function mathAuthoringCrossDocumentEvidenceFiles(
  expected: StableMathAuthoringContext,
  cursorFileId: string,
): readonly string[] {
  return [
    ...new Set(
      allStableEvidenceAnchors(expected)
        .map((anchor) => anchor.location.fileId)
        .filter((fileId) => fileId !== cursorFileId),
    ),
  ].sort();
}

/** Check that a declared breadth facet is present in the reviewed expectation. */
export function mathAuthoringExpectedFacetPresent(
  expected: StableMathAuthoringContext,
  cursorFileId: string,
  facet: MathAuthoringDevelopmentFacet,
  hasPriorSnapshot: boolean,
): boolean {
  switch (facet) {
    case "approximation": return expected.approximation !== undefined;
    case "cap": return expected.truncated && expected.lifecycle.capped &&
      expected.interpretations.truncated &&
      expected.interpretations.analysisLimits.some(
        (limit) => limit.kind === "candidate-set-capped",
      );
    case "claim-evidence": return expected.claimEvidence.length > 0;
    case "clean-incremental": return true;
    case "conditions": return expected.conditions.length > 0;
    case "conventional-candidates": return (expected.conventionalCandidates?.length ?? 0) > 0;
    case "cross-document": return mathAuthoringCrossDocumentEvidenceFiles(
      expected,
      cursorFileId,
    ).length > 0;
    case "equation-links": return expected.equationLinks.length > 0;
    case "generated": return expected.lifecycle.generation === "generated" ||
      expected.interpretations.analysisLimits.some((limit) => limit.kind === "generated-source");
    case "interpretations": return expected.interpretations.hypotheses.length > 0;
    case "lifecycle": return true;
    case "notation": return expected.notationOccurrences.length > 0;
    case "requirements": return expected.requirements.length > 0;
    // A prior snapshot alone is not a semantic retraction contract. Protocol-v2
    // transition constraints must review the before/after authority delta.
    case "retraction-transition": return false;
  }
}

export interface MathAuthoringDevelopmentFixture {
  readonly cases: readonly {
    readonly expected: StableMathAuthoringContext;
    readonly facets: readonly MathAuthoringDevelopmentFacet[];
    readonly id: string;
    readonly probeId: string;
    readonly sourceEdits?: readonly {
      readonly fileId: string;
      readonly replacement: string;
      readonly search: string;
    }[];
  }[];
  readonly pairs: readonly {
    readonly id: string;
    readonly latexCaseId: string;
    readonly markdownCaseId: string;
  }[];
  readonly review: {
    readonly digest: string;
    readonly reviewedAt: string;
    readonly reviewer: string;
  };
  readonly schemaVersion: 1;
  readonly sourceFixture: "fixtures/challenge/document-reasoning-development-v1.json";
}

export function parseMathAuthoringDevelopmentFixture(
  value: unknown,
): MathAuthoringDevelopmentFixture {
  const root = object(value, "fixture", [
    "cases", "pairs", "review", "schemaVersion", "sourceFixture",
  ]);
  if (root.schemaVersion !== 1) throw new Error("fixture.schemaVersion: expected 1");
  if (root.sourceFixture !== "fixtures/challenge/document-reasoning-development-v1.json") {
    throw new Error("fixture.sourceFixture: expected the public development fixture");
  }
  const cases = array(root.cases, "fixture.cases").map((value, index) => {
    const path = `fixture.cases[${index}]`;
    const item = object(
      value,
      path,
      ["expected", "facets", "id", "probeId"],
      ["sourceEdits"],
    );
    const facets = array(item.facets, `${path}.facets`).map((facet, facetIndex) =>
      choice(
        facet,
        MATH_AUTHORING_DEVELOPMENT_FACETS,
        `${path}.facets[${facetIndex}]`,
      ),
    );
    unique(facets, `${path}.facets`);
    return {
      expected: parseMathAuthoringContextExpectation(item.expected, `${path}.expected`),
      facets,
      id: text(item.id, `${path}.id`),
      probeId: text(item.probeId, `${path}.probeId`),
      ...(item.sourceEdits === undefined
        ? {}
        : {
            sourceEdits: array(item.sourceEdits, `${path}.sourceEdits`).map(
              (value, editIndex) => {
                const editPath = `${path}.sourceEdits[${editIndex}]`;
                const edit = object(value, editPath, [
                  "fileId", "replacement", "search",
                ]);
                return {
                  fileId: text(edit.fileId, `${editPath}.fileId`),
                  replacement: text(edit.replacement, `${editPath}.replacement`),
                  search: text(edit.search, `${editPath}.search`),
                };
              },
            ),
          }),
    };
  });
  if (cases.length < 12 || cases.length > 20) {
    throw new Error("fixture.cases: expected 12 to 20 independently reviewed cases");
  }
  unique(cases.map((item) => item.id), "fixture.cases.id");
  unique(cases.map((item) => item.probeId), "fixture.cases.probeId");
  const covered = new Set(cases.flatMap((item) => item.facets));
  for (const facet of MATH_AUTHORING_DEVELOPMENT_FACETS) {
    if (!covered.has(facet)) throw new Error(`fixture.cases: missing ${facet} coverage`);
  }
  const ids = new Set(cases.map((item) => item.id));
  const pairs = array(root.pairs, "fixture.pairs").map((value, index) => {
    const path = `fixture.pairs[${index}]`;
    const item = object(value, path, ["id", "latexCaseId", "markdownCaseId"]);
    const pair = {
      id: text(item.id, `${path}.id`),
      latexCaseId: text(item.latexCaseId, `${path}.latexCaseId`),
      markdownCaseId: text(item.markdownCaseId, `${path}.markdownCaseId`),
    };
    if (!ids.has(pair.latexCaseId) || !ids.has(pair.markdownCaseId)) {
      throw new Error(`${path}: pair references an unknown case`);
    }
    if (pair.latexCaseId === pair.markdownCaseId) {
      throw new Error(`${path}: TeX and Markdown cases must differ`);
    }
    return pair;
  });
  unique(pairs.map((item) => item.id), "fixture.pairs.id");
  const pairedCaseIds = pairs.flatMap((pair) => [
    pair.latexCaseId,
    pair.markdownCaseId,
  ]);
  unique(pairedCaseIds, "fixture.pairs.caseId");
  if (pairedCaseIds.length !== cases.length) {
    throw new Error("fixture.pairs: every reviewed case must belong to one TeX/Markdown pair");
  }
  const review = object(root.review, "fixture.review", [
    "digest", "reviewedAt", "reviewer",
  ]);
  const digest = text(review.digest, "fixture.review.digest");
  if (!/^[0-9a-f]{64}$/u.test(digest)) {
    throw new Error("fixture.review.digest: expected sha256 digest");
  }
  const reviewedAt = text(review.reviewedAt, "fixture.review.reviewedAt");
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(reviewedAt)) {
    throw new Error("fixture.review.reviewedAt: expected YYYY-MM-DD");
  }
  return {
    cases,
    pairs,
    review: { digest, reviewedAt, reviewer: text(review.reviewer, "fixture.review.reviewer") },
    schemaVersion: 1,
    sourceFixture: root.sourceFixture,
  };
}

export function mathAuthoringFixtureReviewPayload(
  fixture: MathAuthoringDevelopmentFixture,
): string {
  const { digest: _digest, ...review } = fixture.review;
  return stableJson({ ...fixture, review });
}

/** Project protocol output to a host-neutral, reviewable identity contract. */
export function projectMathAuthoringContext(
  context: MathAuthoringContext,
): StableMathAuthoringContext {
  const occurrences = new DenseGroups<SourceOccurrenceId>();
  const entities = new DenseGroups<EntityId>();
  const requirements = new DenseGroups<string>();
  const claims = new DenseGroups<string>();
  const facts = new DenseGroups<string>();

  const notation = [...context.notationOccurrences].sort((left, right) =>
    stableJson(notationSemanticKey(left)).localeCompare(stableJson(notationSemanticKey(right))),
  );
  for (const item of notation) {
    occurrences.group(item.occurrenceId);
    occurrences.group(item.entityId.anchor);
    entities.group(item.entityId);
  }

  const allRequirements = [
    ...context.requirements,
    ...context.interpretations.missingDiscriminators,
  ].sort((left, right) =>
    stableJson(requirementSemanticKey(left)).localeCompare(
      stableJson(requirementSemanticKey(right)),
    ),
  );
  for (const requirement of allRequirements) {
    if (requirement.kind === "declaration") occurrences.group(requirement.occurrenceId);
    requirements.group(requirement.requirementId);
  }

  const rawLinks = [...context.equationLinks].sort((left, right) =>
    stableJson(linkSemanticKey(left)).localeCompare(stableJson(linkSemanticKey(right))),
  );
  for (const link of rawLinks) {
    for (const entity of link.sharedEntities) entities.group(entity);
  }

  const rawClaims = [...context.claimEvidence].sort((left, right) =>
    stableJson(claimSemanticKey(left)).localeCompare(stableJson(claimSemanticKey(right))),
  );
  for (const claim of rawClaims) claims.group(claim.claimId);
  for (const claim of rawClaims) {
    for (const supporting of claim.supportingClaimIds) claims.group(supporting);
  }

  const stableRequirements = (items: readonly MathInterpretationRequirementInfo[]) =>
    items
      .map((item) => projectRequirement(item, requirements, occurrences))
      .sort(stableCompare);
  const conditions = context.conditions.map(projectCondition).sort(stableCompare);
  const candidates = context.conventionalCandidates
    ?.map((candidate) => projectCandidate(candidate))
    .sort(stableCompare)
    .map((candidate, candidateGroup) => ({ ...candidate, candidateGroup }));
  const equationLinks = rawLinks
    .map((link, linkGroup) => ({
      evidence: sortEvidence(link.evidence),
      kind: link.kind,
      linkGroup,
      sharedEntityGroups: link.sharedEntities.map((entity) => entities.group(entity)).sort(numeric),
      source: projectFormula(link.source),
      target: projectFormula(link.target),
    }))
    .sort(stableCompare);
  const claimEvidence = rawClaims
    .map((claim) => ({
      claim: projectLocation(claim.claim),
      claimGroup: claims.group(claim.claimId),
      evidence: sortEvidence(claim.evidence),
      modality: claim.modality,
      polarity: claim.polarity,
      strengthCeiling: claim.strengthCeiling,
      supportingClaimGroups: claim.supportingClaimIds.map((id) => claims.group(id)).sort(numeric),
      supportingFormulas: claim.supportingFormulas.map(projectFormula).sort(stableCompare),
    }))
    .sort(stableCompare);
  const notationOccurrences = notation.map((item) => ({
    entityAnchorDocumentVersion: item.entityId.anchor.documentVersion,
    entityAnchorFileId: item.entityId.anchor.fileId,
    entityAnchorOccurrenceGroup: occurrences.group(item.entityId.anchor),
    entityGroup: entities.group(item.entityId),
    entityKind: item.entityId.kind,
    entityScopePath: [...item.entityId.scopePath],
    location: projectLocation(item.location),
    occurrenceDocumentVersion: item.occurrenceId.documentVersion,
    occurrenceFileId: item.occurrenceId.fileId,
    occurrenceGroup: occurrences.group(item.occurrenceId),
    scopePath: [...item.scopePath],
    sourceNotation: item.sourceNotation,
  }));
  const hypotheses = context.interpretations.hypotheses.map((hypothesis, index) => {
    const projected = {
      bindings: hypothesis.bindings.map(projectBinding).sort(stableCompare),
      conditions: hypothesis.conditions.map(projectCondition).sort(stableCompare),
      documentVersion: hypothesis.documentVersion,
      evidence: hypothesis.evidence.map(projectInterpretationEvidence),
      ...(hypothesis.formula ? { formula: projectFormula(hypothesis.formula) } : {}),
      hypothesisGroup: index,
      kind: hypothesis.kind,
      label: hypothesis.label,
      location: projectLocation(hypothesis.location),
      missingDiscriminatorGroups: hypothesis.missingDiscriminatorIds
        .map((id) => requirements.group(id))
        .sort(numeric),
      orderingReasons: hypothesis.orderingReasons.map((reason) => ({
        evidence: reason.evidence.map(projectOrderedEvidenceReference),
        kind: reason.kind,
      })),
      range: projectRange(hypothesis.range),
      rank: hypothesis.rank,
      ...(hypothesis.relation ? { relation: projectRelation(hypothesis.relation) } : {}),
      scopePath: [...hypothesis.scopePath],
      support: hypothesis.support,
    };
    return { ...projected, key: hypothesisKey(projected) };
  });

  return {
    ...(context.approximation
      ? {
          approximation: {
            evidence: sortEvidence(context.approximation.evidence),
            exactness: context.approximation.exactness,
            ...(context.approximation.relatedFactIds
              ? {
                  relatedFactGroups: context.approximation.relatedFactIds.map(
                    (id) => facts.group(id),
                  ),
                }
              : {}),
            relationRange: projectRange(context.approximation.relationRange),
          },
        }
      : {}),
    claimEvidence,
    conditions,
    ...(candidates ? { conventionalCandidates: candidates } : {}),
    disposition: context.disposition,
    equationLinks,
    ...(context.formula ? { formula: projectFormula(context.formula) } : {}),
    lifecycle: { ...context.lifecycle },
    interpretations: {
      analysisLimits: context.interpretations.analysisLimits
        .map((limit) => ({
          evidence: sortEvidenceReferences(limit.evidence),
          kind: limit.kind,
        }))
        .sort(stableCompare),
      ...(context.interpretations.candidateCap
        ? { candidateCap: { ...context.interpretations.candidateCap } }
        : {}),
      exhaustiveness: context.interpretations.exhaustiveness,
      hypotheses,
      missingDiscriminators: stableRequirements(
        context.interpretations.missingDiscriminators,
      ),
      truncated: context.interpretations.truncated,
    },
    notationOccurrences,
    requirements: stableRequirements(context.requirements),
    truncated: context.truncated,
  };
}

export function parseMathAuthoringContextExpectation(
  value: unknown,
  path: string,
): StableMathAuthoringContext {
  const item = object(value, path, [
    "claimEvidence", "conditions", "disposition", "equationLinks", "lifecycle",
    "interpretations", "notationOccurrences", "requirements", "truncated",
  ], ["approximation", "conventionalCandidates", "formula"]);
  optional(item.approximation, `${path}.approximation`, parseApproximation);
  list(item.claimEvidence, `${path}.claimEvidence`, parseClaim);
  list(item.conditions, `${path}.conditions`, parseCondition);
  optionalList(item.conventionalCandidates, `${path}.conventionalCandidates`, parseCandidate);
  choice(item.disposition, ["established", "partial", "conventional", "ambiguous", "conflicting", "unsupported", "engine-limited"], `${path}.disposition`);
  list(item.equationLinks, `${path}.equationLinks`, parseLink);
  optional(item.formula, `${path}.formula`, parseFormula);
  parseLifecycle(item.lifecycle, `${path}.lifecycle`);
  parseInterpretations(item.interpretations, `${path}.interpretations`);
  list(item.notationOccurrences, `${path}.notationOccurrences`, parseNotation);
  list(item.requirements, `${path}.requirements`, parseRequirement);
  bool(item.truncated, `${path}.truncated`);
  const expected = value as StableMathAuthoringContext;
  validateStableExpectation(expected, path);
  return expected;
}

export function parseMathAuthoringReportObservations(
  value: unknown,
): readonly { readonly authoringContext: StableMathAuthoringContext; readonly caseId: string }[] {
  const report = record(value, "report");
  const results = array(report.results, "report.results");
  if (results.length !== 1) throw new Error("report.results: expected exactly one public development result");
  const result = record(results[0], "report.results[0]");
  const observations = array(result.observations, "report.results[0].observations").map(
    (value, index) => {
      const path = `report.results[0].observations[${index}]`;
      const observation = record(value, path);
      const caseId = text(observation.caseId, `${path}.caseId`);
      if (observation.authoringContext === undefined) throw new Error(`${path}.authoringContext: missing complete context`);
      const projected = projectMathAuthoringContext(
        observation.authoringContext as MathAuthoringContext,
      );
      return {
        authoringContext: parseMathAuthoringContextExpectation(
          projected,
          `${path}.authoringContext.stableProjection`,
        ),
        caseId,
      };
    },
  );
  unique(observations.map((item) => item.caseId), "public observation caseId");
  return observations;
}

/** Strictly validate a raw protocol observation through its stable public projection. */
export function parseObservedMathAuthoringContext(
  value: unknown,
  path: string,
): MathAuthoringContext {
  const raw = value as MathAuthoringContext;
  const projected = projectMathAuthoringContext(raw);
  parseMathAuthoringContextExpectation(projected, `${path}.stableProjection`);
  return raw;
}

export function compareMathAuthoringContext(
  expected: StableMathAuthoringContext,
  actual: MathAuthoringContext,
): readonly MathAuthoringContextFailure[] {
  const findings: MathAuthoringContextFailure[] = [];
  compareJson(expected, projectMathAuthoringContext(actual), "authoringContext", findings);
  findings.push(...mathAuthoringContextSafetyFailures(actual));
  return deduplicate(findings);
}

export function evaluateMathAuthoringDevelopment(
  probes: readonly MathAuthoringExpectationProbe[],
  observations: readonly MathAuthoringContextObservation[],
): MathAuthoringDevelopmentReport {
  const findings: MathAuthoringContextFailure[] = [];
  const expectedIds = new Set(probes.map((probe) => probe.id));
  const byId = new Map<string, MathAuthoringContextObservation>();
  for (const observation of observations) {
    if (!expectedIds.has(observation.caseId) || byId.has(observation.caseId)) {
      findings.push({ actual: observation.caseId, kind: "unexpected", path: `${observation.caseId}.observation` });
    } else byId.set(observation.caseId, observation);
  }
  let exactCases = 0;
  for (const probe of probes) {
    const expected = probe.expected.authoringContext;
    const actual = byId.get(probe.id)?.authoringContext;
    const local: MathAuthoringContextFailure[] = [];
    if (!expected) local.push({ kind: "missing", path: `${probe.id}.expected.authoringContext` });
    if (!actual) local.push({ kind: "missing", path: `${probe.id}.observed.authoringContext` });
    if (expected && actual) {
      local.push(...compareMathAuthoringContext(expected, actual).map((finding) => ({ ...finding, path: `${probe.id}.${finding.path}` })));
    }
    if (!local.length) exactCases += 1;
    findings.push(...local);
  }
  return { cases: probes.length, exactCases, failures: findings.map(formatFailure), findings };
}

export function mathAuthoringContextSafetyFailures(
  context: MathAuthoringContext,
): readonly MathAuthoringContextFailure[] {
  const findings: MathAuthoringContextFailure[] = [];
  if ((context.lifecycle.retracted || context.lifecycle.generation === "generated") && context.lifecycle.editable) {
    findings.push({ actual: true, expected: false, kind: "unsafe-lifecycle", path: "authoringContext.lifecycle.editable" });
  }
  const hasEngineLimit = context.interpretations.analysisLimits.some((limit) => limit.kind === "engine-limit");
  if (context.lifecycle.engineLimited !== hasEngineLimit) {
    findings.push({ actual: context.lifecycle.engineLimited, expected: hasEngineLimit, kind: "unsafe-lifecycle", path: "authoringContext.lifecycle.engineLimited" });
  }
  const discriminatorIds = new Set<string>();
  for (const requirement of context.interpretations.missingDiscriminators) {
    if (discriminatorIds.has(requirement.requirementId)) findings.push({ actual: requirement.requirementId, kind: "unexpected", path: "authoringContext.interpretations.missingDiscriminators" });
    discriminatorIds.add(requirement.requirementId);
  }
  const hypothesisIds = new Set<string>();
  for (const [index, hypothesis] of context.interpretations.hypotheses.entries()) {
    const path = `authoringContext.interpretations.hypotheses[${index}]`;
    if (hypothesisIds.has(hypothesis.hypothesisId)) findings.push({ actual: hypothesis.hypothesisId, kind: "unexpected", path: `${path}.hypothesisId` });
    hypothesisIds.add(hypothesis.hypothesisId);
    if (hypothesis.rank !== index) findings.push({ actual: hypothesis.rank, expected: index, kind: "mismatch", path: `${path}.rank` });
    if (!sameRange(hypothesis.range, hypothesis.location.range) || !validLocation(hypothesis.location, hypothesis.documentVersion, hypothesis.scopePath)) findings.push({ kind: "wrong-anchor", path: `${path}.location` });
    validateAuthority(hypothesis, path, findings);
    for (const id of hypothesis.missingDiscriminatorIds) if (!discriminatorIds.has(id)) findings.push({ actual: id, kind: "missing", path: `${path}.missingDiscriminatorIds` });
    for (const [evidenceIndex, evidence] of hypothesis.evidence.entries()) validateEvidenceAnchors(evidence, `${path}.evidence[${evidenceIndex}]`, findings);
    if (hypothesis.support === "contradicted" && !hypothesis.evidence.some((item) => item.role === "contradicting")) findings.push({ actual: "contradicted", kind: "false-conflict", path: `${path}.support` });
  }
  return findings;
}

/** Validate reviewed stable anchors against the exact selected source snapshot. */
export function mathAuthoringExpectationSourceFailures(
  expected: StableMathAuthoringContext,
  documents: readonly MathAuthoringSourceDocument[],
): readonly MathAuthoringContextFailure[] {
  const findings: MathAuthoringContextFailure[] = [];
  const byId = new Map(documents.map((document) => [document.fileId, document]));
  const location = (
    value: Location,
    documentVersion: number,
    path: string,
  ) => {
    const document = byId.get(value.fileId);
    if (!document || document.path !== value.path ||
      document.documentVersion !== documentVersion ||
      value.range.startOffset < 0 ||
      value.range.endOffset > document.content.length ||
      value.range.startOffset >= value.range.endOffset) {
      findings.push({
        actual: value,
        expected: document
          ? {
              documentVersion: document.documentVersion,
              fileId: document.fileId,
              length: document.content.length,
              path: document.path,
            }
          : undefined,
        kind: "wrong-anchor",
        path,
      });
    }
  };
  const formula = (value: MathFormulaAnchorInfo, path: string) => {
    location(value.location, value.documentVersion, `${path}.location`);
    value.provenance?.forEach((range, index) =>
      location(
        { ...value.location, range },
        value.documentVersion,
        `${path}.provenance[${index}]`,
      )
    );
  };
  const references = (
    values: readonly MathInterpretationEvidenceReferenceInfo[],
    path: string,
  ) => values.forEach((reference, referenceIndex) =>
    reference.sourceAnchors.forEach((anchor, anchorIndex) =>
      location(
        anchor.location,
        anchor.documentVersion,
        `${path}[${referenceIndex}].sourceAnchors[${anchorIndex}].location`,
      )
    )
  );
  const requirementReferences = (
    requirement: StableRequirement,
    path: string,
  ) => {
    if (requirement.kind === "condition") {
      references(requirement.condition.evidence, `${path}.condition.evidence`);
      return;
    }
    references(requirement.evidence, `${path}.evidence`);
    if (requirement.kind === "disambiguation") {
      requirement.alternatives.forEach((alternative, alternativeIndex) => {
        const alternativePath = `${path}.alternatives[${alternativeIndex}]`;
        references(alternative.evidence, `${alternativePath}.evidence`);
        if (alternative.relevance) {
          references(alternative.relevance.evidence, `${alternativePath}.relevance.evidence`);
        }
      });
    }
  };

  if (expected.formula) formula(expected.formula, "authoringContext.formula");
  for (const [index, link] of expected.equationLinks.entries()) {
    formula(link.source, `authoringContext.equationLinks[${index}].source`);
    formula(link.target, `authoringContext.equationLinks[${index}].target`);
  }
  for (const [index, claim] of expected.claimEvidence.entries()) {
    location(
      claim.claim,
      expected.lifecycle.documentVersion,
      `authoringContext.claimEvidence[${index}].claim`,
    );
    claim.supportingFormulas.forEach((item, formulaIndex) =>
      formula(
        item,
        `authoringContext.claimEvidence[${index}].supportingFormulas[${formulaIndex}]`,
      )
    );
  }
  for (const [index, occurrence] of expected.notationOccurrences.entries()) {
    location(
      occurrence.location,
      occurrence.occurrenceDocumentVersion,
      `authoringContext.notationOccurrences[${index}].location`,
    );
    validateOccurrenceDocument(
      byId,
      occurrence.entityAnchorFileId,
      occurrence.entityAnchorDocumentVersion,
      `authoringContext.notationOccurrences[${index}].entityAnchor`,
      findings,
    );
  }
  for (const [index, hypothesis] of expected.interpretations.hypotheses.entries()) {
    const hypothesisPath = `authoringContext.interpretations.hypotheses[${index}]`;
    location(hypothesis.location, hypothesis.documentVersion, `${hypothesisPath}.location`);
    if (hypothesis.formula) formula(hypothesis.formula, `${hypothesisPath}.formula`);
    for (const [evidenceIndex, evidence] of hypothesis.evidence.entries()) {
      evidence.sourceAnchors.forEach((anchor, anchorIndex) =>
        location(
          anchor.location,
          anchor.documentVersion,
          `${hypothesisPath}.evidence[${evidenceIndex}].sourceAnchors[${anchorIndex}].location`,
        )
      );
    }
    hypothesis.orderingReasons.forEach((reason, reasonIndex) =>
      references(reason.evidence, `${hypothesisPath}.orderingReasons[${reasonIndex}].evidence`)
    );
  }
  expected.interpretations.analysisLimits.forEach((limit, limitIndex) =>
    references(
      limit.evidence,
      `authoringContext.interpretations.analysisLimits[${limitIndex}].evidence`,
    )
  );
  for (const [collection, requirements] of [
    ["requirements", expected.requirements],
    ["interpretations.missingDiscriminators", expected.interpretations.missingDiscriminators],
  ] as const) {
    requirements.forEach((requirement, index) => {
      requirementReferences(requirement, `authoringContext.${collection}[${index}]`);
      if (requirement.kind === "declaration") {
        validateOccurrenceDocument(
          byId,
          requirement.occurrenceFileId,
          requirement.occurrenceDocumentVersion,
          `authoringContext.${collection}[${index}].occurrence`,
          findings,
        );
      }
    });
  }
  return findings;
}

function validateOccurrenceDocument(
  documents: ReadonlyMap<string, MathAuthoringSourceDocument>,
  fileId: string,
  documentVersion: number,
  path: string,
  findings: MathAuthoringContextFailure[],
): void {
  const document = documents.get(fileId);
  if (!document || document.documentVersion !== documentVersion) {
    findings.push({
      actual: { documentVersion, fileId },
      expected: document
        ? { documentVersion: document.documentVersion, fileId: document.fileId }
        : undefined,
      kind: "wrong-anchor",
      path,
    });
  }
}

function allStableEvidenceAnchors(
  expected: StableMathAuthoringContext,
): readonly MathInterpretationEvidenceInfo["sourceAnchors"][number][] {
  const anchors: MathInterpretationEvidenceInfo["sourceAnchors"][number][] = [];
  const references = (values: readonly MathInterpretationEvidenceReferenceInfo[]) => {
    anchors.push(...values.flatMap((value) => value.sourceAnchors));
  };
  const requirement = (value: StableRequirement) => {
    if (value.kind === "condition") {
      references(value.condition.evidence);
      return;
    }
    references(value.evidence);
    if (value.kind === "disambiguation") {
      for (const alternative of value.alternatives) {
        references(alternative.evidence);
        if (alternative.relevance) references(alternative.relevance.evidence);
      }
    }
  };
  for (const hypothesis of expected.interpretations.hypotheses) {
    anchors.push(...hypothesis.evidence.flatMap((value) => value.sourceAnchors));
    hypothesis.orderingReasons.forEach((reason) => references(reason.evidence));
  }
  expected.interpretations.analysisLimits.forEach((limit) => references(limit.evidence));
  expected.requirements.forEach(requirement);
  expected.interpretations.missingDiscriminators.forEach(requirement);
  return anchors;
}

function projectRequirement(item: MathInterpretationRequirementInfo, groups: DenseGroups<string>, occurrences: DenseGroups<SourceOccurrenceId>): StableRequirement {
  const group = groups.group(item.requirementId);
  switch (item.kind) {
    case "declaration": return { evidence: sortEvidenceReferences(item.evidence), group, kind: item.kind, occurrenceDocumentVersion: item.occurrenceId.documentVersion, occurrenceFileId: item.occurrenceId.fileId, occurrenceGroup: occurrences.group(item.occurrenceId), symbol: item.symbol };
    case "role-declaration": return { constraint: projectConstraint(item.constraint), evidence: sortEvidenceReferences(item.evidence), group, kind: item.kind, parameter: item.parameter, symbol: item.symbol };
    case "condition": return { condition: projectInterpretationCondition(item.condition), group, kind: item.kind };
    case "disambiguation": return { alternatives: projectInterpretationAlternatives(item.alternatives), evidence: sortEvidenceReferences(item.evidence), group, kind: item.kind };
  }
}

function projectInterpretationCondition(item: MathInterpretationConditionInfo): MathInterpretationConditionInfo {
  return {
    conditionId: item.conditionId,
    evidence: sortEvidenceReferences(item.evidence),
    kind: item.kind,
    label: item.label,
    ...(item.operatorProperty ? { operatorProperty: item.operatorProperty } : {}),
    status: item.status,
    subjects: [...item.subjects].sort(),
  };
}

function projectInterpretationAlternatives(items: readonly MathInterpretationAlternativeInfo[]): readonly StableMeaningAlternative[] {
  return items.map((item) => ({
    evidence: sortEvidenceReferences(item.evidence),
    label: item.label,
    range: projectRange(item.range),
    ...(item.relevance ? {
      relevance: projectInterpretationRelevance(item.relevance),
    } : {}),
  })).sort(stableCompare).map((item, alternativeGroup) => ({ ...item, alternativeGroup }));
}

function projectInterpretationRelevance(
  item: MathInterpretationDomainRelevanceInfo,
): MathInterpretationDomainRelevanceInfo {
  return { evidence: sortEvidenceReferences(item.evidence), support: item.support };
}

function projectCandidate(item: ConventionalCandidateInfo): Omit<StableConventionalCandidate, "candidateGroup"> {
  return {
    bindings: item.bindings.map(projectBinding).sort(stableCompare),
    disposition: item.disposition,
    evidence: sortEvidence(item.evidence), lawId: item.lawId, packId: item.packId,
    packVersion: item.packVersion, relation: projectRelation(item.relation),
    relevance: projectRelevance(item.relevance),
    requirements: item.requirements.map(projectConventionalRequirement).sort(stableCompare), title: item.title,
  };
}

function projectConventionalRequirement(item: ConventionalRequirementInfo): StableConventionalRequirement {
  return item.kind === "condition"
    ? { condition: projectCondition(item.condition), kind: item.kind }
    : { constraint: projectConstraint(item.constraint), evidence: sortEvidence(item.evidence), kind: item.kind, parameter: item.parameter, symbol: item.symbol };
}

function projectInterpretationEvidence(item: MathInterpretationEvidenceInfo): MathInterpretationEvidenceInfo {
  return { evidence: projectOrderedEvidence(item.evidence), provenance: item.provenance, role: item.role, sourceAnchors: item.sourceAnchors.map(projectEvidenceSourceAnchor) };
}

function projectEvidenceReference(
  item: MathInterpretationEvidenceReferenceInfo,
): MathInterpretationEvidenceReferenceInfo {
  return {
    evidence: projectOrderedEvidence(item.evidence),
    sourceAnchors: item.sourceAnchors.map(projectEvidenceSourceAnchor),
  };
}

function projectOrderedEvidenceReference(
  item: MathInterpretationEvidenceReferenceInfo,
): MathInterpretationEvidenceReferenceInfo {
  return {
    evidence: projectOrderedEvidence(item.evidence),
    sourceAnchors: item.sourceAnchors.map(projectEvidenceSourceAnchor),
  };
}

function sortEvidenceReferences(
  items: readonly MathInterpretationEvidenceReferenceInfo[],
): readonly MathInterpretationEvidenceReferenceInfo[] {
  return items.map(projectEvidenceReference).sort(stableCompare);
}

function projectEvidenceSourceAnchor(
  anchor: MathInterpretationEvidenceInfo["sourceAnchors"][number],
): MathInterpretationEvidenceInfo["sourceAnchors"][number] {
  return {
    documentVersion: anchor.documentVersion,
    generation: anchor.generation,
    lifecycle: anchor.lifecycle,
    location: projectLocation(anchor.location),
    scopePath: [...anchor.scopePath],
  };
}

function projectFormula(item: MathFormulaAnchorInfo): MathFormulaAnchorInfo { return { documentVersion: item.documentVersion, location: projectLocation(item.location), ...(item.provenance ? { provenance: item.provenance.map(projectRange).sort(stableCompare) } : {}), scopePath: [...item.scopePath], sourceNotation: item.sourceNotation }; }
function projectRelation(item: RelationInfo): RelationInfo { return { conditions: [...item.conditions].sort(), description: item.description, evidence: sortEvidence(item.evidence), range: projectRange(item.range), relationId: item.relationId, roles: [...item.roles].map((role) => ({ ...role })).sort(stableCompare), title: item.title }; }
function projectBinding(item: LawBinding): LawBinding { return { constraint: projectConstraint(item.constraint), evidence: projectEvidence(item.evidence), parameter: item.parameter, proof: item.proof, symbol: item.symbol }; }
function projectCondition(item: LawConditionInfo): LawConditionInfo { return { conditionId: item.conditionId, evidence: sortEvidence(item.evidence), kind: item.kind, label: item.label, ...(item.operatorProperty ? { operatorProperty: item.operatorProperty } : {}), status: item.status, subjects: [...item.subjects].sort() }; }
function projectConstraint(item: SemanticConstraint): SemanticConstraint { return { ...(item.concepts ? { concepts: [...item.concepts].sort() } : {}), ...(item.dimensions ? { dimensions: [...item.dimensions].sort() } : {}), kind: item.kind, ...(item.refinements ? { refinements: [...item.refinements].sort() } : {}) }; }
function projectRelevance(item: DomainRelevance): DomainRelevance { return { evidence: sortEvidence(item.evidence), support: item.support }; }
function projectEvidence(item: Evidence): Evidence { return { kind: item.kind, ruleId: item.ruleId, sourceRanges: item.sourceRanges.map(projectRange).sort(stableCompare), strength: item.strength }; }
function projectOrderedEvidence(item: Evidence): Evidence { return { kind: item.kind, ruleId: item.ruleId, sourceRanges: item.sourceRanges.map(projectRange), strength: item.strength }; }
function sortEvidence(items: readonly Evidence[]): readonly Evidence[] { return items.map(projectEvidence).sort(stableCompare); }
function projectLocation(item: Location): Location { return { fileId: item.fileId, path: item.path, range: projectRange(item.range) }; }
function projectRange(item: SourceRange): SourceRange { return { endOffset: item.endOffset, startOffset: item.startOffset }; }

function requirementSemanticKey(item: MathInterpretationRequirementInfo): unknown {
  const { requirementId: _id, ...rest } = item;
  if (item.kind === "declaration") return { ...rest, occurrenceId: undefined };
  if (item.kind === "disambiguation") {
    return {
      ...rest,
      alternatives: item.alternatives.map(({ alternativeId: _alternativeId, ...alternative }) => alternative),
    };
  }
  return rest;
}
function notationSemanticKey(item: MathAuthoringContext["notationOccurrences"][number]): unknown { return { location: item.location, scopePath: item.scopePath, sourceNotation: item.sourceNotation }; }
function linkSemanticKey(item: MathAuthoringContext["equationLinks"][number]): unknown { return { evidence: item.evidence, kind: item.kind, source: item.source, target: item.target }; }
function claimSemanticKey(item: MathAuthoringContext["claimEvidence"][number]): unknown { return { claim: item.claim, evidence: item.evidence, modality: item.modality, polarity: item.polarity, strengthCeiling: item.strengthCeiling, supportingFormulas: item.supportingFormulas }; }
function hypothesisKey(
  value: Omit<StableInterpretationHypothesis, "key">,
): StableInterpretationKey {
  return {
    documentVersion: value.documentVersion,
    kind: value.kind,
    label: value.label,
    location: value.location,
    scopePath: value.scopePath,
  };
}

class DenseGroups<T> {
  readonly #groups = new Map<string, number>();
  group(value: T): number { const key = stableJson(value); const found = this.#groups.get(key); if (found !== undefined) return found; const next = this.#groups.size; this.#groups.set(key, next); return next; }
}

// Strict expectation parser. Every nested tagged union and public record has a closed key set.
function parseRange(value: unknown, path: string): void { const item = object(value, path, ["endOffset", "startOffset"]); nonnegative(item.startOffset, `${path}.startOffset`); nonnegative(item.endOffset, `${path}.endOffset`); if ((item.endOffset as number) <= (item.startOffset as number)) throw new Error(`${path}: expected a non-empty ordered range`); }
function parseLocation(value: unknown, path: string): void { const item = object(value, path, ["fileId", "path", "range"]); text(item.fileId, `${path}.fileId`); text(item.path, `${path}.path`); parseRange(item.range, `${path}.range`); }
function parseEvidence(value: unknown, path: string): void { const item = object(value, path, ["kind", "ruleId", "sourceRanges", "strength"]); text(item.kind, `${path}.kind`); text(item.ruleId, `${path}.ruleId`); list(item.sourceRanges, `${path}.sourceRanges`, parseRange); text(item.strength, `${path}.strength`); }
function parseEvidenceSourceAnchor(value: unknown, path: string): void { const item = object(value, path, ["documentVersion", "generation", "lifecycle", "location", "scopePath"]); positive(item.documentVersion, `${path}.documentVersion`); choice(item.generation, ["authored", "generated"], `${path}.generation`); choice(item.lifecycle, ["current", "retracted"], `${path}.lifecycle`); parseLocation(item.location, `${path}.location`); integers(item.scopePath, `${path}.scopePath`); }
function parseEvidenceReference(value: unknown, path: string): void { const item = object(value, path, ["evidence", "sourceAnchors"]); parseEvidence(item.evidence, `${path}.evidence`); list(item.sourceAnchors, `${path}.sourceAnchors`, parseEvidenceSourceAnchor); }
function parseFormula(value: unknown, path: string): void { const item = object(value, path, ["documentVersion", "location", "scopePath", "sourceNotation"], ["provenance"]); positive(item.documentVersion, `${path}.documentVersion`); parseLocation(item.location, `${path}.location`); optionalList(item.provenance, `${path}.provenance`, parseRange); integers(item.scopePath, `${path}.scopePath`); text(item.sourceNotation, `${path}.sourceNotation`); }
function parseConstraint(value: unknown, path: string): void { const item = object(value, path, ["kind"], ["concepts", "dimensions", "refinements"]); choice(item.kind, ["distribution", "event", "expression", "function", "graph", "index", "matrix", "proposition", "random-variable", "scalar", "set", "tensor", "vector"], `${path}.kind`); optionalStrings(item.concepts, `${path}.concepts`); optionalStrings(item.dimensions, `${path}.dimensions`); optionalStrings(item.refinements, `${path}.refinements`); }
function parseBinding(value: unknown, path: string): void { const item = object(value, path, ["constraint", "evidence", "parameter", "proof", "symbol"]); parseConstraint(item.constraint, `${path}.constraint`); parseEvidence(item.evidence, `${path}.evidence`); text(item.parameter, `${path}.parameter`); choice(item.proof, ["typed", "derived", "asserted", "candidate"], `${path}.proof`); text(item.symbol, `${path}.symbol`); }
function parseCondition(value: unknown, path: string): void { const item = object(value, path, ["conditionId", "evidence", "kind", "label", "status", "subjects"], ["operatorProperty"]); text(item.conditionId, `${path}.conditionId`); list(item.evidence, `${path}.evidence`, parseEvidence); choice(item.kind, ["assumption", "differentiable", "domain-membership", "maps-between", "nonzero", "operator-property", "positive", "rank-compatible", "same-context", "shape-compatible", "sign-convention", "uniform"], `${path}.kind`); text(item.label, `${path}.label`); optionalChoice(item.operatorProperty, ["adjoint", "bilinear", "gradient", "hessian", "inner-product", "jacobian", "linear", "norm"], `${path}.operatorProperty`); choice(item.status, ["conflicting", "required", "unsupported", "verified"], `${path}.status`); strings(item.subjects, `${path}.subjects`); }
function parseRelation(value: unknown, path: string): void { const item = object(value, path, ["conditions", "description", "evidence", "range", "relationId", "roles", "title"]); strings(item.conditions, `${path}.conditions`); text(item.description, `${path}.description`); list(item.evidence, `${path}.evidence`, parseEvidence); parseRange(item.range, `${path}.range`); text(item.relationId, `${path}.relationId`); list(item.roles, `${path}.roles`, (role, rolePath) => { const entry = object(role, rolePath, ["label", "role", "symbol"], ["conceptId"]); optionalText(entry.conceptId, `${rolePath}.conceptId`); text(entry.label, `${rolePath}.label`); text(entry.role, `${rolePath}.role`); text(entry.symbol, `${rolePath}.symbol`); }); text(item.title, `${path}.title`); }
function parseRelevance(value: unknown, path: string): void { const item = object(value, path, ["evidence", "support"]); list(item.evidence, `${path}.evidence`, parseEvidence); choice(item.support, ["explicit", "supported", "tentative"], `${path}.support`); }
function parseInterpretationRelevance(value: unknown, path: string): void { const item = object(value, path, ["evidence", "support"]); list(item.evidence, `${path}.evidence`, parseEvidenceReference); choice(item.support, ["explicit", "supported", "tentative"], `${path}.support`); }
function parseInterpretationCondition(value: unknown, path: string): void { const item = object(value, path, ["conditionId", "evidence", "kind", "label", "status", "subjects"], ["operatorProperty"]); text(item.conditionId, `${path}.conditionId`); list(item.evidence, `${path}.evidence`, parseEvidenceReference); choice(item.kind, ["assumption", "differentiable", "domain-membership", "maps-between", "nonzero", "operator-property", "positive", "rank-compatible", "same-context", "shape-compatible", "sign-convention", "uniform"], `${path}.kind`); text(item.label, `${path}.label`); optionalChoice(item.operatorProperty, ["adjoint", "bilinear", "gradient", "hessian", "inner-product", "jacobian", "linear", "norm"], `${path}.operatorProperty`); choice(item.status, ["conflicting", "required", "unsupported", "verified"], `${path}.status`); strings(item.subjects, `${path}.subjects`); }
function parseAlternative(value: unknown, path: string): void { const item = object(value, path, ["alternativeGroup", "evidence", "label", "range"], ["relevance"]); nonnegative(item.alternativeGroup, `${path}.alternativeGroup`); list(item.evidence, `${path}.evidence`, parseEvidenceReference); text(item.label, `${path}.label`); parseRange(item.range, `${path}.range`); optional(item.relevance, `${path}.relevance`, parseInterpretationRelevance); }
function parseRequirement(value: unknown, path: string): void { const base = record(value, path); const kind = choice(base.kind, ["declaration", "role-declaration", "condition", "disambiguation"], `${path}.kind`); if (kind === "declaration") { const item = object(value, path, ["evidence", "group", "kind", "occurrenceDocumentVersion", "occurrenceFileId", "occurrenceGroup", "symbol"]); list(item.evidence, `${path}.evidence`, parseEvidenceReference); positive(item.occurrenceDocumentVersion, `${path}.occurrenceDocumentVersion`); text(item.occurrenceFileId, `${path}.occurrenceFileId`); nonnegative(item.occurrenceGroup, `${path}.occurrenceGroup`); text(item.symbol, `${path}.symbol`); nonnegative(item.group, `${path}.group`); } else if (kind === "role-declaration") { const item = object(value, path, ["constraint", "evidence", "group", "kind", "parameter", "symbol"]); parseConstraint(item.constraint, `${path}.constraint`); list(item.evidence, `${path}.evidence`, parseEvidenceReference); nonnegative(item.group, `${path}.group`); text(item.parameter, `${path}.parameter`); text(item.symbol, `${path}.symbol`); } else if (kind === "condition") { const item = object(value, path, ["condition", "group", "kind"]); parseInterpretationCondition(item.condition, `${path}.condition`); nonnegative(item.group, `${path}.group`); } else { const item = object(value, path, ["alternatives", "evidence", "group", "kind"]); list(item.alternatives, `${path}.alternatives`, parseAlternative); list(item.evidence, `${path}.evidence`, parseEvidenceReference); nonnegative(item.group, `${path}.group`); } }
function parseConventionalRequirement(value: unknown, path: string): void { const base = record(value, path); const kind = choice(base.kind, ["role-declaration", "condition"], `${path}.kind`); if (kind === "condition") { const item = object(value, path, ["condition", "kind"]); parseCondition(item.condition, `${path}.condition`); } else { const item = object(value, path, ["constraint", "evidence", "kind", "parameter", "symbol"]); parseConstraint(item.constraint, `${path}.constraint`); list(item.evidence, `${path}.evidence`, parseEvidence); text(item.parameter, `${path}.parameter`); text(item.symbol, `${path}.symbol`); } }
function parseCandidate(value: unknown, path: string): void { const item = object(value, path, ["bindings", "candidateGroup", "disposition", "evidence", "lawId", "packId", "packVersion", "relation", "relevance", "requirements", "title"]); list(item.bindings, `${path}.bindings`, parseBinding); nonnegative(item.candidateGroup, `${path}.candidateGroup`); choice(item.disposition, ["conventional-candidate"], `${path}.disposition`); list(item.evidence, `${path}.evidence`, parseEvidence); text(item.lawId, `${path}.lawId`); text(item.packId, `${path}.packId`); text(item.packVersion, `${path}.packVersion`); parseRelation(item.relation, `${path}.relation`); parseRelevance(item.relevance, `${path}.relevance`); list(item.requirements, `${path}.requirements`, parseConventionalRequirement); text(item.title, `${path}.title`); }
function parseLink(value: unknown, path: string): void { const item = object(value, path, ["evidence", "kind", "linkGroup", "sharedEntityGroups", "source", "target"]); list(item.evidence, `${path}.evidence`, parseEvidence); choice(item.kind, ["derived-law", "shared-entity"], `${path}.kind`); nonnegative(item.linkGroup, `${path}.linkGroup`); integers(item.sharedEntityGroups, `${path}.sharedEntityGroups`); parseFormula(item.source, `${path}.source`); parseFormula(item.target, `${path}.target`); }
function parseApproximation(value: unknown, path: string): void { const item = object(value, path, ["evidence", "exactness", "relationRange"], ["relatedFactGroups"]); list(item.evidence, `${path}.evidence`, parseEvidence); choice(item.exactness, ["approximate"], `${path}.exactness`); optionalIntegers(item.relatedFactGroups, `${path}.relatedFactGroups`); parseRange(item.relationRange, `${path}.relationRange`); }
function parseClaim(value: unknown, path: string): void { const item = object(value, path, ["claim", "claimGroup", "evidence", "modality", "polarity", "strengthCeiling", "supportingClaimGroups", "supportingFormulas"]); parseLocation(item.claim, `${path}.claim`); nonnegative(item.claimGroup, `${path}.claimGroup`); list(item.evidence, `${path}.evidence`, parseEvidence); choice(item.modality, ["asserted", "cited", "hedged", "hypothetical", "quoted"], `${path}.modality`); choice(item.polarity, ["negative", "positive"], `${path}.polarity`); choice(item.strengthCeiling, ["asserted", "qualified", "unusable"], `${path}.strengthCeiling`); integers(item.supportingClaimGroups, `${path}.supportingClaimGroups`); list(item.supportingFormulas, `${path}.supportingFormulas`, parseFormula); }
function parseLifecycle(value: unknown, path: string): void { const item = object(value, path, ["capped", "documentVersion", "editable", "engineLimited", "freshness", "generation", "retracted"]); bool(item.capped, `${path}.capped`); positive(item.documentVersion, `${path}.documentVersion`); bool(item.editable, `${path}.editable`); bool(item.engineLimited, `${path}.engineLimited`); choice(item.freshness, ["current"], `${path}.freshness`); choice(item.generation, ["authored", "generated"], `${path}.generation`); bool(item.retracted, `${path}.retracted`); }
function parseNotation(value: unknown, path: string): void { const item = object(value, path, ["entityAnchorDocumentVersion", "entityAnchorFileId", "entityAnchorOccurrenceGroup", "entityGroup", "entityKind", "entityScopePath", "location", "occurrenceDocumentVersion", "occurrenceFileId", "occurrenceGroup", "scopePath", "sourceNotation"]); positive(item.entityAnchorDocumentVersion, `${path}.entityAnchorDocumentVersion`); text(item.entityAnchorFileId, `${path}.entityAnchorFileId`); nonnegative(item.entityAnchorOccurrenceGroup, `${path}.entityAnchorOccurrenceGroup`); nonnegative(item.entityGroup, `${path}.entityGroup`); text(item.entityKind, `${path}.entityKind`); integers(item.entityScopePath, `${path}.entityScopePath`); parseLocation(item.location, `${path}.location`); positive(item.occurrenceDocumentVersion, `${path}.occurrenceDocumentVersion`); text(item.occurrenceFileId, `${path}.occurrenceFileId`); nonnegative(item.occurrenceGroup, `${path}.occurrenceGroup`); integers(item.scopePath, `${path}.scopePath`); text(item.sourceNotation, `${path}.sourceNotation`); }
function parseInterpretationEvidence(value: unknown, path: string): void { const item = object(value, path, ["evidence", "provenance", "role", "sourceAnchors"]); parseEvidence(item.evidence, `${path}.evidence`); choice(item.provenance, ["explicit-declaration", "typed-structure", "natural-language-extraction", "domain-context", "reviewed-convention", "derived-evidence"], `${path}.provenance`); choice(item.role, ["supporting", "contradicting"], `${path}.role`); list(item.sourceAnchors, `${path}.sourceAnchors`, parseEvidenceSourceAnchor); }
function parseHypothesis(value: unknown, path: string): void { const item = object(value, path, ["bindings", "conditions", "documentVersion", "evidence", "hypothesisGroup", "key", "kind", "label", "location", "missingDiscriminatorGroups", "orderingReasons", "range", "rank", "scopePath", "support"], ["formula", "relation"]); list(item.bindings, `${path}.bindings`, parseBinding); list(item.conditions, `${path}.conditions`, parseCondition); positive(item.documentVersion, `${path}.documentVersion`); list(item.evidence, `${path}.evidence`, parseInterpretationEvidence); optional(item.formula, `${path}.formula`, parseFormula); nonnegative(item.hypothesisGroup, `${path}.hypothesisGroup`); parseHypothesisKey(item.key, `${path}.key`); choice(item.kind, ["source-meaning", "typed-law", "scoped-domain", "structural-alternative", "reviewed-convention"], `${path}.kind`); text(item.label, `${path}.label`); parseLocation(item.location, `${path}.location`); integers(item.missingDiscriminatorGroups, `${path}.missingDiscriminatorGroups`); list(item.orderingReasons, `${path}.orderingReasons`, (reason, reasonPath) => { const entry = object(reason, reasonPath, ["evidence", "kind"]); list(entry.evidence, `${reasonPath}.evidence`, parseEvidenceReference); choice(entry.kind, ["explicit-evidence", "typed-evidence", "derived-evidence", "domain-relevance", "reviewed-convention", "stable-source-order"], `${reasonPath}.kind`); }); parseRange(item.range, `${path}.range`); nonnegative(item.rank, `${path}.rank`); optional(item.relation, `${path}.relation`, parseRelation); integers(item.scopePath, `${path}.scopePath`); choice(item.support, ["explicit", "derived", "supported", "tentative", "contradicted"], `${path}.support`); }
function parseHypothesisKey(value: unknown, path: string): void { const item = object(value, path, ["documentVersion", "kind", "label", "location", "scopePath"]); positive(item.documentVersion, `${path}.documentVersion`); choice(item.kind, ["source-meaning", "typed-law", "scoped-domain", "structural-alternative", "reviewed-convention"], `${path}.kind`); text(item.label, `${path}.label`); parseLocation(item.location, `${path}.location`); integers(item.scopePath, `${path}.scopePath`); }
function parseInterpretations(value: unknown, path: string): void { const item = object(value, path, ["analysisLimits", "exhaustiveness", "hypotheses", "missingDiscriminators", "truncated"], ["candidateCap"]); list(item.analysisLimits, `${path}.analysisLimits`, (limit, limitPath) => { const entry = object(limit, limitPath, ["evidence", "kind"]); list(entry.evidence, `${limitPath}.evidence`, parseEvidenceReference); choice(entry.kind, ["candidate-set-capped", "evidence-truncated", "discriminator-set-capped", "engine-limit", "generated-source", "retracted-source"], `${limitPath}.kind`); }); if (item.candidateCap !== undefined) parseMathInterpretationCandidateCapInfo(item.candidateCap); choice(item.exhaustiveness, ["bounded-open-world"], `${path}.exhaustiveness`); list(item.hypotheses, `${path}.hypotheses`, parseHypothesis); list(item.missingDiscriminators, `${path}.missingDiscriminators`, parseRequirement); bool(item.truncated, `${path}.truncated`); }

function validateStableExpectation(
  expected: StableMathAuthoringContext,
  path: string,
): void {
  validateDenseOrdinals(
    expected.conventionalCandidates?.map((item) => item.candidateGroup) ?? [],
    `${path}.conventionalCandidates.candidateGroup`,
  );
  validateDenseOrdinals(
    expected.equationLinks.map((item) => item.linkGroup),
    `${path}.equationLinks.linkGroup`,
  );
  validateDenseOrdinals(
    expected.claimEvidence.flatMap((item) => [
      item.claimGroup,
      ...item.supportingClaimGroups,
    ]),
    `${path}.claimEvidence.claimGroup`,
  );
  validateDenseOrdinals(
    [
      ...expected.notationOccurrences.map((item) => item.entityGroup),
      ...expected.equationLinks.flatMap((item) => item.sharedEntityGroups),
    ],
    `${path}.notationOccurrences.entityGroup`,
  );
  validateDenseOrdinals(
    expected.approximation?.relatedFactGroups ?? [],
    `${path}.approximation.relatedFactGroups`,
  );

  const requirements = [
    ...expected.requirements,
    ...expected.interpretations.missingDiscriminators,
  ];
  validateDenseOrdinals(
    [
      ...expected.notationOccurrences.map((item) => item.occurrenceGroup),
      ...expected.notationOccurrences.map(
        (item) => item.entityAnchorOccurrenceGroup,
      ),
      ...requirements.flatMap((item) =>
        item.kind === "declaration" ? [item.occurrenceGroup] : []
      ),
    ],
    `${path}.notationOccurrences.occurrenceGroup`,
  );
  validateDenseOrdinals(
    requirements.map((item) => item.group),
    `${path}.requirements.group`,
  );
  const requirementByGroup = new Map<number, string>();
  for (const requirement of requirements) {
    const semantic = stableJson({ ...requirement, group: undefined });
    const known = requirementByGroup.get(requirement.group);
    if (known !== undefined && known !== semantic) {
      throw new Error(
        `${path}.requirements.group: group ${requirement.group} has conflicting semantics`,
      );
    }
    requirementByGroup.set(requirement.group, semantic);
  }
  const missingGroups = new Set(
    expected.interpretations.missingDiscriminators.map((item) => item.group),
  );
  for (const [index, occurrence] of expected.notationOccurrences.entries()) {
    if (occurrence.occurrenceFileId !== occurrence.location.fileId) {
      throw new Error(
        `${path}.notationOccurrences[${index}].occurrenceFileId: does not match location file`,
      );
    }
  }
  for (const requirement of requirements) {
    if (requirement.kind === "disambiguation") {
      validateDenseOrdinals(
        requirement.alternatives.map((item) => item.alternativeGroup),
        `${path}.requirement[${requirement.group}].alternatives.alternativeGroup`,
      );
    }
    validateRequirementEvidenceReferences(
      requirement,
      `${path}.requirement[${requirement.group}]`,
    );
  }

  for (const [index, hypothesis] of expected.interpretations.hypotheses.entries()) {
    const hypothesisPath = `${path}.interpretations.hypotheses[${index}]`;
    if (hypothesis.hypothesisGroup !== index) {
      throw new Error(`${hypothesisPath}.hypothesisGroup: expected ${index}`);
    }
    if (hypothesis.rank !== index) {
      throw new Error(`${hypothesisPath}.rank: expected ${index}`);
    }
    const { key: _key, ...withoutKey } = hypothesis;
    if (stableJson(hypothesis.key) !== stableJson(hypothesisKey(withoutKey))) {
      throw new Error(`${hypothesisPath}.key: does not match typed canonical key`);
    }
    if (!sameRange(hypothesis.range, hypothesis.location.range)) {
      throw new Error(`${hypothesisPath}.range: does not match location range`);
    }
    for (const group of hypothesis.missingDiscriminatorGroups) {
      if (!missingGroups.has(group)) {
        throw new Error(
          `${hypothesisPath}.missingDiscriminatorGroups: unknown group ${group}`,
        );
      }
    }
    const forbiddenAuthority = hypothesis.kind === "reviewed-convention" ||
      hypothesis.kind === "scoped-domain" ||
      hypothesis.kind === "structural-alternative";
    if (forbiddenAuthority &&
      (hypothesis.support === "explicit" || hypothesis.support === "derived")) {
      throw new Error(`${hypothesisPath}.support: unsafe authority escalation`);
    }
    if (hypothesis.support === "contradicted" &&
      !hypothesis.evidence.some((item) => item.role === "contradicting")) {
      throw new Error(`${hypothesisPath}.support: contradiction lacks evidence`);
    }
    for (const [evidenceIndex, evidence] of hypothesis.evidence.entries()) {
      const evidencePath = `${hypothesisPath}.evidence[${evidenceIndex}]`;
      for (const [rangeIndex, range] of evidence.evidence.sourceRanges.entries()) {
        if (!evidence.sourceAnchors.some((anchor) =>
          sameRange(anchor.location.range, range)
        )) {
          throw new Error(`${evidencePath}.sourceAnchors: missing range ${rangeIndex}`);
        }
      }
      for (const [anchorIndex, anchor] of evidence.sourceAnchors.entries()) {
        if (anchor.lifecycle === "retracted" && evidence.role === "supporting") {
          throw new Error(`${evidencePath}.sourceAnchors[${anchorIndex}]: unsafe lifecycle`);
        }
      }
    }
    hypothesis.orderingReasons.forEach((reason, reasonIndex) =>
      validateEvidenceReferences(
        reason.evidence,
        `${hypothesisPath}.orderingReasons[${reasonIndex}].evidence`,
      )
    );
  }
  expected.interpretations.analysisLimits.forEach((limit, limitIndex) =>
    validateEvidenceReferences(
      limit.evidence,
      `${path}.interpretations.analysisLimits[${limitIndex}].evidence`,
    )
  );

  if ((expected.lifecycle.retracted || expected.lifecycle.generation === "generated") &&
    expected.lifecycle.editable) {
    throw new Error(`${path}.lifecycle.editable: unsafe lifecycle`);
  }
  const hasEngineLimit = expected.interpretations.analysisLimits.some(
    (limit) => limit.kind === "engine-limit",
  );
  if (expected.lifecycle.engineLimited !== hasEngineLimit) {
    throw new Error(`${path}.lifecycle.engineLimited: inconsistent analysis limit`);
  }
}

function validateRequirementEvidenceReferences(
  requirement: StableRequirement,
  path: string,
): void {
  if (requirement.kind === "condition") {
    validateEvidenceReferences(requirement.condition.evidence, `${path}.condition.evidence`);
    return;
  }
  validateEvidenceReferences(requirement.evidence, `${path}.evidence`);
  if (requirement.kind === "disambiguation") {
    requirement.alternatives.forEach((alternative, index) => {
      const alternativePath = `${path}.alternatives[${index}]`;
      validateEvidenceReferences(alternative.evidence, `${alternativePath}.evidence`);
      if (alternative.relevance) {
        validateEvidenceReferences(
          alternative.relevance.evidence,
          `${alternativePath}.relevance.evidence`,
        );
      }
    });
  }
}

function validateEvidenceReferences(
  references: readonly MathInterpretationEvidenceReferenceInfo[],
  path: string,
): void {
  references.forEach((reference, referenceIndex) => {
    const referencePath = `${path}[${referenceIndex}]`;
    reference.evidence.sourceRanges.forEach((range, rangeIndex) => {
      if (!reference.sourceAnchors.some((anchor) =>
        sameRange(anchor.location.range, range)
      )) {
        throw new Error(`${referencePath}.sourceAnchors: missing range ${rangeIndex}`);
      }
    });
  });
}

function validateDenseOrdinals(values: readonly number[], path: string): void {
  const distinct = [...new Set(values)].sort(numeric);
  for (const [index, value] of distinct.entries()) {
    if (value !== index) throw new Error(`${path}: expected dense ordinal ${index}, got ${value}`);
  }
}

function validateAuthority(hypothesis: MathInterpretationHypothesisInfo, path: string, findings: MathAuthoringContextFailure[]): void { const forbiddenKind = hypothesis.kind === "reviewed-convention" || hypothesis.kind === "scoped-domain" || hypothesis.kind === "structural-alternative"; if (forbiddenKind && (hypothesis.support === "explicit" || hypothesis.support === "derived")) findings.push({ actual: hypothesis.support, kind: "authority-escalation", path: `${path}.support` }); }
function validateEvidenceAnchors(evidence: MathInterpretationEvidenceInfo, path: string, findings: MathAuthoringContextFailure[]): void { for (const [rangeIndex, range] of evidence.evidence.sourceRanges.entries()) { if (!evidence.sourceAnchors.some((anchor) => sameRange(anchor.location.range, range))) findings.push({ expected: range, kind: "wrong-anchor", path: `${path}.sourceAnchors.range[${rangeIndex}]` }); } for (const [index, anchor] of evidence.sourceAnchors.entries()) { if (!validLocation(anchor.location, anchor.documentVersion, anchor.scopePath)) findings.push({ actual: anchor, kind: "wrong-anchor", path: `${path}.sourceAnchors[${index}]` }); if (anchor.lifecycle === "retracted" && evidence.role === "supporting") findings.push({ actual: "retracted", kind: "unsafe-lifecycle", path: `${path}.sourceAnchors[${index}].lifecycle` }); } }
function validLocation(location: Location, version: number, scope: readonly number[]): boolean { return location.fileId.length > 0 && location.path.length > 0 && validRange(location.range) && Number.isInteger(version) && version > 0 && scope.every((part) => Number.isInteger(part) && part >= 0); }
function validRange(range: SourceRange): boolean { return Number.isInteger(range.startOffset) && Number.isInteger(range.endOffset) && range.startOffset >= 0 && range.startOffset < range.endOffset; }
function sameRange(left: SourceRange, right: SourceRange): boolean { return left.startOffset === right.startOffset && left.endOffset === right.endOffset; }

function compareJson(expected: unknown, actual: unknown, path: string, findings: MathAuthoringContextFailure[]): void { if (Object.is(expected, actual)) return; if (Array.isArray(expected) && Array.isArray(actual)) { for (let index = 0; index < Math.max(expected.length, actual.length); index += 1) { const child = `${path}[${index}]`; if (index >= expected.length) findings.push({ actual: actual[index], kind: anchorPath(child) ? "wrong-anchor" : "unexpected", path: child }); else if (index >= actual.length) findings.push({ expected: expected[index], kind: anchorPath(child) ? "wrong-anchor" : "missing", path: child }); else compareJson(expected[index], actual[index], child, findings); } return; } if (isRecord(expected) && isRecord(actual)) { const keys = new Set([...Object.keys(expected), ...Object.keys(actual)]); for (const key of [...keys].sort()) { const child = `${path}.${key}`; if (!(key in expected)) findings.push({ actual: actual[key], kind: "unexpected", path: child }); else if (!(key in actual)) findings.push({ expected: expected[key], kind: "missing", path: child }); else compareJson(expected[key], actual[key], child, findings); } return; } findings.push({ actual, expected, kind: mismatchKind(path, expected, actual), path }); }
function mismatchKind(path: string, expected: unknown, actual: unknown): MathAuthoringFailureKind { if (anchorPath(path)) return "wrong-anchor"; if (path.includes(".lifecycle.")) return "unsafe-lifecycle"; if (path.endsWith(".support") && typeof expected === "string" && typeof actual === "string" && supportAuthority(actual) > supportAuthority(expected)) return "authority-escalation"; if ((path.endsWith(".disposition") && actual === "conflicting" && expected !== "conflicting") || (path.endsWith(".support") && actual === "contradicted" && expected !== "contradicted") || (path.endsWith(".role") && actual === "contradicting" && expected !== "contradicting")) return "false-conflict"; return "mismatch"; }
function anchorPath(path: string): boolean { return /(?:\.location|\.range|\.documentVersion|\.scopePath|\.sourceAnchors|\.formula|\.source|\.target|\.claim|\.supportingFormulas|\.occurrenceFileId|\.occurrenceDocumentVersion|\.entityAnchorFileId|\.entityAnchorDocumentVersion|\.entityAnchorOccurrenceGroup)(?:\.|\[|$)/u.test(path); }
function supportAuthority(value: string): number { return ({ contradicted: 0, tentative: 1, supported: 2, derived: 3, explicit: 4 } as Record<string, number>)[value] ?? -1; }
function deduplicate(findings: readonly MathAuthoringContextFailure[]): readonly MathAuthoringContextFailure[] { const seen = new Set<string>(); return findings.filter((finding) => { const key = stableJson(finding); if (seen.has(key)) return false; seen.add(key); return true; }); }
function formatFailure(failure: MathAuthoringContextFailure): string { const values = "expected" in failure || "actual" in failure ? `; expected ${stableJson(failure.expected)}, actual ${stableJson(failure.actual)}` : ""; return `${failure.path}: ${failure.kind}${values}`; }

function object(value: unknown, path: string, required: readonly string[], optionalKeys: readonly string[] = []): Readonly<Record<string, unknown>> { const item = record(value, path); const allowed = new Set([...required, ...optionalKeys]); const unexpected = Object.keys(item).filter((key) => !allowed.has(key)); if (unexpected.length) throw new Error(`${path}: unexpected keys ${unexpected.sort().join(", ")}`); const missing = required.filter((key) => !(key in item)); if (missing.length) throw new Error(`${path}: missing keys ${missing.join(", ")}`); return item; }
function record(value: unknown, path: string): Readonly<Record<string, unknown>> { if (!isRecord(value)) throw new Error(`${path}: expected object`); return value; }
function isRecord(value: unknown): value is Readonly<Record<string, unknown>> { return typeof value === "object" && value !== null && !Array.isArray(value); }
function array(value: unknown, path: string): readonly unknown[] { if (!Array.isArray(value)) throw new Error(`${path}: expected array`); return value; }
function list(value: unknown, path: string, parse: (value: unknown, path: string) => void): void { array(value, path).forEach((item, index) => parse(item, `${path}[${index}]`)); }
function optional(value: unknown, path: string, parse: (value: unknown, path: string) => void): void { if (value !== undefined) parse(value, path); }
function optionalList(value: unknown, path: string, parse: (value: unknown, path: string) => void): void { if (value !== undefined) list(value, path, parse); }
function text(value: unknown, path: string): string { if (typeof value !== "string" || value.length === 0) throw new Error(`${path}: expected non-empty string`); return value; }
function optionalText(value: unknown, path: string): void { if (value !== undefined) text(value, path); }
function bool(value: unknown, path: string): void { if (typeof value !== "boolean") throw new Error(`${path}: expected boolean`); }
function nonnegative(value: unknown, path: string): void { if (typeof value !== "number" || !Number.isInteger(value) || value < 0) throw new Error(`${path}: expected nonnegative integer`); }
function positive(value: unknown, path: string): void { if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) throw new Error(`${path}: expected positive integer`); }
function integers(value: unknown, path: string): void { array(value, path).forEach((item, index) => nonnegative(item, `${path}[${index}]`)); }
function optionalIntegers(value: unknown, path: string): void { if (value !== undefined) integers(value, path); }
function strings(value: unknown, path: string): void { array(value, path).forEach((item, index) => text(item, `${path}[${index}]`)); }
function optionalStrings(value: unknown, path: string): void { if (value !== undefined) strings(value, path); }
function choice<const T extends readonly string[]>(value: unknown, options: T, path: string): T[number] { if (typeof value !== "string" || !options.includes(value)) throw new Error(`${path}: expected ${options.join(" or ")}`); return value; }
function optionalChoice<const T extends readonly string[]>(value: unknown, options: T, path: string): void { if (value !== undefined) choice(value, options, path); }
function unique(values: readonly string[], path: string): void { const seen = new Set<string>(); for (const value of values) { if (seen.has(value)) throw new Error(`${path}: duplicate ${value}`); seen.add(value); } }
function numeric(left: number, right: number): number { return left - right; }
function stableCompare(left: unknown, right: unknown): number { return stableJson(left).localeCompare(stableJson(right)); }
function stableJson(value: unknown): string { if (value === undefined) return "undefined"; return JSON.stringify(sortJson(value)); }
function sortJson(value: unknown): unknown { if (Array.isArray(value)) return value.map(sortJson); if (!isRecord(value)) return value; return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined).sort(([left], [right]) => left.localeCompare(right)).map(([key, item]) => [key, sortJson(item)])); }
