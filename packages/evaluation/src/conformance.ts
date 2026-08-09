import type { Corpus, QualityManifest, SupportTier } from "./model";

export interface PackCatalogEntry {
  lawIds: readonly string[];
  packId: string;
}

export interface PackConformanceScore {
  authoredCases: number;
  coveredLaws: number;
  laws: number;
  packId: string;
  tier: SupportTier;
}

export interface PackConformanceReport {
  failures: readonly string[];
  packs: readonly PackConformanceScore[];
  schemaVersion: 1;
}

const TIER_MINIMUMS = {
  evaluated: { dimensions: 6, positives: 30, refusals: 20 },
  probe: { dimensions: 3, positives: 5, refusals: 5 },
} as const;

export function checkPackConformance(
  manifest: QualityManifest,
  catalog: readonly PackCatalogEntry[],
  corpora: ReadonlyMap<string, Corpus>,
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
    for (const suiteId of support.corpusSuiteIds) {
      const existing = suiteOwners.get(suiteId);
      if (existing) failures.push(`${suiteId}: owned by both ${existing} and ${support.packId}`);
      else suiteOwners.set(suiteId, support.packId);
    }
  }
  for (const suite of manifest.suites) {
    if (suite.kind === "global-refusal") {
      if (suiteOwners.has(suite.id)) {
        failures.push(`${suite.id}: global-refusal suite must not be pack-owned`);
      }
      continue;
    }
    const owner = suiteOwners.get(suite.id);
    if (owner !== suite.packId) {
      failures.push(`${suite.id}: suite owner is ${owner ?? "missing"}, expected ${suite.packId}`);
    }
    const support = supportById.get(suite.packId);
    if (support && support.tier !== suite.tier) {
      failures.push(
        `${suite.id}: suite tier ${suite.tier} differs from pack tier ${support.tier}`,
      );
    }
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

  const scores: PackConformanceScore[] = [];
  for (const support of [...manifest.packs].sort((left, right) =>
    left.packId.localeCompare(right.packId),
  )) {
    const pack = catalogById.get(support.packId);
    if (!pack) continue;
    const ownedSuites = support.corpusSuiteIds
      .map((suiteId) => manifest.suites.find((suite) => suite.id === suiteId))
      .filter((suite) => suite?.kind === "law");
    const covered = new Set<string>();
    let authoredCases = 0;
    for (const suite of ownedSuites) {
      const corpus = corpora.get(suite.id);
      if (!corpus) {
        failures.push(`${suite.id}: corpus was not loaded`);
        continue;
      }
      authoredCases += corpus.cases.length;
      for (const lawId of new Set(corpus.cases.flatMap((item) =>
        "lawId" in item ? [item.lawId] : [],
      ))) {
        if (!pack.lawIds.includes(lawId)) {
          failures.push(`${suite.id}: corpus targets unknown law ${lawId}`);
          continue;
        }
        covered.add(lawId);
      }
    }
    for (const lawId of pack.lawIds) {
      if (!covered.has(lawId)) failures.push(`${support.packId}/${lawId}: no corpus coverage`);
    }
    if (support.tier === "vocabulary-only") {
      if (pack.lawIds.length !== 0) {
        failures.push(`${support.packId}: vocabulary-only pack contains laws`);
      }
    } else if (pack.lawIds.length === 0) {
      failures.push(`${support.packId}: ${support.tier} pack contains no laws`);
    }
    scores.push({
      authoredCases,
      coveredLaws: covered.size,
      laws: pack.lawIds.length,
      packId: support.packId,
      tier: support.tier,
    });
  }
  return { failures: [...new Set(failures)].sort(), packs: scores, schemaVersion: 1 };
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
  return { lawIds, packId: pack.packId };
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
