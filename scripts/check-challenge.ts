import { readFile } from "node:fs/promises";
import {
  parseChallengeCorpus,
  parseChallengeV3,
  scoreChallenge,
  type ChallengeObservation,
} from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const base = JSON.parse(
  await readFile(
    new URL("../fixtures/challenge/recognition-v2.json", import.meta.url),
    "utf8",
  ),
);
const corpus = parseChallengeV3(
  base,
  JSON.parse(
    await readFile(
      new URL("../fixtures/challenge/recognition-v3.json", import.meta.url),
      "utf8",
    ),
  ),
);
const results = runSemanticEvaluation(corpus.cases, "recognition-challenge-v3");
const debugIds = new Set(process.env.SEMATH_CHALLENGE_DEBUG?.split(",") ?? []);
const observations = corpus.cases.map((item, index): ChallengeObservation => {
  const result = results[index];
  const view =
    result?.value.kind === "semanticView" ? result.value.view : undefined;
  const known =
    view?.decision.status === "established" ||
    view?.decision.status === "partial"
      ? view.decision.meaning
      : undefined;
  const reasons = view?.decision.reasons ?? [];
  const groundingReasons = reasons.filter(
    (reason) => reason.kind === "proof" || reason.kind === "source-conflict",
  );
  const meaningRelation = known?.relationId
    ? view?.context.relations.find(
        (relation) => relation.relationId === known.relationId,
      )
    : undefined;
  const observation: ChallengeObservation = {
    assumptionValues: (view?.context.assumptions ?? []).map(
      (entry) => entry.value,
    ),
    candidates: (view?.context.candidates ?? []).map((entry) => ({
      family: entry.family,
      interpretation: entry.interpretation,
    })),
    caseId: item.id,
    conceptIds: [
      ...new Set(
        (view?.context.concepts ?? []).map((entry) => entry.conceptId),
      ),
    ],
    definitions: (view?.symbol?.definitions ?? []).map((entry) => ({
      description: entry.description,
      ruleId: entry.evidence.ruleId,
      symbol: entry.symbol,
    })),
    ...(known?.label ? { meaningLabel: known.label } : {}),
    ...(known ? { meaningRelationId: known.relationId } : {}),
    problemCount:
      (view?.decision.status === "conflicting"
        ? view.decision.conflicts.length
        : 0) +
      (view?.diagnostics ?? []).filter(
        (diagnostic) =>
          diagnostic.severity === "error" || diagnostic.severity === "warning",
      ).length,
    reasonKinds: reasons.map((reason) => reason.kind),
    relationIds: [
      ...new Set(
        (view?.context.relations ?? []).map((entry) => entry.relationId),
      ),
    ],
    shapes: [
      ...new Set((view?.symbol?.shapes ?? []).map((entry) => entry.display)),
    ],
    ...(view?.symbol?.sourceNotation
      ? { sourceNotation: view.symbol.sourceNotation }
      : {}),
    sourceGrounded:
      (groundingReasons.length > 0 &&
        groundingReasons.every((reason) =>
          reason.evidence.some(
            (evidence) => evidence.sourceRanges.length > 0,
          ),
        )) ||
      Boolean(
        meaningRelation?.evidence.some(
          (evidence) => evidence.sourceRanges.length > 0,
        ),
      ),
    ...(view ? { status: view.decision.status } : {}),
    symbols: [...new Set(view?.symbol ? [view.symbol.symbol] : [])],
  };
  if (debugIds.has(item.id))
    console.error(JSON.stringify({ item, observation, view }, null, 2));
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
      .join(",")} ` +
    `decisions=${Object.entries(scorecard.decisions)
      .map(([key, value]) => `${key}:${value.passed}/${value.total}`)
      .join(",")} ` +
    `problems=${Object.entries(scorecard.problemPolicy)
      .map(([key, value]) => `${key}:${value.passed}/${value.total}`)
      .join(",")}`,
);
if (process.env.SEMATH_CHALLENGE_REPORT) {
  await Bun.write(
    process.env.SEMATH_CHALLENGE_REPORT,
    `${JSON.stringify(scorecard, null, 2)}\n`,
  );
}
if (
  scorecard.failures.length &&
  process.env.SEMATH_CHALLENGE_ALLOW_FAILURES !== "1"
) {
  throw new Error(
    `recognition challenge failed:\n${scorecard.failures.join("\n")}`,
  );
}
