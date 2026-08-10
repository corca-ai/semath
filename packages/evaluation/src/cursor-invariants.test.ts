import { describe, expect, test } from "bun:test";
import { CURSOR_INVARIANT_FAMILIES, planCursorInvariantSurfaces } from "./cursor-invariants";

describe("cross-stack cursor invariant planning", () => {
  test("covers every reviewed structural family and every cursor edge deterministically", () => {
    const surfaces = planCursorInvariantSurfaces();
    expect(surfaces).toEqual(planCursorInvariantSurfaces());
    expect(new Set(surfaces.map((surface) => surface.family))).toEqual(
      new Set(CURSOR_INVARIANT_FAMILIES),
    );
    expect(surfaces.every((surface) => surface.probes.length >= 2)).toBe(true);
    expect(
      surfaces.flatMap((surface) => surface.probes).some((probe) => probe.id.endsWith("after")),
    ).toBe(true);
  });
});
