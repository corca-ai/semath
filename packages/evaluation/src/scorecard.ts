import type {
  Corpus,
  CorpusCase,
  CorpusExpectation,
  DiversityFacet,
  MetamorphicTransform,
  QualityManifest,
} from "./model";
import { planMetamorphicCases } from "./metamorphic";

export interface CaseObservation {
  caseId: string;
  evidenceIntegrity: boolean;
  establishedLawIds: readonly string[];
  generatedFrom?: {
    caseId: string;
    transform: MetamorphicTransform;
  };
  rolesCorrect: boolean;
  status?:
    | "ambiguous"
    | "conflicting"
    | "established"
    | "partial"
    | "unsupported";
  suiteId: string;
  targetPresent: boolean;
}

export interface Metric {
  denominator: number;
  numerator: number;
  percent: number;
}

export interface LawScore {
  evidenceIntegrity: Metric;
  falsePositives: number;
  lawId: string;
  positives: number;
  precision: Metric;
  recall: Metric;
  refusals: number;
  refusalPreservation: Metric;
  roleAccuracy: Metric;
  suiteId: string;
}

export interface CoverageScore {
  cases: number;
  dimension: string;
  passed: number;
  percent: number;
  suiteId: string;
}

export interface VariationScore {
  cases: number;
  passed: number;
  percent: number;
  tag: string;
}

export interface DiversityScore {
  distinct: number;
  facet: DiversityFacet | "combined-profile";
  largestCell: number;
  largestShare: number;
  suiteId: string;
}

export interface QualityScorecard {
  adversarialRefusal: Metric;
  authoredCases: number;
  coverage: readonly CoverageScore[];
  diversity: readonly DiversityScore[];
  failures: readonly string[];
  generatedCases: number;
  laws: readonly LawScore[];
  metamorphic: Metric;
  refusalCategories: number;
  schemaVersion: 2;
  variations: readonly VariationScore[];
}

interface ExpectedCase {
  case: CorpusCase;
  suiteId: string;
}

interface Counters {
  evidenceEligible: number;
  evidenceValid: number;
  falsePositive: number;
  positive: number;
  recognized: number;
  refusal: number;
  refusalPreserved: number;
  roleCorrect: number;
  roleEligible: number;
}

