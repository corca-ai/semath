import { readFile } from "node:fs/promises";
import {
  observeSelectedFormulaDecision,
  parseChallengeV4,
  scoreChallenge,
  type ChallengeDecisionObservation,
  type ChallengeObservation,
} from "../packages/evaluation/src/index";
import type { SemanticViewInfo } from "../packages/protocol/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const base = JSON.parse(
  await readFile(
    new URL("../fixtures/challenge/recognition-v2.json", import.meta.url),
    "utf8",
  ),
);
const v3 = JSON.parse(
  await readFile(
    new URL("../fixtures/challenge/recognition-v3.json", import.meta.url),
    "utf8",
  ),
);
const corpus = parseChallengeV4(
  base,
  v3,
  JSON.parse(
    await readFile(
      new URL("../fixtures/challenge/recognition-v4.json", import.meta.url),
      "utf8",
    ),
  ),
);
const results = runSemanticEvaluation(corpus.cases, "recognition-challenge-v4");
const debugIds = new Set(process.env.SEMATH_CHALLENGE_DEBUG?.split(",") ?? []);
const observations = corpus.cases.map((item, index): ChallengeObservation => {
  const result = results[index];
  const view =
    result?.value.kind === "semanticView" ? result.value.view : undefined;
  const entityDecision = observeEntityDecision(view);
  const formula = observeSelectedFormulaDecision({
    authoritativeRelationIds: new Set(
      view?.context.relations.map((relation) => relation.relationId) ?? [],
    ),
    disposition: view?.authoringContext.disposition,
    formula: view?.authoringContext.formula,
    hypotheses: view?.authoringContext.interpretations.hypotheses ?? [],
  });
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
    entityDecision,
    formulaDecision: formula.decision,
    ...(entityDecision.meaningLabel
      ? { meaningLabel: entityDecision.meaningLabel }
      : {}),
    ...(entityDecision.meaningRelationId !== undefined
      ? { meaningRelationId: entityDecision.meaningRelationId }
      : {}),
    problemCount: entityDecision.problemCount,
    reasonKinds: entityDecision.reasonKinds,
    recognizedRelations: formula.recognizedRelations,
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
    sourceGrounded: entityDecision.sourceGrounded,
    ...(entityDecision.status ? { status: entityDecision.status } : {}),
    symbols: [...new Set(view?.symbol ? [view.symbol.symbol] : [])],
  };
  if (debugIds.has(item.id))
    console.error(JSON.stringify({ item, observation, view }, null, 2));
  return observation;
});

function observeEntityDecision(
  view: SemanticViewInfo | undefined,
): ChallengeDecisionObservation {
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
  return {
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
  };
}

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
