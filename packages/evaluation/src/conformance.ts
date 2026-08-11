import type {
  CapabilityId,
  CapabilityMaturity,
  Corpus,
  QualityManifest,
  SuiteTier,
} from "./model";

export interface PackCatalogEntry {
  activationRules: number;
  concepts: number;
  lawIds: readonly string[];
  operators: number;
  packId: string;
  quantityKinds: number;
  roles: number;
  units: number;
}

export type PackSummary =
  | "evaluated"
  | "mixed"
  | "probe"
  | "unsupported"
  | "vocabulary-only";

export interface PackConformanceScore {
  capabilities: Readonly<Record<CapabilityId, CapabilityMaturity>>;
  coveredLaws: number;
  laws: number;
  packId: string;
  scoredCases: number;
  summary: PackSummary;
}

export interface PackConformanceReport {
  failures: readonly string[];
  packs: readonly PackConformanceScore[];
  schemaVersion: 2;
}

const TIER_MINIMUMS = {
  evaluated: { dimensions: 6, positives: 30, refusals: 20 },
  probe: { dimensions: 3, positives: 5, refusals: 5 },
} as const;

const CAPABILITY_DIMENSIONS: Partial<Record<CapabilityId, readonly string[]>> = {
  "declarations-roles": ["prose", "roles"],
  "diagnostics-refusal": ["semantic-mutation"],
  "navigation-explanation": ["roles"],
  "project-macro": ["project-context", "macro-provenance"],
  "shape-quantity-unit": ["constraints"],
};

export function checkPackConformance(
  manifest: QualityManifest,
  catalog: readonly PackCatalogEntry[],
  corpora: ReadonlyMap<string, Corpus>,
  foundationCaseCounts: ReadonlyMap<string, number> = new Map(),
): PackConformanceReport {
  const failures: string[] = [];
  const catalogById = uniqueCatalog(catalog, failures);
  const supportById = new Map(manifest.packs.map((pack) => [pack.packId, pack]));
  for (const packId of catalogById.keys()) {
    if (!supportById.has(packId)) failures.push(`${packId}: missing support declaration`);
  }
  for (const packId of supportById.keys()) {
    if (!catalogById.has(packId)) failures.push(`${packId}: support declaration has no pack`);
  }

  const suiteOwners = new Map<string, string>();
  for (const support of manifest.packs) {
    for (const capability of Object.values(support.capabilities)) {
      for (const suiteId of capability.suiteIds) {
        const existing = suiteOwners.get(suiteId);
        if (existing && existing !== support.packId) {
          failures.push(`${suiteId}: owned by both ${existing} and ${support.packId}`);
        } else {
          suiteOwners.set(suiteId, support.packId);
        }
      }
    }
  }
  for (const suite of manifest.suites) {
    if (suite.kind === "global-refusal") {
      if (suiteOwners.has(suite.id)) {
        failures.push(`${suite.id}: global-refusal suite must not be pack-owned`);
      }
      continue;
    }
    if (suiteOwners.get(suite.id) !== suite.packId) {
      failures.push(
        `${suite.id}: suite owner is ${suiteOwners.get(suite.id) ?? "missing"}, expected ${suite.packId}`,
      );
    }
    validateLawSuiteBudget(suite, failures);
  }
  for (const suite of manifest.foundationSuites) {
    if (suiteOwners.get(suite.id) !== suite.packId) {
      failures.push(
        `${suite.id}: suite owner is ${suiteOwners.get(suite.id) ?? "missing"}, expected ${suite.packId}`,
      );
    }
    const count = foundationCaseCounts.get(suite.id);
    if (count === undefined) failures.push(`${suite.id}: foundation corpus was not loaded`);
    else if (count < suite.minimumCases) {
      failures.push(`${suite.id}: ${count} cases; requires ${suite.minimumCases}`);
    }
    const minimumDimensions = suite.tier === "evaluated" ? 4 : 2;
    if (suite.requiredDimensions.length < minimumDimensions) {
      failures.push(
        `${suite.id}: ${suite.tier} foundation requires ${minimumDimensions} dimensions`,
      );
    }
  }

  const scores: PackConformanceScore[] = [];
  for (const support of [...manifest.packs].sort((left, right) =>
    left.packId.localeCompare(right.packId),
  )) {
    const pack = catalogById.get(support.packId);
    if (!pack) continue;
    validateCapabilities(manifest, support, pack, corpora, failures);
    const lawSuites = support.capabilities["law-recognition"].suiteIds
      .map((suiteId) => manifest.suites.find((suite) => suite.id === suiteId))
      .filter((suite) => suite?.kind === "law");
    const covered = new Set<string>();
    let scoredCases = 0;
    for (const suite of lawSuites) {
      const corpus = corpora.get(suite.id);
      if (!corpus) {
        failures.push(`${suite.id}: corpus was not loaded`);
        continue;
      }
      scoredCases += corpus.cases.length;
      for (const lawId of new Set(corpus.cases.flatMap((item) =>
        "lawId" in item ? [item.lawId] : [],
      ))) {
        if (!pack.lawIds.includes(lawId)) {
          failures.push(`${suite.id}: corpus targets unknown law ${lawId}`);
        } else {
          covered.add(lawId);
        }
      }
    }
    for (const lawId of pack.lawIds) {
      if (!covered.has(lawId)) failures.push(`${support.packId}/${lawId}: no corpus coverage`);
    }
    scores.push({
      capabilities: Object.fromEntries(
        Object.entries(support.capabilities).map(([id, capability]) => [
          id,
          capability.maturity,
        ]),
      ) as Readonly<Record<CapabilityId, CapabilityMaturity>>,
      coveredLaws: covered.size,
      laws: pack.lawIds.length,
      packId: support.packId,
      scoredCases,
      summary: summarizeMaturity(support.capabilities),
    });
  }
  return { failures: [...new Set(failures)].sort(), packs: scores, schemaVersion: 2 };
}

