import { describe, expect, test } from "bun:test";
import {
  MATH_AUTHORING_DEVELOPMENT_FEATURES,
  evaluateMathAuthoringDevelopment,
  parseMathAuthoringDevelopmentFixture,
  type MathAuthoringDevelopmentObservation,
} from "./math-authoring-development";

const SOURCE = "Let $x$ be scalar. $y=x$. Claim one. Claim two.";

describe("complete math-authoring development oracle", () => {
  test("accepts the complete reviewed context only with its stale fence", () => {
    const fixture = parseMathAuthoringDevelopmentFixture(fixtureValue());
    const observation = observationValue();

    expect(evaluateMathAuthoringDevelopment(fixture, [observation])).toEqual({
      cases: 1,
      coveredFeatures: MATH_AUTHORING_DEVELOPMENT_FEATURES,
    });
    expect(() =>
      evaluateMathAuthoringDevelopment(fixture, [
        { ...observation, staleRevisionRejected: false },
      ]),
    ).toThrow("stale document revision was accepted");
  });

  test("rejects a changed structured requirement", () => {
    const observation = observationValue();
    const requirement = observation.context.requirements[0]!;
    if (requirement.kind !== "declaration") {
      throw new Error("test fixture requires a declaration");
    }
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          requirements: [
            { ...requirement, symbol: "z" },
          ],
        },
      },
      "requirement",
    );
  });

  test("rejects a changed conventional candidate detail", () => {
    const observation = observationValue();
    const candidate = observation.context.conventionalCandidates[0]!;
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          conventionalCandidates: [{ ...candidate, packVersion: "9.9.9" }],
        },
      },
      "conventional candidate",
    );
  });

  test("rejects rewired claim parent topology", () => {
    const observation = observationValue();
    const [first, second] = observation.context.claimEvidence;
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          claimEvidence: [
            { ...first!, supportingClaimGroups: [0] },
            second!,
          ],
        },
      },
      "claim evidence parent topology",
    );
  });

  test("rejects notation scope or source drift", () => {
    const observation = observationValue();
    const occurrence = observation.context.notationOccurrences[0]!;
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          notationOccurrences: [
            { ...occurrence, scopePath: [7], sourceNotation: "z" },
          ],
        },
      },
      "notation occurrence",
    );
  });

  test("rejects formula metadata and equation-link drift", () => {
    const observation = observationValue();
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          formula: {
            ...observation.context.formula!,
            sourceNotation: "z=x",
          },
        },
      },
      "formula",
    );
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          equationLinks: [
            {
              ...observation.context.equationLinks[0]!,
              sharedEntityGroups: [],
            },
          ],
        },
      },
      "equation link",
    );
  });

  test("rejects removal of a reviewed lifecycle fence", () => {
    const observation = observationValue();
    expectFailure(
      {
        ...observation,
        context: {
          ...observation.context,
          lifecycle: { ...observation.context.lifecycle, editable: true },
        },
      },
      "unsafe lifecycle",
    );
  });

  test("strictly rejects unknown fields and negative occurrences", () => {
    const extra = fixtureValue() as Record<string, unknown>;
    extra.unreviewed = true;
    expect(() => parseMathAuthoringDevelopmentFixture(extra)).toThrow(
      "fixture.unreviewed is not allowed",
    );

    const malformed = fixtureValue();
    malformed.cases[0]!.cursor.occurrence = -1;
    expect(() => parseMathAuthoringDevelopmentFixture(malformed)).toThrow(
      "fixture.cases[0].cursor.occurrence must be a non-negative integer",
    );
  });

  test("fails when reviewed development breadth is incomplete", () => {
    const value = fixtureValue();
    value.cases[0]!.features = ["revision-fence"];
    const fixture = parseMathAuthoringDevelopmentFixture(value);
    expect(() =>
      evaluateMathAuthoringDevelopment(fixture, [observationValue()]),
    ).toThrow("missing development feature approximate-not-exact");
  });
});

function expectFailure(
  observation: MathAuthoringDevelopmentObservation,
  message: string,
): void {
  const fixture = parseMathAuthoringDevelopmentFixture(fixtureValue());
  expect(() => evaluateMathAuthoringDevelopment(fixture, [observation])).toThrow(
    message,
  );
}

