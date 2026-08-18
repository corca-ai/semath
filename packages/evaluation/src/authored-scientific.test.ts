import { describe, expect, test } from "bun:test";
import type { MathAuthoringContext } from "../../protocol/src/index";
import {
  AUTHORED_AREA_ALLOCATION,
  DOCUMENT_REASONING_FAMILIES,
  V036_AUTHORED_HOLDOUT_AREA_ALLOCATION,
  authoredFixtureSealPayload,
  authoredProbeIdentityFailures,
  authoredRelationRangeMatches,
  authoredScenarioReviewPayload,
  compareAuthoredMathAuthoringContext,
  observeAuthoredMathAuthoringContext,
  observeAuthoredRelations,
  parseAuthoredScientificFixture,
  scoreAuthoredScientificFixture,
  validateAuthoredScientificTranche,
  type AuthoredArea,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type ObservedMathAuthoringContext,
  type AuthoredSplit,
  type ScientificDecision,
} from "./authored-scientific";

describe("independently authored scientific corpus", () => {
  test("treats trailing equation tags as presentation outside semantic relation ranges", () => {
    const content = "i_1+i_2+i_3=0. \\tag{2}";
    expect(
      authoredRelationRangeMatches(
        content,
        { startOffset: 0, endOffset: 14 },
        { startOffset: 0, endOffset: content.length },
      ),
    ).toBe(true);
    expect(
      authoredRelationRangeMatches(
        "x=y+z",
        { startOffset: 0, endOffset: 3 },
        { startOffset: 0, endOffset: 5 },
      ),
    ).toBe(false);
    const labeled = "\\label{eq:set}\n A\\cap B=C.";
    expect(
      authoredRelationRangeMatches(
        labeled,
        { startOffset: labeled.indexOf("A"), endOffset: labeled.length },
        { startOffset: 0, endOffset: labeled.length },
      ),
    ).toBe(true);
    expect(
      authoredRelationRangeMatches(
        "x+z=y",
        { startOffset: 2, endOffset: 5 },
        { startOffset: 0, endOffset: 5 },
      ),
    ).toBe(false);
    const system = "K=\\tfrac12 mv^2, \\qquad p=mv.";
    expect(
      authoredRelationRangeMatches(
        system,
        { startOffset: 0, endOffset: system.indexOf(",") },
        { startOffset: 0, endOffset: system.length },
        { startOffset: 0, endOffset: system.length },
      ),
    ).toBe(true);
    expect(
      authoredRelationRangeMatches(
        system,
        { startOffset: 0, endOffset: system.indexOf(",") },
        { startOffset: 0, endOffset: system.indexOf("p") - 1 },
      ),
    ).toBe(true);
    const labeledStatement = "\\label{eq:d} y(x)=z(x).";
    expect(
      authoredRelationRangeMatches(
        labeledStatement,
        { startOffset: 0, endOffset: labeledStatement.length },
        { startOffset: 0, endOffset: labeledStatement.length - 1 },
      ),
    ).toBe(true);
    expect(
      authoredRelationRangeMatches(
        system,
        { startOffset: 0, endOffset: system.length },
        { startOffset: 0, endOffset: system.indexOf(",") },
      ),
    ).toBe(false);
  });

  test("projects a reviewed relation from its own source anchor", () => {
    const range = { startOffset: 10, endOffset: 15 };
    expect(observeAuthoredRelations("earlier.tex", [{
      relationId: "circuits:ohm-law",
      title: "Ohm's law",
      description: "Voltage equals resistance times current.",
      roles: [{
        role: "voltage",
        label: "Voltage",
        symbol: "V",
        conceptId: "circuits:voltage",
      }],
      conditions: [],
      evidence: [{
        ruleId: "semantic-law-unification",
        kind: "canonical-math",
        strength: "hard",
        sourceRanges: [range],
      }],
      range,
    }])).toEqual([{
      fileId: "earlier.tex",
      relationId: "circuits:ohm-law",
      range,
      roles: [{
        conceptId: "circuits:voltage",
        role: "voltage",
        symbol: "V",
      }],
      sourceGrounded: true,
    }]);
  });

  test("keeps lifecycle snapshots explicit and cursors on unique source", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    expect(fixture.scenarios[0]?.snapshots.map((snapshot) => snapshot.id)).toEqual([
      "stage-1",
    ]);
    const broken = fixtureValue("holdout", 1) as FixtureValue;
    broken.probes[0]!.cursor.needle = "missing";
    expect(() => parseAuthoredScientificFixture(broken)).toThrow(
      "anchor needle must identify exactly one occurrence",
    );
  });

  test("selects a reviewed occurrence when exact math repeats", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    value.scenarios[0]!.snapshots[0]!.documents[0]!.content +=
      " The independent scope repeats $x_0=y_0$.";
    value.probes[0]!.cursor.occurrence = 1;
    expect(parseAuthoredScientificFixture(value).probes[0]?.cursor.occurrence).toBe(1);

    delete value.probes[0]!.cursor.occurrence;
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "anchor needle must identify exactly one occurrence",
    );
  });

  test("selects an exact symbol range inside a stable source anchor", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    const needle = "unique relation $x_0=y_0$";
    value.probes[0]!.expected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [
        {
          fileId: "main",
          needle,
          selection: { length: 3, offset: needle.indexOf("x_0") },
        },
      ],
      status: "available",
    };
    expect(
      parseAuthoredScientificFixture(value).probes[0]?.expected.navigation
        .definition.required[0]?.selection,
    ).toEqual({ length: 3, offset: needle.indexOf("x_0") });

    value.probes[0]!.expected.navigation.definition.required[0]!.selection = {
      length: needle.length,
      offset: 1,
    };
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "selection must fall within the anchor needle",
    );
  });

  test("compares cursor identity to a reviewed occurrence instead of a display label", () => {
    const value = fixtureValue("development", 1) as FixtureValue;
    const needle = "unique relation $x_0=y_0$";
    value.probes[0]!.expected.cursorOccurrence = {
      fileId: "main",
      needle,
      selection: { length: 3, offset: needle.indexOf("x_0") },
    };
    const fixture = parseAuthoredScientificFixture(value);
    const probe = fixture.probes[0]!;
    const observation = observationValue();
    const source = fixture.scenarios[0]!.snapshots[0]!.documents[0]!.content;
    const anchorStart = source.indexOf(needle);
    observation.symbolLocation = {
      fileId: "main",
      path: "main.tex",
      range: {
        startOffset: anchorStart + needle.indexOf("x_0"),
        endOffset: anchorStart + needle.indexOf("x_0") + 3,
      },
    };
    expect(authoredProbeIdentityFailures(fixture, probe, observation)).toEqual([]);

    observation.symbolLocation = {
      fileId: "main",
      path: "main.tex",
      range: {
        startOffset: anchorStart + needle.indexOf("y_0"),
        endOffset: anchorStart + needle.indexOf("y_0") + 3,
      },
    };
    expect(authoredProbeIdentityFailures(fixture, probe, observation)).toEqual([
      {
        area: "cursor-symbol",
        basis: "cursor occurrence differs from main:unique relation $x_0=y_0$",
      },
    ]);
  });

  test("rejects same-document relations grounded after the cursor", () => {
    const value = fixtureValue("development", 1) as FixtureValue;
    value.scenarios[0]!.snapshots[0]!.documents[0]!.content +=
      " The later result is $u_0=v_0$.";
    value.probes[0]!.expected.relations = [
      {
        anchor: { fileId: "main", needle: "$u_0=v_0$" },
        relationId: "later-result",
        roles: [
          { role: "left", symbol: "u_0" },
          { role: "right", symbol: "v_0" },
        ],
        sourceGrounded: true,
      },
    ];
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "relation anchor occurs after the cursor evidence boundary",
    );
  });

  test("freezes holdout evidence without coupling it to a baseline run", () => {
    const holdout = fixtureValue("holdout", 1) as FixtureValue;
    delete holdout.scenarios[0]!.review.frozenAt;
    expect(() => parseAuthoredScientificFixture(holdout)).toThrow(
      "holdout review must be frozen",
    );

    const development = fixtureValue("development", 1) as FixtureValue;
    development.scenarios[0]!.review.frozenAt = "2026-08-12T00:00:00Z";
    expect(() => parseAuthoredScientificFixture(development)).toThrow(
      "development scenario must remain editable",
    );
  });

  test("validates exact allocation, decision breadth, law breadth, and split isolation", () => {
    const decisions: ScientificDecision[] = [
      ...Array<ScientificDecision>(56).fill("established"),
      ...Array<ScientificDecision>(36).fill("partial"),
      ...Array<ScientificDecision>(24).fill("ambiguous"),
      ...Array<ScientificDecision>(16).fill("conflicting"),
      ...Array<ScientificDecision>(12).fill("unsupported"),
    ];
    const development = parseAuthoredScientificFixture(
      trancheValue("development", decisions.slice(0, 96)),
    );
    const holdout = parseAuthoredScientificFixture(
      trancheValue("holdout", decisions.slice(96)),
    );
    const summary = validateAuthoredScientificTranche(development, holdout, [
      {
        field: "electromagnetism",
        lawId: "electromagnetism:test-law",
        roles: [
          { id: "left", variadic: false },
          { id: "right", variadic: false },
        ],
      },
    ], ["electromagnetism"]);
    expect(summary.developmentCases).toBe(96);
    expect(summary.holdoutCases).toBe(48);
    expect(summary.decisions).toEqual({
      ambiguous: 24,
      conflicting: 16,
      established: 56,
      partial: 36,
      unsupported: 12,
    });
    expect(Object.values(summary.holdoutFamilies)).toEqual([8, 8, 8, 8, 8, 8]);

    const v036Value = trancheValue(
      "holdout",
      decisions.slice(96),
    ) as FixtureValue;
    const v036Fields = Object.entries(V036_AUTHORED_HOLDOUT_AREA_ALLOCATION)
      .flatMap(([field, count]) =>
        Array.from({ length: count }, () => field as AuthoredArea),
      );
    v036Value.scenarios.forEach((scenario, index) => {
      scenario.field = v036Fields[index]!;
    });
    const v036Holdout = parseAuthoredScientificFixture(v036Value);
    expect(
      validateAuthoredScientificTranche(
        development,
        v036Holdout,
        [
          {
            field: "electromagnetism",
            lawId: "electromagnetism:test-law",
            roles: [
              { id: "left", variadic: false },
              { id: "right", variadic: false },
            ],
          },
        ],
        ["electromagnetism"],
        { holdout: V036_AUTHORED_HOLDOUT_AREA_ALLOCATION },
      ).holdoutCases,
    ).toBe(48);
  });

  test("scores unsafe conclusions and exact source paths above missed coverage", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    value.probes[0]!.expected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [{ fileId: "main", needle: "$x_0=y_0$" }],
      status: "available",
    };
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.decision = "established";
    observation.proofGrounded = true;
    const startOffset = "Case 0 defines the unique relation ".length;
    observation.definitions = [
      {
        fileId: "main",
        path: "other.tex",
        range: { startOffset, endOffset: startOffset + "$x_0=y_0$".length },
      },
    ];
    const score = scoreAuthoredScientificFixture(fixture, [observation]);
    expect(score.risk.falseEstablishment).toBe(1);
    expect(score.risk.navigationOrIdentity).toBe(1);
    expect(score.risk.total).toBeGreaterThan(score.risk.missedCoverage * 2);
  });

  test("review and seal payloads exclude only their own attestations", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    expect(authoredScenarioReviewPayload(fixture, "scenario-0")).not.toContain(
      "semanticReviewDigest",
    );
    expect(authoredScenarioReviewPayload(fixture, "scenario-0")).toContain(
      "proofGrounded",
    );
    expect(authoredFixtureSealPayload(fixture)).not.toContain(fixture.batch.seal!);
  });

  test("review and seal payloads do not depend on JSON key order", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    const scenario = fixture.scenarios[0]!;
    const reorderedScenario = Object.fromEntries(
      Object.entries(scenario).reverse(),
    ) as unknown as typeof scenario;
    const reordered = {
      ...fixture,
      batch: Object.fromEntries(
        Object.entries(fixture.batch).reverse(),
      ),
      scenarios: [reorderedScenario],
    } as unknown as AuthoredScientificFixture;
    expect(authoredScenarioReviewPayload(reordered, scenario.id)).toBe(
      authoredScenarioReviewPayload(fixture, scenario.id),
    );
    expect(authoredFixtureSealPayload(reordered)).toBe(
      authoredFixtureSealPayload(fixture),
    );
  });

  test("projects every math authoring surface without opaque engine ids", () => {
    const snapshot = parseAuthoredScientificFixture(
      fixtureValue("development", 1),
    ).scenarios[0]!.snapshots[0]!;
    const location = {
      fileId: "main",
      path: "main.tex",
      range: { startOffset: 35, endOffset: 44 },
    };
    const formula = {
      documentVersion: 7,
      location,
      provenance: [{ startOffset: 36, endOffset: 39 }],
      scopePath: [1],
      sourceNotation: "x_0=y_0",
    };
    const sourceEvidence = {
      kind: "source",
      ruleId: "reviewed-source",
      sourceRanges: [location.range],
      strength: "hard",
    };
    const condition = {
      conditionId: "opaque-condition",
      evidence: [],
      kind: "operator-property",
      label: "positive parameter",
      operatorProperty: "linear",
      status: "required",
      subjects: ["x_0"],
    } as const;
    const context = {
      approximation: {
        evidence: [sourceEvidence],
        exactness: "approximate",
        relationRange: location.range,
      },
      claimEvidence: [
        {
          claim: location,
          claimId: "opaque-claim",
          evidence: [sourceEvidence],
          modality: "hedged",
          polarity: "positive",
          strengthCeiling: "qualified",
          supportingClaimIds: ["opaque-parent"],
          supportingFormulas: [formula],
        },
        {
          claim: location,
          claimId: "opaque-parent",
          evidence: [sourceEvidence],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimIds: [],
          supportingFormulas: [],
        },
      ],
      conditions: [condition],
      conventionalCandidates: [
        {
          bindings: [
            {
              constraint: { concepts: ["test:value"], kind: "scalar" },
              evidence: {
                kind: "prose",
                ruleId: "reviewed-binding",
                sourceRanges: [location.range],
                strength: "hard",
              },
              parameter: "value",
              proof: "candidate",
              symbol: "x_0",
            },
          ],
          candidateId: "opaque-candidate",
          disposition: "conventional-candidate",
          evidence: [sourceEvidence],
          lawId: "test:law",
          packId: "test",
          packVersion: "1.0.0",
          relation: {
            conditions: ["positive"],
            description: "ignored display copy",
            evidence: [],
            range: location.range,
            relationId: "test:law",
            roles: [
              {
                conceptId: "test:value",
                label: "ignored label",
                role: "value",
                symbol: "x_0",
              },
            ],
            title: "ignored title",
          },
          relevance: { evidence: [], support: "tentative" },
          requirements: [
            {
              condition,
              kind: "condition",
              requirementId: "opaque-candidate-condition",
            },
          ],
          title: "ignored title",
        },
      ],
      disposition: "partial",
      equationLinks: [
        {
          evidence: [sourceEvidence],
          kind: "shared-entity",
          linkId: "opaque-link",
          sharedEntities: [{ opaque: true }],
          source: formula,
          target: formula,
        },
        {
          evidence: [sourceEvidence],
          kind: "derived-law",
          linkId: "opaque-derived-link",
          sharedEntities: [{ opaque: true }, { opaque: false }],
          source: formula,
          target: formula,
        },
      ],
      formula,
      lifecycle: {
        capped: false,
        documentVersion: 7,
        editable: true,
        engineLimited: false,
        freshness: "current",
        generation: "authored",
        retracted: false,
      },
      notationOccurrences: [
        {
          entityId: { opaque: true },
          location: formula.location,
          occurrenceId: { opaque: true },
          scopePath: [1],
          sourceNotation: "x_0",
        },
        {
          entityId: { opaque: true },
          location,
          occurrenceId: { opaque: "second" },
          scopePath: [1],
          sourceNotation: "x_0",
        },
        {
          entityId: { opaque: false },
          location,
          occurrenceId: { opaque: "third" },
          scopePath: [2],
          sourceNotation: "y_0",
        },
      ],
      requirements: [
        {
          evidence: [],
          kind: "declaration",
          occurrenceId: { opaque: true },
          requirementId: "opaque-declaration",
          symbol: "x_0",
        },
        {
          kind: "condition",
          condition,
          requirementId: "opaque-requirement",
        },
        {
          constraint: { concepts: ["test:value"], kind: "scalar" },
          evidence: [],
          kind: "role-declaration",
          parameter: "value",
          requirementId: "opaque-role",
          symbol: "x_0",
        },
        {
          alternatives: [
            {
              alternativeId: "opaque-alternative",
              evidence: [],
              label: "ignored alternative label",
              range: location.range,
              relevance: { evidence: [], support: "tentative" },
            },
          ],
          evidence: [],
          kind: "disambiguation",
          requirementId: "opaque-disambiguation",
        },
      ],
      truncated: false,
    } as unknown as MathAuthoringContext;

    const projected = observeAuthoredMathAuthoringContext(
      "main",
      context,
      snapshot,
    );
    expect(JSON.stringify(projected)).not.toContain("opaque");
    expect(projected.formula).toEqual({
      documentVersion: 7,
      location,
      provenance: [
        {
          fileId: "main",
          path: "main.tex",
          range: { startOffset: 36, endOffset: 39 },
        },
      ],
      scopePath: [1],
      sourceNotation: "x_0=y_0",
    });
    expect(projected.lifecycle).toEqual({
      capped: false,
      documentVersion: 7,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    });
    expect(projected.equationLinks[0]).toMatchObject({
      evidence: [location],
      kind: "shared-entity",
      sharedEntityGroups: [0],
    });
    expect(projected.claimEvidence[0]).toMatchObject({
      evidence: [location],
      modality: "hedged",
      polarity: "positive",
      strengthCeiling: "qualified",
      claimGroup: 0,
      supportingClaimGroups: [1],
    });
    expect(projected.claimEvidence[1]).toMatchObject({
      evidence: [location],
      claimGroup: 1,
      supportingClaimGroups: [],
    });
    expect(projected.equationLinks[1]).toMatchObject({
      evidence: [location],
      kind: "derived-law",
      sharedEntityGroups: [0, 1],
    });
    expect(projected.notationOccurrences.map((item) => item.entityGroup)).toEqual([
      0,
      0,
      1,
    ]);
    expect(projected.approximationEvidence).toEqual([location]);
    expect(projected.truncated).toBe(false);
    expect(projected.conditions[0]).toEqual({
      evidence: [],
      kind: "operator-property",
      operatorProperty: "linear",
      status: "required",
      subjects: ["x_0"],
    });
    expect(projected.requirements[0]).toEqual({
      evidence: [],
      kind: "declaration",
      symbol: "x_0",
    });
    expect(projected.requirements[2]).toMatchObject({
      constraint: { concepts: ["test:value"], kind: "scalar" },
      kind: "role-declaration",
      parameter: "value",
      symbol: "x_0",
    });
    expect(projected.requirements[3]).toMatchObject({
      alternatives: [
        {
          range: location,
          relevance: { evidence: [], support: "tentative" },
        },
      ],
      kind: "disambiguation",
    });
    expect(projected.conventionalCandidates[0]).toMatchObject({
      bindings: [
        {
          constraint: { concepts: ["test:value"], kind: "scalar" },
          parameter: "value",
          proof: "candidate",
          symbol: "x_0",
        },
      ],
      lawId: "test:law",
      packId: "test",
      packVersion: "1.0.0",
      relation: {
        location,
        relationId: "test:law",
        roles: [{ conceptId: "test:value", role: "value", symbol: "x_0" }],
      },
      relevance: { evidence: [], support: "tentative" },
    });
  });

  test("compares exact authoring anchors and separates missing from unsafe additions", () => {
    const value = fixtureValue("development", 1) as FixtureValue;
    const expectation = mathAuthoringExpectationValue();
    (value.probes[0]!.expected as unknown as Record<string, unknown>)[
      "authoringContext"
    ] = expectation;
    const fixture = parseAuthoredScientificFixture(value);
    const probe = fixture.probes[0]!;
    const snapshot = fixture.scenarios[0]!.snapshots[0]!;
    const range = { startOffset: 35, endOffset: 44 };
    const location = { fileId: "main", path: "main.tex", range };
    const notationLocation = {
      fileId: "main",
      path: "main.tex",
      range: { startOffset: 36, endOffset: 39 },
    };
    const observedFormula = {
      documentVersion: 1,
      location,
      provenance: [],
      scopePath: [1],
      sourceNotation: "x_0=y_0",
    };
    const observed: ObservedMathAuthoringContext = {
      approximation: location,
      approximationEvidence: [location],
      claimEvidence: [
        {
          claim: location,
          claimGroup: 0,
          evidence: [location],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimGroups: [1],
          supportingFormulas: [observedFormula],
        },
      ],
      conditions: [
        {
          evidence: [location],
          kind: "positive",
          operatorProperty: null,
          status: "required",
          subjects: ["x_0"],
        },
      ],
      conventionalCandidates: [observedConventionalCandidate(location)],
      disposition: "partial",
      equationLinks: [
        {
          evidence: [location],
          kind: "shared-entity",
          sharedEntityGroups: [0],
          source: observedFormula,
          target: observedFormula,
        },
      ],
      formula: observedFormula,
      lifecycle: expectation.lifecycle,
      notationOccurrences: [
        {
          entityGroup: 0,
          location: notationLocation,
          scopePath: [1],
          sourceNotation: "x_0",
        },
      ],
      requirements: [
        { evidence: [location], kind: "declaration", symbol: "x_0" },
      ],
      truncated: false,
    };
    expect(
      compareAuthoredMathAuthoringContext(
        snapshot,
        probe.expected.authoringContext!,
        observed,
      ),
    ).toEqual({
      falseConflictDisposition: false,
      missing: [],
      moreAuthoritativeDisposition: false,
      unsafeLifecycle: [],
      unexpected: [],
    });

    for (const [label, candidate] of [
      [
        "formula version",
        { ...observed, formula: { ...observedFormula, documentVersion: 2 } },
      ],
      [
        "formula scope",
        { ...observed, formula: { ...observedFormula, scopePath: [2] } },
      ],
      [
        "formula provenance",
        { ...observed, formula: { ...observedFormula, provenance: [location] } },
      ],
      [
        "requirement evidence",
        {
          ...observed,
          requirements: [
            {
              evidence: [notationLocation],
              kind: "declaration" as const,
              symbol: "x_0",
            },
          ],
        },
      ],
      [
        "condition evidence",
        {
          ...observed,
          conditions: [
            {
              evidence: [notationLocation],
              kind: "positive" as const,
              operatorProperty: null,
              status: "required" as const,
              subjects: ["x_0"],
            },
          ],
        },
      ],
      [
        "approximation evidence",
        { ...observed, approximationEvidence: [notationLocation] },
      ],
      [
        "equation evidence",
        {
          ...observed,
          equationLinks: [
            { ...observed.equationLinks[0]!, evidence: [notationLocation] },
          ],
        },
      ],
      [
        "claim evidence source",
        {
          ...observed,
          claimEvidence: [
            { ...observed.claimEvidence[0]!, evidence: [notationLocation] },
          ],
        },
      ],
      [
        "shared entity cardinality",
        {
          ...observed,
          equationLinks: [
            { ...observed.equationLinks[0]!, sharedEntityGroups: [0, 1] },
          ],
        },
      ],
      [
        "supporting claim cardinality",
        {
          ...observed,
          claimEvidence: [
            {
              ...observed.claimEvidence[0]!,
              supportingClaimGroups: [1, 2],
            },
          ],
        },
      ],
      [
        "entity grouping",
        {
          ...observed,
          notationOccurrences: [
            { ...observed.notationOccurrences[0]!, entityGroup: 1 },
          ],
        },
      ],
    ] as const) {
      const grounded = compareAuthoredMathAuthoringContext(
        snapshot,
        probe.expected.authoringContext!,
        candidate,
      );
      expect(grounded.missing.length, `${label} must remove reviewed grounding`).toBeGreaterThan(0);
      expect(
        grounded.unexpected.length,
        `${label} must expose unreviewed grounding`,
      ).toBeGreaterThan(0);
    }
    const lifecycleVersion = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      {
        ...observed,
        lifecycle: { ...observed.lifecycle, documentVersion: 2 },
      },
    );
    expect(lifecycleVersion.missing).toContain(
      "lifecycle documentVersion 1; observed 2",
    );
    expect(lifecycleVersion.unexpected).toEqual([]);
    const truncation = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      { ...observed, truncated: true },
    );
    expect(truncation.missing).toContain("truncated false; observed true");
    expect(truncation.unexpected).toEqual([]);
    const conservativeLifecycle = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      {
        ...observed,
        lifecycle: {
          ...observed.lifecycle,
          capped: true,
          editable: false,
          engineLimited: true,
          generation: "generated",
          retracted: true,
        },
      },
    );
    expect(conservativeLifecycle.unsafeLifecycle).toEqual([]);

    const allowedConventional = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      { ...observed, disposition: "conventional" },
    );
    expect(allowedConventional.moreAuthoritativeDisposition).toBe(false);
    const unreviewedConventional = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      {
        ...observed,
        conventionalCandidates: [
          { ...observed.conventionalCandidates[0]!, lawId: "test:other-law" },
        ],
        disposition: "conventional",
      },
    );
    expect(unreviewedConventional.moreAuthoritativeDisposition).toBe(true);
    const unreviewedStartingPoint = compareAuthoredMathAuthoringContext(
      snapshot,
      { ...probe.expected.authoringContext!, disposition: "ambiguous" },
      { ...observed, disposition: "conventional" },
    );
    expect(unreviewedStartingPoint.moreAuthoritativeDisposition).toBe(true);

    const falseConflict = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      { ...observed, disposition: "conflicting" },
    );
    expect(falseConflict.falseConflictDisposition).toBe(true);

    const alphaRenamed = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      {
        ...observed,
        claimEvidence: observed.claimEvidence.map((claim) => ({
          ...claim,
          claimGroup: 8,
          supportingClaimGroups: [9],
        })),
        equationLinks: observed.equationLinks.map((link) => ({
          ...link,
          sharedEntityGroups: [7],
        })),
        notationOccurrences: observed.notationOccurrences.map((occurrence) => ({
          ...occurrence,
          entityGroup: 7,
        })),
      },
    );
    expect(alphaRenamed.missing).toEqual([]);
    expect(alphaRenamed.unexpected).toEqual([]);

    const changed = {
      ...observed,
      conditions: [],
      disposition: "established" as const,
      formula: {
        ...observedFormula,
        location: {
          ...location,
          range: { startOffset: range.startOffset + 1, endOffset: range.endOffset },
        },
      },
    };
    const comparison = compareAuthoredMathAuthoringContext(
      snapshot,
      probe.expected.authoringContext!,
      changed,
    );
    expect(comparison.moreAuthoritativeDisposition).toBe(true);
    expect(comparison.missing.some((failure) => failure.startsWith("condition "))).toBe(true);
    expect(comparison.missing.some((failure) => failure.startsWith("formula "))).toBe(true);
    expect(comparison.unexpected).toHaveLength(1);
    expect(comparison.unexpected[0]).toStartWith("formula ");

    const missingObservation = observationValue();
    const missingScore = scoreAuthoredScientificFixture(fixture, [
      missingObservation,
    ]);
    expect(missingScore.risk.missedCoverage).toBe(1);
    expect(missingScore.risk.falseEstablishment).toBe(0);
  });

  test("rejects host rhetoric, template, and score keys in authored context", () => {
    for (const forbidden of ["rhetoricId", "templateName", "qualityScore"]) {
      const value = fixtureValue("development", 1) as FixtureValue;
      const expectation = mathAuthoringExpectationValue() as Record<string, unknown>;
      expectation[forbidden] = "host-only";
      (value.probes[0]!.expected as unknown as Record<string, unknown>)[
        "authoringContext"
      ] = expectation;
      expect(() => parseAuthoredScientificFixture(value)).toThrow(
        `host vocabulary ${forbidden === "rhetoricId" ? "rhetoric" : forbidden === "templateName" ? "template" : "score"} is forbidden`,
      );
    }
  });

  test("compares cross-document formula provenance, links, and supporting formulas", () => {
    const snapshot = {
      documents: [
        { content: "Main result $a=b$.", fileId: "main", path: "main.tex" },
        {
          content: "Supporting result $c=d$.",
          fileId: "support",
          path: "support.tex",
        },
      ],
      id: "cross-document",
    };
    const mainAnchor = { fileId: "main", needle: "$a=b$" };
    const supportAnchor = { fileId: "support", needle: "$c=d$" };
    const supportRange = { startOffset: 18, endOffset: 23 };
    const projectedClaim = observeAuthoredMathAuthoringContext(
      "main",
      {
        approximation: null,
        claimEvidence: [
          {
            claim: {
              fileId: "support",
              path: "support.tex",
              range: supportRange,
            },
            claimId: "cross-document-claim",
            evidence: [
              {
                kind: "source",
                ruleId: "claim-source",
                sourceRanges: [supportRange],
                strength: "hard",
              },
            ],
            modality: "asserted",
            polarity: "positive",
            strengthCeiling: "asserted",
            supportingClaimIds: [],
            supportingFormulas: [],
          },
        ],
        conditions: [],
        conventionalCandidates: [],
        disposition: "partial",
        equationLinks: [],
        formula: null,
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
      } as unknown as MathAuthoringContext,
      snapshot,
    );
    expect(projectedClaim.claimEvidence[0]!.evidence).toEqual([
      {
        fileId: "support",
        path: "support.tex",
        range: supportRange,
      },
    ]);
    const expectedFormula = {
      anchor: mainAnchor,
      documentVersion: 3,
      provenance: [supportAnchor],
      scopePath: [0],
      sourceNotation: "a=b",
    };
    const expectedSupport = {
      anchor: supportAnchor,
      documentVersion: 2,
      provenance: [],
      scopePath: [1],
      sourceNotation: "c=d",
    };
    const expected = {
      approximation: null,
      claimEvidence: [
        {
          claim: mainAnchor,
          claimGroup: 0,
          evidence: [mainAnchor],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimGroups: [1],
          supportingFormulas: [expectedSupport],
        },
      ],
      conditions: [],
      conventionalCandidates: [],
      disposition: "partial",
      equationLinks: [
        {
          evidence: [mainAnchor],
          kind: "derived-law",
          sharedEntityGroups: [0],
          source: expectedFormula,
          target: expectedSupport,
        },
      ],
      formula: expectedFormula,
      lifecycle: {
        capped: false,
        documentVersion: 3,
        editable: true,
        engineLimited: false,
        freshness: "current",
        generation: "authored",
        retracted: false,
      },
      notationOccurrences: [],
      requirements: [],
      truncated: false,
    } as const;
    const mainLocation = {
      fileId: "main",
      path: "main.tex",
      range: { startOffset: 12, endOffset: 17 },
    };
    const supportLocation = {
      fileId: "support",
      path: "support.tex",
      range: { startOffset: 18, endOffset: 23 },
    };
    const observedFormula = {
      documentVersion: 3,
      location: mainLocation,
      provenance: [supportLocation],
      scopePath: [0],
      sourceNotation: "a=b",
    };
    const observedSupport = {
      documentVersion: 2,
      location: supportLocation,
      provenance: [],
      scopePath: [1],
      sourceNotation: "c=d",
    };
    const observed: ObservedMathAuthoringContext = {
      approximationEvidence: [],
      claimEvidence: [
        {
          claim: mainLocation,
          claimGroup: 0,
          evidence: [mainLocation],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimGroups: [1],
          supportingFormulas: [observedSupport],
        },
      ],
      conditions: [],
      conventionalCandidates: [],
      disposition: "partial",
      equationLinks: [
        {
          evidence: [mainLocation],
          kind: "derived-law",
          sharedEntityGroups: [0],
          source: observedFormula,
          target: observedSupport,
        },
      ],
      formula: observedFormula,
      lifecycle: expected.lifecycle,
      notationOccurrences: [],
      requirements: [],
      truncated: false,
    };
    expect(
      compareAuthoredMathAuthoringContext(snapshot, expected, observed),
    ).toEqual({
      falseConflictDisposition: false,
      missing: [],
      moreAuthoritativeDisposition: false,
      unsafeLifecycle: [],
      unexpected: [],
    });
    const misplaced = compareAuthoredMathAuthoringContext(snapshot, expected, {
      ...observed,
      equationLinks: [
        { ...observed.equationLinks[0]!, target: observedFormula },
      ],
    });
    expect(misplaced.missing.some((item) => item.startsWith("equation link "))).toBe(
      true,
    );
    expect(
      misplaced.unexpected.some((item) => item.startsWith("equation link ")),
    ).toBe(true);
  });

  test("compares same-anchor claim topology by exact local-label bijection", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("development", 1));
    const snapshot = fixture.scenarios[0]!.snapshots[0]!;
    const anchor = { fileId: "main", needle: "$x_0=y_0$" };
    const location = {
      fileId: "main",
      path: "main.tex",
      range: { startOffset: 35, endOffset: 44 },
    };
    const expected = {
      approximation: null,
      claimEvidence: [
        {
          claim: anchor,
          claimGroup: 0,
          evidence: [anchor],
          modality: "asserted",
          polarity: "positive",
          strengthCeiling: "asserted",
          supportingClaimGroups: [1],
          supportingFormulas: [],
        },
        {
          claim: anchor,
          claimGroup: 1,
          evidence: [anchor],
          modality: "hedged",
          polarity: "positive",
          strengthCeiling: "qualified",
          supportingClaimGroups: [0],
          supportingFormulas: [],
        },
      ],
      conditions: [],
      conventionalCandidates: [],
      disposition: "partial",
      equationLinks: [],
      formula: null,
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
    } as const;
    const evidence = {
      kind: "source",
      ruleId: "claim-source",
      sourceRanges: [location.range],
      strength: "hard",
    };
    const observed = observeAuthoredMathAuthoringContext(
      "main",
      {
        approximation: null,
        claimEvidence: [
          {
            claim: location,
            claimId: "opaque-left",
            evidence: [evidence],
            modality: "hedged",
            polarity: "positive",
            strengthCeiling: "qualified",
            supportingClaimIds: ["opaque-right"],
            supportingFormulas: [],
          },
          {
            claim: location,
            claimId: "opaque-right",
            evidence: [evidence],
            modality: "asserted",
            polarity: "positive",
            strengthCeiling: "asserted",
            supportingClaimIds: ["opaque-left"],
            supportingFormulas: [],
          },
        ],
        conditions: [],
        conventionalCandidates: [],
        disposition: "partial",
        equationLinks: [],
        formula: null,
        lifecycle: expected.lifecycle,
        notationOccurrences: [],
        requirements: [],
        truncated: false,
      } as unknown as MathAuthoringContext,
      snapshot,
    );
    const equivalent = compareAuthoredMathAuthoringContext(
      snapshot,
      expected,
      observed,
    );
    expect(equivalent.missing).toEqual([]);
    expect(equivalent.unexpected).toEqual([]);

    const rewired = compareAuthoredMathAuthoringContext(snapshot, expected, {
      ...observed,
      claimEvidence: observed.claimEvidence.map((claim, index) => ({
        ...claim,
        supportingClaimGroups: [index],
      })),
    });
    expect(rewired.missing).toContain("claim evidence parent topology");
    expect(rewired.unexpected).toContain("claim evidence parent topology");

    const nodeCount = 16;
    const permutation = [11, 4, 15, 2, 9, 0, 13, 6, 1, 14, 7, 10, 3, 12, 5, 8];
    const largeExpected = {
      ...expected,
      claimEvidence: Array.from({ length: nodeCount }, (_, group) => ({
        claim: anchor,
        claimGroup: group,
        evidence: [anchor],
        modality: "asserted" as const,
        polarity: "positive" as const,
        strengthCeiling: "asserted" as const,
        supportingClaimGroups: [(group + 1) % nodeCount],
        supportingFormulas: [],
      })),
    };
    const projectRegularGraph = (splitCycle: boolean) =>
      observeAuthoredMathAuthoringContext(
        "main",
        {
          approximation: null,
          claimEvidence: Array.from(
            { length: nodeCount },
            (_, offset) => nodeCount - offset - 1,
          ).map((group) => {
            const next = splitCycle
              ? group === 7
                ? 0
                : group === 15
                  ? 8
                  : (group + 1) % nodeCount
              : (group + 1) % nodeCount;
            return {
              claim: location,
              claimId: `opaque-${permutation[group]}`,
              evidence: [evidence],
              modality: "asserted",
              polarity: "positive",
              strengthCeiling: "asserted",
              supportingClaimIds: [`opaque-${permutation[next]}`],
              supportingFormulas: [],
            };
          }),
          conditions: [],
          conventionalCandidates: [],
          disposition: "partial",
          equationLinks: [],
          formula: null,
          lifecycle: expected.lifecycle,
          notationOccurrences: [],
          requirements: [],
          truncated: false,
        } as unknown as MathAuthoringContext,
        snapshot,
      );
    const largePermutation = compareAuthoredMathAuthoringContext(
      snapshot,
      largeExpected,
      projectRegularGraph(false),
    );
    expect(largePermutation.missing).toEqual([]);
    expect(largePermutation.unexpected).toEqual([]);

    const splitCycle = compareAuthoredMathAuthoringContext(
      snapshot,
      largeExpected,
      projectRegularGraph(true),
    );
    expect(splitCycle.missing).toContain("claim evidence parent topology");
    expect(splitCycle.unexpected).toContain("claim evidence parent topology");
  });
});

