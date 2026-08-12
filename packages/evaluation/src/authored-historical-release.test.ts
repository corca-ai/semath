import { describe, expect, test } from "bun:test";
import { authoredHistoricalReleaseRegressions } from "./authored-historical-release";
import type {
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "./authored-scientific";

const baseline = {
  approvedFalseEstablishmentIds: ["reviewed-transition"],
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
        [observation("reviewed-transition", "established", true)],
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([]);
  });

  test("rejects substitution by an unreviewed or ungrounded establishment", () => {
    expect(
      authoredHistoricalReleaseRegressions(
        fixture(),
        [observation("ordinary-miss", "established", false)],
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([
      "unreviewed false establishment ordinary-miss",
      "false establishment ordinary-miss is not source grounded",
    ]);
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

function observation(
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
