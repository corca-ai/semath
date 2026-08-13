import { createHash } from "node:crypto";
import { describe, expect, test } from "bun:test";
import {
  authoredFixtureSealPayload,
  authoredScenarioReviewPayload,
  parseAuthoredScientificFixture,
  type AuthoredScientificObservation,
} from "./authored-scientific";
import {
  freshBlindSafetySummary,
  freshBlindSealPayload,
  parseFreshBlindReleaseFixture,
  planFreshBlindSnapshotTransitions,
  validateFreshBlindProfileIsolation,
  validateFreshBlindRelease,
} from "./fresh-blind-release";

describe("fresh blind release evidence", () => {
  test("validates 48 independently commissioned cases without running an engine", () => {
    const release = fixture();
    const summary = validateFreshBlindRelease(release, validation(release));
    expect(summary.scenarios).toBe(48);
    expect(summary.families).toEqual({
      "collision-unsupported": 8,
      "derivation-chain": 8,
      "discourse-reference": 8,
      "edit-lifecycle": 8,
      "guarded-condition": 8,
      "scope-comparison": 8,
    });
  });

  test("reuses the sealed evidence contract across semantic release cycles", () => {
    const next = fixtureValue();
    next.release.id = "v0.29";
    const release = finalize(next);
    expect(validateFreshBlindRelease(release, validation(release)).scenarios).toBe(48);

    const invalid = fixtureValue();
    invalid.release.id = "release-29";
    const unversioned = finalize(invalid);
    expect(() =>
      validateFreshBlindRelease(unversioned, validation(unversioned)),
    ).toThrow("expected a semantic release id");
  });

  test("requires isolated Codex authors, critics, and the complete main review", () => {
    const value = fixtureValue();
    value.fixture.scenarios[0]!.review.criticId = "main-codex";
    const release = finalize(value);
    expect(() =>
      validateFreshBlindRelease(release, validation(release)),
    ).toThrow("author, critic, and main reviewer must be independent");

    const sameWorker = fixtureValue();
    sameWorker.fixture.scenarios[0]!.review.criticId =
      sameWorker.fixture.scenarios[0]!.provenance.authorId;
    expect(() => finalize(sameWorker)).toThrow("critic must be independent");
  });

  test("requires exact reviewed evidence for every available rename", () => {
    const value = fixtureValue();
    const probe = value.fixture.probes[0]!;
    const expected = probe.expected as {
      navigation: { rename: Record<string, unknown> };
    };
    expected.navigation.rename = {
      excluded: [],
      minimum: 1,
      required: [{ fileId: "main", needle: "$x_0=1$" }],
      status: "available",
    };
    const release = finalize(value);
    expect(() =>
      validateFreshBlindRelease(release, validation(release)),
    ).toThrow(
      "available rename requires exact source, replacement, and safety evidence",
    );
  });

  test("rejects exact evidence reuse and suspicious prose lineage", () => {
    const release = fixture();
    const input = validation(release);
    expect(() =>
      validateFreshBlindRelease(release, {
        ...input,
        referenceDocuments: [
          release.fixture.scenarios[0]!.snapshots[0]!.documents[0]!.content,
        ],
      }),
    ).toThrow("duplicates existing evidence");

    expect(() =>
      validateFreshBlindProfileIsolation(
        [
          {
            id: "known",
            mathFingerprints: ["m"],
            proseShingles: ["a", "b"],
          },
        ],
        [
          {
            id: "fresh",
            mathFingerprints: ["m"],
            proseShingles: ["a", "b"],
          },
        ],
      ),
    ).toThrow("lineage similarity requires review");
  });

  test("separates safety failures from honest blind coverage misses", () => {
    const release = fixture();
    const probe = release.fixture.probes.find(
      (candidate) => candidate.expected.decision === "unsupported",
    )!;
    const observation: AuthoredScientificObservation = {
      caseId: probe.id,
      decision: "established",
      definitions: [
        {
          fileId: "main",
          path: "main.md",
          range: { startOffset: 0, endOffset: 1 },
        },
      ],
      diagnostics: [],
      prepareRename: { range: { startOffset: 0, endOffset: 1 } },
      proofGrounded: false,
      references: [],
      relations: [],
      renameEdits: [
        {
          expectedText: "x",
          fileId: "main",
          path: "main.md",
          range: { startOffset: 0, endOffset: 1 },
          replacementText: "y",
        },
      ],
      symbol: null,
    };
    expect(freshBlindSafetySummary(release.fixture, [observation])).toEqual({
      falseConflict: 0,
      falseConflictIds: [],
      falseEstablishment: 1,
      falseEstablishmentIds: [probe.id],
      unsafeNavigationOrEdit: 3,
      unsafeNavigationOrEditIds: [probe.id],
    });
    expect(
      freshBlindSafetySummary(release.fixture, [
        {
          ...observation,
          decision: "conflicting",
          definitions: [],
          prepareRename: {},
          renameEdits: [],
        },
      ]),
    ).toEqual({
      falseConflict: 1,
      falseConflictIds: [probe.id],
      falseEstablishment: 0,
      falseEstablishmentIds: [],
      unsafeNavigationOrEdit: 0,
      unsafeNavigationOrEditIds: [],
    });
  });

  test("plans only actual ordered snapshot transitions", () => {
    const value = fixtureValue();
    const original = value.fixture.scenarios[0]!.snapshots[0]!;
    value.fixture.scenarios[0]!.snapshots.push({
      ...structuredClone(original),
      id: "edited",
    });
    const release = finalize(value);
    expect(planFreshBlindSnapshotTransitions(release.fixture)).toEqual([
      {
        fromSnapshotId: "initial",
        scenarioId: "fresh-00",
        toSnapshotId: "edited",
      },
    ]);
  });
});

