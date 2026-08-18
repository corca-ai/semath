import { describe, expect, test } from "bun:test";
import { authoredHistoricalReleaseRegressions } from "./authored-historical-release";
import type {
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "./authored-scientific";

const baseline = {
  approvedConservativeDecisionIds: [],
  approvedCursorBoundaryIdentityIds: [],
  approvedFalseEstablishmentIds: ["reviewed-transition"],
  approvedNavigationExpansions: [],
  cases: 2,
  maximumMissedCoverage: 1,
  maximumNavigationOrIdentity: 1,
  maximumRisk: 26,
  minimumPassed: 0,
};

describe("authored historical release policy", () => {
  test("accepts only a reviewed, source-grounded frozen-contract mismatch", () => {
    expect(
      authoredHistoricalReleaseRegressions(
        fixture(),
        [observationFor("reviewed-transition", "established", true)],
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([]);
  });

  test("rejects substitution by an unreviewed or ungrounded establishment", () => {
    expect(
      authoredHistoricalReleaseRegressions(
        fixture(),
        [observationFor("ordinary-miss", "established", false)],
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([
      "unreviewed false establishment ordinary-miss",
      "false establishment ordinary-miss is not source grounded",
    ]);
  });

  test("adjudicates only a reviewed unsupported cursor at a formula boundary", () => {
    const reviewed = probe("reviewed-boundary");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          cursor: { ...reviewed.cursor, edge: "after" as const },
          expected: {
            ...reviewed.expected,
            decision: "unsupported" as const,
          },
        },
      ],
      scenarios: [
        {
          field: "calculus-analysis",
          genre: "test",
          id: "scenario",
          lawIds: [],
          provenance: {
            authorId: "test",
            engineBlind: true,
            independenceGroup: "test",
            rawDigest: "digest",
            taskCardDigest: "digest",
          },
          review: {
            correctionSummary: [],
            criticId: "test",
            finalDigest: "digest",
            frozenAt: "2026-08-13T00:00:00Z",
            mainReviewer: "test",
            reviewedAt: "2026-08-13",
            semanticReviewDigest: "digest",
            status: "corrected" as const,
          },
          snapshots: [
            {
              documents: [{ content: "x", fileId: "main", path: "main" }],
              id: "snapshot",
            },
          ],
          variationTags: [],
        },
      ],
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [
          {
            ...observationFor("reviewed-boundary", "unsupported", false),
            symbol: null,
          },
        ],
        score({ navigationOrIdentity: 1, total: 10 }),
        {
          ...baseline,
          approvedCursorBoundaryIdentityIds: ["reviewed-boundary"],
          cases: 2,
          maximumNavigationOrIdentity: 0,
          maximumRisk: 0,
        },
      ),
    ).toEqual([]);
  });

  test("adjudicates one reviewed conservative proof decision without hiding relation loss", () => {
    const reviewed = probe("reviewed-conservative");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: {
            ...reviewed.expected,
            decision: "established",
            proofGrounded: true,
            relations: [
              {
                anchor: { fileId: "main", needle: "x" },
                relationId: "test:law",
                roles: [],
                sourceGrounded: true,
              },
            ],
          },
        },
      ],
    };
    const reviewedObservation = {
      ...observationFor("reviewed-conservative", "partial", false),
      relations: [
        {
          fileId: "main",
          relationId: "test:law",
          range: { startOffset: 0, endOffset: 1 },
          roles: [],
          sourceGrounded: true,
        },
      ],
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [reviewedObservation],
        score({ missedCoverage: 1, total: 2 }),
        {
          ...baseline,
          approvedConservativeDecisionIds: ["reviewed-conservative"],
          cases: 2,
          maximumMissedCoverage: 0,
          maximumRisk: 0,
        },
      ),
    ).toEqual([]);

    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [{ ...reviewedObservation, relations: [] }],
        score({ missedCoverage: 1, total: 2 }),
        {
          ...baseline,
          approvedConservativeDecisionIds: ["reviewed-conservative"],
          cases: 2,
          maximumMissedCoverage: 0,
          maximumRisk: 0,
        },
      ),
    ).toContain(
      "invalid conservative-decision adjudication reviewed-conservative",
    );
  });

  test("adjudicates only an exact source-grounded navigation expansion", () => {
    const reviewed = probe("reviewed-navigation");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          scenarioId: "reviewed-navigation-scenario",
          cursor: {
            fileId: "main",
            needle: "x",
            occurrence: 2,
            snapshotId: "snapshot",
          },
          expected: {
            ...reviewed.expected,
            decision: "established",
            proofGrounded: true,
          },
        },
      ],
      scenarios: [
        {
          field: "optimization-ml",
          genre: "test",
          id: "reviewed-navigation-scenario",
          lawIds: [],
          provenance: {
            authorId: "test",
            engineBlind: true,
            independenceGroup: "test",
            rawDigest: "digest",
            taskCardDigest: "digest",
          },
          review: {
            correctionSummary: [],
            criticId: "test",
            finalDigest: "digest",
            frozenAt: "2026-08-13T00:00:00Z",
            mainReviewer: "test",
            reviewedAt: "2026-08-13",
            semanticReviewDigest: "digest",
            status: "corrected",
          },
          snapshots: [
            {
              documents: [{ content: "x x x", fileId: "main", path: "main" }],
              id: "snapshot",
            },
          ],
          variationTags: [],
        },
      ],
    };
    const location = (startOffset: number) => ({
      fileId: "main",
      path: "main",
      range: { startOffset, endOffset: startOffset + 1 },
    });
    const observation: AuthoredScientificObservation = {
      ...observationFor("reviewed-navigation", "established", true),
      definitions: [location(0)],
      prepareRename: { placeholder: "x", range: location(4).range },
      references: [location(0), location(2), location(4)],
      symbol: "x",
      symbolLocation: location(4),
    };
    const reviewedBaseline = {
      ...baseline,
      approvedFalseEstablishmentIds: [],
      approvedNavigationExpansions: [
        {
          caseId: "reviewed-navigation",
          definitions: [{ fileId: "main", needle: "x", occurrence: 0 }],
          prepareRename: {
            placeholder: "x",
            range: { fileId: "main", needle: "x", occurrence: 2 },
          },
          references: [
            { fileId: "main", needle: "x", occurrence: 0 },
            { fileId: "main", needle: "x", occurrence: 1 },
            { fileId: "main", needle: "x", occurrence: 2 },
          ],
        },
      ],
      cases: 2,
      maximumMissedCoverage: 1,
      maximumNavigationOrIdentity: 0,
      maximumRisk: 0,
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [observation],
        score({ navigationOrIdentity: 1, total: 10 }),
        reviewedBaseline,
      ),
    ).toEqual([]);
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [{ ...observation, references: observation.references.slice(1) }],
        score({ navigationOrIdentity: 1, total: 10 }),
        reviewedBaseline,
      ),
    ).toContain(
      "invalid source-grounded navigation adjudication reviewed-navigation",
    );
  });
});

