import { describe, expect, test } from "bun:test";
import {
  medianSample,
  isReleasePerformanceHost,
  retainedRssBudgetBytes,
  shouldEnforceRetainedRss,
  shouldEnforceTiming,
  timingBudget,
} from "./performance-budget-policy";

const linux = { platform: "linux", architecture: "x64" };
const mac = { platform: "darwin", architecture: "arm64" };

describe("performance budget policy", () => {
  test("keeps calibrated limits on Linux x64 and reports other hosts explicitly", () => {
    expect(isReleasePerformanceHost(linux)).toBe(true);
    expect(isReleasePerformanceHost(mac)).toBe(false);
    expect(isReleasePerformanceHost({ platform: "linux", architecture: "arm64" })).toBe(false);
    expect(shouldEnforceTiming({}, 60, mac)).toBe(false);
    expect(shouldEnforceRetainedRss({}, mac)).toBe(false);
    expect(shouldEnforceTiming({ SEMATH_BUDGET_TIMING_GATE: "1" }, 60, mac)).toBe(true);
    expect(shouldEnforceRetainedRss({ SEMATH_BUDGET_RSS_GATE: "1" }, mac)).toBe(true);
  });

  test("keeps timing strict by default on fast and stable-host runs", () => {
    expect(shouldEnforceTiming({}, 60, linux)).toBe(true);
    expect(shouldEnforceTiming({}, 500, linux)).toBe(false);
    expect(shouldEnforceTiming({ SEMATH_BUDGET_STABLE: "1" }, 500, linux)).toBe(true);
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

  test("keeps the approved retained-memory limits independent of sampling", () => {
    expect(retainedRssBudgetBytes(60)).toBe(112 * 1024 * 1024);
    expect(retainedRssBudgetBytes(500)).toBe(192 * 1024 * 1024);
    expect(shouldEnforceRetainedRss({}, linux)).toBe(true);
    expect(shouldEnforceRetainedRss({ SEMATH_BUDGET_RSS_GATE: "0" })).toBe(false);
    expect(() => shouldEnforceRetainedRss({ SEMATH_BUDGET_RSS_GATE: "yes" })).toThrow();
  });

  test("keeps approved timing limits independent of repeated sampling", () => {
    expect(timingBudget(60, true)).toEqual({
      coldMs: 2_500,
      deltaP95Ms: 25,
      queryP95Ms: 8,
      semanticDeltaMs: 50,
    });
    expect(timingBudget(500, true)).toEqual({
      coldMs: 5_000,
      deltaP95Ms: 50,
      queryP95Ms: 8,
      semanticDeltaMs: 50,
    });
  });

  test("uses an odd isolated-sample median for stable RSS", () => {
    expect(medianSample([125, 108, 123])).toBe(123);
    expect(() => medianSample([])).toThrow();
    expect(() => medianSample([1, 2])).toThrow();
  });
});
