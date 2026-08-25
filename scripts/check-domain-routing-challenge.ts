import { readFile } from "node:fs/promises";
import {
  parseDomainRoutingChallenge,
  scoreDomainRoutingChallenge,
  selectDomainRoutingDecision,
  observeSelectedFormulaDecision,
} from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const challenge = parseDomainRoutingChallenge(JSON.parse(await readFile(new URL("../fixtures/challenge/domain-routing-v1.json", import.meta.url), "utf8")));
const results = runSemanticEvaluation(challenge.cases, "domain-routing-challenge-v1");
const observations = challenge.cases.map((item, index) => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  const formula = observeSelectedFormulaDecision({
    disposition: view?.authoringContext.disposition,
    formula: view?.authoringContext.formula,
    hypotheses: view?.authoringContext.interpretations.hypotheses ?? [],
  });
  const diagnosticProblems = (view?.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.severity === "error" || diagnostic.severity === "warning",
  ).length;
  return {
    caseId: item.id,
    decision: selectDomainRoutingDecision(
      item.decisionDomain,
      view?.decision.status ?? "unsupported",
      view?.authoringContext.disposition ?? "unsupported",
    ),
    decisionDomain: item.decisionDomain,
    domains: (view?.domains ?? []).map((domain) => ({ packId: domain.packId, support: domain.support })),
    problemCount: (item.decisionDomain === "selected-formula"
      ? formula.decision.problemCount
      : view?.decision.status === "conflicting" ? view.decision.conflicts.length : 0) + diagnosticProblems,
    recognizedRelations: item.decisionDomain === "selected-formula" ? formula.recognizedRelations : [],
    sourceGrounded: item.decisionDomain === "selected-formula" ? formula.decision.sourceGrounded : false,
  };
});
const score = scoreDomainRoutingChallenge(challenge, observations);
console.log(`domain routing challenge: ${score.passed}/${score.cases}; baseline protocol ${challenge.baseline.protocolVersion}`);
if (process.env.SEMATH_DOMAIN_REPORT) await Bun.write(process.env.SEMATH_DOMAIN_REPORT, `${JSON.stringify({ ...score, observations }, null, 2)}\n`);
if (score.failures.length && process.env.SEMATH_DOMAIN_ALLOW_FAILURES !== "1") throw new Error(`domain routing challenge failed:\n${score.failures.join("\n")}`);
