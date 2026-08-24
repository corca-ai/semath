import type { AuthoredIntegrityProfile } from "./authored-integrity";

const DIGEST = /^[0-9a-f]{64}$/u;
const COMMIT = /^[0-9a-f]{40}$/u;
const RELEASE_ID = /^v0\.[1-9][0-9]*$/u;

export interface SpentHoldoutProfile {
  readonly documentSha256: readonly string[];
  readonly id: string;
  readonly mathFingerprintSha256: readonly string[];
  readonly proseShingleSha256: readonly string[];
}

export interface SpentHoldoutLineage {
  readonly batchId: string;
  readonly probeIds: readonly string[];
  readonly profiles: readonly SpentHoldoutProfile[];
  readonly rawDigests: readonly string[];
  readonly releaseId: string;
  readonly scenarioIds: readonly string[];
}

export interface SpentHoldoutRegistry {
  readonly entries: readonly {
    readonly lineage: SpentHoldoutLineage;
    readonly outcome: {
      readonly cases: number;
      readonly falseConflict: number;
      readonly falseEstablishment: number;
      readonly mathAuthoringExact: number;
      readonly missedCoverage: number;
      readonly navigationOrIdentity: number;
      readonly passed: number;
      readonly risk: number;
      readonly safetyFailures: number;
    };
    readonly terminal: {
      readonly artifactId: string;
      readonly candidateCommit: string;
      readonly candidateTree: string;
      readonly evaluationSha256: string;
      readonly fixtureSha256: string;
      readonly runId: string;
    };
  }[];
  readonly profileAlgorithm: {
    readonly digest: "sha256";
    readonly document: "utf8-exact-content-v1";
    readonly math: "wasmtex-authored-math-fingerprint-v1";
    readonly prose: "wasmtex-visible-prose-5-shingle-v1";
  };
  readonly schemaVersion: 1;
}

export interface SpentHoldoutIsolationSummary {
  readonly comparedProfiles: number;
  readonly maximumMathSimilarity: number;
  readonly maximumProseSimilarity: number;
  readonly spentReleases: number;
}

export function parseSpentHoldoutRegistry(value: unknown): SpentHoldoutRegistry {
  const root = record(value, "spent holdout registry");
  exact(root, ["schemaVersion", "profileAlgorithm", "entries"], "spent holdout registry");
  if (root.schemaVersion !== 1) throw new Error("spent holdout registry.schemaVersion: must be 1");

  const algorithm = record(root.profileAlgorithm, "spent holdout registry.profileAlgorithm");
  exact(
    algorithm,
    ["digest", "document", "math", "prose"],
    "spent holdout registry.profileAlgorithm",
  );
  if (
    algorithm.digest !== "sha256" ||
    algorithm.document !== "utf8-exact-content-v1" ||
    algorithm.math !== "wasmtex-authored-math-fingerprint-v1" ||
    algorithm.prose !== "wasmtex-visible-prose-5-shingle-v1"
  ) {
    throw new Error("spent holdout registry.profileAlgorithm: unsupported algorithm");
  }

  if (!Array.isArray(root.entries) || root.entries.length === 0) {
    throw new Error("spent holdout registry.entries: must be a nonempty array");
  }
  const entries = root.entries.map((value, index) => parseEntry(value, index));
  sortedUnique(entries.map((entry) => entry.lineage.releaseId), "spent release ids");
  unique(entries.map((entry) => entry.lineage.batchId), "spent batch ids");
  unique(entries.map((entry) => entry.terminal.runId), "spent run ids");
  unique(entries.map((entry) => entry.terminal.artifactId), "spent artifact ids");
  unique(
    entries.map((entry) => entry.terminal.fixtureSha256),
    "spent fixture digests",
  );
  unique(
    entries.map((entry) => entry.terminal.evaluationSha256),
    "spent evaluation digests",
  );

  return {
    entries,
    profileAlgorithm: {
      digest: "sha256",
      document: "utf8-exact-content-v1",
      math: "wasmtex-authored-math-fingerprint-v1",
      prose: "wasmtex-visible-prose-5-shingle-v1",
    },
    schemaVersion: 1,
  };
}

