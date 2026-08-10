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