function trancheValue(
  split: AuthoredSplit,
  decisions: readonly ScientificDecision[],
): unknown {
  const values: unknown[] = [];
  let decisionIndex = split === "development" ? 0 : 96;
  for (const [field, allocation] of Object.entries(AUTHORED_AREA_ALLOCATION) as [
    AuthoredArea,
    { development: number; holdout: number },
  ][]) {
    for (let index = 0; index < allocation[split]; index += 1) {
      const value = fixtureValue(split, 1, decisionIndex, [decisions[decisionIndex - (split === "development" ? 0 : 96)]!], field) as FixtureValue;
      values.push(value);
      decisionIndex += 1;
    }
  }
  const fixtures = values as FixtureValue[];
  const scenarios = fixtures.flatMap((fixture) => fixture.scenarios);
  const probes = fixtures.flatMap((fixture) => fixture.probes);
  if (split === "holdout") {
    probes.forEach((probe, index) => {
      probe.family = DOCUMENT_REASONING_FAMILIES[Math.floor(index / 8)]!;
    });
  }
  const electromagnetic = scenarios.filter(
    (scenario) => scenario.field === "electromagnetism",
  );
  electromagnetic.forEach((scenario, index) => {
    scenario.lawIds = ["electromagnetism:test-law"];
    scenario.genre = index % 2 === 0 ? "lab-note" : "design-memo";
  });
  for (const probe of probes.filter((probe) =>
    electromagnetic.some((scenario) => scenario.id === probe.scenarioId),
  )) {
    probe.expected.relations = [
      {
        anchor: { fileId: "main", needle: probe.cursor.needle },
        relationId: "electromagnetism:test-law",
        roles: [
          { role: "left", symbol: "x" },
          { role: "right", symbol: "y" },
        ],
        sourceGrounded: true,
      },
    ];
  }
  return {
    batch: batchValue(split),
    probes,
    scenarios,
    schemaVersion: 1,
  };
}

