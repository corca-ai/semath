import { describe, expect, test } from "bun:test";
import {
  parseSpentHoldoutRegistry,
  validateSpentHoldoutIsolation,
  type SpentHoldoutLineage,
} from "./spent-holdout";

const digest = (character: string): string => character.repeat(64);

const registryValue = {
  entries: [
    {
      lineage: {
        batchId: "spent-batch",
        probeIds: ["spent-probe"],
        profiles: [
          {
            documentSha256: [digest("1")],
            id: "spent-scenario",
            mathFingerprintSha256: [digest("2")],
            proseShingleSha256: [digest("3"), digest("4")],
          },
          {
            documentSha256: [digest("1")],
            id: "spent-scenario/current/test.md",
            mathFingerprintSha256: [digest("2")],
            proseShingleSha256: [digest("3"), digest("4")],
          },
        ],
        rawDigests: [digest("5")],
        releaseId: "v0.38",
        scenarioIds: ["spent-scenario"],
      },
      outcome: {
        cases: 1,
        falseConflict: 0,
        falseEstablishment: 1,
        mathAuthoringExact: 0,
        missedCoverage: 1,
        navigationOrIdentity: 1,
        passed: 1,
        risk: 488,
        safetyFailures: 1,
      },
      terminal: {
        artifactId: "9440104778",
        candidateCommit: "a".repeat(40),
        candidateTree: "b".repeat(40),
        evaluationSha256: digest("6"),
        fixtureSha256: digest("7"),
        runId: "32462617124",
      },
    },
  ],
  profileAlgorithm: {
    digest: "sha256",
    document: "utf8-exact-content-v1",
    math: "wasmtex-authored-math-fingerprint-v1",
    prose: "wasmtex-visible-prose-5-shingle-v1",
  },
  schemaVersion: 1,
} as const;

const candidate: SpentHoldoutLineage = {
  batchId: "fresh-batch",
  probeIds: ["fresh-probe"],
  profiles: [
    {
      documentSha256: [digest("8")],
      id: "fresh-profile",
      mathFingerprintSha256: [digest("9")],
      proseShingleSha256: [digest("a"), digest("b")],
    },
  ],
  rawDigests: [digest("c")],
  releaseId: "v0.40",
  scenarioIds: ["fresh-scenario"],
};

describe("spent holdout registry", () => {
  test("strictly parses immutable terminal metadata and hashed lineage", () => {
    const registry = parseSpentHoldoutRegistry(registryValue);
    expect(registry.entries[0]?.lineage.releaseId).toBe("v0.38");
    expect(validateSpentHoldoutIsolation(registry, candidate)).toEqual({
      comparedProfiles: 2,
      maximumMathSimilarity: 0,
      maximumProseSimilarity: 0,
      spentReleases: 1,
    });
  });

  test("rejects unknown fields and malformed or noncanonical hashes", () => {
    expect(() =>
      parseSpentHoldoutRegistry({ ...registryValue, surprise: true }),
    ).toThrow("exact keys");
    expect(() =>
      parseSpentHoldoutRegistry({
        ...registryValue,
        entries: [
          {
            ...registryValue.entries[0],
            lineage: {
              ...registryValue.entries[0]!.lineage,
              rawDigests: ["not-a-digest"],
            },
          },
        ],
      }),
    ).toThrow("SHA-256");
    expect(() =>
      parseSpentHoldoutRegistry({
        ...registryValue,
        entries: [
          {
            ...registryValue.entries[0],
            lineage: {
              ...registryValue.entries[0]!.lineage,
              scenarioIds: ["z", "a"],
            },
          },
        ],
      }),
    ).toThrow("must be sorted");
    expect(() =>
      parseSpentHoldoutRegistry({
        ...registryValue,
        entries: [
          {
            ...registryValue.entries[0],
            lineage: {
              ...registryValue.entries[0]!.lineage,
              releaseId: "release-38",
            },
          },
        ],
      }),
    ).toThrow("semantic release id");
    expect(() =>
      parseSpentHoldoutRegistry({
        ...registryValue,
        entries: [
          {
            ...registryValue.entries[0],
            outcome: { ...registryValue.entries[0]!.outcome, passed: 49 },
          },
        ],
      }),
    ).toThrow("must not exceed cases");
  });

  test("rejects truncated, orphaned, or inconsistent lineage profiles", () => {
    const entry = registryValue.entries[0]!;
    const child = entry.lineage.profiles[1]!;
    for (const profiles of [
      [entry.lineage.profiles[0]!],
      [
        entry.lineage.profiles[0]!,
        { ...child, documentSha256: [] },
      ],
      [
        entry.lineage.profiles[0]!,
        { ...child, id: "orphan/current/test.md" },
      ],
      [
        {
          ...entry.lineage.profiles[0]!,
          proseShingleSha256: [digest("3")],
        },
        child,
      ],
    ]) {
      expect(() =>
        parseSpentHoldoutRegistry({
          ...registryValue,
          entries: [
            {
              ...entry,
              lineage: { ...entry.lineage, profiles },
            },
          ],
        }),
      ).toThrow();
    }
  });

  test("fails closed on exact identifiers, digests, and documents", () => {
    const registry = parseSpentHoldoutRegistry(registryValue);
    for (const drift of [
      { releaseId: "v0.38" },
      { batchId: "spent-batch" },
      { scenarioIds: ["spent-scenario"] },
      { probeIds: ["spent-probe"] },
      { rawDigests: [digest("5")] },
      {
        profiles: [
          { ...candidate.profiles[0]!, documentSha256: [digest("1")] },
        ],
      },
    ]) {
      expect(() =>
        validateSpentHoldoutIsolation(registry, { ...candidate, ...drift }),
      ).toThrow("reuses spent");
    }
  });

  test("rejects copied prose and exact math combined with related prose", () => {
    const registry = parseSpentHoldoutRegistry(registryValue);
    expect(() =>
      validateSpentHoldoutIsolation(registry, {
        ...candidate,
        profiles: [
          {
            ...candidate.profiles[0]!,
            proseShingleSha256: [digest("3"), digest("a"), digest("b")],
          },
        ],
      }),
    ).not.toThrow();
    expect(() =>
      validateSpentHoldoutIsolation(registry, {
        ...candidate,
        profiles: [
          {
            ...candidate.profiles[0]!,
            proseShingleSha256: [digest("3")],
          },
        ],
      }),
    ).toThrow("spent-lineage similarity");
    expect(() =>
      validateSpentHoldoutIsolation(registry, {
        ...candidate,
        profiles: [
          {
            ...candidate.profiles[0]!,
            mathFingerprintSha256: [digest("2")],
            proseShingleSha256: [
              digest("3"),
              digest("a"),
              digest("b"),
              digest("c"),
            ],
          },
        ],
      }),
    ).not.toThrow();
    expect(() =>
      validateSpentHoldoutIsolation(registry, {
        ...candidate,
        profiles: [
          {
            ...candidate.profiles[0]!,
            mathFingerprintSha256: [digest("2")],
            proseShingleSha256: [digest("3"), digest("a"), digest("b")],
          },
        ],
      }),
    ).toThrow("spent-lineage similarity");
  });
});