function validateCapabilities(
  manifest: QualityManifest,
  support: QualityManifest["packs"][number],
  pack: PackCatalogEntry,
  corpora: ReadonlyMap<string, Corpus>,
  failures: string[],
): void {
  for (const [id, declaration] of Object.entries(support.capabilities) as [
    CapabilityId,
    (typeof support.capabilities)[CapabilityId],
  ][]) {
    if (id === "concept-vocabulary") {
      if (declaration.maturity !== "unsupported" && pack.concepts === 0) {
        failures.push(`${pack.packId}/${id}: supported vocabulary has no concepts`);
      }
      continue;
    }
    if (id === "law-recognition") {
      validateLawCapability(manifest, support.packId, declaration, pack, corpora, failures);
      continue;
    }
    if (declaration.maturity === "unsupported") continue;
    const suites = declaration.suiteIds.flatMap((suiteId) => [
      ...manifest.suites.flatMap((suite) =>
        suite.id === suiteId && suite.kind === "law" ? [suite] : [],
      ),
      ...manifest.foundationSuites.filter((suite) => suite.id === suiteId),
    ]);
    if (
      declaration.maturity === "evaluated" &&
      !suites.some((suite) => suite.tier === "evaluated")
    ) {
      failures.push(`${pack.packId}/${id}: evaluated capability lacks evaluated evidence`);
    }
    const dimensions = new Set(suites.flatMap((suite) => suite.requiredDimensions));
    for (const required of CAPABILITY_DIMENSIONS[id] ?? []) {
      if (!dimensions.has(required)) {
        failures.push(`${pack.packId}/${id}: evidence lacks ${required} dimension`);
      }
    }
  }
}

