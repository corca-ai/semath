import { describe, expect, test } from "bun:test";
import { differentialEquationsFoundationSuite } from "./differential-equations-foundation-seeds";

describe("differential-equations foundation promotion evidence", () => {
  test("covers each promoted model and condition with balanced seeds", () => {
    expect(differentialEquationsFoundationSuite.packId).toBe("calculus-analysis");
    expect(differentialEquationsFoundationSuite.laws).toHaveLength(14);
    expect(new Set(differentialEquationsFoundationSuite.laws.map((law) => law.lawId)).size)
      .toBe(14);

    for (const law of differentialEquationsFoundationSuite.laws) {
      expect(law.positives, law.lawId).toHaveLength(5);
      expect(law.refusals, law.lawId).toHaveLength(5);
      expect(new Set(law.positives.map((seed) => seed[2])).size, law.lawId).toBe(5);
    }
  });

  test("adds reusable ODE, PDE, and problem-condition families without duplicating field laws", () => {
    const laws = new Set(
      differentialEquationsFoundationSuite.laws.map((law) => law.lawId),
    );
    for (const required of [
      "first-order-ode-model",
      "second-order-ode-model",
      "linear-ode-system",
      "diffusion-equation",
      "poisson-equation",
      "laplace-equation",
      "conservation-form-equation",
      "initial-value-condition",
      "dirichlet-boundary-condition",
      "neumann-boundary-condition",
      "robin-boundary-condition",
      "interface-continuity-condition",
    ]) {
      expect(laws.has(required), required).toBeTrue();
    }
    expect(laws.has("scalar-wave-equation")).toBeFalse();
    expect(laws.has("helmholtz-equation")).toBeFalse();
    expect(laws.has("continuous-state-equation")).toBeFalse();
  });
});
