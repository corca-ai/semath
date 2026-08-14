import { describe, expect, test } from "bun:test";
import { linearAlgebraFoundationSuite } from "./linear-algebra-foundation-seeds";

describe("linear-algebra foundation promotion evidence", () => {
  test("covers the reviewed foundation vertical with balanced seeds", () => {
    expect(linearAlgebraFoundationSuite.packId).toBe("linear-algebra");
    expect(linearAlgebraFoundationSuite.laws).toHaveLength(22);
    expect(new Set(linearAlgebraFoundationSuite.laws.map((law) => law.lawId)).size)
      .toBe(22);

    for (const law of linearAlgebraFoundationSuite.laws) {
      expect(law.positives, law.lawId).toHaveLength(5);
      expect(law.refusals, law.lawId).toHaveLength(5);
      expect(new Set(law.positives.map((seed) => seed[2])).size, law.lawId).toBe(5);
    }
  });

  test("includes the cross-STEM relation families without duplicating optimization ownership", () => {
    const laws = new Set(linearAlgebraFoundationSuite.laws.map((law) => law.lawId));
    for (const required of [
      "matrix-addition",
      "matrix-inverse-definition",
      "inner-product-definition",
      "eigenpair-equation",
      "matrix-diagonalization",
      "positive-definite-quadratic-form",
      "lu-factorization",
      "qr-factorization",
      "cholesky-factorization",
      "singular-value-decomposition",
      "pseudoinverse-solution",
      "rank-nullity-theorem",
    ]) {
      expect(laws.has(required), required).toBeTrue();
    }
    expect(laws.has("least-squares-normal-equation")).toBeFalse();
  });
});
