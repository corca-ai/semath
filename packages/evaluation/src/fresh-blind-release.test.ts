import { createHash } from "node:crypto";
import { describe, expect, test } from "bun:test";
import { freshAuthoringSyntaxFactsForSelections } from "../../../scripts/fresh-blind-evidence";
import {
  AUTHORED_AREA_ALLOCATION,
  authoredFixtureSealPayload,
  authoredScenarioReviewPayload,
  parseAuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredArea,
} from "./authored-scientific";
import type { MathAuthoringContext } from "../../protocol/src/index";
import { projectMathAuthoringContext } from "./math-authoring-development";
import {
  freshBlindAuthoringSafetySummary,
  freshBlindSafetyGateFailed,
  freshBlindSafetySummary,
  freshBlindSealPayload,
  parseFreshBlindReleaseFixture,
  planFreshBlindSnapshotTransitions,
  selectFreshBlindOccurrence,
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
    const wrongField = fixtureValue();
    wrongField.fixture.scenarios[0]!.field = "cross-field";
    const wrongFieldRelease = finalize(wrongField);
    expect(() =>
      validateFreshBlindRelease(wrongFieldRelease, validation(wrongFieldRelease))
    ).toThrow("requires exactly 6 holdout scenarios");
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

  test("retains the exact authoring oracle for immutable v0.37-v0.40 evidence", () => {
    const value = fixtureValue();
    value.release.id = "v0.37";
    const release = finalize(value);
    expect(() => validateFreshBlindRelease(release, validation(release))).toThrow(
      "requires an exact authoring context for every primary and breadth probe",
    );

    const validValue = fixtureValue();
    validValue.release.id = "v0.37";
    addAuthoringExpectations(validValue);
    const valid = finalize(validValue);
    const validInput = validation(valid);
    expect(validateFreshBlindRelease(valid, validInput).scenarios).toBe(48);
    expect(() =>
      validateFreshBlindRelease(valid, { ...validInput, authoringSyntaxFacts: [] })
    ).toThrow("must cover every selected snapshot");
    expect(() =>
      validateFreshBlindRelease(valid, {
        ...validInput,
        authoringSyntaxFacts: [
          validInput.authoringSyntaxFacts[0]!,
          validInput.authoringSyntaxFacts[0]!,
          ...validInput.authoringSyntaxFacts.slice(1),
        ],
      })
    ).toThrow("duplicate fresh authoring syntax facts");
    expect(() =>
      validateFreshBlindRelease(valid, {
        ...validInput,
        authoringSyntaxFacts: [{
          ...validInput.authoringSyntaxFacts[0]!,
          documents: [],
        }, ...validInput.authoringSyntaxFacts.slice(1)],
      })
    ).toThrow("syntax document facts do not match the selected snapshot");
    const firstFact = validInput.authoringSyntaxFacts[0]!;
    const firstDocument = firstFact.documents[0]!;
    expect(() =>
      validateFreshBlindRelease(valid, {
        ...validInput,
        authoringSyntaxFacts: [{
          ...firstFact,
          documents: [{
            ...firstDocument,
            mathRootContentRanges: [
              firstDocument.mathRootContentRanges[0]!,
              firstDocument.mathRootContentRanges[0]!,
            ],
          }],
        }, ...validInput.authoringSyntaxFacts.slice(1)],
      })
    ).toThrow("duplicate math-root facts");
    expect(() =>
      validateFreshBlindRelease(valid, {
        ...validInput,
        authoringSyntaxFacts: [...validInput.authoringSyntaxFacts, {
          documents: [],
          scenarioId: "unknown",
          snapshotId: "initial",
        }],
      })
    ).toThrow("unexpected fresh authoring syntax facts");

    const wrongVersionValue = fixtureValue();
    wrongVersionValue.release.id = "v0.37";
    addAuthoringExpectations(wrongVersionValue);
    const wrongExpected = wrongVersionValue.fixture.probes[0]!.expected as {
      authoringContext: ReturnType<typeof projectMathAuthoringContext>;
    };
    wrongExpected.authoringContext.lifecycle.documentVersion = 2;
    const wrongVersion = finalize(wrongVersionValue);
    expect(() =>
      validateFreshBlindRelease(wrongVersion, validation(wrongVersion))
    ).toThrow("authoringContext.lifecycle.documentVersion");

    const wrongRootValue = fixtureValue();
    wrongRootValue.release.id = "v0.37";
    addAuthoringExpectations(wrongRootValue);
    const wrongRootExpected = wrongRootValue.fixture.probes[0]!.expected as {
      authoringContext: ReturnType<typeof projectMathAuthoringContext>;
    };
    const formula = wrongRootExpected.authoringContext.formula!;
    formula.location.range.endOffset -= 1;
    formula.sourceNotation = formula.sourceNotation.slice(0, -1);
    const wrongRoot = finalize(wrongRootValue);
    expect(() =>
      validateFreshBlindRelease(wrongRoot, validation(wrongRoot))
    ).toThrow("authoringContext.formula.location.range");

    const wrongNotationValue = fixtureValue();
    wrongNotationValue.release.id = "v0.37";
    addAuthoringExpectations(wrongNotationValue);
    const wrongNotationExpected = wrongNotationValue.fixture.probes[0]!.expected as {
      authoringContext: ReturnType<typeof projectMathAuthoringContext>;
    };
    wrongNotationExpected.authoringContext.formula!.sourceNotation = "x = 1";
    const wrongNotation = finalize(wrongNotationValue);
    expect(() =>
      validateFreshBlindRelease(wrongNotation, validation(wrongNotation))
    ).toThrow("authoringContext.formula.sourceNotation");
  });

  test("replaces guessed exact contexts with a sealed safety contract from v0.41", () => {
    const value = fixtureValue();
    addAuthoringSafety(value);
    const release = finalize(value);
    expect(validateFreshBlindRelease(release, validation(release)).scenarios).toBe(48);

    const premature = fixtureValue();
    addAuthoringSafety(premature);
    premature.release.id = "v0.40";
    const prematureRelease = finalize(premature);
    expect(() =>
      validateFreshBlindRelease(prematureRelease, validation(prematureRelease))
    ).toThrow("schema 2 is reserved for v0.41 and later");

    const legacy = fixtureValue();
    legacy.release.id = "v0.41";
    const legacyRelease = finalize(legacy);
    expect(() =>
      validateFreshBlindRelease(legacyRelease, validation(legacyRelease))
    ).toThrow("v0.41+ requires the sealed authoring safety contract");

    const guessed = fixtureValue();
    addAuthoringSafety(guessed);
    addAuthoringExpectations(guessed);
    const guessedRelease = finalize(guessed);
    expect(() =>
      validateFreshBlindRelease(guessedRelease, validation(guessedRelease))
    ).toThrow("forbids guessed exact authoring contexts");

    const incomplete = fixtureValue();
    addAuthoringSafety(incomplete);
    incomplete.authoringSafety!.pop();
    const incompleteRelease = finalize(incomplete);
    expect(() =>
      validateFreshBlindRelease(incompleteRelease, validation(incompleteRelease))
    ).toThrow("must cover every primary and breadth probe");

    const reordered = fixtureValue();
    addAuthoringSafety(reordered);
    reordered.authoringSafety!.reverse();
    const reorderedRelease = finalize(reordered);
    expect(() =>
      validateFreshBlindRelease(reorderedRelease, validation(reorderedRelease))
    ).toThrow("must follow canonical probe order");

    const unallowed = fixtureValue();
    addAuthoringSafety(unallowed);
    unallowed.authoringSafety![0]!.requiredAuthority = [{
      anchor: { fileId: "main", needle: "$x_0=1$" },
      kind: "source-meaning",
      relationId: null,
    }];
    const unallowedRelease = finalize(unallowed);
    expect(() =>
      validateFreshBlindRelease(unallowedRelease, validation(unallowedRelease))
    ).toThrow("required selectors must be allowed");
  });

  test("commissions schema 3 with separate entity and formula decisions", () => {
    const value = fixtureValue();
    addDecisionDomains(value);
    const release = finalize(value);
    const summary = validateFreshBlindRelease(release, validation(release));
    expect(summary.scenarios).toBe(48);
    expect(release).toMatchObject({
      fixture: { schemaVersion: 2 },
      release: { id: "v0.42" },
      schemaVersion: 3,
    });
    expect(summary.entityDecisions).toEqual({
      ambiguous: 10,
      conflicting: 9,
      established: 10,
      partial: 10,
      unsupported: 9,
    });
    expect(summary.formulaDecisions).toEqual({
      ambiguous: 10,
      conflicting: 10,
      established: 10,
      partial: 10,
      unsupported: 8,
    });

    const wrongAllocation = fixtureValue();
    addDecisionDomains(wrongAllocation);
    const wrongProbe = wrongAllocation.fixture.probes[0]!.expected as {
      formulaDecision: { status: string };
    };
    wrongProbe.formulaDecision.status = "partial";
    const wrongAllocationRelease = finalize(wrongAllocation);
    expect(() =>
      validateFreshBlindRelease(
        wrongAllocationRelease,
        validation(wrongAllocationRelease),
      )
    ).toThrow("requires exactly");

    const legacyInner = fixtureValue();
    addAuthoringSafety(legacyInner);
    legacyInner.release.id = "v0.42";
    legacyInner.schemaVersion = 3;
    expect(() => finalize(legacyInner)).toThrow(
      "schema 3 requires authored fixture schema 2",
    );

    const legacyOuter = fixtureValue();
    addDecisionDomains(legacyOuter);
    legacyOuter.schemaVersion = 2;
    expect(() => finalize(legacyOuter)).toThrow(
      "schema 2 requires authored fixture schema 1",
    );

    const wrongFormula = fixtureValue();
    addDecisionDomains(wrongFormula);
    const expected = wrongFormula.fixture.probes[0]!.expected as {
      formulaDecision: {
        anchor: { selection: { length: number; offset: number } };
      };
    };
    expected.formulaDecision.anchor.selection.length -= 1;
    const invalid = finalize(wrongFormula);
    expect(() => validateFreshBlindRelease(invalid, validation(invalid)))
      .toThrow("formulaDecision.anchor must equal one selected math root");

    const boundaryCursor = fixtureValue();
    addDecisionDomains(boundaryCursor);
    (boundaryCursor.fixture.probes[0]!.cursor as Record<string, unknown>).offset =
      "x_0=1".length + 1;
    const boundaryRelease = finalize(boundaryCursor);
    expect(() =>
      validateFreshBlindRelease(boundaryRelease, validation(boundaryRelease))
    ).toThrow("cursor and formulaDecision.anchor must select the same math root");

    const invalidFacts = validation(release);
    invalidFacts.authoringSyntaxFacts[0]!.documents[0]!
      .mathRootContentRanges[0]!.startOffset = -1;
    expect(() => validateFreshBlindRelease(release, invalidFacts))
      .toThrow("invalid formula math-root fact");
    const invalidOccurrenceFacts = validation(release);
    const invalidOccurrence = invalidOccurrenceFacts.authoringSyntaxFacts[0]!
      .documents[0]!.occurrences![0]!;
    invalidOccurrence.selectionRange.startOffset =
      invalidOccurrence.range.startOffset - 1;
    expect(() => validateFreshBlindRelease(release, invalidOccurrenceFacts))
      .toThrow("invalid occurrence syntax fact");

    for (const [formula, composite] of [
      ["x_0=1", "x_0"],
      ["\\hat{x}=1", "\\hat{x}"],
      ["\\mathbf{x}=1", "\\mathbf{x}"],
    ] as const) {
      const baseFocused = fixtureValue();
      addDecisionDomains(baseFocused);
      replaceFirstFormula(baseFocused, formula, composite);
      const baseExpected = baseFocused.fixture.probes[0]!.expected as {
        cursorOccurrence: {
          needle: string;
          selection: { length: number; offset: number };
        };
        symbol: string;
      };
      baseExpected.cursorOccurrence.selection = {
        length: 1,
        offset: baseExpected.cursorOccurrence.needle.indexOf("x"),
      };
      baseExpected.symbol = "x";
      const baseNavigation = baseExpected as typeof baseExpected & {
        navigation: { rename: { newName: string } };
      };
      baseNavigation.navigation.rename.newName = "y";
      const baseRelease = finalize(baseFocused);
      expect(() =>
        validateFreshBlindRelease(baseRelease, validation(baseRelease))
      ).toThrow("must equal syntax-selected cursor ownership");
    }

    const compositeRename = fixtureValue();
    addDecisionDomains(compositeRename);
    replaceFirstFormula(compositeRename, "\\hat{x}=1", "\\hat{x}");
    const compositeExpected = compositeRename.fixture.probes[0]!.expected as
      unknown as AuthoredProbeExpected;
    const compositeAnchor = compositeExpected.cursorOccurrence!;
    const available = {
      allowed: [compositeAnchor],
      excluded: [],
      minimum: 1,
      required: [compositeAnchor],
      status: "available" as const,
    };
    compositeExpected.navigation.definition = structuredClone(available);
    compositeExpected.navigation.references = structuredClone(available);
    compositeExpected.navigation.prepareRename = {
      placeholder: "\\hat{x}",
      range: compositeAnchor,
      status: "available",
    };
    compositeExpected.navigation.rename = {
      ...structuredClone(available),
      expectedText: "\\hat{x}",
      newName: "\\hat{y}",
      replacementText: "\\hat{y}",
      safety: "reviewed composite identity",
    };
    const compositeRenameRelease = finalize(compositeRename);
    expect(() =>
      validateFreshBlindRelease(
        compositeRenameRelease,
        validation(compositeRenameRelease),
      )
    ).not.toThrow();
    const orderedOccurrences = fixtureValue();
    addDecisionDomains(orderedOccurrences);
    replaceFirstFormula(orderedOccurrences, "x+y=1", "x");
    const orderedOccurrenceRelease = finalize(orderedOccurrences);
    const nonCanonicalOccurrences = validation(orderedOccurrenceRelease);
    const [firstOccurrenceFact, ...remainingOccurrenceFacts] =
      nonCanonicalOccurrences.authoringSyntaxFacts;
    const [firstOccurrenceDocument, ...remainingOccurrenceDocuments] =
      firstOccurrenceFact!.documents;
    nonCanonicalOccurrences.authoringSyntaxFacts = [{
      ...firstOccurrenceFact!,
      documents: [{
        ...firstOccurrenceDocument!,
        occurrences: [...firstOccurrenceDocument!.occurrences!].reverse(),
      }, ...remainingOccurrenceDocuments],
    }, ...remainingOccurrenceFacts];
    expect(() =>
      validateFreshBlindRelease(
        orderedOccurrenceRelease,
        nonCanonicalOccurrences,
      )
    ).toThrow("occurrence syntax facts are not canonical");

    const wrongCompositeFamily = fixtureValue();
    addDecisionDomains(wrongCompositeFamily);
    replaceFirstFormula(wrongCompositeFamily, "\\hat{x}=1", "\\hat{x}");
    const wrongCompositeExpected = wrongCompositeFamily.fixture.probes[0]!
      .expected as unknown as AuthoredProbeExpected;
    const wrongCompositeAnchor = wrongCompositeExpected.cursorOccurrence!;
    wrongCompositeExpected.navigation.definition = structuredClone({
      ...available,
      allowed: [wrongCompositeAnchor],
      required: [wrongCompositeAnchor],
    });
    wrongCompositeExpected.navigation.references = structuredClone(
      wrongCompositeExpected.navigation.definition,
    );
    wrongCompositeExpected.navigation.prepareRename = {
      placeholder: "\\hat{x}",
      range: wrongCompositeAnchor,
      status: "available",
    };
    wrongCompositeExpected.navigation.rename = {
      ...structuredClone(wrongCompositeExpected.navigation.definition),
      expectedText: "\\hat{x}",
      newName: "\\bar{x}",
      replacementText: "\\bar{x}",
      safety: "invalid family mutation",
    };
    const wrongCompositeRelease = finalize(wrongCompositeFamily);
    expect(() =>
      validateFreshBlindRelease(
        wrongCompositeRelease,
        validation(wrongCompositeRelease),
      )
    ).toThrow("preserve one exact editable notation family");

    const missingOccurrences = validation(release);
    delete (missingOccurrences.authoringSyntaxFacts[0]!.documents[0]! as {
      occurrences?: unknown;
    }).occurrences;
    expect(() => validateFreshBlindRelease(release, missingOccurrences))
      .toThrow("requires complete occurrence syntax facts");
  });

  test("mirrors exact, trailing, application, gap, and ambiguous cursor ownership", () => {
    const occurrence = (
      startOffset: number,
      endOffset: number,
      selectionStart = startOffset,
      selectionEnd = endOffset,
      applicationEndOffset?: number,
    ) => ({
      ...(applicationEndOffset === undefined ? {} : { applicationEndOffset }),
      kind: "identifier" as const,
      range: { startOffset, endOffset },
      selectionRange: {
        startOffset: selectionStart,
        endOffset: selectionEnd,
      },
      surface: "x",
    });
    const decorated = occurrence(6, 7, 1, 8, 11);
    for (const offset of [1, 6, 7, 8, 11]) {
      expect(selectFreshBlindOccurrence([decorated], offset)).toBe(decorated);
    }
    const left = occurrence(1, 2);
    const right = occurrence(2, 3);
    expect(selectFreshBlindOccurrence([left, right], 2)).toBe(right);

    const indexed = occurrence(1, 4, 1, 2);
    const index = occurrence(3, 4);
    expect(selectFreshBlindOccurrence([indexed, index], 3)).toBe(index);
    expect(selectFreshBlindOccurrence([indexed, index], 4)).toBe(indexed);

    const container = occurrence(1, 10);
    const nested = occurrence(4, 5);
    expect(selectFreshBlindOccurrence([container, nested], 5)).toBe(nested);
    expect(selectFreshBlindOccurrence([
      occurrence(1, 6),
      nested,
    ], 5)?.range).toEqual({ startOffset: 1, endOffset: 6 });

    expect(selectFreshBlindOccurrence([left, right], 6)).toBeUndefined();
    expect(selectFreshBlindOccurrence([
      occurrence(1, 2, 0, 4),
      occurrence(1, 2, 0, 5),
    ], 3)).toBeUndefined();
  });

  test("uses an explicit navigation envelope without treating required as exhaustive", () => {
    const value = fixtureValue();
    addDecisionDomains(value);
    const expected = value.fixture.probes[0]!.expected as unknown as {
      navigation: {
        definition: Record<string, unknown>;
        references: Record<string, unknown>;
      };
    };
    const allowed = {
      fileId: "main",
      needle: "$x_0=1$",
      selection: { length: 3, offset: 1 },
    };
    const locationEnvelope = {
      allowed: [allowed],
      excluded: [],
      minimum: 0,
      required: [],
      status: "available",
    };
    expected.navigation.definition = structuredClone(locationEnvelope);
    expected.navigation.references = structuredClone(locationEnvelope);
    const release = finalize(value);
    expect(() => validateFreshBlindRelease(release, validation(release)))
      .not.toThrow();
    const document = release.fixture.scenarios[0]!.snapshots[0]!.documents[0]!;
    const startOffset = document.content.indexOf("x_0");
    const observation = {
      caseId: "probe-00",
      decision: "partial" as const,
      definitions: [{
        fileId: document.fileId,
        path: document.path,
        range: { startOffset, endOffset: startOffset + 3 },
      }],
      diagnostics: [],
      prepareRename: {},
      proofGrounded: false,
      references: [{
        fileId: document.fileId,
        path: document.path,
        range: { startOffset, endOffset: startOffset + 3 },
      }],
      relations: [],
      renameEdits: [],
      symbol: "x_0",
    };
    expect(freshBlindSafetySummary(release.fixture, [observation]))
      .toMatchObject({ unsafeNavigationOrEditLocations: 0 });
    expect(freshBlindSafetySummary(release.fixture, [{
      ...observation,
      references: [{
        ...observation.references[0]!,
        range: { startOffset: startOffset + 1, endOffset: startOffset + 2 },
      }],
    }])).toMatchObject({ unsafeNavigationOrEditLocations: 1 });
  });

  test("rejects authorized-empty unavailable surfaces and divergent authorizations", () => {
    const value = fixtureValue();
    addDecisionDomains(value);
    const release = finalize(value);
    const probe = release.fixture.probes[0]!;
    const refused = { refusalKind: "unsupported" as const, status: "refused" as const };
    const focusOccurrenceId = {
      documentVersion: 1,
      fileId: "main",
      localId: 1,
    };
    const authorized = {
      entityId: {
        anchor: focusOccurrenceId,
        componentId: "component:a",
        kind: "symbol",
        scopePath: [],
      },
      focusOccurrenceId,
      status: "authorized" as const,
    };
    const scenario = release.fixture.scenarios.find(
      (candidate) => candidate.id === probe.scenarioId,
    )!;
    const document = scenario.snapshots[0]!.documents[0]!;
    const startOffset = document.content.indexOf("x_0");
    const observation: AuthoredScientificObservation = {
      authoringContext: {
        ...unsupportedAuthoringContext(),
        notationOccurrences: [{
          entityId: {
            ...authorized.entityId,
            componentId: "poisoned-authoring-projection",
          },
          location: {
            fileId: document.fileId,
            path: document.path,
            range: { startOffset, endOffset: startOffset + 3 },
          },
          occurrenceId: { ...focusOccurrenceId, localId: 99 },
          scopePath: [],
          sourceNotation: "x_0",
        }],
      },
      caseId: probe.id,
      cursorSurfaceIdentity: {
        entityId: authorized.entityId,
        location: {
          fileId: document.fileId,
          path: document.path,
          range: { startOffset, endOffset: startOffset + 3 },
        },
        occurrenceId: focusOccurrenceId,
      },
      decision: probe.expected.decision,
      definitions: [],
      diagnostics: [],
      prepareRename: {},
      proofGrounded: probe.expected.proofGrounded,
      references: [],
      relations: [],
      renameEdits: [],
      surfaceAuthorizations: {
        definition: authorized,
        prepareRename: refused,
        references: refused,
        rename: refused,
      },
      symbol: null,
    };
    expect(freshBlindSafetySummary(release.fixture, [observation]))
      .toMatchObject({
        unsafeNavigationOrEditCaseIds: [probe.id],
        unsafeNavigationOrEditLocations: 1,
      });

    const navigation = probe.expected.navigation as unknown as {
      definition: {
        allowed?: unknown[];
        minimum: number;
        status: "available" | "unavailable";
      };
      prepareRename: { status: "available" | "unavailable" };
      references: {
        allowed?: unknown[];
        minimum: number;
        status: "available" | "unavailable";
      };
      rename: {
        allowed?: unknown[];
        minimum: number;
        status: "available" | "unavailable";
      };
    };
    navigation.definition.status = "available";
    navigation.references.status = "available";
    navigation.prepareRename.status = "available";
    navigation.rename.status = "available";
    navigation.definition.allowed = [probe.expected.cursorOccurrence];
    navigation.references.allowed = [probe.expected.cursorOccurrence];
    navigation.rename.allowed = [probe.expected.cursorOccurrence];
    const availablePreparation = navigation.prepareRename as
      typeof navigation.prepareRename & {
        placeholder?: string;
        range?: unknown;
      };
    availablePreparation.placeholder = "x_0";
    availablePreparation.range = probe.expected.cursorOccurrence;
    const consistent = {
      ...observation,
      definitions: [{
        fileId: document.fileId,
        path: document.path,
        range: { startOffset, endOffset: startOffset + 3 },
      }],
      prepareRename: {
        placeholder: "x_0",
        range: { startOffset, endOffset: startOffset + 3 },
      },
      references: [{
        fileId: document.fileId,
        path: document.path,
        range: { startOffset, endOffset: startOffset + 3 },
      }],
      renameEdits: [{
        expectedText: "x_0",
        fileId: document.fileId,
        path: document.path,
        range: { startOffset, endOffset: startOffset + 3 },
        replacementText: "y_0",
      }],
      surfaceAuthorizations: {
        definition: authorized,
        prepareRename: authorized,
        references: authorized,
        rename: authorized,
      },
    };
    expect(freshBlindSafetySummary(release.fixture, [consistent]))
      .toMatchObject({ unsafeNavigationOrEditLocations: 0 });
    expect(freshBlindSafetySummary(release.fixture, [{
      ...consistent,
      cursorSurfaceIdentity: {
        ...consistent.cursorSurfaceIdentity!,
        entityId: {
          ...authorized.entityId,
          componentId: "wrong-semantic-view-entity",
        },
      },
    }])).toMatchObject({
      unsafeNavigationOrEditCaseIds: [probe.id],
      unsafeNavigationOrEditLocations: 1,
    });
    expect(freshBlindSafetySummary(release.fixture, [{
      ...consistent,
      surfaceAuthorizations: {
        definition: authorized,
        prepareRename: authorized,
        references: {
          ...authorized,
          focusOccurrenceId: { ...focusOccurrenceId, localId: 2 },
        },
        rename: authorized,
      },
    }])).toMatchObject({
      unsafeNavigationOrEditCaseIds: [probe.id],
      unsafeNavigationOrEditLocations: 1,
    });

    const requiredNavigation = navigation as typeof navigation & {
      definition: { minimum: number };
      prepareRename: { placeholder?: string; range?: unknown };
      references: { minimum: number };
      rename: { minimum: number };
    };
    requiredNavigation.definition.minimum = 1;
    requiredNavigation.references.minimum = 1;
    requiredNavigation.rename.minimum = 1;
    expect(freshBlindSafetySummary(release.fixture, [{
      ...consistent,
      definitions: [],
      prepareRename: {},
      references: [],
      renameEdits: [],
    }]))
      .toMatchObject({
        unsafeNavigationOrEditCaseIds: [probe.id],
        unsafeNavigationOrEditLocations: 4,
      });
  });

  test("gates only reviewed authoring authority, contradiction, and lifecycle boundaries", () => {
    const value = fixtureValue();
    addAuthoringSafety(value);
    const release = finalize(value);
    const observations = release.fixture.probes.map((probe) => ({
      authoringContext: unsupportedAuthoringContext(),
      caseId: probe.id,
      decision: "unsupported" as const,
      definitions: [],
      diagnostics: [],
      prepareRename: {},
      proofGrounded: false,
      references: [],
      relations: [],
      renameEdits: [],
      symbol: null,
    }));
    expect(freshBlindAuthoringSafetySummary(release, observations)).toEqual({
      cases: 48,
      failures: [],
    });

    const first = observations[0]!;
    const document = release.fixture.scenarios[0]!.snapshots[0]!.documents[0]!;
    const needle = "$x_0=1$";
    const startOffset = document.content.indexOf(needle);
    const authority = {
      ...first,
      authoringContext: {
        ...first.authoringContext,
        disposition: "established" as const,
        interpretations: {
          ...first.authoringContext.interpretations,
          hypotheses: [{
            bindings: [],
            conditions: [],
            documentVersion: 1,
            evidence: [],
            formula: {
              documentVersion: 1,
              location: {
                fileId: document.fileId,
                path: document.path,
                range: { startOffset, endOffset: startOffset + needle.length },
              },
              scopePath: [],
              sourceNotation: needle,
            },
            hypothesisId: "unexpected-authority",
            kind: "source-meaning" as const,
            label: "unexpected authority",
            location: {
              fileId: document.fileId,
              path: document.path,
              range: { startOffset, endOffset: startOffset + needle.length },
            },
            missingDiscriminatorIds: [],
            orderingReasons: [],
            range: { startOffset, endOffset: startOffset + needle.length },
            rank: 0,
            scopePath: [],
            support: "explicit" as const,
          }],
        },
        lifecycle: { ...first.authoringContext.lifecycle, editable: false },
      },
    };
    const unsafe = freshBlindAuthoringSafetySummary(release, [
      authority,
      ...observations.slice(1),
    ]);
    expect(unsafe.failures.map((failure) => failure.kind)).toEqual([
      "unsafe-lifecycle",
      "authority-escalation",
      "authority-escalation",
    ]);

    const permitted = fixtureValue();
    addAuthoringSafety(permitted);
    const selector = {
      anchor: { fileId: document.fileId, needle },
      kind: "source-meaning",
      relationId: null,
    };
    permitted.authoringSafety![0] = {
      ...permitted.authoringSafety![0]!,
      allowedAuthority: [selector],
      forbiddenDispositions: [],
      lifecycle: {
        ...permitted.authoringSafety![0]!.lifecycle,
        editable: false,
      },
      requiredAuthority: [selector],
    };
    expect(
      freshBlindAuthoringSafetySummary(finalize(permitted), [
        authority,
        ...observations.slice(1),
      ]).failures,
    ).toEqual([]);

    const missingRequired = freshBlindAuthoringSafetySummary(
      finalize(permitted),
      [{
        ...first,
        authoringContext: {
          ...first.authoringContext,
          lifecycle: { ...first.authoringContext.lifecycle, editable: false },
        },
      }, ...observations.slice(1)],
    );
    expect(missingRequired.failures).toEqual([
      {
        expected: selector,
        kind: "missing",
        path: `${first.caseId}.authoringContext.interpretations.authority.required[0]`,
      },
    ]);

    const contradicted = {
      ...first,
      authoringContext: {
        ...first.authoringContext,
        disposition: "conflicting" as const,
        interpretations: {
          ...first.authoringContext.interpretations,
          hypotheses: [{
            ...authority.authoringContext.interpretations.hypotheses[0]!,
            evidence: [{
              evidence: {
                kind: "synthetic-source-contradiction",
                ruleId: "synthetic-source-contradiction",
                sourceRanges: [{
                  startOffset,
                  endOffset: startOffset + needle.length,
                }],
                strength: "asserted",
              },
              provenance: "explicit-declaration" as const,
              role: "contradicting" as const,
              sourceAnchors: [{
                documentVersion: 1,
                generation: "authored" as const,
                lifecycle: "current" as const,
                location: {
                  fileId: document.fileId,
                  path: document.path,
                  range: {
                    startOffset,
                    endOffset: startOffset + needle.length,
                  },
                },
                scopePath: [],
              }],
            }],
            support: "contradicted" as const,
          }],
        },
      },
    };
    const unexpectedContradiction = freshBlindAuthoringSafetySummary(release, [
      contradicted,
      ...observations.slice(1),
    ]);
    expect(unexpectedContradiction.failures.map((failure) => failure.kind)).toEqual([
      "false-conflict",
      "false-conflict",
    ]);
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

    const invalidFamily = fixtureValue();
    const invalidExpected = invalidFamily.fixture.probes[0]!.expected as {
      navigation: { rename: Record<string, unknown> };
    };
    invalidExpected.navigation.rename = {
      excluded: [],
      expectedText: "x",
      minimum: 1,
      newName: "renamed",
      replacementText: "renamed",
      required: [{ fileId: "main", needle: "$x_0=1$" }],
      safety: "deterministic",
      status: "available",
    };
    const invalidRelease = finalize(invalidFamily);
    expect(() =>
      validateFreshBlindRelease(invalidRelease, validation(invalidRelease)),
    ).toThrow("rename must preserve one exact editable notation family");
  });

  test("requires complete navigation allowlists before an engine run", () => {
    const incomplete = fixtureValue();
    const expected = incomplete.fixture.probes[0]!.expected as {
      navigation: { definition: Record<string, unknown> };
    };
    expected.navigation.definition = {
      excluded: [],
      minimum: 2,
      required: [{ fileId: "main", needle: "$x_0=1$" }],
      status: "available",
    };
    const release = finalize(incomplete);
    expect(() =>
      validateFreshBlindRelease(release, validation(release)),
    ).toThrow("definition must enumerate its complete location allowlist");

    const incompletePreparation = fixtureValue();
    const preparation = incompletePreparation.fixture.probes[0]!.expected as {
      navigation: { prepareRename: Record<string, unknown> };
    };
    preparation.navigation.prepareRename = { status: "available" };
    const preparationRelease = finalize(incompletePreparation);
    expect(() =>
      validateFreshBlindRelease(
        preparationRelease,
        validation(preparationRelease),
      ),
    ).toThrow(
      "available prepareRename requires an exact range and placeholder",
    );
  });

  test("uses reviewed syntax-scoped entity envelopes from v0.35 onward", () => {
    const value = fixtureValue();
    value.release.id = "v0.35";
    const probe = value.fixture.probes[0]!;
    const expected = probe.expected as AuthoredProbeExpected;
    expected.symbol = "x_0";
    const exact = {
      fileId: "main",
      needle: "$x_0=1$",
      selection: { offset: 1, length: 3 },
    };
    expected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [exact],
      status: "available",
    };
    expected.navigation.references = {
      excluded: [],
      minimum: 1,
      required: [exact],
      status: "available",
    };
    const release = finalize(value);
    expect(validateFreshBlindRelease(release, validation(release)).scenarios).toBe(48);

    const incomplete = fixtureValue();
    incomplete.release.id = "v0.35";
    const incompleteProbe = incomplete.fixture.probes[0]!;
    const incompleteExpected = incompleteProbe.expected as AuthoredProbeExpected;
    incompleteExpected.symbol = "x_0";
    incomplete.fixture.scenarios[0]!.snapshots[0]!.documents[0]!.content +=
      " The same quantity is $x_0$.";
    incompleteExpected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [exact],
      status: "available",
    };
    incompleteExpected.navigation.references = {
      excluded: [],
      minimum: 1,
      required: [exact],
      status: "available",
    };
    const incompleteRelease = finalize(incomplete);
    expect(() =>
      validateFreshBlindRelease(
        incompleteRelease,
        validation(incompleteRelease),
      )
    ).not.toThrow();

    const nonOccurrence = fixtureValue();
    nonOccurrence.release.id = "v0.35";
    const nonOccurrenceExpected = nonOccurrence.fixture.probes[0]!.expected as
      AuthoredProbeExpected;
    nonOccurrenceExpected.symbol = "x_0";
    const equals = {
      fileId: "main",
      needle: "$x_0=1$",
      selection: { offset: 4, length: 1 },
    };
    nonOccurrenceExpected.navigation.definition = {
      excluded: [],
      minimum: 1,
      required: [equals],
      status: "available",
    };
    nonOccurrenceExpected.navigation.references = {
      excluded: [],
      minimum: 1,
      required: [equals],
      status: "available",
    };
    const nonOccurrenceRelease = finalize(nonOccurrence);
    expect(() =>
      validateFreshBlindRelease(
        nonOccurrenceRelease,
        validation(nonOccurrenceRelease),
      )
    ).toThrow("must equal one syntax occurrence");
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
    authoringSyntaxFacts: freshAuthoringSyntaxFactsForSelections(
      release.fixture.scenarios.map((scenario) => ({
        scenarioId: scenario.id,
        snapshot: scenario.snapshots[0]!,
      })),
    ),
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

function replaceFirstFormula(
  value: FixtureValue,
  formula: string,
  composite: string,
): void {
  const scenario = value.fixture.scenarios[0]!;
  const document = scenario.snapshots[0]!.documents[0]!;
  document.content = document.content.replace("x_0=1", formula);
  const probe = value.fixture.probes[0]!;
  const cursor = probe.cursor as { needle: string; offset: number };
  cursor.needle = `$${formula}$`;
  cursor.offset = cursor.needle.indexOf("x");
  const expected = probe.expected as {
    cursorOccurrence: {
      fileId: string;
      needle: string;
      selection: { length: number; offset: number };
    };
    formulaDecision: {
      anchor: {
        fileId: string;
        needle: string;
        selection: { length: number; offset: number };
      };
    };
    symbol: string;
  };
  expected.cursorOccurrence = {
    fileId: "main",
    needle: cursor.needle,
    selection: {
      length: composite.length,
      offset: cursor.needle.indexOf(composite),
    },
  };
  expected.formulaDecision.anchor = {
    fileId: "main",
    needle: cursor.needle,
    selection: { length: formula.length, offset: 1 },
  };
  expected.symbol = composite;
  const navigation = expected as typeof expected & {
    navigation: { rename: { newName: string } };
  };
  navigation.navigation.rename.newName = composite.replace("x", "y");
  for (const selector of [
    ...value.authoringSafety![0]!.allowedAuthority,
    ...value.authoringSafety![0]!.requiredAuthority,
  ] as Array<{ anchor: unknown }>) {
    selector.anchor = structuredClone(expected.formulaDecision.anchor);
  }
}

function addAuthoringExpectations(value: FixtureValue): void {
  value.fixture.probes.forEach((probe, index) => {
    const scenario = value.fixture.scenarios[index]!;
    const document = scenario.snapshots[0]!.documents[0]!;
    const sourceNotation = `${testSymbol(index)}=1`;
    const startOffset = document.content.indexOf(sourceNotation);
    const range = {
      endOffset: startOffset + sourceNotation.length,
      startOffset,
    };
    const formula = {
      documentVersion: 1,
      location: { fileId: document.fileId, path: document.path, range },
      scopePath: [],
      sourceNotation,
    };
    const context: MathAuthoringContext = {
      claimEvidence: [],
      conditions: [],
      disposition: "unsupported",
      equationLinks: [],
      formula,
      lifecycle: {
        capped: false,
        documentVersion: 1,
        editable: true,
        engineLimited: false,
        freshness: "current",
        generation: "authored",
        retracted: false,
      },
      interpretations: {
        analysisLimits: [],
        exhaustiveness: "bounded-open-world",
        hypotheses: [],
        missingDiscriminators: [],
        truncated: false,
      },
      notationOccurrences: [],
      requirements: [],
      truncated: false,
    };
    (probe.expected as Record<string, unknown>).authoringContext =
      projectMathAuthoringContext(context);
  });
}

function addAuthoringSafety(value: FixtureValue): void {
  value.release.id = "v0.41";
  value.schemaVersion = 2;
  value.authoringSafety = value.fixture.probes.map((probe) => ({
    allowedAuthority: [],
    allowedContradictions: [],
    forbiddenDispositions: ["conflicting", "established"],
    lifecycle: {
      capped: false,
      editable: true,
      engineLimited: false,
      generation: "authored",
      retracted: false,
    },
    probeId: String(probe.id),
    requiredAuthority: [],
    requiredContradictions: [],
  }));
}

function addDecisionDomains(value: FixtureValue): void {
  addAuthoringSafety(value);
  value.release.id = "v0.42";
  value.schemaVersion = 3;
  value.fixture.schemaVersion = 2;
  value.fixture.probes.forEach((probe, index) => {
    const cursor = probe.cursor as Record<string, unknown>;
    delete cursor.edge;
    cursor.offset = 1;
    const expected = probe.expected as Record<string, unknown>;
    const formulaStatus = schema3FormulaDecision(index);
    expected.proofGrounded = false;
    const symbol = testSymbol(index);
    expected.symbol = symbol;
    expected.cursorOccurrence = {
      fileId: "main",
      needle: `$${symbol}=1$`,
      selection: { length: symbol.length, offset: 1 },
    };
    expected.formulaDecision = {
      anchor: {
        fileId: "main",
        needle: `$${symbol}=1$`,
        selection: { length: `${symbol}=1`.length, offset: 1 },
      },
      status: formulaStatus,
    };
    const safety = value.authoringSafety![index]!;
    if (formulaStatus === "established") {
      const selector = {
        anchor: {
          fileId: "main",
          needle: `$${symbol}=1$`,
          selection: { length: `${symbol}=1`.length, offset: 1 },
        },
        kind: "source-meaning",
        relationId: null,
      };
      safety.allowedAuthority = [selector];
      safety.requiredAuthority = [selector];
      safety.forbiddenDispositions = ["conflicting"];
    } else if (formulaStatus === "conflicting") {
      const selector = {
        anchor: {
          fileId: "main",
          needle: `$${symbol}=1$`,
          selection: { length: `${symbol}=1`.length, offset: 1 },
        },
        kind: "source-meaning",
        relationId: null,
      };
      safety.allowedContradictions = [selector];
      safety.requiredContradictions = [selector];
      safety.forbiddenDispositions = ["established"];
    }
    const navigation = expected.navigation as {
      rename: Record<string, unknown>;
    };
    navigation.rename.newName = symbol.replace("x", "y");
  });
}

function schema3FormulaDecision(index: number) {
  if (index < 10) return "established" as const;
  if (index < 20) return "partial" as const;
  if (index < 30) return "ambiguous" as const;
  if (index < 40) return "conflicting" as const;
  return "unsupported" as const;
}

function unsupportedAuthoringContext(): MathAuthoringContext {
  return {
    claimEvidence: [],
    conditions: [],
    disposition: "unsupported",
    equationLinks: [],
    lifecycle: {
      capped: false,
      documentVersion: 1,
      editable: true,
      engineLimited: false,
      freshness: "current",
      generation: "authored",
      retracted: false,
    },
    interpretations: {
      analysisLimits: [],
      exhaustiveness: "bounded-open-world",
      hypotheses: [],
      missingDiscriminators: [],
      truncated: false,
    },
    notationOccurrences: [],
    requirements: [],
    truncated: false,
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
  const fields = Object.entries(AUTHORED_AREA_ALLOCATION).flatMap(
    ([field, allocation]) =>
      Array.from({ length: allocation.holdout }, () => field as AuthoredArea),
  );
  const scenarios = Array.from({ length: 48 }, (_, index) => ({
    field: fields[index]!,
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
            content: `Independent scientific scene ${index}. The reviewed value is $${testSymbol(index)}=1$.`,
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
          needle: `$${testSymbol(index)}=1$`,
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

function testSymbol(index: number): string {
  return index < 10 ? `x_${index}` : `x_{${index}}`;
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

type FixtureValue = ReturnType<typeof fixtureValueShape>;
type AuthoredProbeExpected = {
  cursorOccurrence?: {
    fileId: string;
    needle: string;
    selection?: { length: number; offset: number };
  } | null;
  navigation: {
    definition: Record<string, unknown>;
    prepareRename: Record<string, unknown>;
    references: Record<string, unknown>;
    rename: Record<string, unknown>;
  };
  symbol?: string;
};

function fixtureValueShape() {
  return {} as {
    authoringSafety?: Array<{
      allowedAuthority: unknown[];
      allowedContradictions: unknown[];
      forbiddenDispositions: string[];
      lifecycle: {
        capped: boolean;
        editable: boolean;
        engineLimited: boolean;
        generation: string;
        retracted: boolean;
      };
      probeId: string;
      requiredAuthority: unknown[];
      requiredContradictions: unknown[];
    }>;
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
        field: AuthoredArea;
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
      schemaVersion: 1 | 2;
    };
    release: {
      createdAt: string;
      frozenAt: string;
      id: string;
      seal: string;
      taskCardDigest: string;
    };
    schemaVersion: 1 | 2 | 3;
  };
}
