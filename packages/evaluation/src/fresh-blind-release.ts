import type { SourceRange } from "../../protocol/src/index";
import {
  DOCUMENT_REASONING_FAMILIES,
  V036_AUTHORED_HOLDOUT_AREA_ALLOCATION,
  authoredScenarioFor,
  authoredSnapshotFor,
  compareAuthoredMathAuthoringContext,
  parseAuthoredScientificFixture,
  resolveAuthoredAnchor,
  scoreAuthoredScientificFixture,
  type AuthoredLawCatalogEntry,
  type AuthoredArea,
  type AuthoredLocationExpectation,
  type AuthoredScientificFixture,
  type AuthoredScientificObservation,
} from "./authored-scientific";
import {
  compareAuthoredIntegrityProfiles,
  type AuthoredIntegrityProfile,
} from "./authored-integrity";

const DIGEST = /^[0-9a-f]{64}$/u;
const RELEASE_ID = /^v0\.[1-9][0-9]*$/u;
const REQUIRED_SCENARIOS = 48;
const REQUIRED_FAMILY_SCENARIOS = 8;

export interface FreshBlindReleaseFixture {
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
  readonly schemaVersion: 1;
}

export interface FreshBlindValidationInput {
  readonly authoredSealDigest: string;
  readonly freshIsolationProfiles: readonly AuthoredIntegrityProfile[];
  readonly freshProfiles: readonly AuthoredIntegrityProfile[];
  readonly lawCatalog: readonly AuthoredLawCatalogEntry[];
  readonly referenceDocuments: readonly string[];
  readonly referenceProfiles: readonly AuthoredIntegrityProfile[];
  readonly reviewDigests: Readonly<Record<string, string>>;
  readonly sealDigest: string;
}

export interface FreshBlindValidationSummary {
  readonly decisions: Readonly<Record<string, number>>;
  readonly families: Readonly<Record<string, number>>;
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
  /** Authoring contexts that introduced a conflict absent from review. */
  readonly falseAuthoringConflict: number;
  readonly falseAuthoringConflictIds: readonly string[];
  readonly falseEstablishment: number;
  readonly falseEstablishmentIds: readonly string[];
  /** Probe ids whose authoring disposition exceeded the reviewed authority. */
  readonly moreAuthoritativeDispositionIds: readonly string[];
  readonly moreAuthoritativeDispositions: number;
  /** Source-grounded authoring facts outside the reviewed exact allowlist. */
  readonly unsafeAuthoringContextFacts: number;
  /** Probe ids with at least one unreviewed authoring fact. */
  readonly unsafeAuthoringContextCaseIds: readonly string[];
  /** Authority-increasing or revision-fence-removing lifecycle transitions. */
  readonly unsafeLifecycleTransitions: number;
  readonly unsafeLifecycleCaseIds: readonly string[];
  /** Concrete source locations exposed or edited outside the review contract. */
  readonly unsafeNavigationOrEditLocations: number;
  /** Probe ids with at least one unsafe navigation or edit location. */
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
  exact(
    root,
    ["schemaVersion", "release", "commissioning", "fixture"],
    "fresh blind release",
  );
  if (root.schemaVersion !== 1) {
    throw new Error("fresh blind release.schemaVersion: must be 1");
  }
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
  return {
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
    fixture: parseAuthoredScientificFixture(root.fixture),
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
    schemaVersion: 1,
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
        !sameRenameNotationFamily(rename.expectedText!, rename.newName!))
    ) {
      throw new Error(
        `${probe.id}: rename must preserve one exact editable notation family`,
      );
    }
    if (
      rename.status === "unavailable" &&
      contract.some((value) => value !== undefined)
    ) {
      throw new Error(
        `${probe.id}: unavailable rename cannot define an edit contract`,
      );
    }
    if (semanticReleaseNumber(release.release.id) >= 35) {
      validateEntitySurfaceCommissioning(probe.id, probe.expected, snapshot);
    }
  }
  if (semanticReleaseNumber(release.release.id) >= 36) {
    validateMathAuthoringContextCommissioning(primary);
    validateV036FieldAllocation(fixture);
  }
  const families = count(primary.map((probe) => probe.family));
  for (const family of DOCUMENT_REASONING_FAMILIES) {
    if (families[family] !== REQUIRED_FAMILY_SCENARIOS) {
      throw new Error(
        `${family}: fresh blind fixture requires ${REQUIRED_FAMILY_SCENARIOS} primary probes`,
      );
    }
  }
  const decisions = count(primary.map((probe) => probe.expected.decision));
  for (const decision of [
    "established",
    "partial",
    "ambiguous",
    "conflicting",
    "unsupported",
  ]) {
    if (!decisions[decision]) {
      throw new Error(
        `${decision}: fresh blind fixture requires reviewed coverage`,
      );
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
    families,
    laws,
    maximumMathSimilarity: isolation.maximumMath,
    maximumProseSimilarity: isolation.maximumProse,
    probes: fixture.probes.length,
    scenarios: fixture.scenarios.length,
  };
}

