import type {
  AuthoredFalseEstablishmentCause,
  AuthoredSourceAnchor,
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
  ObservedLocation,
} from "./authored-scientific";
import {
  authoredFalseEstablishmentCases,
  authoredProbeIdentityFailures,
  authoredScenarioFor,
  authoredSnapshotFor,
  resolveAuthoredAnchor,
  scoreAuthoredScientificFixture,
} from "./authored-scientific";
import {
  observeSelectedFormulaDecision,
  selectedFormulaMeaningIsEstablishmentGrade,
} from "./challenge-observation";

export interface ApprovedSourceGroundedNavigationRecovery {
  readonly caseId: string;
  readonly definition: AuthoredSourceAnchor;
  readonly references: readonly AuthoredSourceAnchor[];
  readonly symbol: string;
  readonly symbolOccurrence: AuthoredSourceAnchor;
}

export interface ApprovedFalseEstablishment {
  readonly caseId: string;
  readonly causes: readonly AuthoredFalseEstablishmentCause[];
}

export interface AuthoredHistoricalReleaseBaseline {
  readonly approvedConservativeDecisionIds: readonly string[];
  readonly approvedCursorBoundaryIdentityIds: readonly string[];
  readonly approvedFalseEstablishments: readonly ApprovedFalseEstablishment[];
  readonly approvedFormulaDecisionDomainIds: readonly string[];
  readonly approvedSourceGroundedNavigationRecoveries: readonly ApprovedSourceGroundedNavigationRecovery[];
  readonly cases: number;
  readonly maximumMissedCoverage: number;
  readonly maximumNavigationOrIdentity: number;
  readonly maximumRisk: number;
  readonly minimumPassed: number;
}

