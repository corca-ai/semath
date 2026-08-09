import { describe, expect, test } from "bun:test";
import {
  builtInPacks,
  loadPack,
  SEMATH_PACK_SCHEMA_VERSION,
  validatePack,
  validatePackCatalog,
} from "./index";

describe("public pack contract", () => {
  test("enumerates and validates the complete built-in catalog", () => {
    const packs = builtInPacks();
    expect(packs.map((pack) => pack.packId)).toEqual([
      "linear-algebra",
      "probability",
      "calculus-analysis",
      "optimization-ml",
      "discrete-math",
      "quantities-units",
      "classical-mechanics",
      "circuits",
      "control-systems",
    ]);
    expect(packs.every((pack) => validatePack(pack).ok)).toBe(true);
    expect(
      packs.reduce((count, pack) => count + pack.patterns.length, 0),
    ).toBeGreaterThanOrEqual(60);
    expect(packs.every(Object.isFrozen)).toBe(true);
  });

  test("fails an unknown primitive at a deterministic path", () => {
    const pack = structuredClone(builtInPacks()[0]);
    if (!pack) throw new Error("expected linear algebra pack");
    const pattern = pack.patterns[0];
    if (!pattern) throw new Error("expected pattern");
    pattern.matcher.primitive = "execute-javascript";
    const result = validatePack(pack);
    expect(result).toEqual({
      errors: [
        {
          message: "is not a safe built-in primitive",
          path: "patterns[0].matcher.primitive",
        },
      ],
      ok: false,
    });
  });

  test("prevents recognition-only entries from producing edits", () => {
    const pack = structuredClone(builtInPacks()[2]);
    if (!pack) throw new Error("expected calculus pack");
    const pattern = pack.patterns[0];
    if (!pattern) throw new Error("expected pattern");
    pattern.generationTemplate = "{{expression}}";
    const result = validatePack(pack);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.errors[0]?.path).toBe("patterns[0].generationTemplate");
    }
  });

  test("reports malformed JSON and future schemas without throwing", () => {
    expect(loadPack("{")).toMatchObject({ ok: false });
    expect(
      loadPack(
        JSON.stringify({
          packId: "future",
          schemaVersion: SEMATH_PACK_SCHEMA_VERSION + 1,
        }),
      ),
    ).toEqual({
      errors: [
        {
          message: "unsupported schema 4; expected 3",
          path: "schemaVersion",
        },
      ],
      ok: false,
    });
  });

  test("rejects unresolved cross-pack concepts and capability contracts", () => {
    const packs = structuredClone(builtInPacks());
    const mechanics = packs.find((pack) => pack.packId === "classical-mechanics");
    if (!mechanics?.laws[0]?.roles[0]) throw new Error("expected mechanics law");
    mechanics.laws[0].roles[0].concept = "quantities-units:unknown-force";
    const unknownConcept = validatePackCatalog(packs);
    expect(unknownConcept.ok).toBe(false);
    if (!unknownConcept.ok) {
      expect(unknownConcept.errors[0]?.path).toBe(
        "packs[6].laws[0].roles[0].concept",
      );
    }

    const missingDependency = structuredClone(builtInPacks());
    const control = missingDependency.find(
      (pack) => pack.packId === "control-systems",
    );
    if (!control) throw new Error("expected control pack");
    control.dependencies = [];
    const missingCapability = validatePackCatalog(missingDependency);
    expect(missingCapability.ok).toBe(false);
    if (!missingCapability.ok) {
      expect(missingCapability.errors[0]?.path).toBe(
        "packs[8].capabilities.requires",
      );
    }
  });
});