export function validateSpentHoldoutIsolation(
  registry: SpentHoldoutRegistry,
  candidate: SpentHoldoutLineage,
): SpentHoldoutIsolationSummary {
  const candidateIds = {
    probe: new Set(candidate.probeIds),
    raw: new Set(candidate.rawDigests),
    scenario: new Set(candidate.scenarioIds),
  };
  let comparedProfiles = 0;
  let maximumMathSimilarity = 0;
  let maximumProseSimilarity = 0;

  for (const entry of registry.entries) {
    const spent = entry.lineage;
    refuseEqual(candidate.releaseId, spent.releaseId, "release id");
    refuseEqual(candidate.batchId, spent.batchId, "batch id");
    refuseOverlap(candidateIds.scenario, spent.scenarioIds, "scenario id");
    refuseOverlap(candidateIds.probe, spent.probeIds, "probe id");
    refuseOverlap(candidateIds.raw, spent.rawDigests, "scenario raw digest");

    for (const fresh of candidate.profiles) {
      for (const historical of spent.profiles) {
        comparedProfiles++;
        refuseOverlap(
          new Set(fresh.documentSha256),
          historical.documentSha256,
          "document digest",
        );
        const mathSimilarity = jaccard(
          fresh.mathFingerprintSha256,
          historical.mathFingerprintSha256,
        );
        const proseSimilarity = jaccard(
          fresh.proseShingleSha256,
          historical.proseShingleSha256,
        );
        maximumMathSimilarity = Math.max(maximumMathSimilarity, mathSimilarity);
        maximumProseSimilarity = Math.max(maximumProseSimilarity, proseSimilarity);
        const exactMath =
          fresh.mathFingerprintSha256.length > 0 &&
          setsEqual(
            fresh.mathFingerprintSha256,
            historical.mathFingerprintSha256,
          );
        if (proseSimilarity >= 0.5 || (exactMath && proseSimilarity >= 0.25)) {
          throw new Error(
            `fresh blind spent-lineage similarity requires review: ${fresh.id}/${historical.id}`,
          );
        }
      }
    }
  }

  return {
    comparedProfiles,
    maximumMathSimilarity,
    maximumProseSimilarity,
    spentReleases: registry.entries.length,
  };
}

/** Hashing happens at the effectful boundary; this helper documents the profile shape. */
export function spentHoldoutProfile(
  id: string,
  documentSha256: readonly string[],
  profile: AuthoredIntegrityProfile,
  digest: (value: string) => string,
): SpentHoldoutProfile {
  return {
    documentSha256: sortedDigests(documentSha256, `${id}.documentSha256`),
    id: nonempty(id, "spent holdout profile.id"),
    mathFingerprintSha256: sortedDigests(
      profile.mathFingerprints.map(digest),
      `${id}.mathFingerprintSha256`,
    ),
    proseShingleSha256: sortedDigests(
      profile.proseShingles.map(digest),
      `${id}.proseShingleSha256`,
    ),
  };
}

function parseEntry(value: unknown, index: number): SpentHoldoutRegistry["entries"][number] {
  const label = `spent holdout registry.entries[${index}]`;
  const entry = record(value, label);
  exact(entry, ["terminal", "outcome", "lineage"], label);

  const terminal = record(entry.terminal, `${label}.terminal`);
  exact(
    terminal,
    ["runId", "artifactId", "candidateCommit", "candidateTree", "fixtureSha256", "evaluationSha256"],
    `${label}.terminal`,
  );
  const outcome = record(entry.outcome, `${label}.outcome`);
  exact(
    outcome,
    [
      "cases",
      "passed",
      "risk",
      "falseEstablishment",
      "falseConflict",
      "navigationOrIdentity",
      "missedCoverage",
      "mathAuthoringExact",
      "safetyFailures",
    ],
    `${label}.outcome`,
  );
  const lineage = parseLineage(entry.lineage, `${label}.lineage`);
  const cases = count(outcome.cases, `${label}.outcome.cases`);
  if (cases !== lineage.scenarioIds.length) {
    throw new Error(`${label}.outcome.cases: must equal scenario count`);
  }
  const bounded = (value: unknown, name: string): number => {
    const parsed = count(value, `${label}.outcome.${name}`);
    if (parsed > cases) {
      throw new Error(`${label}.outcome.${name}: must not exceed cases`);
    }
    return parsed;
  };

  return {
    lineage,
    outcome: {
      cases,
      falseConflict: bounded(outcome.falseConflict, "falseConflict"),
      falseEstablishment: bounded(
        outcome.falseEstablishment,
        "falseEstablishment",
      ),
      mathAuthoringExact: bounded(
        outcome.mathAuthoringExact,
        "mathAuthoringExact",
      ),
      missedCoverage: bounded(outcome.missedCoverage, "missedCoverage"),
      navigationOrIdentity: bounded(
        outcome.navigationOrIdentity,
        "navigationOrIdentity",
      ),
      passed: bounded(outcome.passed, "passed"),
      risk: count(outcome.risk, `${label}.outcome.risk`),
      safetyFailures: count(outcome.safetyFailures, `${label}.outcome.safetyFailures`),
    },
    terminal: {
      artifactId: integerString(terminal.artifactId, `${label}.terminal.artifactId`),
      candidateCommit: commit(terminal.candidateCommit, `${label}.terminal.candidateCommit`),
      candidateTree: commit(terminal.candidateTree, `${label}.terminal.candidateTree`),
      evaluationSha256: digest(terminal.evaluationSha256, `${label}.terminal.evaluationSha256`),
      fixtureSha256: digest(terminal.fixtureSha256, `${label}.terminal.fixtureSha256`),
      runId: integerString(terminal.runId, `${label}.terminal.runId`),
    },
  };
}

