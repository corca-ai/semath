import { describe, expect, test } from "bun:test";
import { probabilityStatisticsFoundationSuite } from "./probability-statistics-foundation-seeds";

describe("probability-statistics foundation promotion evidence", () => {
  test("covers every promoted family with balanced reviewed seeds", () => {
    expect(probabilityStatisticsFoundationSuite.laws).toHaveLength(17);
    for (const law of probabilityStatisticsFoundationSuite.laws) {
      expect(law.positives).toHaveLength(5);
      expect(law.refusals).toHaveLength(5);
    }
  });

  test("spans distributions, moments, estimation, regression, and stochastic processes", () => {
    const laws = new Set(
      probabilityStatisticsFoundationSuite.laws.map((law) => law.lawId),
    );
    for (const lawId of [
      "density-normalization",
      "cdf-from-density",
      "covariance-value-definition",
      "sample-mean-definition",
      "confidence-upper-bound",
      "linear-regression-model",
      "stochastic-state-transition",
      "process-autocovariance-definition",
    ]) {
      expect(laws.has(lawId)).toBe(true);
    }
  });
});