function fixtureValue(
  split: AuthoredSplit,
  count: number,
  start = 0,
  decisions: readonly ScientificDecision[] = Array<ScientificDecision>(count).fill(
    "partial",
  ),
  field: AuthoredArea = "cross-field",
): unknown {
  const scenarios = Array.from({ length: count }, (_, localIndex) => {
    const index = start + localIndex;
    const finalDigest = hex(index + 3000);
    return {
      field,
      genre: index % 2 ? "lab-note" : "design-memo",
      id: `scenario-${index}`,
      lawIds: [],
      provenance: {
        authorId: `author-${index}`,
        engineBlind: true,
        independenceGroup: `${split}-${index}`,
        rawDigest: hex(index + 2000),
        taskCardDigest: hex(index + 1000),
      },
      review: {
        correctionSummary: [],
        criticId: `critic-${index}`,
        finalDigest,
        ...(split === "holdout" ? { frozenAt: "2026-08-12T00:00:00Z" } : {}),
        mainReviewer: "main-codex",
        reviewedAt: "2026-08-12",
        semanticReviewDigest: finalDigest,
        status: "approved",
      },
      snapshots: [
        {
          documents: [
            {
              content: `Case ${index} defines the unique relation $x_${index}=y_${index}$.`,
              fileId: "main",
              path: "main.tex",
            },
          ],
          id: "stage-1",
        },
      ],
      variationTags: ["document-shaped", `case-${index}`],
    };
  });
  return {
    batch: batchValue(split),
    probes: scenarios.map((scenario, localIndex) => ({
      cursor: {
        edge: "after",
        fileId: "main",
        needle: `$x_${start + localIndex}=y_${start + localIndex}$`,
        snapshotId: "stage-1",
      },
      expected: {
        decision: decisions[localIndex],
        diagnostics: { excludedCodes: [], maximum: 0, required: [] },
        excludedRelationIds: [],
        navigation: {
          definition: { excluded: [], minimum: 0, required: [], status: "unavailable" },
          prepareRename: { status: "unavailable" },
          references: { excluded: [], minimum: 0, required: [], status: "unavailable" },
          rename: { excluded: [], minimum: 0, required: [], status: "unavailable" },
        },
        proofGrounded: false,
        relations: [],
        symbol: `x_${start + localIndex}`,
      },
      family: DOCUMENT_REASONING_FAMILIES[localIndex % DOCUMENT_REASONING_FAMILIES.length],
      id: `probe-${start + localIndex}`,
      kind: "primary",
      scenarioId: scenario.id,
    })),
    scenarios,
    schemaVersion: 1,
  };
}

