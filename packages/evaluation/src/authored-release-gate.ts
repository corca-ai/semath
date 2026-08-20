import type { AuthoredScientificScorecard } from "./authored-scientific";

export interface AuthoredReleaseBaseline {
  readonly cases: number;
  readonly maximumRisk: number;
  readonly minimumPassed: number;
}

export interface AuthoredReleaseResult {
  readonly score: AuthoredScientificScorecard;
  readonly evidenceGraded: {
    readonly failures: readonly string[];
  };
  readonly mathAuthoring: {
    readonly cases: number;
    readonly exactCases: number;
    readonly failures: readonly string[];
    readonly required: boolean;
  };
}

export function parseAuthoredReleaseReport(value: unknown): AuthoredReleaseResult {
  const report = record(value, "report");
  if (!Array.isArray(report.results) || report.results.length !== 1) {
    throw new Error("report.results: expected exactly one result");
  }
  const result = record(report.results[0], "report.results[0]");
  const evidenceGraded = record(
    result.evidenceGraded,
    "report.results[0].evidenceGraded",
  );
  const mathAuthoring = record(
    result.mathAuthoring,
    "report.results[0].mathAuthoring",
  );
  return {
    score: parseScore(result.score, "report.results[0].score"),
    evidenceGraded: {
      failures: strings(
        evidenceGraded.failures,
        "report.results[0].evidenceGraded.failures",
      ),
    },
    mathAuthoring: {
      cases: nonnegativeInteger(
        mathAuthoring.cases,
        "report.results[0].mathAuthoring.cases",
      ),
      exactCases: nonnegativeInteger(
        mathAuthoring.exactCases,
        "report.results[0].mathAuthoring.exactCases",
      ),
      failures: strings(
        mathAuthoring.failures,
        "report.results[0].mathAuthoring.failures",
      ),
      required: boolean(
        mathAuthoring.required,
        "report.results[0].mathAuthoring.required",
      ),
    },
  };
}

export function authoredReleaseResultRegressions(
  result: AuthoredReleaseResult,
  baseline: AuthoredReleaseBaseline,
): readonly string[] {
  return [
    ...authoredReleaseRegressions(result.score, baseline),
    ...result.evidenceGraded.failures.map(
      (failure) => `evidence-graded safety: ${failure}`,
    ),
    ...mathAuthoringReleaseRegressions(result.mathAuthoring),
  ];
}

export function mathAuthoringReleaseRegressions(
  report: AuthoredReleaseResult["mathAuthoring"],
): readonly string[] {
  if (!report.required) {
    return report.cases === 0 && report.exactCases === 0 && report.failures.length === 0
      ? []
      : ["non-required authoring context must report 0/0 with no failures"];
  }
  const regressions = report.failures.map(
    (failure) => `authoring-context safety: ${failure}`,
  );
  if (report.cases <= 0) {
    regressions.unshift("required authoring context has no cases");
  } else if (report.exactCases !== report.cases) {
    regressions.unshift(
      `exact authoring context ${report.exactCases}/${report.cases}; required ${report.cases}/${report.cases}`,
    );
  }
  return regressions;
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

function parseScore(value: unknown, path: string): AuthoredScientificScorecard {
  const score = record(value, path);
  const risk = record(score.risk, `${path}.risk`);
  return {
    cases: nonnegativeInteger(score.cases, `${path}.cases`),
    failures: strings(score.failures, `${path}.failures`),
    passed: nonnegativeInteger(score.passed, `${path}.passed`),
    risk: {
      falseConflict: nonnegativeInteger(
        risk.falseConflict,
        `${path}.risk.falseConflict`,
      ),
      falseEstablishment: nonnegativeInteger(
        risk.falseEstablishment,
        `${path}.risk.falseEstablishment`,
      ),
      missedCoverage: nonnegativeInteger(
        risk.missedCoverage,
        `${path}.risk.missedCoverage`,
      ),
      navigationOrIdentity: nonnegativeInteger(
        risk.navigationOrIdentity,
        `${path}.risk.navigationOrIdentity`,
      ),
      total: nonnegativeInteger(risk.total, `${path}.risk.total`),
    },
  };
}

function record(value: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: expected object`);
  }
  return value as Readonly<Record<string, unknown>>;
}

function strings(value: unknown, path: string): readonly string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new Error(`${path}: expected string array`);
  }
  return value;
}

function nonnegativeInteger(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw new Error(`${path}: expected nonnegative integer`);
  }
  return value;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${path}: expected boolean`);
  return value;
}
