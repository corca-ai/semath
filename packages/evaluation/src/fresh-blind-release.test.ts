import { createHash } from "node:crypto";
import { describe, expect, test } from "bun:test";
import {
  authoredFixtureSealPayload,
  authoredScenarioReviewPayload,
  parseAuthoredScientificFixture,
  type AuthoredScientificObservation,
} from "./authored-scientific";
import {
  freshBlindSafetyGateFailed,
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
      diagnosticsOverLimit: 0,
      diagnosticsOverLimitIds: [],
      falseConflict: 0,
      falseConflictIds: [],
      falseEstablishment: 1,
      falseEstablishmentIds: [probe.id],
      unsafeNavigationOrEditCaseIds: [probe.id],
      unsafeNavigationOrEditLocations: 3,
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
      diagnosticsOverLimit: 0,
      diagnosticsOverLimitIds: [],
      falseConflict: 1,
      falseConflictIds: [probe.id],
      falseEstablishment: 0,
      falseEstablishmentIds: [],
      unsafeNavigationOrEditCaseIds: [],
      unsafeNavigationOrEditLocations: 0,
    });
  });

  test("keeps case counts aligned with ids and gates warning diagnostics over the reviewed limit", () => {
    const release = fixture();
    const establishmentProbe = release.fixture.probes.find(
      (probe) => probe.expected.decision === "unsupported",
    )!;
    const conflictProbe = release.fixture.probes.find(
      (probe) => probe.expected.decision === "partial",
    )!;
    const diagnosticProbe = release.fixture.probes.find(
      (probe) => probe.expected.decision === "ambiguous",
    )!;
    const proofProbe = release.fixture.probes.find(
      (probe) =>
        probe.id !== establishmentProbe.id &&
        probe.id !== conflictProbe.id &&
        probe.id !== diagnosticProbe.id,
    )!;
    const hintProbe = release.fixture.probes.find(
      (probe) =>
        ![
          establishmentProbe.id,
          conflictProbe.id,
          diagnosticProbe.id,
          proofProbe.id,
        ].includes(probe.id),
    )!;
    const observation = (
      probe: (typeof release.fixture.probes)[number],
    ): AuthoredScientificObservation => ({
      caseId: probe.id,
      decision: probe.expected.decision,
      definitions: [],
      diagnostics: [],
      prepareRename: {},
      proofGrounded: false,
      references: [],
      relations: [],
      renameEdits: [],
      symbol: null,
    });
    const summary = freshBlindSafetySummary(release.fixture, [
      { ...observation(establishmentProbe), decision: "established" },
      { ...observation(conflictProbe), decision: "conflicting" },
      {
        ...observation(diagnosticProbe),
        diagnostics: [
          {
            code: "review-limit",
            fileId: "main",
            range: { startOffset: 0, endOffset: 1 },
            severity: "warning",
          },
        ],
      },
      { ...observation(proofProbe), proofGrounded: true },
    ]);

    expect(summary.falseEstablishment).toBe(
      summary.falseEstablishmentIds.length,
    );
    expect(summary.falseConflict).toBe(summary.falseConflictIds.length);
    expect(summary.diagnosticsOverLimit).toBe(
      summary.diagnosticsOverLimitIds.length,
    );
    expect(summary.falseEstablishmentIds).toEqual(
      [establishmentProbe.id, proofProbe.id].sort(),
    );
    expect(summary.diagnosticsOverLimitIds).toEqual([diagnosticProbe.id]);
    expect(freshBlindSafetyGateFailed(summary)).toBe(true);

    const hintOnly = freshBlindSafetySummary(release.fixture, [
      {
        ...observation(hintProbe),
        diagnostics: [
          {
            code: "informational",
            fileId: "main",
            range: { startOffset: 0, endOffset: 1 },
            severity: "hint",
          },
        ],
      },
    ]);
    expect(hintOnly.diagnosticsOverLimit).toBe(0);
    expect(freshBlindSafetyGateFailed(hintOnly)).toBe(false);
  });

  test("reports unsafe location count separately from affected case ids", () => {
    const release = fixture();
    const probe = release.fixture.probes[0]!;
    const location = (startOffset: number) => ({
      fileId: "main",
      path: "main.md",
      range: { startOffset, endOffset: startOffset + 1 },
    });
    const summary = freshBlindSafetySummary(release.fixture, [
      {
        caseId: probe.id,
        decision: probe.expected.decision,
        definitions: [location(0), location(1)],
        diagnostics: [],
        prepareRename: { range: location(4).range },
        proofGrounded: false,
        references: [location(2), location(3)],
        relations: [],
        renameEdits: [
          {
            ...location(5),
            expectedText: "x",
            replacementText: "y",
          },
          {
            ...location(6),
            expectedText: "x",
            replacementText: "y",
          },
        ],
        symbol: null,
      },
    ]);

    expect(summary.unsafeNavigationOrEditLocations).toBe(7);
    expect(summary.unsafeNavigationOrEditCaseIds).toEqual([probe.id]);
  });

  test("rejects every available navigation or edit outside its exact allowlist", () => {
    const release = fixture();
    const probe = release.fixture.probes[0]!;
    const scenario = release.fixture.scenarios[0]!;
    const document = scenario.snapshots[0]!.documents[0]!;
    const needle = "$x_0=1$";
    const startOffset = document.content.indexOf(needle);
    const range = {
      startOffset,
      endOffset: startOffset + needle.length,
    };
    const anchor = { fileId: document.fileId, needle };
    const expected = probe.expected.navigation as unknown as {
      definition: Record<string, unknown>;
      references: Record<string, unknown>;
      rename: Record<string, unknown>;
    };
    expected.definition = {
      excluded: [],
      minimum: 1,
      required: [anchor],
      status: "available",
    };
    expected.references = {
      excluded: [],
      minimum: 1,
      required: [anchor],
      status: "available",
    };
    expected.rename = {
      excluded: [],
      expectedText: needle,
      minimum: 1,
      newName: "y",
      replacementText: "y",
      required: [anchor],
      safety: "reviewed exact notation",
      status: "available",
    };
    const location = (candidate: { startOffset: number; endOffset: number }) => ({
      fileId: document.fileId,
      path: document.path,
      range: candidate,
    });
    const unexpectedRange = {
      startOffset: range.startOffset + 1,
      endOffset: range.endOffset,
    };
    const summary = freshBlindSafetySummary(release.fixture, [
      {
        caseId: probe.id,
        decision: probe.expected.decision,
        definitions: [location(range), location(unexpectedRange)],
        diagnostics: [],
        prepareRename: {},
        proofGrounded: probe.expected.proofGrounded,
        references: [location(range), location(unexpectedRange)],
        relations: [],
        renameEdits: [
          {
            ...location(range),
            expectedText: needle,
            replacementText: "y",
          },
          {
            ...location(unexpectedRange),
            expectedText: needle,
            replacementText: "y",
          },
        ],
        renameSafety: "reviewed exact notation",
        symbol: null,
      },
    ]);

    expect(summary.unsafeNavigationOrEditLocations).toBe(3);
    expect(summary.unsafeNavigationOrEditCaseIds).toEqual([probe.id]);
  });

  test("gates each reviewed safety category and accepts a clean summary", () => {
    const clean = {
      diagnosticsOverLimit: 0,
      diagnosticsOverLimitIds: [],
      falseConflict: 0,
      falseConflictIds: [],
      falseEstablishment: 0,
      falseEstablishmentIds: [],
      unsafeNavigationOrEditCaseIds: [],
      unsafeNavigationOrEditLocations: 0,
    };
    expect(freshBlindSafetyGateFailed(clean)).toBe(false);

    for (const unsafe of [
      {
        ...clean,
        diagnosticsOverLimit: 1,
        diagnosticsOverLimitIds: ["diagnostic-case"],
      },
      {
        ...clean,
        falseConflict: 1,
        falseConflictIds: ["conflict-case"],
      },
      {
        ...clean,
        falseEstablishment: 1,
        falseEstablishmentIds: ["establishment-case"],
      },
      {
        ...clean,
        unsafeNavigationOrEditCaseIds: ["navigation-case"],
        unsafeNavigationOrEditLocations: 2,
      },
    ]) {
      expect(freshBlindSafetyGateFailed(unsafe)).toBe(true);
    }
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