function batchValue(split: AuthoredSplit): Record<string, unknown> {
  return {
    createdAt: "2026-08-12",
    ...(split === "holdout"
      ? { frozenAt: "2026-08-12T00:00:00Z", seal: "d".repeat(64) }
      : {}),
    id: `${split}-batch`,
    reviewPolicyVersion: 1,
    split,
    taskCardDigest: split === "holdout" ? "b".repeat(64) : "c".repeat(64),
  };
}

function observationValue(): WritableObservation {
  return {
    caseId: "probe-0",
    decision: "partial",
    definitions: [],
    diagnostics: [],
    prepareRename: {},
    proofGrounded: false,
    references: [],
    relations: [],
    renameEdits: [],
    symbol: "x_0",
  };
}

function authoredConventionalCandidate(anchor: { fileId: string; needle: string }) {
  const constraint = { concepts: ["test:value"], kind: "scalar" } as const;
  return {
    bindings: [
      {
        constraint,
        evidence: [anchor],
        parameter: "value",
        proof: "candidate",
        symbol: "x_0",
      },
    ],
    evidence: [anchor],
    lawId: "test:law",
    packId: "test",
    packVersion: "1.0.0",
    relation: {
      anchor,
      conditions: ["positive"],
      evidence: [anchor],
      relationId: "test:law",
      roles: [{ conceptId: "test:value", role: "value", symbol: "x_0" }],
    },
    relevance: { evidence: [anchor], support: "tentative" },
    requirements: [
      {
        constraint,
        evidence: [anchor],
        kind: "role-declaration",
        parameter: "value",
        symbol: "x_0",
      },
    ],
  } as const;
}

