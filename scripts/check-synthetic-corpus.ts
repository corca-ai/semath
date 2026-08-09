import { readdir, readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { builtInPacks } from "../packages/packs/src/index";
import {
  assertSyntheticFormulaResults,
  buildSyntheticFormulaFixture,
  observeSyntheticFormulaResults,
  parseSyntheticDomainCorpus,
} from "./synthetic-corpus";
import {
  assertSyntheticProseResults,
  buildSyntheticProseFixture,
  observeSyntheticProseResults,
  parseSyntheticProseCorpus,
} from "./synthetic-prose-corpus";
import {
  aggregateSemanticQuality,
  evaluateSemanticQualityBudgets,
  type SemanticQualityBudget,
} from "./semantic-quality";

const corpusRoot = new URL("../fixtures/synthetic/v1/", import.meta.url);
const names = (await readdir(corpusRoot, { withFileTypes: true }))
  .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
  .map((entry) => entry.name)
  .sort();
if (names.length === 0) throw new Error("synthetic corpus has no domain files");

const corpora = await Promise.all(
  names.map(async (name) =>
    parseSyntheticDomainCorpus(
      JSON.parse(await readFile(new URL(name, corpusRoot), "utf8")),
      name,
    ),
  ),
);
const patternIds = new Set(
  builtInPacks().flatMap((pack) => pack.patterns.map((pattern) => pattern.id)),
);
for (const corpus of corpora) {
  if (corpus.cases.length < 50) {
    throw new Error(`${corpus.domain}: requires at least 50 independent cases`);
  }
  for (const entry of corpus.cases) {
    for (const pattern of entry.expectedPatterns) {
      if (!patternIds.has(pattern)) {
        throw new Error(`${corpus.domain}/${entry.id}: unknown pattern ${pattern}`);
      }
    }
  }
}

const { expectations, fixture } = buildSyntheticFormulaFixture(corpora);
const formulaResults = runNative(fixture, "formula corpus");
const scorecards = assertSyntheticFormulaResults(
  formulaResults,
  expectations,
);
const qualityObservations = observeSyntheticFormulaResults(
  formulaResults,
  expectations,
);
for (const score of scorecards) {
  console.log(
    `synthetic corpus: ${score.domain} ${score.cases} cases, ${score.recognition} recognized surfaces, ${score.refusals} refusals, ${score.supportedCoverageTargets}/${score.coverageTargets} coverage targets supported (${score.semanticCoveragePercent}%)`,
  );
}
console.log(
  `synthetic corpus OK: ${scorecards.length} independent domains, ${expectations.length} exact queries`,
);

const proseRoot = new URL("../fixtures/synthetic/v1/prose/", import.meta.url);
const proseNames = (await readdir(proseRoot, { withFileTypes: true }))
  .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
  .map((entry) => entry.name)
  .sort();
if (proseNames.length === 0) throw new Error("synthetic prose corpus is empty");
const proseCorpora = await Promise.all(
  proseNames.map(async (name) =>
    parseSyntheticProseCorpus(
      JSON.parse(await readFile(new URL(name, proseRoot), "utf8")),
      name,
    ),
  ),
);
for (const corpus of proseCorpora) {
  if (corpus.cases.length < 50) {
    throw new Error(`${corpus.domain}: requires at least 50 prose cases`);
  }
}
const prose = buildSyntheticProseFixture(proseCorpora);
if (prose.expectations.length < 180) {
  throw new Error(
    `synthetic prose corpus requires at least 180 independent cases, got ${prose.expectations.length}`,
  );
}
const prosePurposeCounts = new Map(
  (["recognition", "refusal", "coverage"] as const).map((purpose) => [
    purpose,
    prose.expectations.filter((entry) => entry.case.purpose === purpose).length,
  ]),
);
for (const [purpose, minimum] of [
  ["recognition", 90],
  ["refusal", 45],
  ["coverage", 30],
] as const) {
  const count = prosePurposeCounts.get(purpose) ?? 0;
  if (count < minimum) {
    throw new Error(
      `synthetic prose corpus requires at least ${minimum} ${purpose} cases, got ${count}`,
    );
  }
}
const proseResults = runNative(prose.fixture, "prose corpus");
const proseScore = assertSyntheticProseResults(proseResults, prose.expectations);
qualityObservations.push(
  ...observeSyntheticProseResults(proseResults, prose.expectations),
);
console.log(
  `synthetic prose: ${proseScore.cases} cases, ${proseScore.recognition} recognized definitions, ${proseScore.refusals} refusals, ${proseScore.supportedCoverageTargets}/${proseScore.coverageTargets} coverage targets supported (${proseScore.semanticCoveragePercent}%)`,
);

const qualityBudgetFile = JSON.parse(
  await readFile(
    new URL("../fixtures/v0.15/semantic-quality-budgets.json", import.meta.url),
    "utf8",
  ),
) as { schemaVersion?: unknown; budgets?: unknown };
if (qualityBudgetFile.schemaVersion !== 1 || !Array.isArray(qualityBudgetFile.budgets)) {
  throw new Error("semantic quality budget file must use schemaVersion 1");
}
const qualityScores = aggregateSemanticQuality(qualityObservations, [
  "field",
  "domain",
  "capability",
]);
for (const score of qualityScores) {
  console.log(
    `quality: ${score.field}/${score.domain}/${score.capability} cases=${score.cases} exact=${score.caseAccuracyPercent}% precision=${score.precisionPercent}% recall=${score.recallPercent}% unexpected=${score.unexpectedItems}`,
  );
}
const topicScores = aggregateSemanticQuality(qualityObservations, [
  "field",
  "domain",
  "topic",
  "capability",
]);
const imperfectTopics = topicScores.filter(
  (score) =>
    score.caseAccuracyPercent < 100 ||
    score.precisionPercent < 100 ||
    score.recallPercent < 100 ||
    score.unexpectedItems > 0,
);
console.log(
  `quality topics: ${topicScores.length} field/domain/topic/capability cells, ${imperfectTopics.length} regressions`,
);
const budgetResults = evaluateSemanticQualityBudgets(
  qualityObservations,
  (qualityBudgetFile.budgets as SemanticQualityBudget[]).filter(
    (budget) => budget.selector.field !== "product",
  ),
);
const violations = budgetResults.flatMap((result) =>
  result.violations.map((violation) => `${result.budgetId}: ${violation}`),
);
if (violations.length > 0) {
  throw new Error(`semantic quality budget regression:\n${violations.join("\n")}`);
}
console.log(`semantic quality budgets OK: ${budgetResults.length} versioned gates`);

function runNative(fixture: unknown, label: string) {
  const native = spawnSync(
    "cargo",
    ["run", "--quiet", "--locked", "-p", "semath-native"],
    {
      encoding: "utf8",
      input: JSON.stringify(fixture),
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (native.status !== 0) {
    throw new Error(native.stderr || `synthetic ${label} native run failed`);
  }
  return JSON.parse(native.stdout);
}
