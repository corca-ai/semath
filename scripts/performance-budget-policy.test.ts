import { describe, expect, test } from "bun:test";
import { shouldEnforceTiming } from "./performance-budget-policy";

describe("performance budget policy", () => {
  test("keeps timing strict by default on fast and stable-host runs", () => {
    expect(shouldEnforceTiming({}, 60)).toBe(true);
    expect(shouldEnforceTiming({}, 500)).toBe(false);
    expect(shouldEnforceTiming({ SEMATH_BUDGET_STABLE: "1" }, 500)).toBe(true);
  });

  test("lets a shared runner report timings without treating them as stable", () => {
    expect(
      shouldEnforceTiming(
        { SEMATH_BUDGET_STABLE: "1", SEMATH_BUDGET_TIMING_GATE: "0" },
        500,
      ),
    ).toBe(false);
    expect(shouldEnforceTiming({ SEMATH_BUDGET_TIMING_GATE: "1" }, 500)).toBe(true);
    expect(() => shouldEnforceTiming({ SEMATH_BUDGET_TIMING_GATE: "yes" }, 60)).toThrow();
  });
});
