import { describe, expect, test } from "bun:test";
import type {
  MathAuthoringContext,
  MathInterpretationHypothesisInfo,
} from "../../protocol/src/index";
import { authoredHistoricalReleaseRegressions } from "./authored-historical-release";
import type { AuthoredHistoricalReleaseBaseline } from "./authored-historical-release";
import {
  authoredFalseEstablishmentCases,
  scoreAuthoredScientificFixture,
} from "./authored-scientific";
import type {
  AuthoredScientificFixture,
  AuthoredScientificObservation,
  AuthoredScientificScorecard,
} from "./authored-scientific";

const baseline: AuthoredHistoricalReleaseBaseline = {
  approvedConservativeDecisionIds: [],
  approvedCursorBoundaryIdentityIds: [],
  approvedFalseEstablishments: [
    { caseId: "reviewed-transition", causes: ["decision"] },
  ],
  approvedFormulaDecisionDomainIds: [],
  approvedSourceGroundedNavigationRecoveries: [],
  cases: 2,
  maximumMissedCoverage: 1,
  maximumNavigationOrIdentity: 1,
  maximumRisk: 26,
  minimumPassed: 0,
};

describe("authored historical release policy", () => {
  test("accepts only a reviewed, source-grounded frozen-contract mismatch", () => {
    const reviewed = probe("reviewed-transition");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: { ...reviewed.expected, proofGrounded: true },
        },
      ],
      scenarios: [scenario()],
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
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

  test("rejects cause substitution for a reviewed false establishment", () => {
    const reviewed = probe("reviewed-transition");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: {
            ...reviewed.expected,
            excludedRelationIds: ["test:excluded-law"],
          },
        },
        probe("ordinary-miss"),
      ],
    };
    const reviewedObservation: AuthoredScientificObservation = {
      ...observation("reviewed-transition", "partial", false),
      relations: [
        {
          fileId: "main",
          relationId: "test:excluded-law",
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
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([
      "false establishment reviewed-transition causes excluded-relation differ from approved decision",
    ]);
  });

  test("rejects an extra relation leak beside an approved decision cause", () => {
    const reviewed = probe("reviewed-transition");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: {
            ...reviewed.expected,
            excludedRelationIds: ["test:excluded-law"],
            proofGrounded: true,
          },
        },
      ],
      scenarios: [scenario()],
    };
    const reviewedObservation: AuthoredScientificObservation = {
      ...observation("reviewed-transition", "established", true),
      relations: [
        {
          fileId: "main",
          relationId: "test:excluded-law",
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
        score({ falseEstablishment: 1, total: 26 }),
        baseline,
      ),
    ).toEqual([
      "false establishment reviewed-transition causes decision,excluded-relation differ from approved decision",
    ]);
  });

  test("classifies every false-establishment cause from one shared contract", () => {
    const reviewed = probe("reviewed-transition");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: {
            ...reviewed.expected,
            excludedRelationIds: ["test:excluded-law"],
            relations: [
              {
                anchor: { fileId: "main", needle: "x" },
                relationId: "test:expected-law",
                roles: [],
                sourceGrounded: false,
              },
            ],
          },
        },
      ],
      scenarios: [scenario()],
    };
    const reviewedObservation: AuthoredScientificObservation = {
      ...observation("reviewed-transition", "established", true),
      relations: [
        {
          fileId: "main",
          relationId: "test:expected-law",
          range: { startOffset: 0, endOffset: 1 },
          roles: [],
          sourceGrounded: true,
        },
        {
          fileId: "main",
          relationId: "test:excluded-law",
          range: { startOffset: 0, endOffset: 1 },
          roles: [],
          sourceGrounded: true,
        },
      ],
    };

    expect(
      authoredFalseEstablishmentCases(reviewedFixture, [reviewedObservation]),
    ).toEqual([
      {
        caseId: "reviewed-transition",
        causes: [
          "decision",
          "proof-grounding",
          "relation-grounding",
          "excluded-relation",
        ],
        sourceGrounded: true,
      },
    ]);
  });

  test("keeps an independent coverage miss beside a relation leak", () => {
    const reviewed = probe("reviewed-transition");
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          expected: {
            ...reviewed.expected,
            excludedRelationIds: ["test:excluded-law"],
          },
        },
      ],
      scenarios: [scenario()],
    };
    const reviewedObservation: AuthoredScientificObservation = {
      ...observation("reviewed-transition", "unsupported", false),
      relations: [
        {
          fileId: "main",
          relationId: "test:excluded-law",
          range: { startOffset: 0, endOffset: 1 },
          roles: [],
          sourceGrounded: true,
        },
      ],
    };

    expect(
      scoreAuthoredScientificFixture(reviewedFixture, [reviewedObservation]).risk,
    ).toEqual({
      falseConflict: 0,
      falseEstablishment: 1,
      missedCoverage: 1,
      navigationOrIdentity: 0,
      total: 14,
    });
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
        [{ ...observation("reviewed-boundary", "unsupported", false), symbol: null }],
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
      scenarios: [scenario()],
    };
    const reviewedObservation = {
      ...observation("reviewed-conservative", "partial", false),
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

  test("adjudicates a legacy cursor decision only from exact establishment-grade formula evidence", () => {
    const reviewed = probe("reviewed-formula-domain");
    const reviewedProbe = {
      ...reviewed,
      expected: {
        ...reviewed.expected,
        decision: "established" as const,
        proofGrounded: true,
        relations: [
          {
            anchor: { fileId: "main", needle: "x" },
            relationId: "test:law",
            roles: [{ role: "value", symbol: "x" }],
            sourceGrounded: true,
          },
        ],
      },
    };
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [reviewedProbe],
      scenarios: [scenario()],
    };
    const reviewedObservation = formulaDomainObservation();
    const reviewedBaseline: AuthoredHistoricalReleaseBaseline = {
      ...baseline,
      approvedFalseEstablishments: [],
      approvedFormulaDecisionDomainIds: ["reviewed-formula-domain"],
      cases: 1,
      maximumMissedCoverage: 0,
      maximumNavigationOrIdentity: 0,
      maximumRisk: 0,
      minimumPassed: 1,
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [reviewedObservation],
        scoreAuthoredScientificFixture(reviewedFixture, [reviewedObservation]),
        reviewedBaseline,
      ),
    ).toEqual([]);

    const context = reviewedObservation.authoringContext!;
    const hypothesis = context.interpretations.hypotheses[0]!;
    const assertedObservation: AuthoredScientificObservation = {
      ...reviewedObservation,
      authoringContext: {
        ...context,
        interpretations: {
          ...context.interpretations,
          hypotheses: [
            {
              ...hypothesis,
              bindings: hypothesis.bindings.map((binding) => ({
                ...binding,
                proof: "asserted" as const,
              })),
            },
          ],
        },
      },
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [assertedObservation],
        scoreAuthoredScientificFixture(reviewedFixture, [assertedObservation]),
        reviewedBaseline,
      ),
    ).toContain(
      "invalid formula-decision-domain adjudication reviewed-formula-domain",
    );
  });

  test("adjudicates only an exact source-grounded navigation recovery", () => {
    const reviewed = probe("reviewed-navigation");
    const content = "Define $x$ here; use $x$ there.";
    const reviewedFixture: AuthoredScientificFixture = {
      ...fixture(),
      probes: [
        {
          ...reviewed,
          cursor: {
            ...reviewed.cursor,
            needle: "$x$ there",
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
          ...scenario(),
          snapshots: [
            {
              documents: [{ content, fileId: "main", path: "main" }],
              id: "snapshot",
            },
          ],
        },
      ],
    };
    const definition = location(8, 9);
    const use = location(22, 23);
    const recovery = {
      caseId: "reviewed-navigation",
      definition: {
        fileId: "main",
        needle: "x",
        occurrence: 0,
      },
      references: [
        { fileId: "main", needle: "x", occurrence: 0 },
        { fileId: "main", needle: "x", occurrence: 1 },
      ],
      symbol: "x",
      symbolOccurrence: {
        fileId: "main",
        needle: "x",
        occurrence: 1,
      },
    } as const;
    const reviewedObservation: AuthoredScientificObservation = {
      ...observation("reviewed-navigation", "established", true),
      definitions: [definition],
      prepareRename: { placeholder: "x", range: use.range },
      references: [definition, use],
      symbolLocation: use,
    };
    const reviewedBaseline = {
      ...baseline,
      approvedSourceGroundedNavigationRecoveries: [recovery],
      cases: 2,
      maximumNavigationOrIdentity: 0,
      maximumRisk: 0,
    };
    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [reviewedObservation],
        score({ navigationOrIdentity: 1, total: 10 }),
        reviewedBaseline,
      ),
    ).toEqual([]);

    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [{ ...reviewedObservation, references: [use, definition] }],
        score({ navigationOrIdentity: 1, total: 10 }),
        reviewedBaseline,
      ),
    ).toContain(
      "invalid source-grounded navigation adjudication reviewed-navigation",
    );

    expect(
      authoredHistoricalReleaseRegressions(
        reviewedFixture,
        [reviewedObservation],
        score({ navigationOrIdentity: 1, total: 10 }),
        {
          ...reviewedBaseline,
          approvedSourceGroundedNavigationRecoveries: [recovery, recovery],
        },
      ),
    ).toContain(
      "duplicate source-grounded navigation adjudication reviewed-navigation",
    );
  });
});

