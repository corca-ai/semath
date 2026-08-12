import type { AuthoredScientificScorecard } from "./authored-scientific";

export interface AuthoredReleaseBaseline {
  readonly cases: number;
  readonly maximumRisk: number;
  readonly minimumPassed: number;
}

export function authoredReleaseRegressions(
  score: AuthoredScientificScorecard,
  baseline: AuthoredReleaseBaseline,
): readonly string[] {
  const regressions: string[] = [];
  if (score.cases !== baseline.cases) {
    regressions.push(`case count ${score.cases} differs from ${baseline.cases}`);
  }
  if (score.passed < baseline.minimumPassed) {
    regressions.push(`passed ${score.passed} is below ${baseline.minimumPassed}`);
  }
  if (score.risk.total > baseline.maximumRisk) {
    regressions.push(`risk ${score.risk.total} exceeds ${baseline.maximumRisk}`);
  }
  if (score.risk.falseEstablishment > 0) {
    regressions.push(`false establishment ${score.risk.falseEstablishment} is unsafe`);
  }
  if (score.risk.falseConflict > 0) {
    regressions.push(`false conflict ${score.risk.falseConflict} is unsafe`);
  }
  if (score.risk.navigationOrIdentity > 0) {
    regressions.push(
      `navigation or identity risk ${score.risk.navigationOrIdentity} is unsafe`,
    );
  }
  return regressions;
}