export function scoreQuality(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
  observations: readonly CaseObservation[],
): QualityScorecard {
  const failures: string[] = [];
  const expected = expectedCases(manifest, corpora, failures);
  const baseObservations = new Map<string, CaseObservation>();
  const generatedObservations = new Map<string, CaseObservation>();
  for (const observation of observations) {
    if (observation.generatedFrom) {
      const key = caseKey(observation.suiteId, observation.caseId);
      if (generatedObservations.has(key)) {
        failures.push(`${displayKey(key)}: duplicate metamorphic observation`);
      }
      generatedObservations.set(key, observation);
      continue;
    }
    const key = caseKey(observation.suiteId, observation.caseId);
    if (baseObservations.has(key)) {
      failures.push(`${displayKey(key)}: duplicate observation`);
    }
    baseObservations.set(key, observation);
  }
  for (const key of baseObservations.keys()) {
    if (!expected.has(key)) {
      failures.push(`${displayKey(key)}: unexpected observation`);
    }
  }

  const lawCounters = new Map<string, Counters>();
  const variationCounters = new Map<string, { cases: number; passed: number }>();
  const dimensionCounters = new Map<string, { cases: number; passed: number }>();
  const refusalCategories = new Set<string>();
  let adversarialCases = 0;
  let adversarialPassed = 0;
  for (const [key, item] of expected) {
    const observation = baseObservations.get(key);
    if (!observation) {
      failures.push(`${displayKey(key)}: missing observation`);
      continue;
    }
    const passed = casePassed(item.case, observation);
    if ("lawId" in item.case) {
      const lawKey = caseKey(item.suiteId, item.case.lawId);
      const counters = lawCounters.get(lawKey) ?? emptyCounters();
      countCase(counters, item.case, observation);
      lawCounters.set(lawKey, counters);
    }
    if (item.case.expectation === "refused" && !("lawId" in item.case)) {
      adversarialCases += 1;
      if (observation.establishedLawIds.length === 0) adversarialPassed += 1;
      else {
        failures.push(
          `${displayKey(key)}: refusal established unexpected laws ${observation.establishedLawIds.join(", ")}`,
        );
      }
    }
    for (const tag of item.case.variationTags) {
      const cell = variationCounters.get(tag) ?? { cases: 0, passed: 0 };
      cell.cases += 1;
      if (passed) cell.passed += 1;
      variationCounters.set(tag, cell);
    }
    for (const dimension of manifest.dimensions) {
      if (!item.case.variationTags.some((tag) => dimension.tags.includes(tag))) continue;
      const dimensionKey = caseKey(item.suiteId, dimension.id);
      const cell = dimensionCounters.get(dimensionKey) ?? { cases: 0, passed: 0 };
      cell.cases += 1;
      if (passed) cell.passed += 1;
      dimensionCounters.set(dimensionKey, cell);
    }
    if ("refusalCategory" in item.case) {
      refusalCategories.add(item.case.refusalCategory);
    }
  }

  const laws = [...lawCounters]
    .map(([key, counters]) => lawScore(key, counters))
    .sort(compareLawScores);
  for (const law of laws) {
    const suite = manifest.suites.find((item) => item.id === law.suiteId);
    if (!suite || suite.kind !== "law") continue;
    if (law.positives < suite.minimumPositiveCasesPerLaw) {
      failures.push(
        `${law.suiteId}/${law.lawId}: ${law.positives} positive cases; requires ${suite.minimumPositiveCasesPerLaw}`,
      );
    }
    if (law.refusals < suite.minimumRefusalCasesPerLaw) {
      failures.push(
        `${law.suiteId}/${law.lawId}: ${law.refusals} refusal cases; requires ${suite.minimumRefusalCasesPerLaw}`,
      );
    }
    threshold(failures, law, "recall", manifest.thresholds.lawRecall);
    threshold(failures, law, "precision", manifest.thresholds.lawPrecision);
    threshold(failures, law, "roleAccuracy", manifest.thresholds.roleAccuracy);
    threshold(
      failures,
      law,
      "evidenceIntegrity",
      manifest.thresholds.evidenceIntegrity,
    );
    threshold(
      failures,
      law,
      "refusalPreservation",
      manifest.thresholds.refusalPreservation,
    );
  }

  const diversity = scoreDiversity(manifest, corpora, failures);

  const coverage = [...dimensionCounters]
    .map(([key, cell]) => {
      const [suiteId, dimension] = splitKey(key);
      return {
        cases: cell.cases,
        dimension,
        passed: cell.passed,
        percent: percent(cell.passed, cell.cases),
        suiteId,
      };
    })
    .sort((left, right) =>
      left.suiteId.localeCompare(right.suiteId) ||
      left.dimension.localeCompare(right.dimension),
    );
  for (const suite of manifest.suites) {
    for (const dimension of suite.requiredDimensions) {
      if (
        !coverage.some(
          (score) => score.suiteId === suite.id && score.dimension === dimension,
        )
      ) {
        failures.push(`${suite.id}: required dimension ${dimension} has no authored cases`);
      }
    }
  }

  const plannedMetamorphic = planMetamorphicCases(manifest, corpora);
  const plannedKeys = new Set(
    plannedMetamorphic.map((item) => caseKey(item.suiteId, item.case.id)),
  );
  for (const [key, observation] of generatedObservations) {
    if (!plannedKeys.has(key)) {
      failures.push(`${observation.suiteId}/${observation.caseId}: unexpected metamorphic observation`);
    }
  }
  let metamorphicPassed = 0;
  for (const planned of plannedMetamorphic) {
    const observation = generatedObservations.get(
      caseKey(planned.suiteId, planned.case.id),
    );
    if (!observation) {
      failures.push(`${planned.suiteId}/${planned.case.id}: missing metamorphic observation`);
      continue;
    }
    const source = expected.get(caseKey(planned.suiteId, planned.sourceCaseId));
    if (!source) {
      failures.push(`${planned.suiteId}/${planned.case.id}: unknown metamorphic source`);
      continue;
    }
    if (
      observation.generatedFrom?.caseId !== planned.sourceCaseId ||
      observation.generatedFrom.transform !== planned.transform
    ) {
      failures.push(`${planned.suiteId}/${planned.case.id}: metamorphic provenance mismatch`);
      continue;
    }
    if (casePassed(source.case, observation)) {
      metamorphicPassed += 1;
    } else {
      failures.push(
        `${planned.suiteId}/${planned.case.id}: metamorphic ${planned.transform} changed the expected result`,
      );
    }
  }
  if (metamorphicPassed !== plannedMetamorphic.length) {
    failures.push(
      `metamorphic invariance: ${metamorphicPassed}/${plannedMetamorphic.length} cases passed`,
    );
  }

  return {
    adversarialRefusal: metric(adversarialPassed, adversarialCases),
    authoredCases: expected.size,
    coverage,
    diversity,
    failures: [...new Set(failures)].sort(),
    generatedCases: plannedMetamorphic.length,
    laws,
    metamorphic: metric(metamorphicPassed, plannedMetamorphic.length),
    refusalCategories: refusalCategories.size,
    schemaVersion: 2,
    variations: [...variationCounters]
      .map(([tag, cell]) => ({
        cases: cell.cases,
        passed: cell.passed,
        percent: percent(cell.passed, cell.cases),
        tag,
      }))
      .sort((left, right) => left.tag.localeCompare(right.tag)),
  };
}