function validateLawCapability(
  manifest: QualityManifest,
  packId: string,
  declaration: QualityManifest["packs"][number]["capabilities"]["law-recognition"],
  pack: PackCatalogEntry,
  corpora: ReadonlyMap<string, Corpus>,
  failures: string[],
): void {
  if (declaration.maturity === "unsupported") {
    if (pack.lawIds.length) failures.push(`${packId}: unsupported law capability contains laws`);
    return;
  }
  if (!pack.lawIds.length) {
    failures.push(`${packId}: ${declaration.maturity} law capability contains no laws`);
    return;
  }
  const minimum = TIER_MINIMUMS[declaration.maturity];
  const suites = declaration.suiteIds
    .map((suiteId) => manifest.suites.find((suite) => suite.id === suiteId))
    .filter((suite) => suite?.kind === "law");
  if (
    declaration.maturity === "evaluated" &&
    !suites.some((suite) => suite.tier === "evaluated")
  ) {
    failures.push(`${packId}/law-recognition: evaluated capability lacks evaluated evidence`);
  }
  for (const lawId of pack.lawIds) {
    let positives = 0;
    let refusals = 0;
    const dimensions = new Set<string>();
    for (const suite of suites) {
      const cases = corpora.get(suite.id)?.cases.filter(
        (item) => "lawId" in item && item.lawId === lawId,
      ) ?? [];
      positives += cases.filter((item) => item.expectation === "recognized").length;
      refusals += cases.filter((item) => item.expectation === "refused").length;
      if (cases.length) suite.requiredDimensions.forEach((dimension) => dimensions.add(dimension));
    }
    if (positives < minimum.positives) {
      failures.push(`${packId}/${lawId}: ${positives} positives; requires ${minimum.positives}`);
    }
    if (refusals < minimum.refusals) {
      failures.push(`${packId}/${lawId}: ${refusals} refusals; requires ${minimum.refusals}`);
    }
    if (dimensions.size < minimum.dimensions) {
      failures.push(`${packId}/${lawId}: ${dimensions.size} dimensions; requires ${minimum.dimensions}`);
    }
  }
}

function validateLawSuiteBudget(
  suite: Extract<QualityManifest["suites"][number], { kind: "law" }>,
  failures: string[],
): void {
  const minimum = TIER_MINIMUMS[suite.tier];
  if (suite.minimumPositiveCasesPerLaw < minimum.positives) {
    failures.push(
      `${suite.id}: ${suite.tier} requires at least ${minimum.positives} positive cases per law`,
    );
  }
  if (suite.minimumRefusalCasesPerLaw < minimum.refusals) {
    failures.push(
      `${suite.id}: ${suite.tier} requires at least ${minimum.refusals} refusal cases per law`,
    );
  }
  if (suite.requiredDimensions.length < minimum.dimensions) {
    failures.push(
      `${suite.id}: ${suite.tier} requires at least ${minimum.dimensions} coverage dimensions`,
    );
  }
}

export function summarizePack(value: unknown, path: string): PackCatalogEntry {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path}: pack must be an object`);
  }
  const pack = value as Record<string, unknown>;
  if (typeof pack.packId !== "string" || !pack.packId) {
    throw new Error(`${path}.packId: must be nonempty`);
  }
  if (!Array.isArray(pack.laws)) throw new Error(`${path}.laws: must be an array`);
  const lawIds = pack.laws.map((law, index) => {
    if (!law || typeof law !== "object" || Array.isArray(law)) {
      throw new Error(`${path}.laws[${index}]: must be an object`);
    }
    const id = (law as Record<string, unknown>).id;
    if (typeof id !== "string" || !id) {
      throw new Error(`${path}.laws[${index}].id: must be nonempty`);
    }
    return id;
  });
  if (new Set(lawIds).size !== lawIds.length) {
    throw new Error(`${path}.laws: duplicate law id`);
  }
  return {
    activationRules: arrayLength(pack.activationRules),
    concepts: arrayLength(pack.concepts),
    lawIds,
    operators: arrayLength(pack.operators),
    packId: pack.packId,
    quantityKinds: arrayLength(pack.quantityKinds),
    roles: arrayLength(pack.roles),
    units: arrayLength(pack.units),
  };
}

function summarizeMaturity(
  capabilities: QualityManifest["packs"][number]["capabilities"],
): PackSummary {
  const supported = Object.entries(capabilities).filter(
    ([, capability]) => capability.maturity !== "unsupported",
  );
  if (!supported.length) return "unsupported";
  if (
    supported.length === 1 &&
    supported[0]![0] === "concept-vocabulary"
  ) return "vocabulary-only";
  const maturities = new Set(supported.map(([, capability]) => capability.maturity));
  if (maturities.size > 1) return "mixed";
  return maturities.has("evaluated") ? "evaluated" : "probe";
}

function arrayLength(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function uniqueCatalog(
  catalog: readonly PackCatalogEntry[],
  failures: string[],
): Map<string, PackCatalogEntry> {
  const result = new Map<string, PackCatalogEntry>();
  for (const pack of catalog) {
    if (result.has(pack.packId)) failures.push(`${pack.packId}: duplicate pack catalog entry`);
    else result.set(pack.packId, pack);
  }
  return result;
}
