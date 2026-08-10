import {
  type FoundationObservation,
  scoreFoundation,
} from "../packages/evaluation/src/index";
import {
  loadFoundationFixtures,
  loadQualityFixtures,
} from "./evaluation-fixtures";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const { manifest } = await loadQualityFixtures();
const corpora = await loadFoundationFixtures(manifest);
const planned = manifest.foundationSuites.flatMap((suite) => {
  const corpus = corpora.get(suite.id);
  if (!corpus) throw new Error(`${suite.id}: foundation corpus was not loaded`);
  return corpus.cases.map((item) => ({ case: item, suite }));
});
const results = runSemanticEvaluation(
  planned.map((item) => ({
    cursor: item.case.cursor,
    documents: item.case.documents,
    id: `${item.suite.id}/${item.case.id}`,
  })),
  "foundation-corpus",
);

const observations = planned.map((item, index): FoundationObservation => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const quantities = [
    ...(view?.context.quantities ?? []),
    ...(view?.symbol?.quantities ?? []),
  ];
  const observation = {
    assumptions: (view?.context.assumptions ?? []).map((item) => ({
      kind: item.kind,
      subjects: item.subjects ?? [],
      value: item.value,
    })),
    caseId: item.case.id,
    conceptIds: [...new Set((view?.context.concepts ?? []).map((entry) => entry.conceptId))],
    diagnosticCodes: [...new Set((view?.diagnostics ?? []).map((entry) => entry.code))],
    definitions: (view?.symbol?.definitions ?? []).map((item) => ({
      description: item.description,
      evidenceRuleIds: [item.evidence.ruleId],
      symbol: item.symbol,
    })),
    dimensions: [...new Set(quantities.map((entry) => entry.dimension.display))],
    quantityKindIds: [...new Set(quantities.flatMap((entry) =>
      entry.quantityKindId ? [entry.quantityKindId] : [],
    ))],
    relationIds: [...new Set((view?.context.relations ?? []).map((entry) => entry.relationId))],
    ...(view ? { status: view.decision.status } : {}),
    suiteId: item.suite.id,
    symbols: [...new Set([
      ...quantities.map((entry) => entry.symbol),
      ...(view?.symbol ? [view.symbol.symbol] : []),
    ])],
    unitIds: [...new Set(quantities.flatMap((entry) => entry.unitId ? [entry.unitId] : []))],
  };
  if (process.env.SEMATH_FOUNDATION_DEBUG?.split(",").includes(item.case.id)) {
    console.error(JSON.stringify({ item, observation, result }, null, 2));
  }
  return observation;
});

for (const suite of manifest.foundationSuites) {
  const corpus = corpora.get(suite.id)!;
  const scorecard = scoreFoundation(
    suite,
    corpus,
    observations.filter((item) => item.suiteId === suite.id),
    new Map(manifest.dimensions.map((dimension) => [dimension.id, dimension.tags])),
  );
  console.log(
    `${suite.id}: passed=${scorecard.passed}/${scorecard.cases} dimensions=${Object.entries(scorecard.dimensions).map(([id, count]) => `${id}:${count}`).join(",")} metrics=${Object.entries(scorecard.metrics).map(([id, value]) => `${id}:${value.passed}/${value.cases}`).join(",")}`,
  );
  if (scorecard.failures.length) {
    throw new Error(`foundation quality gate failed:\n${scorecard.failures.join("\n")}`);
  }
}
