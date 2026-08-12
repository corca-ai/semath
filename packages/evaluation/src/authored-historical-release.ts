import type {
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "./authored-scientific";

export interface AuthoredHistoricalReleaseBaseline {
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
  if (score.passed < baseline.minimumPassed) {
    regressions.push(
      `passed ${score.passed} is below ${baseline.minimumPassed}`,
    );
  }
  if (score.risk.total > baseline.maximumRisk) {
    regressions.push(
      `risk ${score.risk.total} exceeds ${baseline.maximumRisk}`,
    );
  }
  if (score.risk.falseConflict > 0) {
    regressions.push(`false conflict ${score.risk.falseConflict} is unsafe`);
  }
  if (score.risk.navigationOrIdentity > baseline.maximumNavigationOrIdentity) {
    regressions.push(
      `navigation or identity risk ${score.risk.navigationOrIdentity} exceeds ${baseline.maximumNavigationOrIdentity}`,
    );
  }
  if (score.risk.missedCoverage > baseline.maximumMissedCoverage) {
    regressions.push(
      `missed coverage ${score.risk.missedCoverage} exceeds ${baseline.maximumMissedCoverage}`,
    );
  }
  return regressions;
}
