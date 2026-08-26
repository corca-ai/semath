import type { SourceRange } from "../../protocol/src/index";
import {
  AUTHORED_AREA_ALLOCATION,
  DOCUMENT_REASONING_FAMILIES,
  authoredRenameNotationFamilyMatches,
  authoredScenarioFor,
  authoredSnapshotFor,
  parseAuthoredScientificFixture,
  resolveAuthoredAnchor,
  scoreAuthoredScientificFixture,
  type AuthoredLawCatalogEntry,
  type AuthoredArea,
  type AuthoredLocationExpectation,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
  type ScientificDecision,
} from "./authored-scientific";
import {
  compareAuthoredIntegrityProfiles,
  type AuthoredIntegrityProfile,
} from "./authored-integrity";
import {
  mathAuthoringExpectationCanonicalFailures,
  mathAuthoringExpectationFormulaRootFailures,
  mathAuthoringExpectationSourceFailures,
  type MathAuthoringSyntaxDocument,
} from "./math-authoring-development";
import {
  parseFreshBlindAuthoringSafety,
  validateFreshAuthoringSafetyExpectations,
  type FreshBlindAuthoringSafetyExpectation,
} from "./fresh-blind-authoring-safety";
export {
  freshBlindAuthoringSafetySummary,
  type FreshBlindAuthoringHypothesisSelector,
  type FreshBlindAuthoringSafetyExpectation,
  type FreshBlindAuthoringSafetySummary,
} from "./fresh-blind-authoring-safety";

const DIGEST = /^[0-9a-f]{64}$/u;
const RELEASE_ID = /^v0\.[1-9][0-9]*$/u;
const REQUIRED_SCENARIOS = 48;
const REQUIRED_FAMILY_SCENARIOS = 8;
const REQUIRED_SCHEMA_3_FORMULA_DECISIONS = {
  ambiguous: 10,
  conflicting: 10,
  established: 10,
  partial: 10,
  unsupported: 8,
} as const satisfies Readonly<Record<ScientificDecision, number>>;

export interface FreshBlindReleaseFixture {
  readonly authoringSafety?: readonly FreshBlindAuthoringSafetyExpectation[];
  readonly commissioning: {
    readonly authoringMethod: "isolated-codex-subagents";
    readonly criticMethod: "independent-codex-subagents";
    readonly engineExecutionsBeforeSeal: 0;
    readonly mainReviewMethod: "complete-source-and-expectation-review";
    readonly mainReviewerId: string;
  };
  readonly fixture: AuthoredScientificFixture;
  readonly release: {
    readonly createdAt: string;
    readonly frozenAt: string;
    readonly id: string;
    readonly seal: string;
    readonly taskCardDigest: string;
  };
  readonly schemaVersion: 1 | 2 | 3;
}

export interface FreshBlindValidationInput {
  readonly authoringSyntaxFacts: readonly FreshBlindSnapshotSyntaxFacts[];
  readonly authoredSealDigest: string;
  readonly freshIsolationProfiles: readonly AuthoredIntegrityProfile[];
  readonly freshProfiles: readonly AuthoredIntegrityProfile[];
  readonly lawCatalog: readonly AuthoredLawCatalogEntry[];
  readonly referenceDocuments: readonly string[];
  readonly referenceProfiles: readonly AuthoredIntegrityProfile[];
  readonly reviewDigests: Readonly<Record<string, string>>;
  readonly sealDigest: string;
}

export interface FreshBlindSnapshotSyntaxFacts {
  readonly documents: readonly {
    readonly fileId: string;
    readonly mathRootContentRanges: readonly SourceRange[];
    readonly occurrences?: readonly FreshBlindOccurrenceFact[];
  }[];
  readonly scenarioId: string;
  readonly snapshotId: string;
}

export interface FreshBlindOccurrenceFact {
  readonly applicationEndOffset?: number;
  readonly kind: "command" | "identifier" | "named-operator";
  readonly range: SourceRange;
  readonly selectionRange: SourceRange;
  readonly surface: string;
}

export interface FreshBlindValidationSummary {
  readonly decisions: Readonly<Record<string, number>>;
  readonly entityDecisions: Readonly<Record<string, number>>;
  readonly families: Readonly<Record<string, number>>;
  readonly formulaDecisions: Readonly<Record<string, number>>;
  readonly fields: Readonly<Record<string, number>>;
  readonly laws: number;
  readonly maximumMathSimilarity: number;
  readonly maximumProseSimilarity: number;
  readonly probes: number;
  readonly scenarios: number;
}

export interface FreshBlindSafetySummary {
  readonly diagnosticsOverLimit: number;
  readonly diagnosticsOverLimitIds: readonly string[];
  readonly falseConflict: number;
  readonly falseConflictIds: readonly string[];
  readonly falseEstablishment: number;
  readonly falseEstablishmentIds: readonly string[];
  /** Unsafe source locations or entity-surface authorization facets. */
  readonly unsafeNavigationOrEditLocations: number;
  /** Probe ids with at least one unsafe navigation/edit surface. */
  readonly unsafeNavigationOrEditCaseIds: readonly string[];
}

export interface FreshBlindSnapshotTransition {
  readonly fromSnapshotId: string;
  readonly scenarioId: string;
  readonly toSnapshotId: string;
}