export function authoredHistoricalReleaseRegressions(
  fixture: AuthoredScientificFixture,
  observations: readonly AuthoredScientificObservation[],
  score: AuthoredScientificScorecard,
  baseline: AuthoredHistoricalReleaseBaseline,
): readonly string[] {
  const regressions: string[] = [];
  const falseEstablishmentCases = authoredFalseEstablishmentCases(
    fixture,
    observations,
  );
  const falseEstablishmentIds = falseEstablishmentCases
    .map((item) => item.caseId)
    .sort();
  const approved = new Map<string, readonly AuthoredFalseEstablishmentCause[]>();
  for (const item of baseline.approvedFalseEstablishments) {
    if (approved.has(item.caseId)) {
      regressions.push(`duplicate false establishment adjudication ${item.caseId}`);
      continue;
    }
    approved.set(item.caseId, item.causes);
  }
  for (const falseEstablishment of falseEstablishmentCases) {
    const caseId = falseEstablishment.caseId;
    const approvedCauses = approved.get(caseId);
    if (!approvedCauses) {
      regressions.push(`unreviewed false establishment ${caseId}`);
    } else if (!sameCauseSet(falseEstablishment.causes, approvedCauses)) {
      regressions.push(
        `false establishment ${caseId} causes ${[...falseEstablishment.causes].sort().join(",")} differ from approved ${[...approvedCauses].sort().join(",")}`,
      );
    }
    if (!falseEstablishment.sourceGrounded) {
      regressions.push(`false establishment ${caseId} is not source grounded`);
    }
  }
  if (score.risk.falseEstablishment !== falseEstablishmentIds.length) {
    regressions.push(
      `false establishment count ${score.risk.falseEstablishment} does not match observations ${falseEstablishmentIds.length}`,
    );
  }
  if (score.cases !== baseline.cases) {
    regressions.push(
      `case count ${score.cases} differs from ${baseline.cases}`,
    );
  }
  const approvedBoundaryIdentity = new Set(
    baseline.approvedCursorBoundaryIdentityIds,
  );
  const adjudicatedBoundaryIdentityIds = fixture.probes
    .filter((probe) => approvedBoundaryIdentity.has(probe.id))
    .filter((probe) => {
      const observation = observations.find((item) => item.caseId === probe.id);
      if (!observation) return false;
      const failures = authoredProbeIdentityFailures(fixture, probe, observation);
      return (
        probe.cursor.edge === "after" &&
        probe.expected.decision === "unsupported" &&
        probe.expected.symbol !== undefined &&
        observation.symbol === null &&
        failures.length === 1 &&
        failures[0]?.area === "cursor-symbol"
      );
    })
    .map((probe) => probe.id);
  for (const caseId of approvedBoundaryIdentity) {
    if (!adjudicatedBoundaryIdentityIds.includes(caseId)) {
      regressions.push(`invalid cursor-boundary identity adjudication ${caseId}`);
    }
  }
  const approvedFormulaDecisionDomain = new Set(
    baseline.approvedFormulaDecisionDomainIds,
  );
  if (
    approvedFormulaDecisionDomain.size !==
    baseline.approvedFormulaDecisionDomainIds.length
  ) {
    regressions.push("duplicate formula-decision-domain adjudication");
  }
  const adjudicatedFormulaDecisionDomainIds = fixture.probes
    .filter((probe) => approvedFormulaDecisionDomain.has(probe.id))
    .filter((probe) => {
      const observation = observations.find((item) => item.caseId === probe.id);
      return (
        observation !== undefined &&
        legacyFormulaDecisionIsEstablished(fixture, probe, observation)
      );
    })
    .map((probe) => probe.id);
  for (const caseId of approvedFormulaDecisionDomain) {
    if (!adjudicatedFormulaDecisionDomainIds.includes(caseId)) {
      regressions.push(
        `invalid formula-decision-domain adjudication ${caseId}`,
      );
    }
  }
  const adjudicatedSourceGroundedNavigationIds =
    baseline.approvedSourceGroundedNavigationRecoveries
      .filter(
        (recovery, index, recoveries) =>
          recoveries.findIndex((item) => item.caseId === recovery.caseId) ===
          index,
      )
      .filter((approvedRecovery) => {
        const probe = fixture.probes.find(
          (item) => item.id === approvedRecovery.caseId,
        );
        const observation = observations.find(
          (item) => item.caseId === approvedRecovery.caseId,
        );
        if (!probe || !observation) return false;
        const snapshot = authoredSnapshotFor(
          authoredScenarioFor(fixture, probe),
          probe,
        );
        const definition = resolveAuthoredAnchor(
          snapshot,
          approvedRecovery.definition,
        );
        const references = approvedRecovery.references.map((anchor) =>
          resolveAuthoredAnchor(snapshot, anchor),
        );
        const symbolOccurrence = resolveAuthoredAnchor(
          snapshot,
          approvedRecovery.symbolOccurrence,
        );
        const failures = authoredProbeIdentityFailures(
          fixture,
          probe,
          observation,
        );
        return (
          probe.expected.decision === "established" &&
          probe.expected.proofGrounded &&
          probe.expected.navigation.definition.status === "unavailable" &&
          probe.expected.navigation.references.status === "unavailable" &&
          probe.expected.navigation.prepareRename.status === "unavailable" &&
          probe.expected.navigation.rename.status === "unavailable" &&
          observation.decision === "established" &&
          observation.proofGrounded &&
          observation.symbol === approvedRecovery.symbol &&
          sameLocations(observation.definitions, [definition]) &&
          sameLocations(observation.references, references) &&
          sameLocation(observation.symbolLocation, symbolOccurrence) &&
          observation.prepareRename.placeholder === approvedRecovery.symbol &&
          observation.prepareRename.range !== undefined &&
          sameRange(observation.prepareRename.range, symbolOccurrence.range) &&
          observation.renameEdits.length === 0 &&
          failures.length === 3 &&
          ["definition", "prepare-rename", "references"].every((area) =>
            failures.some((failure) => failure.area === area),
          )
        );
      })
      .map((recovery) => recovery.caseId);
  for (const recovery of baseline.approvedSourceGroundedNavigationRecoveries) {
    if (
      baseline.approvedSourceGroundedNavigationRecoveries.filter(
        (item) => item.caseId === recovery.caseId,
      ).length !== 1
    ) {
      regressions.push(
        `duplicate source-grounded navigation adjudication ${recovery.caseId}`,
      );
      continue;
    }
    if (!adjudicatedSourceGroundedNavigationIds.includes(recovery.caseId)) {
      regressions.push(
        `invalid source-grounded navigation adjudication ${recovery.caseId}`,
      );
    }
  }
  const reviewedScore = applyReviewedScoreAdjustments(
    fixture,
    observations,
    score,
    adjudicatedFormulaDecisionDomainIds,
    adjudicatedBoundaryIdentityIds,
  );
  const adjudicatedNavigationOrIdentity =
    reviewedScore.risk.navigationOrIdentity -
    adjudicatedSourceGroundedNavigationIds.length;
  const approvedConservativeDecision = new Set(
    baseline.approvedConservativeDecisionIds,
  );
  const adjudicatedConservativeDecisionIds = fixture.probes
    .filter((probe) => approvedConservativeDecision.has(probe.id))
    .filter((probe) => {
      const observation = observations.find((item) => item.caseId === probe.id);
      return (
        probe.expected.decision === "established" &&
        probe.expected.proofGrounded &&
        observation?.decision === "partial" &&
        !observation.proofGrounded &&
        probe.expected.relations.every((expected) =>
          observation.relations.some(
            (actual) =>
              actual.relationId === expected.relationId && actual.sourceGrounded,
          ),
        )
      );
    })
    .map((probe) => probe.id);
  for (const caseId of approvedConservativeDecision) {
    if (!adjudicatedConservativeDecisionIds.includes(caseId)) {
      regressions.push(`invalid conservative-decision adjudication ${caseId}`);
    }
  }
  const adjudicatedMissedCoverage =
    reviewedScore.risk.missedCoverage -
    adjudicatedConservativeDecisionIds.length;
  const adjudicatedRisk =
    reviewedScore.risk.total -
    adjudicatedSourceGroundedNavigationIds.length * 10 -
    adjudicatedConservativeDecisionIds.length * 2;
  if (reviewedScore.passed < baseline.minimumPassed) {
    regressions.push(
      `adjudicated passed ${reviewedScore.passed} is below ${baseline.minimumPassed}`,
    );
  }
  if (adjudicatedRisk > baseline.maximumRisk) {
    regressions.push(
      `adjudicated risk ${adjudicatedRisk} exceeds ${baseline.maximumRisk}`,
    );
  }
  if (score.risk.falseConflict > 0) {
    regressions.push(`false conflict ${score.risk.falseConflict} is unsafe`);
  }
  if (adjudicatedNavigationOrIdentity > baseline.maximumNavigationOrIdentity) {
    regressions.push(
      `adjudicated navigation or identity risk ${adjudicatedNavigationOrIdentity} exceeds ${baseline.maximumNavigationOrIdentity}`,
    );
  }
  if (adjudicatedMissedCoverage > baseline.maximumMissedCoverage) {
    regressions.push(
      `adjudicated missed coverage ${adjudicatedMissedCoverage} exceeds ${baseline.maximumMissedCoverage}`,
    );
  }
  return regressions;
}

