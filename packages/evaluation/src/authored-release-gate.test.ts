import { describe, expect, test } from "bun:test";
import { authoredReleaseRegressions } from "./authored-release-gate";

const baseline = { cases: 115, maximumRisk: 130, minimumPassed: 50 };

describe("authored development release gate", () => {
  test("accepts missed coverage at the reviewed safety baseline", () => {
    expect(
      authoredReleaseRegressions(
        {
          cases: 115,
          failures: ["reviewed coverage miss"],
          passed: 50,
          risk: {
            falseConflict: 0,
            falseEstablishment: 0,
            missedCoverage: 65,
            navigationOrIdentity: 0,
            total: 130,
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
          cases: 115,
          failures: [],
          passed: 49,
          risk: {
            falseConflict: 1,
            falseEstablishment: 1,
            missedCoverage: 66,
            navigationOrIdentity: 1,
            total: 160,
          },
        },
        baseline,
      ),
    ).toHaveLength(5);
  });
});