function scenario(): AuthoredScientificFixture["scenarios"][number] {
  return {
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
      status: "corrected",
    },
    snapshots: [
      {
        documents: [{ content: "x", fileId: "main", path: "main" }],
        id: "snapshot",
      },
    ],
    variationTags: [],
  };
}

function location(startOffset: number, endOffset: number) {
  return {
    fileId: "main",
    path: "main",
    range: { startOffset, endOffset },
  };
}

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

function formulaDomainObservation(): AuthoredScientificObservation {
  const formula = {
    documentVersion: 1,
    location: location(0, 1),
    scopePath: [] as readonly number[],
    sourceNotation: "x",
  };
  const evidence = {
    kind: "canonical-math" as const,
    ruleId: "test/evidence",
    sourceRanges: [{ startOffset: 0, endOffset: 1 }],
    strength: "hard" as const,
  };
  const hypothesis: MathInterpretationHypothesisInfo = {
    bindings: [
      {
        constraint: { kind: "scalar" },
        evidence,
        parameter: "value",
        proof: "typed",
        symbol: "x",
      },
    ],
    conditions: [],
    documentVersion: 1,
    evidence: [
      {
        evidence,
        provenance: "typed-structure",
        role: "supporting",
        sourceAnchors: [
          {
            documentVersion: 1,
            generation: "authored",
            lifecycle: "current",
            location: location(0, 1),
            scopePath: [],
          },
        ],
      },
    ],
    formula,
    hypothesisId: "test:law",
    kind: "typed-law",
    label: "Test law",
    location: formula.location,
    missingDiscriminatorIds: [],
    orderingReasons: [],
    range: formula.location.range,
    rank: 0,
    relation: {
      conditions: [],
      description: "Test law",
      evidence: [],
      range: formula.location.range,
      relationId: "test:law",
      roles: [{ label: "Value", role: "value", symbol: "x" }],
      title: "Test law",
    },
    scopePath: [],
    support: "explicit",
  };
  const authoringContext: MathAuthoringContext = {
    claimEvidence: [],
    conditions: [],
    disposition: "established",
    equationLinks: [],
    formula,
    interpretations: {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [hypothesis],
      missingDiscriminators: [],
      truncated: false,
    },
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    },
    notationOccurrences: [],
    requirements: [],
    truncated: false,
  };
  return {
    ...observation("reviewed-formula-domain", "partial", false),
    authoringContext,
    relations: [
      {
        fileId: "main",
        formulaRange: formula.location.range,
        range: formula.location.range,
        relationId: "test:law",
        roles: [{ role: "value", symbol: "x" }],
        sourceGrounded: true,
      },
    ],
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
