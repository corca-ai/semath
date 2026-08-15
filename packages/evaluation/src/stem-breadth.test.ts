import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import {
  parseAuthoredScientificFixture,
  type AuthoredFirstLossRecord,
} from "./index";
import {
  parseStemBreadthManifest,
  scoreStemBreadth,
  validateStemBreadthBenchmark,
} from "./stem-breadth";

const root = new URL("../../../", import.meta.url);

describe("practical STEM breadth benchmark", () => {
  test("validates the reviewed public-development matrix", async () => {
    const { manifest, fixture } = await inputs();
    expect(validateStemBreadthBenchmark(manifest, fixture)).toEqual({
      commissionedGaps: 0,
      fields: {
        "shared-foundations": { gaps: 0, measuredCapabilities: 10 },
        "linear-algebra": { gaps: 0, measuredCapabilities: 10 },
        "differential-equations": { gaps: 0, measuredCapabilities: 10 },
        "probability-statistics": { gaps: 0, measuredCapabilities: 10 },
        "numerical-analysis": { gaps: 0, measuredCapabilities: 10 },
      },
      measuredCells: 50,
      referencedProbes: 104,
    });
  });

  test("rejects unknown evidence instead of inflating a cell", async () => {
    const { rawManifest, fixture } = await inputs();
    const changed = mutableManifest(rawManifest);
    const field = changed.fields[0];
    const cell = field?.capabilities[0];
    if (!cell) throw new Error("test fixture is missing its first cell");
    cell.probeIds = ["invented-probe"];
    expect(() =>
      validateStemBreadthBenchmark(
        parseStemBreadthManifest(changed),
        fixture,
      ),
    ).toThrow("unknown development probe invented-probe");
  });

  test("keeps commissioned gaps distinct from measured cells", async () => {
    const { rawManifest } = await inputs();
    const changed = mutableManifest(rawManifest);
    const gap = changed.fields[0]?.capabilities[0];
    if (!gap) throw new Error("test fixture is missing its first cell");
    gap.status = "commissioned-gap";
    gap.plannedIssue = "https://github.com/corca-ai/semath/issues/999";
    gap.probeIds = ["linear-algebra-development-matvec-01"];
    expect(() => parseStemBreadthManifest(changed)).not.toThrow();
    const fixture = parseAuthoredScientificFixture(
      await json("fixtures/challenge/document-reasoning-development-v1.json"),
    );
    expect(() =>
      validateStemBreadthBenchmark(
        parseStemBreadthManifest(changed),
        fixture,
      ),
    ).toThrow("commissioned gaps require one issue and no probes");
  });

  test("scores each capability at its first authoritative layer", async () => {
    const { manifest } = await inputs();
    const selected = {
      ...manifest,
      fields: manifest.fields.map((field) => ({
        ...field,
        capabilities: field.capabilities.map((cell) => ({
          ...cell,
          probeIds: cell.status === "measured" ? ["probe"] : [],
        })),
      })),
    };
    const record: AuthoredFirstLossRecord = {
      basis: "typed evidence is missing",
      caseId: "probe",
      expectedDecision: "established",
      family: "guarded-condition",
      field: "calculus-analysis",
      reason: "typed-fact-condition-missing",
      split: "development",
      stage: "typed-fact",
    };
    const score = scoreStemBreadth(selected, [record]);
    expect(score.capabilities["document-attachment"]).toEqual({
      cases: 5,
      passed: 5,
    });
    expect(score.capabilities.typing).toEqual({ cases: 5, passed: 0 });
    expect(score.capabilities["decision-quality"]).toEqual({
      cases: 5,
      passed: 0,
    });
  });
});

async function inputs() {
  const rawManifest = await json("fixtures/development/stem-breadth-v1.json");
  return {
    fixture: parseAuthoredScientificFixture(
      await json("fixtures/challenge/document-reasoning-development-v1.json"),
    ),
    manifest: parseStemBreadthManifest(rawManifest),
    rawManifest,
  };
}

async function json(path: string): Promise<unknown> {
  return JSON.parse(await readFile(new URL(path, root), "utf8"));
}

function mutableManifest(value: unknown): {
  fields: {
    capabilities: {
      plannedIssue?: string;
      probeIds: string[];
      status: string;
    }[];
  }[];
} {
  return structuredClone(value) as {
    fields: {
      capabilities: {
        plannedIssue?: string;
        probeIds: string[];
        status: string;
      }[];
    }[];
  };
}