export function parseFreshBlindReleaseFixture(
  value: unknown,
): FreshBlindReleaseFixture {
  const root = record(value, "fresh blind release");
  const schemaVersion = root.schemaVersion;
  if (schemaVersion !== 1 && schemaVersion !== 2 && schemaVersion !== 3) {
    throw new Error("fresh blind release.schemaVersion: must be 1, 2, or 3");
  }
  exact(
    root,
    schemaVersion === 1
      ? ["schemaVersion", "release", "commissioning", "fixture"]
      : [
          "schemaVersion",
          "release",
          "commissioning",
          "fixture",
          "authoringSafety",
        ],
    "fresh blind release",
  );
  const release = record(root.release, "fresh blind release.release");
  exact(
    release,
    ["id", "createdAt", "frozenAt", "taskCardDigest", "seal"],
    "fresh blind release.release",
  );
  const commissioning = record(
    root.commissioning,
    "fresh blind release.commissioning",
  );
  exact(
    commissioning,
    [
      "authoringMethod",
      "criticMethod",
      "engineExecutionsBeforeSeal",
      "mainReviewMethod",
      "mainReviewerId",
    ],
    "fresh blind release.commissioning",
  );
  literal(
    commissioning.authoringMethod,
    "isolated-codex-subagents",
    "fresh blind release.commissioning.authoringMethod",
  );
  literal(
    commissioning.criticMethod,
    "independent-codex-subagents",
    "fresh blind release.commissioning.criticMethod",
  );
  literal(
    commissioning.mainReviewMethod,
    "complete-source-and-expectation-review",
    "fresh blind release.commissioning.mainReviewMethod",
  );
  if (commissioning.engineExecutionsBeforeSeal !== 0) {
    throw new Error(
      "fresh blind release.commissioning.engineExecutionsBeforeSeal: must be 0",
    );
  }
  const authoringSafety = schemaVersion >= 2
    ? parseFreshBlindAuthoringSafety(
        root.authoringSafety,
        "fresh blind release.authoringSafety",
      )
    : undefined;
  const fixture = parseAuthoredScientificFixture(root.fixture);
  if (schemaVersion === 3 && fixture.schemaVersion !== 2) {
    throw new Error(
      "fresh blind release schema 3 requires authored fixture schema 2",
    );
  }
  if (schemaVersion < 3 && fixture.schemaVersion !== 1) {
    throw new Error(
      `fresh blind release schema ${schemaVersion} requires authored fixture schema 1`,
    );
  }
  return {
    ...(authoringSafety === undefined ? {} : { authoringSafety }),
    commissioning: {
      authoringMethod: "isolated-codex-subagents",
      criticMethod: "independent-codex-subagents",
      engineExecutionsBeforeSeal: 0,
      mainReviewMethod: "complete-source-and-expectation-review",
      mainReviewerId: text(
        commissioning.mainReviewerId,
        "fresh blind release.commissioning.mainReviewerId",
      ),
    },
    fixture,
    release: {
      createdAt: date(
        release.createdAt,
        "fresh blind release.release.createdAt",
      ),
      frozenAt: timestamp(
        release.frozenAt,
        "fresh blind release.release.frozenAt",
      ),
      id: text(release.id, "fresh blind release.release.id"),
      seal: digest(release.seal, "fresh blind release.release.seal"),
      taskCardDigest: digest(
        release.taskCardDigest,
        "fresh blind release.release.taskCardDigest",
      ),
    },
    schemaVersion,
  };
}

export function freshBlindSealPayload(
  release: FreshBlindReleaseFixture,
): string {
  const { seal: _seal, ...metadata } = release.release;
  return stableJson({ ...release, release: metadata });
}

