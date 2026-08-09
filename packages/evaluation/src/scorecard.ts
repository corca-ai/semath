import type {
  Corpus,
  CorpusCase,
  CorpusExpectation,
  MetamorphicTransform,
  QualityManifest,
} from "./model";
import { planMetamorphicCases } from "./metamorphic";

export interface CaseObservation {
  caseId: string;
  evidenceIntegrity: boolean;
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

export interface QualityScorecard {
  authoredCases: number;
  coverage: readonly CoverageScore[];
  failures: readonly string[];
  generatedCases: number;
  laws: readonly LawScore[];
  metamorphic: Metric;
  refusalCategories: number;
  schemaVersion: 1;
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
  for (const [key, item] of expected) {
    const observation = baseObservations.get(key);
    if (!observation) {
      failures.push(`${displayKey(key)}: missing observation`);
      continue;
    }
    const passed = casePassed(item.case, observation);
    const lawKey = caseKey(item.suiteId, item.case.lawId);
    const counters = lawCounters.get(lawKey) ?? emptyCounters();
    countCase(counters, item.case, observation);
    lawCounters.set(lawKey, counters);
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
    if (item.case.refusalCategory) refusalCategories.add(item.case.refusalCategory);
  }

  const laws = [...lawCounters]
    .map(([key, counters]) => lawScore(key, counters))
    .sort(compareLawScores);
  for (const law of laws) {
    const suite = manifest.suites.find((item) => item.id === law.suiteId);
    if (!suite) continue;
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
    authoredCases: expected.size,
    coverage,
    failures: [...new Set(failures)].sort(),
    generatedCases: plannedMetamorphic.length,
    laws,
    metamorphic: metric(metamorphicPassed, plannedMetamorphic.length),
    refusalCategories: refusalCategories.size,
    schemaVersion: 1,
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
  return item.expectation === "established"
    ? recognized(item.expectation, observation) &&
        observation.rolesCorrect &&
        observation.evidenceIntegrity
    : !observation.targetPresent;
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
