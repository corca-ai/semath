import { readFile } from "node:fs/promises";
import {
  observeSelectedFormulaDecision,
  parseEquivalenceChallenge,
  scoreEquivalenceChallenge,
  selectEquivalenceObservation,
} from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const challenge = parseEquivalenceChallenge(JSON.parse(await readFile(new URL("../fixtures/challenge/equivalence-v1.json", import.meta.url), "utf8")));
const results = runSemanticEvaluation(challenge.cases, "equivalence-challenge-v1");
const observations = challenge.cases.map((item, index) => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const meaning = view?.decision.status === "established" || view?.decision.status === "partial" ? view.decision.meaning : undefined;
  const diagnosticProblems = (view?.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.severity === "error" || diagnostic.severity === "warning",
  ).length;
  const formula = observeSelectedFormulaDecision({
    disposition: view?.authoringContext.disposition,
    formula: view?.authoringContext.formula,
    hypotheses: view?.authoringContext.interpretations.hypotheses ?? [],
  });
  return selectEquivalenceObservation(
    item.id,
    item.decisionDomain,
    {
      decision: view?.decision.status ?? "unsupported",
      problemCount:
        diagnosticProblems +
        (view?.decision.status === "conflicting" ? view.decision.conflicts.length : 0),
      relationIds: meaning?.relationId ? [meaning.relationId] : [],
    },
    {
      decision: view?.authoringContext.disposition ?? "unsupported",
      problemCount: diagnosticProblems + formula.decision.problemCount,
      relationIds: formula.recognizedRelations.map((relation) => relation.relationId),
    },
  );
});
const score = scoreEquivalenceChallenge(challenge, observations);
console.log(`equivalence challenge: ${score.passed}/${score.cases}; schema ${challenge.schemaVersion}`);
if (process.env.SEMATH_EQUIVALENCE_REPORT) await Bun.write(process.env.SEMATH_EQUIVALENCE_REPORT, `${JSON.stringify({ ...score, observations }, null, 2)}\n`);
if (score.failures.length && process.env.SEMATH_EQUIVALENCE_ALLOW_FAILURES !== "1") throw new Error(`equivalence challenge failed:\n${score.failures.join("\n")}`);
