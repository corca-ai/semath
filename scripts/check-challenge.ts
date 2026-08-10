import { readFile } from "node:fs/promises";
import {
  parseChallengeCorpus,
  scoreChallenge,
  type ChallengeObservation,
} from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const corpus = parseChallengeCorpus(
  JSON.parse(
    await readFile(
      new URL("../fixtures/challenge/recognition-v2.json", import.meta.url),
      "utf8",
    ),
  ),
);
const results = runSemanticEvaluation(corpus.cases, "recognition-challenge-v2");
const debugIds = new Set(process.env.SEMATH_CHALLENGE_DEBUG?.split(",") ?? []);
const observations = corpus.cases.map((item, index): ChallengeObservation => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const observation: ChallengeObservation = {
    assumptionValues: (view?.context.assumptions ?? []).map((entry) => entry.value),
    candidates: (view?.context.candidates ?? []).map((entry) => ({
      family: entry.family,
      interpretation: entry.interpretation,
    })),
    caseId: item.id,
    conceptIds: [...new Set((view?.context.concepts ?? []).map((entry) => entry.conceptId))],
    definitions: (view?.symbol?.definitions ?? []).map((entry) => ({
      description: entry.description,
      ruleId: entry.evidence.ruleId,
      symbol: entry.symbol,
    })),
    relationIds: [...new Set((view?.context.relations ?? []).map((entry) => entry.relationId))],
    shapes: [...new Set((view?.symbol?.shapes ?? []).map((entry) => entry.display))],
    ...(view?.symbol?.sourceNotation
      ? { sourceNotation: view.symbol.sourceNotation }
      : {}),
    ...(view ? { status: view.status } : {}),
    symbols: [...new Set(view?.symbol ? [view.symbol.symbol] : [])],
  };
  if (debugIds.has(item.id)) console.error(JSON.stringify({ item, observation, view }, null, 2));
  return observation;
});
const scorecard = scoreChallenge(corpus, observations);
console.log(
  `recognition challenge: passed=${scorecard.passed}/${scorecard.cases} ` +
    `layers=${Object.entries(scorecard.layers)
      .map(([key, value]) => `${key}:${value.passed}/${value.total}`)
      .join(",")} ` +
    `metrics=${Object.entries(scorecard.metrics)
      .map(([key, value]) => `${key}:${value.passed}/${value.total}`)
      .join(",")}`,
);
if (process.env.SEMATH_CHALLENGE_REPORT) {
  await Bun.write(
    process.env.SEMATH_CHALLENGE_REPORT,
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
}
if (scorecard.failures.length && process.env.SEMATH_CHALLENGE_ALLOW_FAILURES !== "1") {
  throw new Error(`recognition challenge failed:\n${scorecard.failures.join("\n")}`);
}