function parseLineage(value: unknown, label: string): SpentHoldoutLineage {
  const lineage = record(value, label);
  exact(
    lineage,
    ["releaseId", "batchId", "scenarioIds", "probeIds", "rawDigests", "profiles"],
    label,
  );
  if (!Array.isArray(lineage.profiles) || lineage.profiles.length === 0) {
    throw new Error(`${label}.profiles: must be a nonempty array`);
  }
  const profiles = lineage.profiles.map((value, index) => {
    const profileLabel = `${label}.profiles[${index}]`;
    const profile = record(value, profileLabel);
    exact(
      profile,
      ["id", "documentSha256", "mathFingerprintSha256", "proseShingleSha256"],
      profileLabel,
    );
    return {
      documentSha256: digestArray(profile.documentSha256, `${profileLabel}.documentSha256`),
      id: nonempty(profile.id, `${profileLabel}.id`),
      mathFingerprintSha256: digestArray(
        profile.mathFingerprintSha256,
        `${profileLabel}.mathFingerprintSha256`,
      ),
      proseShingleSha256: digestArray(
        profile.proseShingleSha256,
        `${profileLabel}.proseShingleSha256`,
      ),
    };
  });
  sortedUnique(profiles.map((profile) => profile.id), `${label}.profile ids`);
  const scenarioIds = stringArray(lineage.scenarioIds, `${label}.scenarioIds`);
  const rawDigests = digestArray(lineage.rawDigests, `${label}.rawDigests`);
  if (rawDigests.length !== scenarioIds.length) {
    throw new Error(`${label}.rawDigests: must equal scenario count`);
  }
  validateProfileCompleteness(scenarioIds, profiles, label);
  return {
    batchId: nonempty(lineage.batchId, `${label}.batchId`),
    probeIds: stringArray(lineage.probeIds, `${label}.probeIds`),
    profiles,
    rawDigests,
    releaseId: releaseId(lineage.releaseId, `${label}.releaseId`),
    scenarioIds,
  };
}

function validateProfileCompleteness(
  scenarioIds: readonly string[],
  profiles: readonly SpentHoldoutProfile[],
  label: string,
): void {
  for (const scenarioId of scenarioIds) {
    const aggregate = profiles.filter((profile) => profile.id === scenarioId);
    if (aggregate.length !== 1) {
      throw new Error(
        `${label}.profiles: ${scenarioId} requires one aggregate profile`,
      );
    }
    const children = profiles.filter((profile) =>
      profile.id.startsWith(`${scenarioId}/`),
    );
    if (children.length === 0) {
      throw new Error(
        `${label}.profiles: ${scenarioId} requires a document profile`,
      );
    }
    for (const child of children) {
      if (child.documentSha256.length !== 1) {
        throw new Error(
          `${label}.profiles: ${child.id} requires one document digest`,
        );
      }
    }
    const expected = aggregate[0]!;
    if (expected.documentSha256.length === 0) {
      throw new Error(
        `${label}.profiles: ${scenarioId} requires document digests`,
      );
    }
    requireUnion(
      expected.documentSha256,
      children.flatMap((profile) => profile.documentSha256),
      `${scenarioId}.documentSha256`,
    );
    requireUnion(
      expected.mathFingerprintSha256,
      children.flatMap((profile) => profile.mathFingerprintSha256),
      `${scenarioId}.mathFingerprintSha256`,
    );
    requireUnion(
      expected.proseShingleSha256,
      children.flatMap((profile) => profile.proseShingleSha256),
      `${scenarioId}.proseShingleSha256`,
    );
  }
  for (const profile of profiles) {
    const owners = scenarioIds.filter(
      (scenarioId) =>
        profile.id === scenarioId || profile.id.startsWith(`${scenarioId}/`),
    );
    if (owners.length !== 1) {
      throw new Error(
        `${label}.profiles: ${profile.id} has no unique scenario owner`,
      );
    }
  }
}