function expectedCases(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
  failures: string[],
): Map<string, ExpectedCase> {
  const result = new Map<string, ExpectedCase>();
  for (const suite of manifest.suites) {
    const corpus = corpora.get(suite.id);
    if (!corpus) {
      failures.push(`${suite.id}: corpus was not loaded`);
      continue;
    }
    for (const item of corpus.cases) {
      const key = caseKey(suite.id, item.id);
      if (result.has(key)) {
        failures.push(`${displayKey(key)}: duplicate expected case`);
      }
      result.set(key, { case: item, suiteId: suite.id });
    }
  }
  for (const suiteId of corpora.keys()) {
    if (!manifest.suites.some((suite) => suite.id === suiteId)) {
      failures.push(`${suiteId}: corpus has no manifest suite`);
    }
  }
  return result;
}

function countCase(
  counters: Counters,
  item: CorpusCase,
  observation: CaseObservation,
): void {
  if (item.expectation === "established") {
    counters.positive += 1;
    counters.roleEligible += 1;
    counters.evidenceEligible += 1;
    if (recognized(item.expectation, observation)) counters.recognized += 1;
    if (observation.rolesCorrect) counters.roleCorrect += 1;
    if (observation.evidenceIntegrity) counters.evidenceValid += 1;
    return;
  }
  counters.refusal += 1;
  if (observation.targetPresent) counters.falsePositive += 1;
  else counters.refusalPreserved += 1;
}

function casePassed(item: CorpusCase, observation: CaseObservation): boolean {
  const allowed = "lawId" in item ? new Set([item.lawId]) : new Set<string>();
  const hasUnexpectedLaw = observation.establishedLawIds.some(
    (lawId) => !allowed.has(lawId),
  );
  return item.expectation === "established"
    ? recognized(item.expectation, observation) &&
        observation.rolesCorrect &&
        observation.evidenceIntegrity &&
        !hasUnexpectedLaw
    : "lawId" in item
      ? !observation.targetPresent
      : observation.establishedLawIds.length === 0;
}