function fixture() {
  return finalize(fixtureValue());
}

function finalize(value: FixtureValue) {
  let authored = parseAuthoredScientificFixture(value.fixture);
  for (const scenario of value.fixture.scenarios) {
    const digest = sha256(authoredScenarioReviewPayload(authored, scenario.id));
    scenario.review.finalDigest = digest;
    scenario.review.semanticReviewDigest = digest;
  }
  authored = parseAuthoredScientificFixture(value.fixture);
  value.fixture.batch.seal = sha256(authoredFixtureSealPayload(authored));
  const provisional = parseFreshBlindReleaseFixture(value);
  value.release.seal = sha256(freshBlindSealPayload(provisional));
  return parseFreshBlindReleaseFixture(value);
}

function validation(release: ReturnType<typeof fixture>) {
  const reviewDigests = Object.fromEntries(
    release.fixture.scenarios.map((scenario) => [
      scenario.id,
      sha256(authoredScenarioReviewPayload(release.fixture, scenario.id)),
    ]),
  );
  return {
    authoredSealDigest: sha256(authoredFixtureSealPayload(release.fixture)),
    freshIsolationProfiles: release.fixture.scenarios.map((scenario) => ({
      id: `${scenario.id}/initial/main`,
      mathFingerprints: [`math-${scenario.id}`],
      proseShingles: [`prose-${scenario.id}`],
    })),
    freshProfiles: release.fixture.scenarios.map((scenario) => ({
      id: scenario.id,
      mathFingerprints: [`math-${scenario.id}`],
      proseShingles: [`prose-${scenario.id}`],
    })),
    lawCatalog: [
      {
        field: "cross-field",
        lawId: "test:law",
        roles: [],
      },
    ],
    referenceDocuments: [],
    referenceProfiles: [],
    reviewDigests,
    sealDigest: sha256(freshBlindSealPayload(release)),
  };
}

