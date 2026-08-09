import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import {
  explainQualityCase,
  findCorpusDuplicates,
  observeQualityRun,
  planQualityRun,
  scoreQuality,
} from "../packages/evaluation/src/index";
import type { QueryResult } from "../packages/protocol/src/index";
import { loadQualityFixtures } from "./evaluation-fixtures";

const { corpora, manifest } = await loadQualityFixtures();
const corpusIntegrityFailures = findCorpusDuplicates([...corpora.values()]);
if (corpusIntegrityFailures.length) {
  throw new Error(`corpus integrity gate failed:\n${corpusIntegrityFailures.join("\n")}`);
}
const plan = planQualityRun(manifest, corpora);
const native = spawnSync(
  "cargo",
  ["run", "--quiet", "--locked", "-p", "semath-native"],
  {
    encoding: "utf8",
    input: JSON.stringify({ queries: plan.queries, snapshot: plan.snapshot }),
    maxBuffer: 128 * 1024 * 1024,
  },
);
if (native.status !== 0) throw new Error(native.stderr || "native corpus run failed");
const results = JSON.parse(native.stdout) as QueryResult[];
const observations = observeQualityRun(plan, results);
const scorecard = scoreQuality(manifest, corpora, observations);

for (const law of scorecard.laws) {
  console.log(
    [
      `${law.suiteId}/${law.lawId}`,
      `recall=${format(law.recall.percent)}`,
      `precision=${format(law.precision.percent)}`,
      `roles=${format(law.roleAccuracy.percent)}`,
      `evidence=${format(law.evidenceIntegrity.percent)}`,
      `refusal=${format(law.refusalPreservation.percent)}`,
      `cases=${law.positives}+${law.refusals}`,
    ].join(" "),
  );
}
console.log(
  `adversarial refusal=${format(scorecard.adversarialRefusal.percent)} cases=${scorecard.adversarialRefusal.denominator}`,
);
for (const suite of manifest.suites) {
  const dimensions = scorecard.coverage
    .filter((score) => score.suiteId === suite.id)
    .map((score) => `${score.dimension}:${score.cases}`)
    .join(",");
  console.log(`${suite.id}: dimensions=${dimensions}`);
  const diversity = scorecard.diversity
    .filter((score) => score.suiteId === suite.id)
    .map((score) =>
      score.facet === "combined-profile"
        ? `profiles:${score.distinct},max:${(score.largestShare * 100).toFixed(1)}%`
        : `${score.facet}:${score.distinct}`,
    )
    .join(",");
  console.log(`${suite.id}: diversity=${diversity}`);
}
console.log(
  `metamorphic invariance=${format(scorecard.metamorphic.percent)} cases=${scorecard.generatedCases}`,
);

if (process.env.SEMATH_CORPUS_REPORT) {
  for (const [index, item] of plan.planned.entries()) {
    const explanation = explainQualityCase(item, results[index]);
    const observation = observations[index];
    if (
      observation &&
      ((item.case.expectation === "established" && !observation.targetPresent) ||
        (item.case.expectation === "refused" && observation.targetPresent))
    ) {
      console.error(`case ${explanation.suiteId}/${explanation.caseId}: ${explanation.reason}`);
    }
  }
  for (const variation of scorecard.variations) {
    console.error(
      `variation ${variation.tag}: pass=${format(variation.percent)} cases=${variation.cases}`,
    );
  }
}
if (process.env.SEMATH_SCORECARD_PATH) {
  await mkdir(dirname(process.env.SEMATH_SCORECARD_PATH), { recursive: true });
  await writeFile(
    process.env.SEMATH_SCORECARD_PATH,
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
  console.log(`scorecard: ${process.env.SEMATH_SCORECARD_PATH}`);
}
if (scorecard.failures.length) {
  throw new Error(`corpus quality gate failed:\n${scorecard.failures.join("\n")}`);
}
console.log(
  `corpus quality OK: ${scorecard.authoredCases} authored cases, ${scorecard.generatedCases} generated cases, ${scorecard.variations.length} variation tags, ${scorecard.refusalCategories} refusal categories`,
);

function format(value: number): string {
  return `${value.toFixed(1)}%`;
}