function fixtureValue() {
  const anchor = (needle: string, occurrence = 0) => ({
    fileId: "main",
    needle,
    occurrence,
  });
  const formula = {
    anchor: anchor("y=x"),
    documentVersion: 2,
    provenance: [],
    scopePath: [],
    sourceNotation: "y=x",
  };
  const condition = {
    evidence: [anchor("x", 0)],
    kind: "positive",
    operatorProperty: null,
    status: "required",
    subjects: ["x"],
  };
  return {
    cases: [
      {
        cursor: anchor("y=x"),
        documents: [{ content: "old", fileId: "main", path: "main.tex" }],
        expected: {
          authoringContext: {
            approximation: null,
            claimEvidence: [
              {
                claim: anchor("Claim one."),
                claimGroup: 0,
                evidence: [anchor("Claim one.")],
                modality: "asserted",
                polarity: "positive",
                strengthCeiling: "asserted",
                supportingClaimGroups: [1],
                supportingFormulas: [formula],
              },
              {
                claim: anchor("Claim two."),
                claimGroup: 1,
                evidence: [anchor("Claim two.")],
                modality: "hedged",
                polarity: "positive",
                strengthCeiling: "qualified",
                supportingClaimGroups: [],
                supportingFormulas: [],
              },
            ],
            conditions: [condition],
            conventionalCandidates: [
              {
                bindings: [
                  {
                    constraint: { kind: "scalar" },
                    evidence: [anchor("x", 0)],
                    parameter: "value",
                    proof: "candidate",
                    symbol: "x",
                  },
                ],
                evidence: [anchor("y=x")],
                lawId: "test-law",
                packId: "test-pack",
                packVersion: "1.0.0",
                relation: {
                  anchor: anchor("y=x"),
                  conditions: [],
                  evidence: [anchor("y=x")],
                  relationId: "test-pack:test-law",
                  roles: [
                    { conceptId: null, role: "value", symbol: "x" },
                  ],
                },
                relevance: {
                  evidence: [anchor("y=x")],
                  support: "tentative",
                },
                requirements: [
                  {
                    constraint: { kind: "scalar" },
                    evidence: [anchor("x", 0)],
                    kind: "role-declaration",
                    parameter: "value",
                    symbol: "x",
                  },
                ],
              },
            ],
            disposition: "conventional",
            equationLinks: [
              {
                evidence: [anchor("y=x")],
                kind: "shared-entity",
                sharedEntityGroups: [0],
                source: formula,
                target: formula,
              },
            ],
            formula,
            lifecycle: {
              capped: false,
              documentVersion: 2,
              editable: false,
              engineLimited: false,
              freshness: "current",
              generation: "generated",
              retracted: false,
            },
            notationOccurrences: [
              {
                anchor: anchor("x", 1),
                entityGroup: 0,
                scopePath: [],
                sourceNotation: "x",
              },
            ],
            requirements: [
              {
                evidence: [anchor("x", 0)],
                kind: "declaration",
                symbol: "x",
              },
            ],
            truncated: false,
          },
          definitionAuthorized: false,
        },
        features: [...MATH_AUTHORING_DEVELOPMENT_FEATURES],
        id: "complete",
        kind: "revision",
        mainFileId: "main",
        revisedDocuments: [
          { content: SOURCE, fileId: "main", path: "main.tex" },
        ],
        staleDocumentVersion: 1,
      },
    ],
    reviewedAt: "2026-08-18",
    reviewedBy: "test-reviewer",
    reviewSummary: "Complete host-neutral structure reviewed for mutation tests.",
    schemaVersion: 1,
  };
}

function observationValue(): MathAuthoringDevelopmentObservation {
  const location = (needle: string, occurrence = 0) => {
    let start = -1;
    let from = 0;
    for (let index = 0; index <= occurrence; index += 1) {
      start = SOURCE.indexOf(needle, from);
      from = start + needle.length;
    }
    return {
      fileId: "main",
      path: "main.tex",
      range: { endOffset: start + needle.length, startOffset: start },
    };
  };
  const formula = {
    documentVersion: 2,
    location: location("y=x"),
    provenance: [],
    scopePath: [],
    sourceNotation: "y=x",
  };
  const condition = {
    evidence: [location("x", 0)],
    kind: "positive" as const,
    operatorProperty: null,
    status: "required" as const,
    subjects: ["x"],
  };
  return {
    caseId: "complete",
    context: {
      approximationEvidence: [],
      claimEvidence: [
        {
          claim: location("Claim one."),
          claimGroup: 0,
          evidence: [location("Claim one.")],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimGroups: [1],
          supportingFormulas: [formula],
        },
        {
          claim: location("Claim two."),
          claimGroup: 1,
          evidence: [location("Claim two.")],
          modality: "hedged",
          polarity: "positive",
          strengthCeiling: "qualified",
          supportingClaimGroups: [],
          supportingFormulas: [],
        },
      ],
      conditions: [condition],
      conventionalCandidates: [
        {
          bindings: [
            {
              constraint: { kind: "scalar" },
              evidence: [location("x", 0)],
              parameter: "value",
              proof: "candidate",
              symbol: "x",
            },
          ],
          evidence: [location("y=x")],
          lawId: "test-law",
          packId: "test-pack",
          packVersion: "1.0.0",
          relation: {
            conditions: [],
            evidence: [location("y=x")],
            location: location("y=x"),
            relationId: "test-pack:test-law",
            roles: [{ conceptId: null, role: "value", symbol: "x" }],
          },
          relevance: { evidence: [location("y=x")], support: "tentative" },
          requirements: [
            {
              constraint: { kind: "scalar" },
              evidence: [location("x", 0)],
              kind: "role-declaration",
              parameter: "value",
              symbol: "x",
            },
          ],
        },
      ],
      disposition: "conventional",
      equationLinks: [
        {
          evidence: [location("y=x")],
          kind: "shared-entity",
          sharedEntityGroups: [0],
          source: formula,
          target: formula,
        },
      ],
      formula,
      lifecycle: {
        capped: false,
        documentVersion: 2,
        editable: false,
        engineLimited: false,
        freshness: "current",
        generation: "generated",
        retracted: false,
      },
      notationOccurrences: [
        {
          entityGroup: 0,
          location: location("x", 1),
          scopePath: [],
          sourceNotation: "x",
        },
      ],
      requirements: [
        {
          evidence: [location("x", 0)],
          kind: "declaration",
          symbol: "x",
        },
      ],
      truncated: false,
    },
    definitionAuthorized: false,
    staleRevisionRejected: true,
  };
}