function scoreDiversity(
  manifest: QualityManifest,
  corpora: ReadonlyMap<string, Corpus>,
  failures: string[],
): DiversityScore[] {
  const facets = [
    "semanticSkeleton",
    "syntaxStructure",
    "proseFamily",
    "projectTopology",
    "mutationFamily",
  ] as const;
  const scores: DiversityScore[] = [];
  for (const suite of manifest.suites) {
    const cases = corpora.get(suite.id)?.cases ?? [];
    if (suite.kind === "global-refusal" && cases.length < suite.minimumCases) {
      failures.push(
        `${suite.id}: ${cases.length} cases; requires ${suite.minimumCases}`,
      );
    }
    for (const facet of facets) {
      const counts = frequencies(cases.map((item) => item.diversity[facet]));
      const largestCell = Math.max(0, ...counts.values());
      scores.push({
        distinct: counts.size,
        facet,
        largestCell,
        largestShare: cases.length ? largestCell / cases.length : 0,
        suiteId: suite.id,
      });
      const required = suite.requiredDiversity.minimumDistinct[facet];
      if (counts.size < required) {
        failures.push(
          `${suite.id}: diversity ${facet} has ${counts.size} distinct values; requires ${required}`,
        );
      }
    }
    const profiles = cases.map((item) =>
      facets.map((facet) => item.diversity[facet]).join("\u0000"),
    );
    const counts = frequencies(profiles);
    const largestCell = Math.max(0, ...counts.values());
    const largestShare = cases.length ? largestCell / cases.length : 0;
    scores.push({
      distinct: counts.size,
      facet: "combined-profile",
      largestCell,
      largestShare,
      suiteId: suite.id,
    });
    if (largestShare > suite.requiredDiversity.maximumProfileShare) {
      failures.push(
        `${suite.id}: largest diversity profile is ${(largestShare * 100).toFixed(1)}%; maximum ${(suite.requiredDiversity.maximumProfileShare * 100).toFixed(1)}%`,
      );
    }
    if (suite.kind === "law") {
      const lawIds = new Set(cases.flatMap((item) =>
        "lawId" in item ? [item.lawId] : [],
      ));
      for (const lawId of lawIds) {
        const lawCases = cases.filter(
          (item) => "lawId" in item && item.lawId === lawId,
        );
        const skeletonProse = frequencies(lawCases.map((item) =>
          `${item.diversity.semanticSkeleton}\u0000${item.diversity.proseFamily}`,
        ));
        const largest = Math.max(0, ...skeletonProse.values());
        const share = lawCases.length ? largest / lawCases.length : 0;
        if (share > suite.requiredDiversity.maximumProfileShare) {
          failures.push(
            `${suite.id}/${lawId}: largest semantic-skeleton/prose family is ${(share * 100).toFixed(1)}%; maximum ${(suite.requiredDiversity.maximumProfileShare * 100).toFixed(1)}%`,
          );
        }
      }
    }
  }
  return scores.sort((left, right) =>
    left.suiteId.localeCompare(right.suiteId) ||
    left.facet.localeCompare(right.facet),
  );
}

function frequencies(values: readonly string[]): Map<string, number> {
  const result = new Map<string, number>();
  for (const value of values) result.set(value, (result.get(value) ?? 0) + 1);
  return result;
}

function recognized(
  expectation: CorpusExpectation,
  observation: CaseObservation,
): boolean {
  return (
    expectation === "established" &&
    observation.status === "established" &&
    observation.targetPresent
  );
}

function lawScore(key: string, counters: Counters): LawScore {
  const [suiteId, lawId] = splitKey(key);
  return {
    evidenceIntegrity: metric(counters.evidenceValid, counters.evidenceEligible),
    falsePositives: counters.falsePositive,
    lawId,
    positives: counters.positive,
    precision: metric(
      counters.recognized,
      counters.recognized + counters.falsePositive,
    ),
    recall: metric(counters.recognized, counters.positive),
    refusals: counters.refusal,
    refusalPreservation: metric(counters.refusalPreserved, counters.refusal),
    roleAccuracy: metric(counters.roleCorrect, counters.roleEligible),
    suiteId,
  };
}

function threshold(
  failures: string[],
  law: LawScore,
  key:
    | "evidenceIntegrity"
    | "precision"
    | "recall"
    | "refusalPreservation"
    | "roleAccuracy",
  minimum: number,
): void {
  const actual = law[key].percent;
  if (actual < minimum) {
    failures.push(
      `${law.suiteId}/${law.lawId}: ${key} ${actual.toFixed(1)}% is below ${minimum}%`,
    );
  }
}

function emptyCounters(): Counters {
  return {
    evidenceEligible: 0,
    evidenceValid: 0,
    falsePositive: 0,
    positive: 0,
    recognized: 0,
    refusal: 0,
    refusalPreserved: 0,
    roleCorrect: 0,
    roleEligible: 0,
  };
}

function metric(numerator: number, denominator: number): Metric {
  return { denominator, numerator, percent: percent(numerator, denominator) };
}

function percent(numerator: number, denominator: number): number {
  return denominator === 0 ? 100 : (numerator / denominator) * 100;
}

function caseKey(left: string, right: string): string {
  return `${left}\u0000${right}`;
}

function splitKey(key: string): [string, string] {
  const [left, right] = key.split("\u0000");
  if (left === undefined || right === undefined) throw new Error("invalid score key");
  return [left, right];
}

function displayKey(key: string): string {
  return splitKey(key).join("/");
}

function compareLawScores(left: LawScore, right: LawScore): number {
  return (
    left.suiteId.localeCompare(right.suiteId) || left.lawId.localeCompare(right.lawId)
  );
}