function validateMathAuthoringContextCommissioning(
  primary: readonly AuthoredScientificFixture["probes"][number][],
): void {
  const missing = primary.find((probe) => !probe.expected.authoringContext);
  if (missing) {
    throw new Error(
      `${missing.id}: v0.36 primary probe requires an authored math authoring context`,
    );
  }
  const contexts = primary.map((probe) => probe.expected.authoringContext!);
  for (let index = 0; index < contexts.length; index += 1) {
    validateV036ContextContract(primary[index]!.id, contexts[index]!);
  }
  const covered = {
    approximation: contexts.some((context) => context.approximation !== null),
    claimEvidence: contexts.some((context) => context.claimEvidence.length > 0),
    conditions: contexts.some((context) => context.conditions.length > 0),
    conventionalCandidates: contexts.some(
      (context) => context.conventionalCandidates.length > 0,
    ),
    equationLinks: contexts.some((context) => context.equationLinks.length > 0),
    formula: contexts.some((context) => context.formula !== null),
    notationOccurrences: contexts.some(
      (context) => context.notationOccurrences.length > 0,
    ),
    requirements: contexts.some((context) => context.requirements.length > 0),
  };
  const uncovered = Object.entries(covered)
    .filter(([, present]) => !present)
    .map(([surface]) => surface);
  if (uncovered.length) {
    throw new Error(
      `v0.36 fresh blind tranche lacks authoring-context coverage: ${uncovered.join(", ")}`,
    );
  }
  requireFreshContextValues(
    "disposition",
    contexts.map((context) => context.disposition),
    [
      "ambiguous",
      "conflicting",
      "conventional",
      "engine-limited",
      "established",
      "partial",
      "unsupported",
    ],
  );
  const lifecycle = contexts.map((context) => context.lifecycle);
  for (const [label, present] of [
    ["generated lifecycle", lifecycle.some((item) => item.generation === "generated")],
    ["retracted lifecycle", lifecycle.some((item) => item.retracted)],
    ["noneditable lifecycle", lifecycle.some((item) => !item.editable)],
    ["capped lifecycle", lifecycle.some((item) => item.capped)],
    ["engine-limited lifecycle", lifecycle.some((item) => item.engineLimited)],
  ] as const) {
    if (!present) throw new Error(`v0.36 fresh blind tranche lacks ${label}`);
  }
  requireFreshContextValues(
    "requirement kind",
    contexts.flatMap((context) =>
      context.requirements.map((requirement) => requirement.kind),
    ),
    ["condition", "declaration", "disambiguation", "role-declaration"],
  );
  requireFreshContextValues(
    "equation link kind",
    contexts.flatMap((context) => context.equationLinks.map((link) => link.kind)),
    ["derived-law", "shared-entity"],
  );
  requireFreshContextValues(
    "condition status",
    contexts.flatMap((context) => [
      ...context.conditions.map((condition) => condition.status),
      ...context.requirements.flatMap((requirement) =>
        requirement.kind === "condition" ? [requirement.condition.status] : [],
      ),
    ]),
    ["conflicting", "required", "unsupported", "verified"],
  );
  requireFreshContextValues(
    "claim polarity",
    contexts.flatMap((context) => context.claimEvidence.map((link) => link.polarity)),
    ["negative", "positive"],
  );
  requireFreshContextValues(
    "claim modality",
    contexts.flatMap((context) => context.claimEvidence.map((link) => link.modality)),
    ["asserted", "cited", "hedged", "hypothetical", "quoted"],
  );
  requireFreshContextValues(
    "claim strength ceiling",
    contexts.flatMap((context) =>
      context.claimEvidence.map((link) => link.strengthCeiling),
    ),
    ["asserted", "qualified", "unusable"],
  );
}