function applyReviewedScoreAdjustments(
  fixture: AuthoredScientificFixture,
  observations: readonly AuthoredScientificObservation[],
  score: AuthoredScientificScorecard,
  formulaDecisionIds: readonly string[],
  boundaryIdentityIds: readonly string[],
): AuthoredScientificScorecard {
  let passed = score.passed;
  let falseConflict = score.risk.falseConflict;
  let falseEstablishment = score.risk.falseEstablishment;
  let missedCoverage = score.risk.missedCoverage;
  let navigationOrIdentity = score.risk.navigationOrIdentity;
  let total = score.risk.total;
  for (const caseId of [...formulaDecisionIds, ...boundaryIdentityIds]) {
    const probe = fixture.probes.find((item) => item.id === caseId);
    const observation = observations.find((item) => item.caseId === caseId);
    if (!probe || !observation) continue;
    const scenario = authoredScenarioFor(fixture, probe);
    const singleFixture = {
      ...fixture,
      probes: [probe],
      scenarios: [scenario],
    };
    const raw = scoreAuthoredScientificFixture(singleFixture, [observation]);
    const reviewedObservation = formulaDecisionIds.includes(caseId)
      ? {
          ...observation,
          decision: probe.expected.decision,
          proofGrounded: probe.expected.proofGrounded,
        }
      : { ...observation, symbol: probe.expected.symbol ?? null };
    const reviewed = scoreAuthoredScientificFixture(singleFixture, [
      reviewedObservation,
    ]);
    passed += reviewed.passed - raw.passed;
    falseConflict += reviewed.risk.falseConflict - raw.risk.falseConflict;
    falseEstablishment +=
      reviewed.risk.falseEstablishment - raw.risk.falseEstablishment;
    missedCoverage += reviewed.risk.missedCoverage - raw.risk.missedCoverage;
    navigationOrIdentity +=
      reviewed.risk.navigationOrIdentity - raw.risk.navigationOrIdentity;
    total += reviewed.risk.total - raw.risk.total;
  }
  return {
    ...score,
    passed,
    risk: {
      falseConflict,
      falseEstablishment,
      missedCoverage,
      navigationOrIdentity,
      total,
    },
  };
}

