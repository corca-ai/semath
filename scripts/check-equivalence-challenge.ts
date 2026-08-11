import { readFile } from "node:fs/promises";
import { parseEquivalenceChallenge, scoreEquivalenceChallenge } from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const challenge = parseEquivalenceChallenge(JSON.parse(await readFile(new URL("../fixtures/challenge/equivalence-v1.json", import.meta.url), "utf8")));
const results = runSemanticEvaluation(challenge.cases, "equivalence-challenge-v1");
const observations = challenge.cases.map((item, index) => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const meaning = view?.decision.status === "established" || view?.decision.status === "partial" ? view.decision.meaning : undefined;
  return {
    caseId: item.id,
    decision: view?.decision.status ?? "unsupported",
    problemCount: (view?.decision.status === "conflicting" ? view.decision.conflicts.length : 0) + (view?.diagnostics ?? []).filter((diagnostic) => diagnostic.severity === "error" || diagnostic.severity === "warning").length,
    relationId: meaning?.relationId ?? null,
  };
});
const score = scoreEquivalenceChallenge(challenge, observations);
console.log(`equivalence challenge: ${score.passed}/${score.cases}; baseline ${challenge.baseline.passed}/${challenge.baseline.total}`);
if (process.env.SEMATH_EQUIVALENCE_REPORT) await Bun.write(process.env.SEMATH_EQUIVALENCE_REPORT, `${JSON.stringify({ ...score, observations }, null, 2)}\n`);
if (score.failures.length && process.env.SEMATH_EQUIVALENCE_ALLOW_FAILURES !== "1") throw new Error(`equivalence challenge failed:\n${score.failures.join("\n")}`);