function requireUnion(
  aggregate: readonly string[],
  childValues: readonly string[],
  label: string,
): void {
  const union = [...new Set(childValues)].sort();
  if (
    aggregate.length !== union.length ||
    aggregate.some((value, index) => value !== union[index])
  ) {
    throw new Error(`spent holdout profile union mismatch: ${label}`);
  }
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label}: must be an object`);
  }
  return value as Record<string, unknown>;
}

function exact(value: Record<string, unknown>, keys: readonly string[], label: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${label}: expected exact keys ${expected.join(", ")}`);
  }
}

function nonempty(value: unknown, label: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${label}: must be a nonempty string`);
  return value;
}

function digest(value: unknown, label: string): string {
  const parsed = nonempty(value, label);
  if (!DIGEST.test(parsed)) throw new Error(`${label}: must be a lowercase SHA-256 digest`);
  return parsed;
}

function commit(value: unknown, label: string): string {
  const parsed = nonempty(value, label);
  if (!COMMIT.test(parsed)) throw new Error(`${label}: must be a lowercase full Git SHA`);
  return parsed;
}

function releaseId(value: unknown, label: string): string {
  const parsed = nonempty(value, label);
  if (!RELEASE_ID.test(parsed)) {
    throw new Error(`${label}: must be a semantic release id`);
  }
  return parsed;
}

function integerString(value: unknown, label: string): string {
  const parsed = nonempty(value, label);
  if (!/^[1-9][0-9]*$/u.test(parsed)) throw new Error(`${label}: must be a positive integer string`);
  return parsed;
}

function count(value: unknown, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error(`${label}: must be a nonnegative integer`);
  return value as number;
}

function stringArray(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value) || value.length === 0) throw new Error(`${label}: must be a nonempty array`);
  const parsed = value.map((item, index) => nonempty(item, `${label}[${index}]`));
  sortedUnique(parsed, label);
  return parsed;
}

function digestArray(value: unknown, label: string): readonly string[] {
  if (!Array.isArray(value)) throw new Error(`${label}: must be an array`);
  const parsed = value.map((item, index) => digest(item, `${label}[${index}]`));
  sortedUnique(parsed, label);
  return parsed;
}

function sortedDigests(values: readonly string[], label: string): readonly string[] {
  return [
    ...new Set(
      values.map((value, index) => digest(value, `${label}[${index}]`)),
    ),
  ].sort();
}

function sortedUnique(values: readonly string[], label: string): void {
  unique(values, label);
  const sorted = [...values].sort();
  if (values.some((value, index) => value !== sorted[index])) throw new Error(`${label}: must be sorted`);
}

function unique(values: readonly string[], label: string): void {
  if (new Set(values).size !== values.length) throw new Error(`${label}: must not contain duplicates`);
}

function refuseEqual(left: string, right: string, label: string): void {
  if (left === right) throw new Error(`fresh blind fixture reuses spent ${label}: ${left}`);
}

function refuseOverlap(left: ReadonlySet<string>, right: readonly string[], label: string): void {
  const overlap = right.find((value) => left.has(value));
  if (overlap) throw new Error(`fresh blind fixture reuses spent ${label}: ${overlap}`);
}

function jaccard(left: readonly string[], right: readonly string[]): number {
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  const union = new Set([...leftSet, ...rightSet]);
  if (union.size === 0) return 0;
  let intersection = 0;
  for (const value of leftSet) if (rightSet.has(value)) intersection++;
  return intersection / union.size;
}

function setsEqual(left: readonly string[], right: readonly string[]): boolean {
  const leftSet = new Set(left);
  const rightSet = new Set(right);
  return leftSet.size === rightSet.size && [...leftSet].every((value) => rightSet.has(value));
}
