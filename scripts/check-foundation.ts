import { readFile, readdir } from "node:fs/promises";
import {
  type FoundationObservation,
  parseFoundationCorpus,
  scoreFoundation,
} from "./testing/foundation";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const directory = new URL("../fixtures/foundation/", import.meta.url);
const paths = (await readdir(directory)).filter((path) => path.endsWith(".json")).sort();
if (!paths.length) throw new Error("foundation fixtures are missing");
const corpora = await Promise.all(paths.map(async (path) =>
  parseFoundationCorpus(JSON.parse(await readFile(new URL(path, directory), "utf8"))),
));
const planned = corpora.flatMap((corpus) =>
  corpus.cases.map((item) => ({ case: item, suiteId: corpus.domain })),
);
const results = runSemanticEvaluation(
  planned.map((item) => ({
    cursor: item.case.cursor,
    documents: item.case.documents,
    id: `${item.suiteId}/${item.case.id}`,
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
    relationIds: [
      ...new Set(
        (view?.formulaAnalysis.interpretations.hypotheses ?? []).flatMap((hypothesis) => {
          const formula = view?.formulaAnalysis.formula;
          const anchor = hypothesis.formula;
          return hypothesis.kind === "typed-law" && hypothesis.support !== "contradicted" &&
            hypothesis.relation && formula && anchor &&
            anchor.location.fileId === formula.location.fileId &&
            anchor.documentVersion === formula.documentVersion &&
            anchor.location.range.startOffset === formula.location.range.startOffset &&
            anchor.location.range.endOffset === formula.location.range.endOffset
            ? [hypothesis.relation.relationId] : [];
        }),
      ),
    ],
    ...(view ? { status: view.formulaAnalysis.disposition } : {}),
    suiteId: item.suiteId,
    symbols: [...new Set([
      ...quantities.map((entry) => entry.symbol),
      ...(view?.symbol ? [view.symbol.symbol] : []),
      ...(view?.symbol?.definitions ?? []).map((entry) => entry.symbol),
    ])],
    unitIds: [...new Set(quantities.flatMap((entry) => entry.unitId ? [entry.unitId] : []))],
  };
  if (process.env.SEMATH_FOUNDATION_DEBUG?.split(",").includes(item.case.id)) {
    console.error(JSON.stringify({ item, observation, result }, null, 2));
  }
  return observation;
});

for (const corpus of corpora) {
  const result = scoreFoundation(corpus.domain, corpus, observations.filter((item) => item.suiteId === corpus.domain));
  console.log(`${corpus.domain}: passed=${result.passed}/${result.cases}`);
  if (result.failures.length) throw new Error(`foundation regression failed:\n${result.failures.join("\n")}`);
}