function fixture(): AuthoredScientificFixture {
  return {
    batch: {
      createdAt: "2026-08-12",
      id: "historical-test",
      reviewPolicyVersion: 1,
      split: "holdout",
      taskCardDigest: "digest",
    },
    probes: [probe("reviewed-transition"), probe("ordinary-miss")],
    scenarios: [],
    schemaVersion: 1,
  };
}

function probe(id: string): AuthoredScientificFixture["probes"][number] {
  const location = {
    excluded: [],
    minimum: 0,
    required: [],
    status: "unavailable" as const,
  };
  return {
    cursor: { fileId: "main", needle: "x", snapshotId: "snapshot" },
    expected: {
      decision: "partial",
      diagnostics: { excludedCodes: [], maximum: 0, required: [] },
      excludedRelationIds: [],
      navigation: {
        definition: location,
        prepareRename: { status: "unavailable" },
        references: location,
        rename: location,
      },
      proofGrounded: false,
      relations: [],
      symbol: "x",
    },
    family: "edit-lifecycle",
    id,
    kind: "supplemental",
    scenarioId: "scenario",
  };
}

function observationFor(
  caseId: string,
  decision: AuthoredScientificObservation["decision"],
  proofGrounded: boolean,
): AuthoredScientificObservation {
  return {
    caseId,
    decision,
    definitions: [],
    diagnostics: [],
    prepareRename: {},
    proofGrounded,
    references: [],
    relations: [],
    renameEdits: [],
    symbol: "x",
  };
}

function score(
  risk: Partial<AuthoredScientificScorecard["risk"]>,
): AuthoredScientificScorecard {
  return {
    cases: 2,
    failures: [],
    passed: 0,
    risk: {
      falseConflict: 0,
      falseEstablishment: 0,
      missedCoverage: 1,
      navigationOrIdentity: 1,
      total: 0,
      ...risk,
    },
  };
}
