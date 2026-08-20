import { describe, expect, test } from "bun:test";
import {
  authoredReleaseRegressions,
  authoredReleaseResultRegressions,
  mathAuthoringReleaseRegressions,
  parseAuthoredReleaseReport,
} from "./authored-release-gate";

const baseline = { cases: 166, maximumRisk: 116, minimumPassed: 108 };

describe("authored development release gate", () => {
  test("accepts missed coverage at the reviewed safety baseline", () => {
    expect(
      authoredReleaseRegressions(
        {
          cases: 166,
          failures: ["reviewed coverage miss"],
          passed: 108,
          risk: {
            falseConflict: 0,
            falseEstablishment: 0,
            missedCoverage: 58,
            navigationOrIdentity: 0,
            total: 116,
          },
        },
        baseline,
      ),
    ).toEqual([]);
  });

  test("rejects coverage regression and every unsafe risk class", () => {
    expect(
      authoredReleaseRegressions(
        {
          cases: 166,
          failures: [],
          passed: 107,
          risk: {
            falseConflict: 1,
            falseEstablishment: 1,
            missedCoverage: 59,
            navigationOrIdentity: 1,
            total: 148,
          },
        },
        baseline,
      ),
    ).toHaveLength(5);
  });

  test("parses the report boundary and rejects evidence-graded safety failures", () => {
    const result = parseAuthoredReleaseReport({
      results: [
        {
          score: {
            cases: 166,
            failures: [],
            passed: 108,
            risk: {
              falseConflict: 0,
              falseEstablishment: 0,
              missedCoverage: 58,
              navigationOrIdentity: 0,
              total: 116,
            },
          },
          evidenceGraded: { failures: ["unsupported contradiction"] },
          mathAuthoring: {
            cases: 0,
            exactCases: 0,
            failures: [],
            required: false,
          },
        },
      ],
    });

    expect(authoredReleaseResultRegressions(result, baseline)).toEqual([
      "evidence-graded safety: unsupported contradiction",
    ]);
  });

  test("rejects malformed report fields instead of trusting JSON assertions", () => {
    expect(() =>
      parseAuthoredReleaseReport({
        results: [
          {
            score: {
              cases: "166",
              failures: [],
              passed: 108,
              risk: {
                falseConflict: 0,
                falseEstablishment: 0,
                missedCoverage: 58,
                navigationOrIdentity: 0,
                total: 116,
              },
            },
            evidenceGraded: { failures: [] },
            mathAuthoring: {
              cases: 0,
              exactCases: 0,
              failures: [],
              required: false,
            },
          },
        ],
      }),
    ).toThrow("report.results[0].score.cases: expected nonnegative integer");
    expect(() =>
      parseAuthoredReleaseReport({
        results: [
          {
            score: {
              cases: 166,
              failures: [],
              passed: 108,
              risk: {
                falseConflict: 0,
                falseEstablishment: 0,
                missedCoverage: 58,
                navigationOrIdentity: 0,
                total: 116,
              },
            },
            evidenceGraded: { failures: [false] },
            mathAuthoring: {
              cases: 0,
              exactCases: 0,
              failures: [],
              required: false,
            },
          },
        ],
      }),
    ).toThrow(
      "report.results[0].evidenceGraded.failures: expected string array",
    );
  });

  test("distinguishes explicit legacy 0/0 from a required exact oracle", () => {
    expect(
      mathAuthoringReleaseRegressions({
        cases: 0,
        exactCases: 0,
        failures: [],
        required: false,
      }),
    ).toEqual([]);
    expect(
      mathAuthoringReleaseRegressions({
        cases: 0,
        exactCases: 0,
        failures: [],
        required: true,
      }),
    ).toEqual(["required authoring context has no cases"]);
    expect(
      mathAuthoringReleaseRegressions({
        cases: 12,
        exactCases: 11,
        failures: ["unsafe lifecycle"],
        required: true,
      }),
    ).toEqual([
      "exact authoring context 11/12; required 12/12",
      "authoring-context safety: unsafe lifecycle",
    ]);
  });

});