function observedConventionalCandidate(location: {
  fileId: string;
  path: string;
  range: { startOffset: number; endOffset: number };
}) {
  const constraint = { concepts: ["test:value"], kind: "scalar" } as const;
  return {
    bindings: [
      {
        constraint,
        evidence: [location],
        parameter: "value",
        proof: "candidate",
        symbol: "x_0",
      },
    ],
    evidence: [location],
    lawId: "test:law",
    packId: "test",
    packVersion: "1.0.0",
    relation: {
      conditions: ["positive"],
      evidence: [location],
      location,
      relationId: "test:law",
      roles: [{ conceptId: "test:value", role: "value", symbol: "x_0" }],
    },
    relevance: { evidence: [location], support: "tentative" },
    requirements: [
      {
        constraint,
        evidence: [location],
        kind: "role-declaration",
        parameter: "value",
        symbol: "x_0",
      },
    ],
  } as const;
}

function mathAuthoringExpectationValue() {
  const anchor = { fileId: "main", needle: "$x_0=y_0$" };
  const formula = {
    anchor,
    documentVersion: 1,
    provenance: [],
    scopePath: [1],
    sourceNotation: "x_0=y_0",
  };
  return {
    approximation: { evidence: [anchor], range: anchor },
    claimEvidence: [
      {
        claim: anchor,
        claimGroup: 0,
        evidence: [anchor],
        modality: "asserted",
        polarity: "positive",
        strengthCeiling: "asserted",
        supportingClaimGroups: [1],
        supportingFormulas: [formula],
      },
    ],
    conditions: [
      {
        evidence: [anchor],
        kind: "positive",
        operatorProperty: null,
        status: "required",
        subjects: ["x_0"],
      },
    ],
    conventionalCandidates: [authoredConventionalCandidate(anchor)],
    disposition: "partial",
    equationLinks: [
      {
        evidence: [anchor],
        kind: "shared-entity",
        sharedEntityGroups: [0],
        source: formula,
        target: formula,
      },
    ],
    formula,
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      generation: "authored",
      freshness: "current",
      retracted: false,
    },
    notationOccurrences: [
      {
        anchor: {
          fileId: "main",
          needle: "$x_0=y_0$",
          selection: { length: 3, offset: 1 },
        },
        entityGroup: 0,
        scopePath: [1],
        sourceNotation: "x_0",
      },
    ],
    requirements: [
      { evidence: [anchor], kind: "declaration", symbol: "x_0" },
    ],
    truncated: false,
  } as const;
}

function hex(value: number): string {
  return value.toString(16).padStart(64, "0");
}

type WritableObservation = {
  -readonly [Key in keyof AuthoredScientificObservation]: AuthoredScientificObservation[Key];
};

interface FixtureValue {
  batch: Record<string, unknown>;
  probes: {
    cursor: { needle: string; occurrence?: number };
    expected: {
      cursorOccurrence?: {
        fileId: string;
        needle: string;
        selection?: { length: number; offset: number };
      } | null;
      navigation: {
        definition: {
          excluded: { fileId: string; needle: string }[];
          minimum: number;
          required: {
            fileId: string;
            needle: string;
            selection?: { length: number; offset: number };
          }[];
          status: "available" | "unavailable";
        };
      };
      relations: {
        anchor: { fileId: string; needle: string };
        relationId: string;
        roles: { role: string; symbol: string }[];
        sourceGrounded: boolean;
      }[];
    };
    family: (typeof DOCUMENT_REASONING_FAMILIES)[number];
    scenarioId: string;
  }[];
  scenarios: {
    field: AuthoredArea;
    genre: string;
    id: string;
    lawIds: string[];
    review: { frozenAt?: string };
    snapshots: {
      documents: { content: string }[];
    }[];
  }[];
}
