import { describe, expect, test } from "bun:test";
import type { MathInterpretationSetInfo } from "../../protocol/src/index";
import {
  AUTHORED_AREA_ALLOCATION,
  DOCUMENT_REASONING_FAMILIES,
  authoredCursorSourceAnchor,
  authoredFixtureSealPayload,
  authoredProbeIdentityFailures,
  authoredRenameNotationFamilyMatches,
  authoredRelationRangeMatches,
  authoredScenarioRawPayload,
  authoredScenarioReviewPayload,
  observeAuthoredScientificProbe,
  observeAuthoredRelations,
  parseAuthoredScientificFixture,
  scoreAuthoredScientificFixture,
  sortAuthoredAnchors,
  validateAuthoredScientificTranche,
  type AuthoredArea,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredScientificSurfaceResults,
  type AuthoredSplit,
  type ScientificDecision,
} from "./authored-scientific";

describe("independently authored scientific corpus", () => {
  test("uses one notation-family rule for atomic and composite rename", () => {
    expect(authoredRenameNotationFamilyMatches("x", "y")).toBe(true);
    expect(authoredRenameNotationFamilyMatches("x_0", "y_0")).toBe(true);
    expect(authoredRenameNotationFamilyMatches("\\hat{x}", "\\hat{y}"))
      .toBe(true);
    expect(authoredRenameNotationFamilyMatches("\\hat{x}", "\\bar{x}"))
      .toBe(false);
    expect(authoredRenameNotationFamilyMatches("x_0", "y_1")).toBe(false);
  });

  test("retains exact public entity-surface authorizations in observations", () => {
    const probe = parseAuthoredScientificFixture(fixtureValue("holdout", 1))
      .probes[0]!;
    const focusOccurrenceId = {
      documentVersion: 1,
      fileId: "main",
      localId: 1,
    };
    const authorized = {
      entityId: {
        anchor: focusOccurrenceId,
        componentId: "component-1",
        kind: "symbol",
        scopePath: [],
      },
      focusOccurrenceId,
      status: "authorized",
    } as const;
    const refused = {
      reason: { kind: "non-editable", message: "not editable" },
      status: "refused",
    } as const;
    const cursorSurfaceIdentity = {
      entityId: authorized.entityId,
      location: {
        fileId: "main",
        path: "main.md",
        range: { startOffset: 1, endOffset: 2 },
      },
      occurrenceId: focusOccurrenceId,
    };
    const result = (value: unknown) => ({ value });
    const results = {
      definition: result({ authorization: authorized, kind: "locations", locations: [] }),
      diagnostics: result({ diagnostics: [], kind: "diagnostics" }),
      prepareRename: result({ authorization: refused, kind: "renamePreparation" }),
      references: result({ authorization: authorized, kind: "locations", locations: [] }),
      rename: result({ authorization: refused, kind: "editProposal" }),
      semanticView: result({
        kind: "semanticView",
        view: {
          authoringContext: {
            disposition: "unsupported",
            interpretations: { hypotheses: [] },
          },
          context: { relations: [] },
          decision: { reasons: [], status: "partial" },
          symbol: {
            ...cursorSurfaceIdentity,
            symbol: "x",
          },
        },
      }),
    } as unknown as AuthoredScientificSurfaceResults;

    const observation = observeAuthoredScientificProbe(probe, results);
    expect(observation.cursorSurfaceIdentity).toEqual(cursorSurfaceIdentity);
    expect(observation.surfaceAuthorizations).toEqual({
        definition: authorized,
        prepareRename: { refusalKind: "non-editable", status: "refused" },
        references: authorized,
        rename: { refusalKind: "non-editable", status: "refused" },
    });
  });

  test("does not borrow selected-formula proof for a missing cursor entity", () => {
    const probe = parseAuthoredScientificFixture(fixtureValue("holdout", 1))
      .probes[0]!;
    const refused = {
      reason: { kind: "no-entity", message: "no entity at cursor" },
      status: "refused",
    } as const;
    const result = (value: unknown) => ({ value });
    const results = {
      definition: result({ authorization: refused, kind: "locations", locations: [] }),
      diagnostics: result({ diagnostics: [], kind: "diagnostics" }),
      prepareRename: result({ authorization: refused, kind: "renamePreparation" }),
      references: result({ authorization: refused, kind: "locations", locations: [] }),
      rename: result({ authorization: refused, kind: "editProposal" }),
      semanticView: result({
        kind: "semanticView",
        view: {
          authoringContext: {
            disposition: "established",
            interpretations: { hypotheses: [] },
          },
          context: { relations: [] },
          decision: {
            reasons: [{
              evidence: [{
                kind: "formula",
                ruleId: "selected-formula-proof",
                sourceRanges: [{ startOffset: 1, endOffset: 4 }],
                strength: "explicit",
              }],
              kind: "proof",
            }],
            status: "partial",
          },
        },
      }),
    } as unknown as AuthoredScientificSurfaceResults;

    expect(observeAuthoredScientificProbe(probe, results).proofGrounded).toBe(false);
  });

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

  test("counts an excluded candidate relation as false establishment only with reviewed authority", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    value.probes[0]!.expected.excludedRelationIds = ["candidate-law"];
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.relations = [{
      fileId: "main",
      range: { startOffset: 35, endOffset: 44 },
      relationId: "candidate-law",
      roles: [],
      sourceGrounded: false,
    }];

    expect(scoreAuthoredScientificFixture(fixture, [observation]).risk)
      .toMatchObject({ falseEstablishment: 1 });

    observation.interpretations = interpretationSet(
      "candidate-law",
      "structural-alternative",
      "tentative",
    );
    const candidateOnly = scoreAuthoredScientificFixture(fixture, [observation]);
    expect(candidateOnly.failures).toContain(
      "probe-0: leaked relation candidate-law",
    );
    expect(candidateOnly.risk.falseEstablishment).toBe(0);

    observation.interpretations = authoritativeSourceMeaningSet(
      "candidate-law",
    );
    expect(scoreAuthoredScientificFixture(fixture, [observation]).risk)
      .toMatchObject({ falseEstablishment: 1 });
  });

  test("keeps diagnostic overruns separate from false conflict risk", () => {
    for (const decision of ["ambiguous", "conflicting"] as const) {
      const fixture = parseAuthoredScientificFixture(
        fixtureValue("holdout", 1, 0, [decision]),
      );
      const observation = observationValue();
      observation.decision = decision;
      observation.diagnostics = [{
        code: "candidate-analysis-limit",
        fileId: "main",
        range: { startOffset: 35, endOffset: 44 },
        severity: "warning",
      }];
      const score = scoreAuthoredScientificFixture(fixture, [observation]);
      expect(score.failures).toContain(
        "probe-0: problems 1; expected at most 0",
      );
      expect(score.risk.falseConflict).toBe(0);
    }
  });

  test("separates cursor-entity and selected-formula decisions in fixture schema 2", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue & {
      schemaVersion: number;
    };
    value.schemaVersion = 2;
    const probe = value.probes[0]!;
    const expected = probe.expected as typeof probe.expected & {
      decision: ScientificDecision;
      formulaDecision?: {
        anchor: {
          fileId: string;
          needle: string;
          selection: { length: number; offset: number };
        };
        status: "partial";
      };
      navigation: typeof probe.expected.navigation & {
        rename: { newName?: string; status: "unavailable" };
      };
      symbol?: string;
    };
    expected.decision = "established";
    expected.symbol = "x";
    expected.formulaDecision = {
      anchor: {
        fileId: "main",
        needle: "$x_0=y_0$",
        selection: { length: 7, offset: 1 },
      },
      status: "partial",
    };

    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "unavailable rename requires a same-family newName",
    );
    expected.navigation.rename.newName = "y";
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.decision = "established";
    observation.symbol = "x";
    observation.formulaDecision = {
      location: {
        fileId: "main",
        path: "main.tex",
        range: { startOffset: 36, endOffset: 43 },
      },
      status: "partial",
    };
    observation.surfaceAuthorizations = refusedSurfaceAuthorizations();
    const score = scoreAuthoredScientificFixture(fixture, [observation]);
    expect(score.failures).toEqual([]);
    expect(score.risk.falseEstablishment).toBe(0);

    observation.formulaDecision = {
      ...observation.formulaDecision,
      status: "established",
    };
    expect(scoreAuthoredScientificFixture(fixture, [observation]).risk)
      .toMatchObject({ falseEstablishment: 1 });
  });

  test("rejects authorized-empty schema-2 surfaces when refusal is reviewed", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue & {
      schemaVersion: number;
    };
    value.schemaVersion = 2;
    const expected = value.probes[0]!.expected as typeof value.probes[0]["expected"] & {
      cursorOccurrence?: { fileId: string; needle: string };
      formulaDecision?: null;
      navigation: typeof value.probes[0]["expected"]["navigation"] & {
        rename: { newName?: string; status: "unavailable" };
      };
    };
    expected.cursorOccurrence = { fileId: "main", needle: "x_0" };
    expected.formulaDecision = null;
    expected.navigation.rename.newName = "y_0";
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.formulaDecision = { location: null, status: "unsupported" };
    observation.cursorSurfaceIdentity = {
      entityId: {
        anchor: { documentVersion: 1, fileId: "main", localId: 1 },
        componentId: "component-1",
        kind: "symbol",
        scopePath: [],
      },
      location: {
        fileId: "main",
        path: "main.tex",
        range: { startOffset: 36, endOffset: 39 },
      },
      occurrenceId: { documentVersion: 1, fileId: "main", localId: 1 },
    };
    observation.symbolLocation = observation.cursorSurfaceIdentity.location;
    observation.surfaceAuthorizations = refusedSurfaceAuthorizations();
    expect(scoreAuthoredScientificFixture(fixture, [observation]).failures)
      .toEqual([]);

    observation.surfaceAuthorizations = {
      ...refusedSurfaceAuthorizations(),
      definition: {
        entityId: {
          anchor: { documentVersion: 1, fileId: "main", localId: 1 },
          componentId: "component-1",
          kind: "symbol",
          scopePath: [],
        },
        focusOccurrenceId: { documentVersion: 1, fileId: "main", localId: 1 },
        status: "authorized",
      },
    };
    const score = scoreAuthoredScientificFixture(fixture, [observation]);
    expect(score.failures[0]).toContain(
      "definition authorization authorized; expected refused",
    );
    expect(score.risk.navigationOrIdentity).toBe(1);
  });

  test("distinguishes an authorized empty schema-2 result from a refusal", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue & {
      schemaVersion: number;
    };
    value.schemaVersion = 2;
    const expected = value.probes[0]!.expected as typeof value.probes[0]["expected"] & {
      cursorOccurrence?: { fileId: string; needle: string };
      formulaDecision?: null;
      navigation: typeof value.probes[0]["expected"]["navigation"] & {
        definition: { authorization?: "authorized"; status: "unavailable" };
        rename: { newName?: string; status: "unavailable" };
      };
    };
    expected.cursorOccurrence = { fileId: "main", needle: "x_0" };
    expected.formulaDecision = null;
    expected.navigation.definition.authorization = "authorized";
    expected.navigation.rename.newName = "y_0";
    const fixture = parseAuthoredScientificFixture(value);
    const observation = observationValue();
    observation.formulaDecision = { location: null, status: "unsupported" };
    observation.cursorSurfaceIdentity = {
      entityId: {
        anchor: { documentVersion: 1, fileId: "main", localId: 1 },
        componentId: "component-1",
        kind: "symbol",
        scopePath: [],
      },
      location: {
        fileId: "main",
        path: "main.tex",
        range: { startOffset: 36, endOffset: 39 },
      },
      occurrenceId: { documentVersion: 1, fileId: "main", localId: 1 },
    };
    observation.symbolLocation = observation.cursorSurfaceIdentity.location;
    observation.surfaceAuthorizations = {
      ...refusedSurfaceAuthorizations(),
      definition: {
        entityId: {
          anchor: { documentVersion: 1, fileId: "main", localId: 1 },
          componentId: "component-1",
          kind: "symbol",
          scopePath: [],
        },
        focusOccurrenceId: { documentVersion: 1, fileId: "main", localId: 1 },
        status: "authorized",
      },
    };
    expect(scoreAuthoredScientificFixture(fixture, [observation]).failures)
      .toEqual([]);
  });

  test("requires a null schema-2 cursor occurrence to refuse entity authority", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue & {
      schemaVersion: number;
    };
    value.schemaVersion = 2;
    const expected = value.probes[0]!.expected as typeof value.probes[0]["expected"] & {
      cursorOccurrence?: null;
      formulaDecision?: null;
    };
    expected.cursorOccurrence = null;
    expected.formulaDecision = null;
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "a missing cursor occurrence must refuse cursor-entity authority",
    );
  });

  test("keeps fixture schema 1 byte-compatible and rejects decision-domain fields", () => {
    const value = fixtureValue("holdout", 1) as FixtureValue;
    (value.probes[0]!.expected as Record<string, unknown>).formulaDecision = {
      anchor: { fileId: "main", needle: "$x_0=y_0$" },
      status: "partial",
    };
    expect(() => parseAuthoredScientificFixture(value)).toThrow(
      "unavailable in fixture schema 1",
    );

    const navigation = fixtureValue("holdout", 1) as FixtureValue;
    const expected = navigation.probes[0]!.expected as unknown as {
      navigation: { references: Record<string, unknown> };
    };
    expected.navigation.references.allowed = [];
    expect(() => parseAuthoredScientificFixture(navigation)).toThrow(
      "navigation.references.allowed: unknown field",
    );
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

  test("raw lineage is non-self-referential and ignores review corrections", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    const original = authoredScenarioRawPayload(fixture, "scenario-0");
    const scenario = fixture.scenarios[0]!;
    const probe = fixture.probes[0]!;
    const corrected: AuthoredScientificFixture = {
      ...fixture,
      probes: [{
        ...probe,
        expected: { ...probe.expected, decision: "ambiguous" },
      }],
      scenarios: [{
        ...scenario,
        lawIds: ["test:corrected-law"],
        review: {
          ...scenario.review,
          correctionSummary: ["reviewed expectation correction"],
        },
      }],
    };

    expect(authoredScenarioRawPayload(corrected, "scenario-0")).toBe(original);
    expect(original).not.toContain(scenario.provenance.rawDigest);
    expect(original).not.toContain("finalDigest");
    expect(original).not.toContain("test:corrected-law");

    const document = scenario.snapshots[0]!.documents[0]!;
    const changedSource: AuthoredScientificFixture = {
      ...fixture,
      scenarios: [{
        ...scenario,
        snapshots: [{
          ...scenario.snapshots[0]!,
          documents: [{ ...document, content: `${document.content} Changed.` }],
        }],
      }],
    };
    expect(authoredScenarioRawPayload(changedSource, "scenario-0")).not.toBe(
      original,
    );
  });

  test("projects cursor fields and sorts navigation by resolved source keys", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    const probe = fixture.probes[0]!;
    expect(authoredCursorSourceAnchor(probe.cursor)).toEqual({
      fileId: "main",
      needle: "$x_0=y_0$",
    });
    const snapshot = fixture.scenarios[0]!.snapshots[0]!;
    const anchors = [
      { fileId: "main", needle: "y_0" },
      { fileId: "main", needle: "x_0" },
    ];
    expect(sortAuthoredAnchors(snapshot, anchors)).toEqual([
      anchors[1]!,
      anchors[0]!,
    ]);
  });

  test("keeps cursor-entity and selected-formula decisions independent", () => {
    const fixture = parseAuthoredScientificFixture(fixtureValue("holdout", 1));
    const probe = fixture.probes[0]!;
    const schema2: AuthoredScientificFixture = {
      ...fixture,
      probes: [{
        ...probe,
        expected: {
          ...probe.expected,
          cursorOccurrence: { fileId: "main", needle: "x_0" },
          decision: "partial",
          formulaDecision: {
            anchor: { fileId: "main", needle: "x_0=y_0" },
            status: "ambiguous",
          },
          navigation: {
            ...probe.expected.navigation,
            rename: {
              ...probe.expected.navigation.rename,
              newName: "z_0",
            },
          },
        },
      }],
      schemaVersion: 2,
    };
    expect(() => parseAuthoredScientificFixture(schema2)).not.toThrow();
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

function refusedSurfaceAuthorizations(): NonNullable<
  AuthoredScientificObservation["surfaceAuthorizations"]
> {
  const refusal = { refusalKind: "unsupported", status: "refused" } as const;
  return {
    definition: refusal,
    prepareRename: refusal,
    references: refusal,
    rename: refusal,
  };
}

function interpretationSet(
  relationId: string,
  kind: "structural-alternative" | "typed-law",
  support: "supported" | "tentative",
): MathInterpretationSetInfo {
  return {
    hypotheses: [{ kind, relation: { relationId }, support }],
  } as unknown as MathInterpretationSetInfo;
}

function authoritativeSourceMeaningSet(
  relationId: string,
): MathInterpretationSetInfo {
  const range = { startOffset: 35, endOffset: 44 };
  const formula = {
    documentVersion: 1,
    location: { fileId: "main", path: "main.tex", range },
    scopePath: [],
    sourceNotation: "$x_0=y_0$",
  };
  const evidence = {
    kind: "canonical-math" as const,
    ruleId: "test/source-meaning",
    sourceRanges: [range],
    strength: "hard" as const,
  };
  return {
    hypotheses: [{
      bindings: [],
      conditions: [],
      documentVersion: 1,
      evidence: [{
        evidence,
        provenance: "explicit-declaration",
        role: "supporting",
        sourceAnchors: [{
          documentVersion: 1,
          generation: "authored",
          lifecycle: "current",
          location: formula.location,
          scopePath: [],
        }],
      }],
      formula,
      hypothesisId: relationId,
      kind: "source-meaning",
      label: relationId,
      location: formula.location,
      missingDiscriminatorIds: [],
      orderingReasons: [],
      range,
      rank: 0,
      relation: { relationId },
      scopePath: [],
      support: "explicit",
    }],
  } as unknown as MathInterpretationSetInfo;
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
        rename?: Record<string, unknown>;
      };
      excludedRelationIds: string[];
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