export function validateFreshBlindRelease(
  release: FreshBlindReleaseFixture,
  input: FreshBlindValidationInput,
): FreshBlindValidationSummary {
  const fixture = release.fixture;
  if (!RELEASE_ID.test(release.release.id)) {
    throw new Error(
      "fresh blind release.release.id: expected a semantic release id such as v0.29",
    );
  }
  if (fixture.batch.split !== "holdout") {
    throw new Error("fresh blind fixture must use the frozen holdout split");
  }
  if (
    fixture.batch.taskCardDigest !== release.release.taskCardDigest ||
    fixture.batch.frozenAt !== release.release.frozenAt
  ) {
    throw new Error("fresh blind outer release and authored batch disagree");
  }
  if (release.release.seal !== input.sealDigest) {
    throw new Error(
      "fresh blind release seal does not cover the frozen fixture",
    );
  }
  if (fixture.batch.seal !== input.authoredSealDigest) {
    throw new Error(
      "fresh blind authored seal does not cover the frozen fixture",
    );
  }
  if (fixture.scenarios.length !== REQUIRED_SCENARIOS) {
    throw new Error(
      `fresh blind fixture requires exactly ${REQUIRED_SCENARIOS} scenarios`,
    );
  }
  const primary = fixture.probes.filter((probe) => probe.kind === "primary");
  if (primary.length !== REQUIRED_SCENARIOS) {
    throw new Error(
      "fresh blind fixture requires one primary probe per scenario",
    );
  }
  const releaseNumber = semanticReleaseNumber(release.release.id);
  if (releaseNumber <= 40 && release.schemaVersion !== 1) {
    throw new Error("fresh blind schema 2 is reserved for v0.41 and later");
  }
  if (releaseNumber >= 37 && releaseNumber <= 40) {
    const missingAuthoring = fixture.probes
      .filter((probe) => probe.expected.authoringContext === undefined)
      .map((probe) => probe.id);
    if (missingAuthoring.length) {
      throw new Error(
        `fresh blind v0.37-v0.40 requires an exact authoring context for every primary and breadth probe: ${missingAuthoring.join(", ")}`,
      );
    }
    validateFreshAuthoringExpectations(release, input.authoringSyntaxFacts);
  } else if (releaseNumber === 41) {
    if (release.schemaVersion !== 2 || !release.authoringSafety) {
      throw new Error(
        "fresh blind v0.41+ requires the sealed authoring safety contract",
      );
    }
    const forbiddenExact = fixture.probes
      .filter((probe) => probe.expected.authoringContext !== undefined)
      .map((probe) => probe.id);
    if (forbiddenExact.length) {
      throw new Error(
        `fresh blind v0.41+ forbids guessed exact authoring contexts: ${forbiddenExact.join(", ")}`,
      );
    }
    validateFreshAuthoringSafetyExpectations(release);
  } else if (releaseNumber >= 42) {
    if (
      release.schemaVersion !== 3 || fixture.schemaVersion !== 2 ||
      !release.authoringSafety
    ) {
      throw new Error(
        "fresh blind v0.42+ requires release schema 3, authored fixture schema 2, and the sealed authoring safety contract",
      );
    }
    const forbiddenExact = fixture.probes
      .filter((probe) => probe.expected.authoringContext !== undefined)
      .map((probe) => probe.id);
    if (forbiddenExact.length) {
      throw new Error(
        `fresh blind v0.42+ forbids guessed exact authoring contexts: ${forbiddenExact.join(", ")}`,
      );
    }
    validateFreshAuthoringSafetyExpectations(release);
    validateFormulaDecisionExpectations(release, input.authoringSyntaxFacts);
  }
  for (const probe of fixture.probes) {
    const scenario = authoredScenarioFor(fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
    validateExactLocationContract(
      probe.id,
      "definition",
      probe.expected.navigation.definition,
      snapshot,
    );
    validateExactLocationContract(
      probe.id,
      "references",
      probe.expected.navigation.references,
      snapshot,
    );
    validateExactLocationContract(
      probe.id,
      "rename",
      probe.expected.navigation.rename,
      snapshot,
    );
    const preparation = probe.expected.navigation.prepareRename;
    if (
      preparation.status === "available" &&
      (preparation.range === undefined || preparation.placeholder === undefined)
    ) {
      throw new Error(
        `${probe.id}: available prepareRename requires an exact range and placeholder`,
      );
    }
    if (
      preparation.status === "unavailable" &&
      (preparation.range !== undefined || preparation.placeholder !== undefined)
    ) {
      throw new Error(
        `${probe.id}: unavailable prepareRename cannot define a range or placeholder`,
      );
    }
    const rename = probe.expected.navigation.rename;
    const contract = [
      rename.expectedText,
      rename.newName,
      rename.replacementText,
      rename.safety,
    ];
    if (
      rename.status === "available" &&
      contract.some((value) => value === undefined)
    ) {
      throw new Error(
        `${probe.id}: available rename requires exact source, replacement, and safety evidence`,
      );
    }
    if (
      rename.status === "available" &&
      (rename.newName !== rename.replacementText ||
        !authoredRenameNotationFamilyMatches(
          rename.expectedText!,
          rename.newName!,
        ))
    ) {
      throw new Error(
        `${probe.id}: rename must preserve one exact editable notation family`,
      );
    }
    const unavailableContract = fixture.schemaVersion === 2
      ? [rename.expectedText, rename.replacementText, rename.safety]
      : contract;
    if (
      rename.status === "unavailable" &&
      unavailableContract.some((value) => value !== undefined)
    ) {
      throw new Error(
        `${probe.id}: unavailable rename cannot define an edit result contract`,
      );
    }
    if (semanticReleaseNumber(release.release.id) >= 35) {
      const syntaxFact = input.authoringSyntaxFacts.find(
        (fact) =>
          fact.scenarioId === scenario.id &&
          fact.snapshotId === snapshot.id,
      );
      validateEntitySurfaceCommissioning(
        probe.id,
        probe.expected,
        snapshot,
        syntaxFact,
        release.schemaVersion,
      );
    }
  }
  const families = count(primary.map((probe) => probe.family));
  for (const family of DOCUMENT_REASONING_FAMILIES) {
    if (families[family] !== REQUIRED_FAMILY_SCENARIOS) {
      throw new Error(
        `${family}: fresh blind fixture requires ${REQUIRED_FAMILY_SCENARIOS} primary probes`,
      );
    }
  }
  const fields = count(fixture.scenarios.map((scenario) => scenario.field));
  for (const field of Object.keys(AUTHORED_AREA_ALLOCATION) as AuthoredArea[]) {
    const required = AUTHORED_AREA_ALLOCATION[field].holdout;
    if (fields[field] !== required) {
      throw new Error(
        `${field}: fresh blind fixture requires exactly ${required} holdout scenarios`,
      );
    }
  }
  const entityDecisions = count(primary.map((probe) => probe.expected.decision));
  const formulaDecisions = count(primary.flatMap((probe) =>
    probe.expected.formulaDecision == null
      ? []
      : [probe.expected.formulaDecision.status]
  ));
  const decisions = count(primary.map((probe) =>
    probe.expected.formulaDecision?.status ?? probe.expected.decision
  ));
  for (const decision of [
    "established",
    "partial",
    "ambiguous",
    "conflicting",
    "unsupported",
  ]) {
    if (!entityDecisions[decision]) {
      throw new Error(
        `${decision}: fresh blind fixture requires reviewed entity-decision coverage`,
      );
    }
  }
  if (release.schemaVersion === 3) {
    for (const [decision, required] of Object.entries(
      REQUIRED_SCHEMA_3_FORMULA_DECISIONS,
    )) {
      if (formulaDecisions[decision] !== required) {
        throw new Error(
          `${decision}: fresh blind schema 3 requires exactly ${required} selected-formula decisions`,
        );
      }
    }
  }
  validateCommissioning(release);
  for (const scenario of fixture.scenarios) {
    if (scenario.review.finalDigest !== input.reviewDigests[scenario.id]) {
      throw new Error(`${scenario.id}: final review digest is stale`);
    }
  }
  const laws = validateLaws(fixture, input.lawCatalog);
  rejectExactLeakage(fixture, input.referenceDocuments);
  const isolation = validateFreshBlindProfileIsolation(
    input.referenceProfiles,
    input.freshIsolationProfiles,
  );
  if (input.freshProfiles.length !== fixture.scenarios.length) {
    throw new Error("fresh blind integrity profiles must cover every scenario");
  }
  const freshProfileIds = new Set(
    input.freshProfiles.map((profile) => profile.id),
  );
  if (
    freshProfileIds.size !== fixture.scenarios.length ||
    fixture.scenarios.some((scenario) => !freshProfileIds.has(scenario.id))
  ) {
    throw new Error("fresh blind integrity profile identities are incomplete");
  }
  return {
    decisions,
    entityDecisions,
    families,
    fields,
    formulaDecisions,
    laws,
    maximumMathSimilarity: isolation.maximumMath,
    maximumProseSimilarity: isolation.maximumProse,
    probes: fixture.probes.length,
    scenarios: fixture.scenarios.length,
  };
}

function validateFreshAuthoringExpectations(
  release: FreshBlindReleaseFixture,
  facts: readonly FreshBlindSnapshotSyntaxFacts[],
): void {
  const fixture = release.fixture;
  const selected = new Map<string, ReturnType<typeof authoredSnapshotFor>>();
  for (const probe of fixture.probes) {
    const scenario = authoredScenarioFor(fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
    selected.set(`${scenario.id}\0${snapshot.id}`, snapshot);
  }
  const byKey = new Map<string, FreshBlindSnapshotSyntaxFacts>();
  for (const item of facts) {
    const key = `${item.scenarioId}\0${item.snapshotId}`;
    if (!selected.has(key)) {
      throw new Error(
        `${item.scenarioId}/${item.snapshotId}: unexpected fresh authoring syntax facts`,
      );
    }
    if (byKey.has(key)) {
      throw new Error(
        `${item.scenarioId}/${item.snapshotId}: duplicate fresh authoring syntax facts`,
      );
    }
    const snapshot = selected.get(key)!;
    const documentIds = item.documents.map((document) => document.fileId);
    if (new Set(documentIds).size !== documentIds.length) {
      throw new Error(
        `${item.scenarioId}/${item.snapshotId}: duplicate syntax document facts`,
      );
    }
    const expectedIds = snapshot.documents.map((document) => document.fileId).sort();
    if (stableJson([...documentIds].sort()) !== stableJson(expectedIds)) {
      throw new Error(
        `${item.scenarioId}/${item.snapshotId}: syntax document facts do not match the selected snapshot`,
      );
    }
    for (const document of item.documents) {
      const source = snapshot.documents.find(
        (candidate) => candidate.fileId === document.fileId,
      )!;
      const keys = document.mathRootContentRanges.map(
        (range) => `${range.startOffset}:${range.endOffset}`,
      );
      if (new Set(keys).size !== keys.length) {
        throw new Error(
          `${item.scenarioId}/${item.snapshotId}/${document.fileId}: duplicate math-root facts`,
        );
      }
      for (const range of document.mathRootContentRanges) {
        if (
          !Number.isInteger(range.startOffset) ||
          !Number.isInteger(range.endOffset) ||
          range.startOffset < 0 ||
          range.startOffset >= range.endOffset ||
          range.endOffset > source.content.length
        ) {
          throw new Error(
            `${item.scenarioId}/${item.snapshotId}/${document.fileId}: invalid math-root fact`,
          );
        }
      }
    }
    byKey.set(key, item);
  }
  if (byKey.size !== selected.size) {
    throw new Error("fresh authoring syntax facts must cover every selected snapshot");
  }

  for (const probe of fixture.probes) {
    const expected = probe.expected.authoringContext!;
    const scenario = authoredScenarioFor(fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
    const item = byKey.get(`${scenario.id}\0${snapshot.id}`)!;
    const rootsByFile = new Map(
      item.documents.map((document) => [document.fileId, document.mathRootContentRanges]),
    );
    const documents: MathAuthoringSyntaxDocument[] = snapshot.documents.map(
      (document) => ({
        ...document,
        documentVersion: 1,
        mathRootContentRanges: rootsByFile.get(document.fileId)!,
      }),
    );
    const cursorDocument = documents.find(
      (document) => document.fileId === probe.cursor.fileId,
    )!;
    const failures = [
      ...(expected.lifecycle.documentVersion === cursorDocument.documentVersion
        ? []
        : [{
            actual: expected.lifecycle.documentVersion,
            expected: cursorDocument.documentVersion,
            kind: "wrong-anchor" as const,
            path: "authoringContext.lifecycle.documentVersion",
          }]),
      ...mathAuthoringExpectationCanonicalFailures(expected),
      ...mathAuthoringExpectationSourceFailures(expected, documents),
      ...mathAuthoringExpectationFormulaRootFailures(expected, documents),
    ];
    const first = failures[0];
    if (first) {
      throw new Error(
        `${probe.id}: invalid exact authoring context at ${first.path} (${first.kind})`,
      );
    }
  }
}

function validateFormulaDecisionExpectations(
  release: FreshBlindReleaseFixture,
  facts: readonly FreshBlindSnapshotSyntaxFacts[],
): void {
  const expectedKeys = new Set(
    release.fixture.probes.map((probe) =>
      `${probe.scenarioId}\0${probe.cursor.snapshotId}`
    ),
  );
  const factsByKey = new Map<string, FreshBlindSnapshotSyntaxFacts>();
  for (const fact of facts) {
    const key = `${fact.scenarioId}\0${fact.snapshotId}`;
    if (!expectedKeys.has(key)) {
      throw new Error(
        `${fact.scenarioId}/${fact.snapshotId}: unexpected formula syntax facts`,
      );
    }
    if (factsByKey.has(key)) {
      throw new Error(
        `${fact.scenarioId}/${fact.snapshotId}: duplicate formula syntax facts`,
      );
    }
    const probe = release.fixture.probes.find((candidate) =>
      candidate.scenarioId === fact.scenarioId &&
      candidate.cursor.snapshotId === fact.snapshotId
    )!;
    const snapshot = authoredSnapshotFor(
      authoredScenarioFor(release.fixture, probe),
      probe,
    );
    const documentIds = fact.documents.map((document) => document.fileId);
    if (new Set(documentIds).size !== documentIds.length) {
      throw new Error(
        `${fact.scenarioId}/${fact.snapshotId}: duplicate formula syntax documents`,
      );
    }
    const expectedDocumentIds = snapshot.documents
      .map((document) => document.fileId)
      .sort();
    if (stableJson([...documentIds].sort()) !== stableJson(expectedDocumentIds)) {
      throw new Error(
        `${fact.scenarioId}/${fact.snapshotId}: formula syntax documents do not match the selected snapshot`,
      );
    }
    for (const document of fact.documents) {
      const source = snapshot.documents.find(
        (candidate) => candidate.fileId === document.fileId,
      )!;
      const ranges = document.mathRootContentRanges.map(
        (range) => `${range.startOffset}:${range.endOffset}`,
      );
      if (new Set(ranges).size !== ranges.length) {
        throw new Error(
          `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: duplicate formula math-root facts`,
        );
      }
      for (const range of document.mathRootContentRanges) {
        if (
          !Number.isInteger(range.startOffset) ||
          !Number.isInteger(range.endOffset) ||
          range.startOffset < 0 || range.startOffset >= range.endOffset ||
          range.endOffset > source.content.length
        ) {
          throw new Error(
            `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: invalid formula math-root fact`,
          );
        }
      }
      if (release.schemaVersion === 3 && document.occurrences === undefined) {
        throw new Error(
          `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: schema 3 requires complete occurrence syntax facts`,
        );
      }
      const occurrenceKeys = new Set<string>();
      for (const occurrence of document.occurrences ?? []) {
        const occurrenceKey = stableJson(occurrence);
        if (occurrenceKeys.has(occurrenceKey)) {
          throw new Error(
            `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: duplicate occurrence syntax fact`,
          );
        }
        occurrenceKeys.add(occurrenceKey);
        if (
          !validSourceRange(occurrence.range, source.content.length) ||
          !validSourceRange(occurrence.selectionRange, source.content.length) ||
          occurrence.selectionRange.startOffset < occurrence.range.startOffset ||
          occurrence.selectionRange.endOffset > occurrence.range.endOffset ||
          occurrence.surface.length === 0 ||
          (occurrence.applicationEndOffset !== undefined &&
            (!Number.isInteger(occurrence.applicationEndOffset) ||
              occurrence.applicationEndOffset < 0 ||
              occurrence.applicationEndOffset > source.content.length))
        ) {
          throw new Error(
            `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: invalid occurrence syntax fact`,
          );
        }
      }
      if (
        document.occurrences !== undefined &&
        stableJson(document.occurrences) !==
          stableJson([...document.occurrences].sort(compareOccurrenceFacts))
      ) {
        throw new Error(
          `${fact.scenarioId}/${fact.snapshotId}/${document.fileId}: occurrence syntax facts are not canonical`,
        );
      }
    }
    factsByKey.set(key, fact);
  }
  if (factsByKey.size !== expectedKeys.size) {
    throw new Error("formula syntax facts must cover every selected snapshot");
  }

  const safetyById = new Map(
    (release.authoringSafety ?? []).map((item) => [item.probeId, item]),
  );
  for (const probe of release.fixture.probes) {
    const expected = probe.expected.formulaDecision;
    if (!expected) {
      throw new Error(
        `${probe.id}: fresh release requires an exact selected formula decision`,
      );
    }
    const scenario = authoredScenarioFor(release.fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
    const selected = resolveAuthoredAnchor(snapshot, expected.anchor);
    const fact = factsByKey.get(
      `${probe.scenarioId}\0${probe.cursor.snapshotId}`,
    )!;
    const document = fact.documents.find(
      (candidate) => candidate.fileId === selected.fileId,
    );
    const roots = document?.mathRootContentRanges.filter(
      (range) =>
        range.startOffset === selected.range.startOffset &&
        range.endOffset === selected.range.endOffset,
    ) ?? [];
    if (roots.length !== 1) {
      throw new Error(
        `${probe.id}: formulaDecision.anchor must equal one selected math root`,
      );
    }
    const cursorAnchor = resolveAuthoredAnchor(snapshot, probe.cursor);
    const cursorOffset = probe.cursor.offset !== undefined
      ? cursorAnchor.range.startOffset + probe.cursor.offset
      : probe.cursor.edge === "after"
      ? cursorAnchor.range.endOffset
      : cursorAnchor.range.startOffset;
    const root = roots[0]!;
    if (
      cursorAnchor.fileId !== selected.fileId ||
      cursorOffset < root.startOffset || cursorOffset >= root.endOffset
    ) {
      throw new Error(
        `${probe.id}: cursor and formulaDecision.anchor must select the same math root`,
      );
    }
    const occurrence = selectFreshBlindOccurrence(
      document?.occurrences ?? [],
      cursorOffset,
    );
    if (probe.expected.cursorOccurrence === undefined) {
      throw new Error(`${probe.id}: schema 3 requires an explicit cursorOccurrence`);
    }
    const expectedOccurrence = probe.expected.cursorOccurrence === null
      ? null
      : resolveAuthoredAnchor(snapshot, probe.expected.cursorOccurrence);
    if (
      occurrence === undefined
        ? expectedOccurrence !== null
        : expectedOccurrence === null ||
          expectedOccurrence.fileId !== cursorAnchor.fileId ||
          !sameRange(expectedOccurrence.range, occurrence.range) ||
          probe.expected.symbol !== occurrence.surface
    ) {
      throw new Error(
        `${probe.id}: cursorOccurrence and symbol must equal syntax-selected cursor ownership`,
      );
    }
    const safety = safetyById.get(probe.id)!;
    if (safety.forbiddenDispositions.includes(expected.status)) {
      throw new Error(
        `${probe.id}: formula decision is forbidden by authoring safety`,
      );
    }
    if (expected.status === "established" && !safety.requiredAuthority.length) {
      throw new Error(
        `${probe.id}: established formula decision requires reviewed authority`,
      );
    }
    if (
      expected.status === "conflicting" &&
      !safety.requiredContradictions.length
    ) {
      throw new Error(
        `${probe.id}: conflicting formula decision requires reviewed contradiction`,
      );
    }
  }
}

function compareOccurrenceFacts(
  left: FreshBlindOccurrenceFact,
  right: FreshBlindOccurrenceFact,
): number {
  return left.selectionRange.startOffset - right.selectionRange.startOffset ||
    left.selectionRange.endOffset - right.selectionRange.endOffset ||
    left.range.startOffset - right.range.startOffset ||
    left.range.endOffset - right.range.endOffset ||
    compareOccurrenceText(left.surface, right.surface) ||
    compareOccurrenceText(left.kind, right.kind) ||
    (left.applicationEndOffset ?? -1) - (right.applicationEndOffset ?? -1);
}

function compareOccurrenceText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

export function selectFreshBlindOccurrence(
  occurrences: readonly FreshBlindOccurrenceFact[],
  cursorOffset: number,
): FreshBlindOccurrenceFact | undefined {
  const eligible = occurrences.flatMap((occurrence, index) => {
    const priority = occurrenceOwnershipPriority(occurrence, cursorOffset);
    return priority === undefined ? [] : [{ index, occurrence, priority }];
  });
  const shadowedByNestedTrailingEdge = (index: number, priority: number) =>
    priority === 0 && eligible.some((candidate) =>
      candidate.priority === 1 && candidate.index !== index &&
      strictlyContains(
        occurrences[index]!.range,
        occurrences[candidate.index]!.range,
      )
    );
  const unshadowed = eligible.filter((candidate) =>
    !shadowedByNestedTrailingEdge(candidate.index, candidate.priority)
  );
  const bestPriority = Math.min(...unshadowed.map((candidate) => candidate.priority));
  if (!Number.isFinite(bestPriority)) return undefined;
  const candidates = unshadowed
    .filter((candidate) => candidate.priority === bestPriority)
    .sort((left, right) => {
      const leftOccurrence = left.occurrence;
      const rightOccurrence = right.occurrence;
      const leftWidth = leftOccurrence.range.endOffset -
        leftOccurrence.range.startOffset;
      const rightWidth = rightOccurrence.range.endOffset -
        rightOccurrence.range.startOffset;
      const occurrenceOrder = bestPriority === 1
        ? rightWidth - leftWidth
        : leftWidth - rightWidth;
      const leftSelectionWidth = leftOccurrence.selectionRange.endOffset -
        leftOccurrence.selectionRange.startOffset;
      const rightSelectionWidth = rightOccurrence.selectionRange.endOffset -
        rightOccurrence.selectionRange.startOffset;
      return occurrenceOrder || leftSelectionWidth - rightSelectionWidth ||
        leftOccurrence.selectionRange.startOffset -
          rightOccurrence.selectionRange.startOffset;
    });
  const selected = candidates[0]?.occurrence;
  const next = candidates[1]?.occurrence;
  if (
    selected && next && sameRange(selected.range, next.range) &&
    !sameRange(selected.selectionRange, next.selectionRange)
  ) return undefined;
  return selected;
}

function occurrenceOwnershipPriority(
  occurrence: FreshBlindOccurrenceFact,
  offset: number,
): number | undefined {
  if (rangeContains(occurrence.range, offset)) return 0;
  if (nonemptyTrailingEdge(occurrence.range, offset)) return 1;
  if (rangeContains(occurrence.selectionRange, offset)) return 2;
  if (nonemptyTrailingEdge(occurrence.selectionRange, offset)) return 3;
  return occurrence.applicationEndOffset === offset ? 4 : undefined;
}

function rangeContains(range: SourceRange, offset: number): boolean {
  return range.startOffset <= offset && offset < range.endOffset;
}

function nonemptyTrailingEdge(range: SourceRange, offset: number): boolean {
  return range.startOffset < range.endOffset && range.endOffset === offset;
}

function strictlyContains(container: SourceRange, child: SourceRange): boolean {
  return container.startOffset <= child.startOffset &&
    child.endOffset + 1 < container.endOffset;
}

function validSourceRange(range: SourceRange, contentLength: number): boolean {
  return Number.isInteger(range.startOffset) &&
    Number.isInteger(range.endOffset) &&
    range.startOffset >= 0 &&
    range.startOffset < range.endOffset &&
    range.endOffset <= contentLength;
}

function validateEntitySurfaceCommissioning(
  probeId: string,
  expected: AuthoredScientificFixture["probes"][number]["expected"],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  syntaxFact: FreshBlindSnapshotSyntaxFacts | undefined,
  releaseSchemaVersion: FreshBlindReleaseFixture["schemaVersion"],
): void {
  const { definition, prepareRename, references, rename } = expected.navigation;
  if (definition.status !== references.status) {
    throw new Error(
      `${probeId}: definition and references must share one entity-surface authorization`,
    );
  }
  if (prepareRename.status !== rename.status) {
    throw new Error(
      `${probeId}: prepareRename and rename must share one edit authorization`,
    );
  }
  if (definition.status === "available") {
    if (
      releaseSchemaVersion === 3 &&
      (definition.allowed === undefined || references.allowed === undefined)
    ) {
      throw new Error(
        `${probeId}: schema 3 available navigation requires explicit allowed envelopes`,
      );
    }
    const definitions = resolvedLocationKeys(
      definition.allowed ?? definition.required,
      snapshot,
    );
    const referenceAnchors = references.allowed ?? references.required;
    const referenceKeys = new Set(
      resolvedLocationKeys(referenceAnchors, snapshot),
    );
    if (definitions.some((location) => !referenceKeys.has(location))) {
      throw new Error(
        `${probeId}: every definition must be present in the complete reference surface`,
      );
    }
    const symbol = expected.symbol;
    const referenceOccurrences = referenceAnchors.map((anchor) =>
      reviewedSyntaxOccurrence(probeId, anchor, snapshot, syntaxFact)
    );
    if (!symbol || referenceOccurrences.some((occurrence) =>
      occurrence.surface !== symbol
    )) {
      throw new Error(
        `${probeId}: authorized references must select one reviewed syntax occurrence identity`,
      );
    }
  }
  if (rename.status === "available") {
    if (releaseSchemaVersion === 3 && rename.allowed === undefined) {
      throw new Error(
        `${probeId}: schema 3 available rename requires an explicit allowed envelope`,
      );
    }
    const referenceKeys = resolvedLocationKeys(
      references.allowed ?? references.required,
      snapshot,
    );
    const renameKeys = resolvedLocationKeys(
      rename.allowed ?? rename.required,
      snapshot,
    );
    if (
      referenceKeys.length !== renameKeys.length ||
      referenceKeys.some((location, index) => location !== renameKeys[index])
    ) {
      throw new Error(
        `${probeId}: rename edits must equal the complete ordered reference surface`,
      );
    }
    if (
      expected.symbol !== rename.expectedText ||
      prepareRename.placeholder !== rename.expectedText ||
      !prepareRename.range ||
      !renameKeys.includes(
        resolvedLocationKeys([prepareRename.range], snapshot)[0]!,
      )
    ) {
      throw new Error(
        `${probeId}: prepareRename must select the same exact notation authorized for rename`,
      );
    }
  }
}

function reviewedSyntaxOccurrence(
  probeId: string,
  anchor: AuthoredScientificFixture["probes"][number]["expected"]["navigation"]["references"]["required"][number],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  syntaxFact: FreshBlindSnapshotSyntaxFacts | undefined,
): FreshBlindOccurrenceFact {
  const resolved = resolveAuthoredAnchor(snapshot, anchor);
  const document = syntaxFact?.documents.find(
    (candidate) => candidate.fileId === resolved.fileId,
  );
  const matches = document?.occurrences?.filter((occurrence) =>
    sameRange(occurrence.range, resolved.range)
  ) ?? [];
  if (matches.length !== 1) {
    throw new Error(
      `${probeId}: reviewed navigation anchor must equal one syntax occurrence`,
    );
  }
  return matches[0]!;
}

function semanticReleaseNumber(releaseId: string): number {
  return Number.parseInt(releaseId.slice("v0.".length), 10);
}

function resolvedLocationKeys(
  anchors: readonly AuthoredScientificFixture["probes"][number]["expected"]["navigation"]["references"]["required"][number][],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): string[] {
  return anchors
    .map((anchor) => {
      const resolved = resolveAuthoredAnchor(snapshot, anchor);
      return `${resolved.fileId}:${resolved.range.startOffset}:${resolved.range.endOffset}`;
    })
    .sort();
}

function validateExactLocationContract(
  probeId: string,
  surface: string,
  expected: AuthoredLocationExpectation,
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): void {
  const envelope = expected.allowed ?? expected.required;
  if (expected.allowed === undefined && expected.minimum !== expected.required.length) {
    throw new Error(
      `${probeId}: ${surface} must enumerate its complete location allowlist`,
    );
  }
  if (expected.status === "available" && envelope.length === 0) {
    throw new Error(`${probeId}: available ${surface} requires a source location`);
  }
  const locations = envelope.map((anchor) => {
    const resolved = resolveAuthoredAnchor(snapshot, anchor);
    return `${resolved.fileId}:${resolved.range.startOffset}:${resolved.range.endOffset}`;
  });
  if (new Set(locations).size !== locations.length) {
    throw new Error(`${probeId}: ${surface} repeats a reviewed source location`);
  }
  if (
    expected.allowed !== undefined &&
    stableJson(locations) !== stableJson([...locations].sort())
  ) {
    throw new Error(`${probeId}: ${surface} allowed envelope is not canonical`);
  }
  const allowed = new Set(locations);
  const required = resolvedLocationKeys(expected.required, snapshot);
  if (required.some((location) => !allowed.has(location))) {
    throw new Error(`${probeId}: ${surface} required locations must be allowed`);
  }
  const excluded = resolvedLocationKeys(expected.excluded, snapshot);
  if (excluded.some((location) => allowed.has(location))) {
    throw new Error(`${probeId}: ${surface} cannot both allow and exclude a location`);
  }
  if (expected.minimum > envelope.length) {
    throw new Error(`${probeId}: ${surface} minimum exceeds its allowed envelope`);
  }
}

/** Keep similarity policy pure; the effectful validator supplies fingerprints
 * extracted from wasmtex CSTs for both reference and fresh documents. */
export function validateFreshBlindProfileIsolation(
  referenceProfiles: readonly AuthoredIntegrityProfile[],
  freshProfiles: readonly AuthoredIntegrityProfile[],
): { readonly maximumMath: number; readonly maximumProse: number } {
  const comparisons = compareAuthoredIntegrityProfiles(
    referenceProfiles,
    freshProfiles,
  );
  const suspicious = comparisons.filter(
    (comparison) =>
      comparison.proseSimilarity >= 0.5 ||
      (comparison.exactMath && comparison.proseSimilarity >= 0.25),
  );
  if (suspicious.length) {
    const first = suspicious.sort(
      (left, right) => right.proseSimilarity - left.proseSimilarity,
    )[0]!;
    throw new Error(
      `fresh blind lineage similarity requires review: ${first.developmentId}/${first.holdoutId}`,
    );
  }
  return {
    maximumMath: Math.max(
      0,
      ...comparisons.map((comparison) => comparison.mathSimilarity),
    ),
    maximumProse: Math.max(
      0,
      ...comparisons.map((comparison) => comparison.proseSimilarity),
    ),
  };
}

export function freshBlindSafetySummary(
  fixture: AuthoredScientificFixture,
  observations: readonly AuthoredScientificObservation[],
): FreshBlindSafetySummary {
  const probeIds = new Set(fixture.probes.map((probe) => probe.id));
  const byId = new Map<string, AuthoredScientificObservation>();
  for (const observation of observations) {
    if (!probeIds.has(observation.caseId)) {
      throw new Error(
        `${observation.caseId}: unexpected fresh blind observation`,
      );
    }
    if (byId.has(observation.caseId)) {
      throw new Error(
        `${observation.caseId}: duplicate fresh blind observation`,
      );
    }
    byId.set(observation.caseId, observation);
  }
  let unsafeNavigationOrEditLocations = 0;
  const diagnosticsOverLimitIds: string[] = [];
  const falseConflictIds: string[] = [];
  const falseEstablishmentIds: string[] = [];
  const unsafeNavigationOrEditCaseIds = new Set<string>();
  for (const probe of fixture.probes) {
    const observed = byId.get(probe.id);
    if (!observed) continue;
    const scenario = authoredScenarioFor(fixture, probe);
    const risk = scoreAuthoredScientificFixture(
      { ...fixture, probes: [probe], scenarios: [scenario] },
      [observed],
    ).risk;
    if (risk.falseEstablishment) {
      falseEstablishmentIds.push(probe.id);
    }
    if (risk.falseConflict) {
      falseConflictIds.push(probe.id);
    }
    const problems = observed.diagnostics.filter(
      (diagnostic) =>
        diagnostic.severity === "error" || diagnostic.severity === "warning",
    );
    if (problems.length > probe.expected.diagnostics.maximum) {
      diagnosticsOverLimitIds.push(probe.id);
    }
    const snapshot = authoredSnapshotFor(scenario, probe);
    const unsafeDefinitions = unsafeLocations(
      observed.definitions,
      probe.expected.navigation.definition,
      snapshot,
    );
    const unsafeReferences = unsafeLocations(
      observed.references,
      probe.expected.navigation.references,
      snapshot,
    );
    const unsafePreparation = unsafePrepareRenameLocation(
      observed.prepareRename,
      probe.expected.navigation.prepareRename,
      snapshot,
    );
    const unsafeRename = unsafeLocations(
      observed.renameEdits,
      probe.expected.navigation.rename,
      snapshot,
      (edit) =>
        (probe.expected.navigation.rename.expectedText !== undefined &&
          edit.expectedText !==
            probe.expected.navigation.rename.expectedText) ||
        (probe.expected.navigation.rename.replacementText !== undefined &&
          edit.replacementText !==
            probe.expected.navigation.rename.replacementText) ||
        (probe.expected.navigation.rename.safety !== undefined &&
          observed.renameSafety !== probe.expected.navigation.rename.safety),
    );
    const unsafeAuthorizations = unsafeSurfaceAuthorizations(
      observed.surfaceAuthorizations,
      probe.expected.navigation,
      probe.expected.cursorOccurrence,
      observed.cursorSurfaceIdentity,
      snapshot,
    );
    const unsafeCaseLocations =
      unsafeDefinitions +
      unsafeReferences +
      unsafePreparation +
      unsafeRename +
      unsafeAuthorizations;
    unsafeNavigationOrEditLocations += unsafeCaseLocations;
    if (unsafeCaseLocations > 0) {
      unsafeNavigationOrEditCaseIds.add(probe.id);
    }
  }
  return {
    diagnosticsOverLimit: diagnosticsOverLimitIds.length,
    diagnosticsOverLimitIds: diagnosticsOverLimitIds.sort(),
    falseConflict: falseConflictIds.length,
    falseConflictIds: falseConflictIds.sort(),
    falseEstablishment: falseEstablishmentIds.length,
    falseEstablishmentIds: falseEstablishmentIds.sort(),
    unsafeNavigationOrEditCaseIds: [...unsafeNavigationOrEditCaseIds].sort(),
    unsafeNavigationOrEditLocations,
  };
}

export function freshBlindSafetyGateFailed(
  summary: FreshBlindSafetySummary,
): boolean {
  return (
    summary.diagnosticsOverLimit > 0 ||
    summary.falseConflict > 0 ||
    summary.falseEstablishment > 0 ||
    summary.unsafeNavigationOrEditLocations > 0
  );
}

export function planFreshBlindSnapshotTransitions(
  fixture: AuthoredScientificFixture,
): readonly FreshBlindSnapshotTransition[] {
  return fixture.scenarios.flatMap((scenario) =>
    scenario.snapshots.slice(1).map((snapshot, index) => ({
      fromSnapshotId: scenario.snapshots[index]!.id,
      scenarioId: scenario.id,
      toSnapshotId: snapshot.id,
    })),
  );
}

function validateCommissioning(release: FreshBlindReleaseFixture): void {
  const seenGroups = new Set<string>();
  for (const scenario of release.fixture.scenarios) {
    if (scenario.provenance.taskCardDigest !== release.release.taskCardDigest) {
      throw new Error(
        `${scenario.id}: authored task card differs from the frozen release`,
      );
    }
    if (scenario.review.frozenAt !== release.release.frozenAt) {
      throw new Error(
        `${scenario.id}: review freeze differs from the frozen release`,
      );
    }
    if (!seenGroups.add(scenario.provenance.independenceGroup)) {
      throw new Error(`${scenario.id}: independence group is reused`);
    }
    if (
      scenario.review.mainReviewer !== release.commissioning.mainReviewerId ||
      scenario.provenance.authorId === scenario.review.criticId ||
      scenario.provenance.authorId === scenario.review.mainReviewer ||
      scenario.review.criticId === scenario.review.mainReviewer
    ) {
      throw new Error(
        `${scenario.id}: author, critic, and main reviewer must be independent`,
      );
    }
  }
}

function validateLaws(
  fixture: AuthoredScientificFixture,
  catalog: readonly AuthoredLawCatalogEntry[],
): number {
  const byId = new Map(catalog.map((law) => [law.lawId, law]));
  if (byId.size !== catalog.length)
    throw new Error("law catalog ids are not unique");
  const covered = new Set<string>();
  for (const scenario of fixture.scenarios) {
    for (const lawId of scenario.lawIds) {
      if (!byId.has(lawId))
        throw new Error(`${scenario.id}: unknown law ${lawId}`);
      covered.add(lawId);
    }
  }
  for (const probe of fixture.probes) {
    const scenario = authoredScenarioFor(fixture, probe);
    for (const relation of probe.expected.relations) {
      const law = byId.get(relation.relationId);
      if (!law || !scenario.lawIds.includes(relation.relationId)) {
        throw new Error(
          `${probe.id}: expected relation is absent from scenario law coverage`,
        );
      }
      for (const role of law.roles) {
        const matches = relation.roles.filter(
          (candidate) => candidate.role === role.id,
        );
        if (role.variadic ? matches.length === 0 : matches.length !== 1) {
          throw new Error(
            `${probe.id}: ${relation.relationId} has invalid role coverage`,
          );
        }
      }
      const knownRoles = new Set(law.roles.map((role) => role.id));
      if (relation.roles.some((role) => !knownRoles.has(role.role))) {
        throw new Error(
          `${probe.id}: ${relation.relationId} has an unknown role`,
        );
      }
    }
  }
  return covered.size;
}

function rejectExactLeakage(
  fixture: AuthoredScientificFixture,
  references: readonly string[],
): void {
  const known = new Set(references.map(normalizeDocument));
  for (const scenario of fixture.scenarios) {
    for (const snapshot of scenario.snapshots) {
      for (const document of snapshot.documents) {
        if (known.has(normalizeDocument(document.content))) {
          throw new Error(
            `${scenario.id}: frozen document duplicates existing evidence`,
          );
        }
      }
    }
  }
}

function unsafeSurfaceAuthorizations(
  observed: AuthoredScientificObservation["surfaceAuthorizations"],
  expected: AuthoredScientificFixture["probes"][number]["expected"]["navigation"],
  expectedCursorOccurrence: AuthoredScientificFixture["probes"][number]["expected"]["cursorOccurrence"],
  cursorSurfaceIdentity: AuthoredScientificObservation["cursorSurfaceIdentity"],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): number {
  // Retained legacy observations predate public authorization capture.
  if (!observed) return 0;
  const surfaces = [
    ["definition", observed.definition, expected.definition.status],
    ["references", observed.references, expected.references.status],
    ["prepareRename", observed.prepareRename, expected.prepareRename.status],
    ["rename", observed.rename, expected.rename.status],
  ] as const;
  // Refusing an expected surface is a coverage loss, not an unsafe host
  // mutation. Only authority that the reviewed envelope forbids is unsafe.
  let failures = surfaces.filter(([, authorization, expectedStatus]) =>
    expectedStatus === "unavailable" && authorization.status === "authorized"
  ).length;
  const authorized = surfaces.flatMap(([, authorization]) =>
    authorization.status === "authorized"
      ? [{
          entityId: authorization.entityId,
          focusOccurrenceId: authorization.focusOccurrenceId,
        }]
      : []
  );
  // One case-level failure records every identity inconsistency without
  // multiplying one bad surface across pairwise comparisons.
  let unsafeIdentity = false;
  if (authorized.length > 0) {
    const expectedLocation = expectedCursorOccurrence === undefined ||
        expectedCursorOccurrence === null
      ? undefined
      : resolveAuthoredAnchor(snapshot, expectedCursorOccurrence);
    if (
      expectedLocation === undefined || cursorSurfaceIdentity == null ||
      cursorSurfaceIdentity.entityId === null ||
      cursorSurfaceIdentity.location.fileId !== expectedLocation.fileId ||
      cursorSurfaceIdentity.location.path !== expectedLocation.path ||
      !sameRange(cursorSurfaceIdentity.location.range, expectedLocation.range) ||
      cursorSurfaceIdentity.occurrenceId.fileId !== expectedLocation.fileId
    ) {
      unsafeIdentity = true;
    } else {
      if (authorized.some((authorization) =>
        stableJson(authorization.focusOccurrenceId) !==
            stableJson(cursorSurfaceIdentity.occurrenceId) ||
        stableJson(authorization.entityId) !==
            stableJson(cursorSurfaceIdentity.entityId)
      )) unsafeIdentity = true;
    }
  }
  failures += Number(unsafeIdentity);
  return failures;
}

function unsafeLocations<
  Location extends { readonly fileId: string; readonly range: SourceRange },
>(
  observed: readonly Location[],
  expected: {
    readonly allowed?: readonly {
      readonly fileId: string;
      readonly needle: string;
      readonly occurrence?: number;
      readonly selection?: { readonly length: number; readonly offset: number };
    }[];
    readonly excluded: readonly {
      readonly fileId: string;
      readonly needle: string;
      readonly occurrence?: number;
      readonly selection?: { readonly length: number; readonly offset: number };
    }[];
    readonly required: readonly {
      readonly fileId: string;
      readonly needle: string;
      readonly occurrence?: number;
      readonly selection?: { readonly length: number; readonly offset: number };
    }[];
    readonly minimum: number;
    readonly status: "available" | "unavailable";
  },
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  additionalUnsafe: (location: Location) => boolean = () => false,
): number {
  if (expected.status === "unavailable") return observed.length;
  const allowed = (expected.allowed ?? expected.required).map((anchor) =>
    resolveAuthoredAnchor(snapshot, anchor),
  );
  const excluded = expected.excluded.map((anchor) =>
    resolveAuthoredAnchor(snapshot, anchor),
  );
  const unsafe = observed.filter(
    (location) =>
      additionalUnsafe(location) ||
      !allowed.some(
        (anchor) =>
          location.fileId === anchor.fileId &&
          sameRange(location.range, anchor.range),
      ) ||
      excluded.some(
        (anchor) =>
          location.fileId === anchor.fileId &&
          sameRange(location.range, anchor.range),
      ),
  ).length;
  return unsafe;
}

function unsafePrepareRenameLocation(
  observed: AuthoredScientificObservation["prepareRename"],
  expected: AuthoredScientificFixture["probes"][number]["expected"]["navigation"]["prepareRename"],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): number {
  if (!observed.range) {
    return observed.placeholder !== undefined ? 1 : 0;
  }
  if (expected.status === "unavailable") return 1;
  if (
    expected.range &&
    !sameRange(
      observed.range,
      resolveAuthoredAnchor(snapshot, expected.range).range,
    )
  ) {
    return 1;
  }
  return expected.placeholder !== undefined &&
    observed.placeholder !== expected.placeholder
    ? 1
    : 0;
}

function normalizeDocument(value: string): string {
  return value
    .normalize("NFKC")
    .toLocaleLowerCase("en-US")
    .replaceAll(/\s+/gu, " ")
    .trim();
}

function count(values: readonly string[]): Record<string, number> {
  const output: Record<string, number> = {};
  for (const value of values) output[value] = (output[value] ?? 0) + 1;
  return output;
}

function sameRange(left: SourceRange, right: SourceRange): boolean {
  return (
    left.startOffset === right.startOffset && left.endOffset === right.endOffset
  );
}

function stableJson(value: unknown): string {
  return JSON.stringify(sortJson(value));
}

function sortJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortJson);
  if (typeof value !== "object" || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, child]) => [key, sortJson(child)]),
  );
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${path}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function exact(
  value: Record<string, unknown>,
  fields: readonly string[],
  path: string,
): void {
  const expected = new Set(fields);
  const unexpected = Object.keys(value).filter((field) => !expected.has(field));
  const missing = fields.filter((field) => !(field in value));
  if (unexpected.length || missing.length) {
    throw new Error(
      `${path}: fields differ (missing ${missing.join(", ") || "none"}; unexpected ${unexpected.join(", ") || "none"})`,
    );
  }
}

function literal<T extends string>(
  value: unknown,
  expected: T,
  path: string,
): T {
  if (value !== expected) throw new Error(`${path}: must be ${expected}`);
  return expected;
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${path}: expected non-empty text`);
  }
  return value;
}

function digest(value: unknown, path: string): string {
  const result = text(value, path);
  if (!DIGEST.test(result))
    throw new Error(`${path}: expected a lowercase SHA-256 digest`);
  return result;
}

function date(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(result))
    throw new Error(`${path}: expected YYYY-MM-DD`);
  return result;
}

function timestamp(value: unknown, path: string): string {
  const result = text(value, path);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(result)) {
    throw new Error(
      `${path}: expected a UTC timestamp without fractional seconds`,
    );
  }
  return result;
}
