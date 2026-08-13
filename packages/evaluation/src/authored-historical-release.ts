import type {
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "./authored-scientific";
import { authoredProbeIdentityFailures } from "./authored-scientific";

export interface AuthoredHistoricalReleaseBaseline {
  readonly approvedConservativeDecisionIds: readonly string[];
  readonly approvedCursorBoundaryIdentityIds: readonly string[];
  readonly approvedFalseEstablishmentIds: readonly string[];
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
  const expectedById = new Map(
    fixture.probes.map((probe) => [probe.id, probe.expected] as const),
  );
  const falseEstablishmentIds = observations
    .filter((observation) => {
      const expected = expectedById.get(observation.caseId);
      return (
        expected?.decision !== "established" &&
        observation.decision === "established"
      );
    })
    .map((observation) => observation.caseId)
    .sort();
  const approved = new Set(baseline.approvedFalseEstablishmentIds);
  for (const caseId of falseEstablishmentIds) {
    if (!approved.has(caseId)) {
      regressions.push(`unreviewed false establishment ${caseId}`);
    }
    const observation = observations.find((item) => item.caseId === caseId);
    if (!observation?.proofGrounded) {
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
  const adjudicatedNavigationOrIdentity =
    score.risk.navigationOrIdentity - adjudicatedBoundaryIdentityIds.length;
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
    score.risk.missedCoverage - adjudicatedConservativeDecisionIds.length;
  const adjudicatedRisk =
    score.risk.total -
    adjudicatedBoundaryIdentityIds.length * 10 -
    adjudicatedConservativeDecisionIds.length * 2;
  if (score.passed < baseline.minimumPassed) {
    regressions.push(
      `passed ${score.passed} is below ${baseline.minimumPassed}`,
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
