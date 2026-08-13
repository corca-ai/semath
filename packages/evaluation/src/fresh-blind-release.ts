import type { SourceRange } from "../../protocol/src/index";
import {
  DOCUMENT_REASONING_FAMILIES,
  authoredScenarioFor,
  authoredSnapshotFor,
  parseAuthoredScientificFixture,
  resolveAuthoredAnchor,
  scoreAuthoredScientificFixture,
  type AuthoredLawCatalogEntry,
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
  readonly falseConflict: number;
  readonly falseConflictIds: readonly string[];
  readonly falseEstablishment: number;
  readonly falseEstablishmentIds: readonly string[];
  readonly unsafeNavigationOrEdit: number;
  readonly unsafeNavigationOrEditIds: readonly string[];
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
      rename.status === "unavailable" &&
      contract.some((value) => value !== undefined)
    ) {
      throw new Error(
        `${probe.id}: unavailable rename cannot define an edit contract`,
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
  const byId = new Map(
    observations.map((observation) => [observation.caseId, observation]),
  );
  const risk = scoreAuthoredScientificFixture(fixture, observations).risk;
  let unsafeNavigationOrEdit = 0;
  const falseConflictIds: string[] = [];
  const falseEstablishmentIds: string[] = [];
  const unsafeNavigationOrEditIds = new Set<string>();
  for (const probe of fixture.probes) {
    const observed = byId.get(probe.id);
    if (!observed) continue;
    if (
      observed.decision === "established" &&
      probe.expected.decision !== "established"
    ) {
      falseEstablishmentIds.push(probe.id);
    }
    if (
      (observed.decision === "conflicting" &&
        probe.expected.decision !== "conflicting") ||
      observed.diagnostics.length > probe.expected.diagnostics.maximum
    ) {
      falseConflictIds.push(probe.id);
    }
    const snapshot = authoredSnapshotFor(
      authoredScenarioFor(fixture, probe),
      probe,
    );
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
    unsafeNavigationOrEdit += unsafeDefinitions + unsafeReferences;
    if (
      probe.expected.navigation.prepareRename.status === "unavailable" &&
      observed.prepareRename.range
    ) {
      unsafeNavigationOrEdit += 1;
    }
    const unsafeRename = unsafeLocations(
      observed.renameEdits,
      probe.expected.navigation.rename,
      snapshot,
    );
    unsafeNavigationOrEdit += unsafeRename;
    if (
      unsafeDefinitions ||
      unsafeReferences ||
      unsafeRename ||
      (probe.expected.navigation.prepareRename.status === "unavailable" &&
        observed.prepareRename.range)
    ) {
      unsafeNavigationOrEditIds.add(probe.id);
    }
  }
  return {
    falseConflict: risk.falseConflict,
    falseConflictIds: falseConflictIds.sort(),
    falseEstablishment: risk.falseEstablishment,
    falseEstablishmentIds: falseEstablishmentIds.sort(),
    unsafeNavigationOrEdit,
    unsafeNavigationOrEditIds: [...unsafeNavigationOrEditIds].sort(),
  };
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

function unsafeLocations(
  observed: readonly { readonly fileId: string; readonly range: SourceRange }[],
  expected: {
    readonly excluded: readonly {
      readonly fileId: string;
      readonly needle: string;
      readonly occurrence?: number;
      readonly selection?: { readonly length: number; readonly offset: number };
    }[];
    readonly status: "available" | "unavailable";
  },
  snapshot: ReturnType<typeof authoredSnapshotFor>,
): number {
  if (expected.status === "unavailable") return observed.length ? 1 : 0;
  const excluded = expected.excluded.map((anchor) =>
    resolveAuthoredAnchor(snapshot, anchor),
  );
  return observed.some((location) =>
    excluded.some(
      (anchor) =>
        location.fileId === anchor.fileId &&
        sameRange(location.range, anchor.range),
    ),
  )
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