function validateV036FieldAllocation(fixture: AuthoredScientificFixture): void {
  for (const [field, expected] of Object.entries(
    V036_AUTHORED_HOLDOUT_AREA_ALLOCATION,
  ) as [AuthoredArea, number][]) {
    const actual = fixture.scenarios.filter(
      (scenario) => scenario.field === field,
    ).length;
    if (actual !== expected) {
      throw new Error(
        `${field}: v0.36 fresh blind requires exactly ${expected} cases, got ${actual}`,
      );
    }
  }
}

function validateV036ContextContract(
  probeId: string,
  context: NonNullable<AuthoredScientificFixture["probes"][number]["expected"]["authoringContext"]>,
): void {
  if (context.lifecycle.documentVersion !== 1) {
    throw new Error(`${probeId}: v0.36 lifecycle documentVersion must be 1`);
  }
  const formulaAnchors = [
    ...(context.formula ? [context.formula] : []),
    ...context.equationLinks.flatMap((link) => [link.source, link.target]),
    ...context.claimEvidence.flatMap((claim) => claim.supportingFormulas),
  ];
  if (formulaAnchors.some((formula) => formula.documentVersion !== 1)) {
    throw new Error(`${probeId}: v0.36 formula anchor documentVersion must be 1`);
  }
  if (context.approximation && context.approximation.evidence.length === 0) {
    throw new Error(`${probeId}: v0.36 approximation requires exact source evidence`);
  }
  if (context.equationLinks.some((link) => link.evidence.length === 0)) {
    throw new Error(`${probeId}: v0.36 equation link requires exact source evidence`);
  }
  if (context.claimEvidence.some((claim) => claim.evidence.length === 0)) {
    throw new Error(`${probeId}: v0.36 claim evidence requires exact source evidence`);
  }
  validateDenseGroups(
    probeId,
    "entity",
    [
      ...context.notationOccurrences.map((item) => item.entityGroup),
      ...context.equationLinks.flatMap((link) => link.sharedEntityGroups),
    ],
  );
  validateDenseGroups(
    probeId,
    "claim",
    context.claimEvidence.flatMap((claim) => [
      claim.claimGroup,
      ...claim.supportingClaimGroups,
    ]),
  );
  for (const link of context.equationLinks) {
    if (new Set(link.sharedEntityGroups).size !== link.sharedEntityGroups.length) {
      throw new Error(`${probeId}: equation link repeats an entity group`);
    }
  }
  const occurrenceGroups = new Set(
    context.notationOccurrences.map((occurrence) => occurrence.entityGroup),
  );
  if (
    context.equationLinks.some((link) =>
      link.sharedEntityGroups.some((group) => !occurrenceGroups.has(group)),
    )
  ) {
    throw new Error(`${probeId}: equation link references an unknown entity group`);
  }
  const definedClaims = new Map<number, string>();
  for (const claim of context.claimEvidence) {
    const identity = JSON.stringify(claim.claim);
    const existing = definedClaims.get(claim.claimGroup);
    if (existing !== undefined && existing !== identity) {
      throw new Error(`${probeId}: claim group maps to multiple claim anchors`);
    }
    definedClaims.set(claim.claimGroup, identity);
  }
  if (
    context.claimEvidence.some((claim) =>
      claim.supportingClaimGroups.some((group) => !definedClaims.has(group)),
    )
  ) {
    throw new Error(`${probeId}: claim evidence references an unknown claim group`);
  }
}