function fixtureValue(): FixtureValue {
  const taskCardDigest = "c".repeat(64);
  const families = [
    "scope-comparison",
    "derivation-chain",
    "guarded-condition",
    "discourse-reference",
    "collision-unsupported",
    "edit-lifecycle",
  ] as const;
  const decisions = [
    "established",
    "partial",
    "ambiguous",
    "conflicting",
    "unsupported",
  ] as const;
  const scenarios = Array.from({ length: 48 }, (_, index) => ({
    field: "cross-field",
    genre: "methods note",
    id: `fresh-${String(index).padStart(2, "0")}`,
    lawIds: ["test:law"],
    provenance: {
      authorId: `author-${index}`,
      engineBlind: true,
      independenceGroup: `group-${index}`,
      rawDigest: "a".repeat(64),
      taskCardDigest,
    },
    review: {
      correctionSummary: [],
      criticId: `critic-${index}`,
      finalDigest: "d".repeat(64),
      frozenAt: "2026-08-13T00:00:00Z",
      mainReviewer: "main-codex",
      reviewedAt: "2026-08-13",
      semanticReviewDigest: "d".repeat(64),
      status: "approved",
    },
    snapshots: [
      {
        documents: [
          {
            content: `Independent scientific scene ${index}. The reviewed value is $x_${index}=1$.`,
            fileId: "main",
            path: "main.md",
          },
        ],
        id: "initial",
      },
    ],
    variationTags: ["independent-prose", `case-${index}`],
  }));
  const unavailable = () => ({
    excluded: [],
    minimum: 0,
    required: [],
    status: "unavailable",
  });
  return {
    commissioning: {
      authoringMethod: "isolated-codex-subagents",
      criticMethod: "independent-codex-subagents",
      engineExecutionsBeforeSeal: 0,
      mainReviewMethod: "complete-source-and-expectation-review",
      mainReviewerId: "main-codex",
    },
    fixture: {
      batch: {
        createdAt: "2026-08-13",
        frozenAt: "2026-08-13T00:00:00Z",
        id: "v028-fresh-blind",
        reviewPolicyVersion: 2,
        seal: "b".repeat(64),
        split: "holdout",
        taskCardDigest,
      },
      probes: scenarios.map((scenario, index) => ({
        cursor: {
          edge: "before",
          fileId: "main",
          needle: `$x_${index}=1$`,
          snapshotId: "initial",
        },
        expected: {
          decision: decisions[index % decisions.length]!,
          diagnostics: { excludedCodes: [], maximum: 0, required: [] },
          excludedRelationIds: [],
          navigation: {
            definition: unavailable(),
            prepareRename: { status: "unavailable" },
            references: unavailable(),
            rename: unavailable(),
          },
          proofGrounded: false,
          relations: [],
        },
        family: families[Math.floor(index / 8)]!,
        id: `probe-${String(index).padStart(2, "0")}`,
        kind: "primary",
        scenarioId: scenario.id,
      })),
      scenarios,
      schemaVersion: 1,
    },
    release: {
      createdAt: "2026-08-13",
      frozenAt: "2026-08-13T00:00:00Z",
      id: "v0.28",
      seal: "e".repeat(64),
      taskCardDigest,
    },
    schemaVersion: 1,
  };
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

type FixtureValue = ReturnType<typeof fixtureValueShape>;

function fixtureValueShape() {
  return {} as {
    commissioning: {
      authoringMethod: "isolated-codex-subagents";
      criticMethod: "independent-codex-subagents";
      engineExecutionsBeforeSeal: 0;
      mainReviewMethod: "complete-source-and-expectation-review";
      mainReviewerId: string;
    };
    fixture: {
      batch: Record<string, unknown> & { seal: string };
      probes: Record<string, unknown>[];
      scenarios: Array<{
        id: string;
        provenance: {
          authorId: string;
        };
        review: {
          criticId: string;
          finalDigest: string;
          semanticReviewDigest: string;
        };
        snapshots: Array<{
          documents: Array<{ content: string; fileId: string; path: string }>;
          id: string;
        }>;
      }>;
      schemaVersion: 1;
    };
    release: {
      createdAt: string;
      frozenAt: string;
      id: string;
      seal: string;
      taskCardDigest: string;
    };
    schemaVersion: 1;
  };
}
