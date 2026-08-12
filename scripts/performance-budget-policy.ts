export function shouldEnforceTiming(
  environment: Readonly<Record<string, string | undefined>>,
  documentCount: number,
): boolean {
  const override = environment.SEMATH_BUDGET_TIMING_GATE;
  if (override !== undefined) {
    if (override === "1") return true;
    if (override === "0") return false;
    throw new Error("SEMATH_BUDGET_TIMING_GATE must be 0 or 1");
  }
  return environment.SEMATH_BUDGET_STABLE === "1" || documentCount < 500;
}

export function retainedRssBudgetBytes(documentCount: number): number {
  return (documentCount >= 500 ? 192 : 112) * 1024 * 1024;
}

export interface TimingBudget {
  readonly coldMs: number;
  readonly deltaP95Ms: number;
  readonly queryP95Ms: number;
  readonly semanticDeltaMs: number;
}

export function timingBudget(documentCount: number, stableHost: boolean): TimingBudget {
  return {
    coldMs: documentCount >= 500 ? 5_000 : 2_500,
    deltaP95Ms: stableHost ? (documentCount >= 500 ? 50 : 25) : 75,
    queryP95Ms: 8,
    semanticDeltaMs: 50,
  };
}

export function shouldEnforceRetainedRss(
  environment: Readonly<Record<string, string | undefined>>,
): boolean {
  const override = environment.SEMATH_BUDGET_RSS_GATE;
  if (override === undefined || override === "1") return true;
  if (override === "0") return false;
  throw new Error("SEMATH_BUDGET_RSS_GATE must be 0 or 1");
}

export function medianSample(values: readonly number[]): number {
  if (values.length === 0 || values.length % 2 === 0) {
    throw new Error("median sample requires an odd, non-empty sample");
  }
  return [...values].sort((left, right) => left - right)[Math.floor(values.length / 2)]!;
}
