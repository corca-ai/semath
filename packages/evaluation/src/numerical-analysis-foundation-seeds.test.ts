import { describe, expect, test } from "bun:test";
import { numericalAnalysisFoundationSuite } from "./numerical-analysis-foundation-seeds";

describe("numerical-analysis foundation promotion evidence", () => {
  test("covers every promoted family with balanced positive and refusal seeds", () => {
    expect(numericalAnalysisFoundationSuite.packId).toBe("numerical-analysis");
    expect(numericalAnalysisFoundationSuite.laws).toHaveLength(22);
    expect(new Set(numericalAnalysisFoundationSuite.laws.map((law) => law.lawId)).size)
      .toBe(22);
    for (const law of numericalAnalysisFoundationSuite.laws) {
      expect(law.positives, law.lawId).toHaveLength(5);
      expect(law.refusals, law.lawId).toHaveLength(5);
      expect(new Set(law.positives.map((seed) => seed[2])).size, law.lawId).toBe(5);
    }
  });

  test("spans approximation safety and representative computational methods", () => {
    const laws = new Set(numericalAnalysisFoundationSuite.laws.map((law) => law.lawId));
    for (const required of [
      "approximate-value-relation",
      "asymptotic-order-membership",
      "convergence-envelope",
      "newton-root-update",
      "trapezoidal-quadrature",
      "forward-difference-derivative",
      "explicit-euler-step",
      "relaxed-residual-iteration",
      "least-squares-approximation",
      "perturbation-stability-bound",
      "discrete-model-equation",
    ]) expect(laws.has(required), required).toBeTrue();
  });
});