function validateDenseGroups(
  probeId: string,
  surface: "claim" | "entity",
  groups: readonly number[],
): void {
  const distinct = [...new Set(groups)].sort((left, right) => left - right);
  if (distinct.some((group, ordinal) => group !== ordinal)) {
    throw new Error(`${probeId}: ${surface} groups must be dense and zero-based`);
  }
}

function requireFreshContextValues(
  label: string,
  actual: readonly string[],
  required: readonly string[],
): void {
  const values = new Set(actual);
  const missing = required.filter((value) => !values.has(value));
  if (missing.length) {
    throw new Error(
      `v0.36 fresh blind tranche lacks ${label}: ${missing.join(", ")}`,
    );
  }
}

function validateEntitySurfaceCommissioning(
  probeId: string,
  expected: AuthoredScientificFixture["probes"][number]["expected"],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
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
    const definitions = resolvedLocationKeys(definition.required, snapshot);
    const referenceKeys = new Set(
      resolvedLocationKeys(references.required, snapshot),
    );
    if (definitions.some((location) => !referenceKeys.has(location))) {
      throw new Error(
        `${probeId}: every definition must be present in the complete reference surface`,
      );
    }
    const symbol = expected.symbol;
    if (!symbol || !references.required.every((anchor) => selectedAnchorText(anchor, snapshot) === symbol)) {
      throw new Error(
        `${probeId}: authorized references require one exact atomic source spelling`,
      );
    }
    const authoredOccurrences = exactAtomicOccurrences(snapshot, symbol);
    if (authoredOccurrences.length !== references.required.length) {
      throw new Error(
        `${probeId}: reference allowlist must enumerate every exact atomic source occurrence`,
      );
    }
  }
  if (rename.status === "available") {
    const referenceKeys = resolvedLocationKeys(references.required, snapshot);
    const renameKeys = resolvedLocationKeys(rename.required, snapshot);
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

function selectedAnchorText(
  anchor: AuthoredScientificFixture["probes"][number]["expected"]["navigation"]["references"]["required"][number],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): string {
  const document = snapshot.documents.find((candidate) => candidate.fileId === anchor.fileId)!;
  const range = resolveAuthoredAnchor(snapshot, anchor).range;
  return document.content.slice(range.startOffset, range.endOffset);
}

function exactAtomicOccurrences(
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  symbol: string,
): string[] {
  const output: string[] = [];
  const identifier = /[\p{L}\p{N}_]/u;
  for (const document of snapshot.documents) {
    for (let start = document.content.indexOf(symbol); start >= 0;) {
      const end = start + symbol.length;
      const before = document.content.slice(Math.max(0, start - 1), start);
      const after = document.content.slice(end, end + 1);
      if (!identifier.test(before) && !identifier.test(after)) {
        output.push(`${document.fileId}:${start}:${end}`);
      }
      start = document.content.indexOf(symbol, start + Math.max(symbol.length, 1));
    }
  }
  return output.sort();
}

function validateExactLocationContract(
  probeId: string,
  surface: string,
  expected: AuthoredLocationExpectation,
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): void {
  if (expected.minimum !== expected.required.length) {
    throw new Error(
      `${probeId}: ${surface} must enumerate its complete location allowlist`,
    );
  }
  if (expected.status === "available" && expected.required.length === 0) {
    throw new Error(`${probeId}: available ${surface} requires a source location`);
  }
  const locations = expected.required.map((anchor) => {
    const resolved = resolveAuthoredAnchor(snapshot, anchor);
    return `${resolved.fileId}:${resolved.range.startOffset}:${resolved.range.endOffset}`;
  });
  if (new Set(locations).size !== locations.length) {
    throw new Error(`${probeId}: ${surface} repeats a reviewed source location`);
  }
}

function sameRenameNotationFamily(current: string, replacement: string): boolean {
  const controlSequence = /^\\\p{L}+$/u;
  const plainIdentifier = /^\p{L}$/u;
  return controlSequence.test(current)
    ? controlSequence.test(replacement)
    : plainIdentifier.test(current) && plainIdentifier.test(replacement);
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
  const falseAuthoringConflictIds: string[] = [];
  const falseEstablishmentIds: string[] = [];
  const moreAuthoritativeDispositionIds: string[] = [];
  let unsafeAuthoringContextFacts = 0;
  const unsafeAuthoringContextCaseIds = new Set<string>();
  let unsafeLifecycleTransitions = 0;
  const unsafeLifecycleCaseIds = new Set<string>();
  const unsafeNavigationOrEditCaseIds = new Set<string>();
  for (const probe of fixture.probes) {
    const observed = byId.get(probe.id);
    if (!observed) continue;
    const scenario = authoredScenarioFor(fixture, probe);
    const snapshot = authoredSnapshotFor(scenario, probe);
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
    if (probe.expected.authoringContext) {
      const context = compareAuthoredMathAuthoringContext(
        snapshot,
        probe.expected.authoringContext,
        observed.authoringContext,
      );
      if (context.moreAuthoritativeDisposition) {
        moreAuthoritativeDispositionIds.push(probe.id);
      }
      if (context.falseConflictDisposition) {
        falseAuthoringConflictIds.push(probe.id);
      }
      unsafeAuthoringContextFacts += context.unexpected.length;
      if (context.unexpected.length) {
        unsafeAuthoringContextCaseIds.add(probe.id);
      }
      unsafeLifecycleTransitions += context.unsafeLifecycle.length;
      if (context.unsafeLifecycle.length) unsafeLifecycleCaseIds.add(probe.id);
    }
    const problems = observed.diagnostics.filter(
      (diagnostic) =>
        diagnostic.severity === "error" || diagnostic.severity === "warning",
    );
    if (problems.length > probe.expected.diagnostics.maximum) {
      diagnosticsOverLimitIds.push(probe.id);
    }
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
    const unsafeCaseLocations =
      unsafeDefinitions +
      unsafeReferences +
      unsafePreparation +
      unsafeRename;
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
    falseAuthoringConflict: falseAuthoringConflictIds.length,
    falseAuthoringConflictIds: falseAuthoringConflictIds.sort(),
    falseEstablishment: falseEstablishmentIds.length,
    falseEstablishmentIds: falseEstablishmentIds.sort(),
    moreAuthoritativeDispositionIds: moreAuthoritativeDispositionIds.sort(),
    moreAuthoritativeDispositions: moreAuthoritativeDispositionIds.length,
    unsafeAuthoringContextCaseIds: [...unsafeAuthoringContextCaseIds].sort(),
    unsafeAuthoringContextFacts,
    unsafeLifecycleCaseIds: [...unsafeLifecycleCaseIds].sort(),
    unsafeLifecycleTransitions,
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
    summary.falseAuthoringConflict > 0 ||
    summary.falseEstablishment > 0 ||
    summary.moreAuthoritativeDispositions > 0 ||
    summary.unsafeAuthoringContextFacts > 0 ||
    summary.unsafeLifecycleTransitions > 0 ||
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

function unsafeLocations<
  Location extends { readonly fileId: string; readonly range: SourceRange },
>(
  observed: readonly Location[],
  expected: {
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
    readonly status: "available" | "unavailable";
  },
  snapshot: ReturnType<typeof authoredSnapshotFor>,
  additionalUnsafe: (location: Location) => boolean = () => false,
): number {
  if (expected.status === "unavailable") return observed.length;
  const required = expected.required.map((anchor) =>
    resolveAuthoredAnchor(snapshot, anchor),
  );
  const excluded = expected.excluded.map((anchor) =>
    resolveAuthoredAnchor(snapshot, anchor),
  );
  return observed.filter(
    (location) =>
      additionalUnsafe(location) ||
      !required.some(
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
}

function unsafePrepareRenameLocation(
  observed: AuthoredScientificObservation["prepareRename"],
  expected: AuthoredScientificFixture["probes"][number]["expected"]["navigation"]["prepareRename"],
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): number {
  if (!observed.range) return 0;
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
