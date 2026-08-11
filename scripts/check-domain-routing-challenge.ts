import { readFile } from "node:fs/promises";
import { parseDomainRoutingChallenge, scoreDomainRoutingChallenge } from "../packages/evaluation/src/index";
import { runSemanticEvaluation } from "./semantic-evaluation-runner";

const challenge = parseDomainRoutingChallenge(JSON.parse(await readFile(new URL("../fixtures/challenge/domain-routing-v1.json", import.meta.url), "utf8")));
const results = runSemanticEvaluation(challenge.cases, "domain-routing-challenge-v1");
const observations = challenge.cases.map((item, index) => {
  const result = results[index];
  const view = result?.value.kind === "semanticView" ? result.value.view : undefined;
  return {
    caseId: item.id,
    decision: view?.decision.status ?? "unsupported",
    domains: (view?.domains ?? []).map((domain) => ({ packId: domain.packId, support: domain.support })),
    problemCount: (view?.decision.status === "conflicting" ? view.decision.conflicts.length : 0) + (view?.diagnostics ?? []).filter((diagnostic) => diagnostic.severity === "error" || diagnostic.severity === "warning").length,
  };
});
const score = scoreDomainRoutingChallenge(challenge, observations);
console.log(`domain routing challenge: ${score.passed}/${score.cases}; baseline protocol ${challenge.baseline.protocolVersion}`);
if (process.env.SEMATH_DOMAIN_REPORT) await Bun.write(process.env.SEMATH_DOMAIN_REPORT, `${JSON.stringify({ ...score, observations }, null, 2)}\n`);
if (score.failures.length && process.env.SEMATH_DOMAIN_ALLOW_FAILURES !== "1") throw new Error(`domain routing challenge failed:\n${score.failures.join("\n")}`);