function legacyFormulaDecisionIsEstablished(
  fixture: AuthoredScientificFixture,
  probe: AuthoredScientificFixture["probes"][number],
  observation: AuthoredScientificObservation,
): boolean {
  if (
    fixture.schemaVersion !== 1 ||
    probe.expected.decision !== "established" ||
    !probe.expected.proofGrounded ||
    (observation.decision === "established" && observation.proofGrounded)
  ) {
    return false;
  }
  const context = observation.authoringContext;
  const formula = context?.formula;
  if (
    !context ||
    !formula ||
    context.disposition !== "established" ||
    context.lifecycle.freshness !== "current" ||
    context.lifecycle.retracted ||
    context.lifecycle.capped ||
    context.lifecycle.engineLimited ||
    context.truncated
  ) {
    return false;
  }
  const snapshot = authoredSnapshotFor(
    authoredScenarioFor(fixture, probe),
    probe,
  );
  const document = snapshot.documents.find(
    (item) => item.fileId === formula.location.fileId,
  );
  if (
    !document ||
    document.path !== formula.location.path ||
    !Number.isInteger(formula.location.range.startOffset) ||
    !Number.isInteger(formula.location.range.endOffset) ||
    formula.location.range.startOffset < 0 ||
    formula.location.range.endOffset <= formula.location.range.startOffset ||
    formula.location.range.endOffset > document.content.length ||
    document.content.slice(
      formula.location.range.startOffset,
      formula.location.range.endOffset,
    ) !== formula.sourceNotation
  ) {
    return false;
  }
  const cursor = resolveAuthoredAnchor(snapshot, probe.cursor);
  const cursorOffset =
    probe.cursor.offset === undefined
      ? probe.cursor.edge === "after"
        ? cursor.range.endOffset
        : cursor.range.startOffset
      : cursor.range.startOffset + probe.cursor.offset;
  if (
    cursor.fileId !== formula.location.fileId ||
    cursor.path !== formula.location.path ||
    cursorOffset < formula.location.range.startOffset ||
    cursorOffset > formula.location.range.endOffset
  ) {
    return false;
  }
  const selectedFormula = observeSelectedFormulaDecision({
    disposition: context.disposition,
    formula,
    hypotheses: context.interpretations.hypotheses,
  });
  const expectedRelationIds = new Set(
    probe.expected.relations
      .filter((relation) => relation.sourceGrounded)
      .map((relation) => relation.relationId),
  );
  const authoritative = selectedFormula.recognizedRelations.filter(
    (relation) => relation.authority === "authoritative",
  );
  return (
    selectedFormula.decision.status === "established" &&
    selectedFormula.decision.sourceGrounded &&
    selectedFormula.decision.problemCount === 0 &&
    selectedFormulaMeaningIsEstablishmentGrade({
      disposition: context.disposition,
      formula,
      hypotheses: context.interpretations.hypotheses,
    }) &&
    typeof selectedFormula.decision.meaningRelationId === "string" &&
    expectedRelationIds.has(selectedFormula.decision.meaningRelationId) &&
    authoritative.length === 1 &&
    authoritative[0]?.relationId ===
      selectedFormula.decision.meaningRelationId
  );
}

function sameCauseSet(
  left: readonly AuthoredFalseEstablishmentCause[],
  right: readonly AuthoredFalseEstablishmentCause[],
): boolean {
  const uniqueLeft = [...new Set(left)].sort();
  const uniqueRight = [...new Set(right)].sort();
  return (
    uniqueLeft.length === left.length &&
    uniqueRight.length === right.length &&
    uniqueLeft.length === uniqueRight.length &&
    uniqueLeft.every((cause, index) => cause === uniqueRight[index])
  );
}

function sameLocations(
  actual: readonly ObservedLocation[],
  expected: readonly ObservedLocation[],
): boolean {
  return (
    actual.length === expected.length &&
    actual.every((location, index) => sameLocation(location, expected[index]))
  );
}

function sameLocation(
  actual: ObservedLocation | undefined,
  expected: ObservedLocation | undefined,
): boolean {
  return (
    actual !== undefined &&
    expected !== undefined &&
    actual.fileId === expected.fileId &&
    actual.path === expected.path &&
    sameRange(actual.range, expected.range)
  );
}

function sameRange(
  actual: ObservedLocation["range"],
  expected: ObservedLocation["range"],
): boolean {
  return (
    actual.startOffset === expected.startOffset &&
    actual.endOffset === expected.endOffset
  );
}
