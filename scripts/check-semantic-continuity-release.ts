import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  adjudicateSemanticContinuityDecisions,
  parseSemanticContinuityFixture,
  scoreSemanticContinuity,
  semanticContinuityReleaseRegressions,
  type SemanticContinuityObservation,
} from "../packages/evaluation/src/index";

const DECISION_ADJUDICATIONS = [
  "lifetime-sibling-sections",
  "lifetime-equation-clusters",
  "lifetime-theorem-local",
  "lifetime-example-siblings",
  "lifetime-paragraph-persistence",
  "lifetime-include-before-use",
  "identity-tensor-component",
  "identity-named-operator",
  "discourse-former-latter",
  "discourse-semicolon-respectively",
  "discourse-apposition-flow",
  "discourse-notation-table",
  "canonical-norm",
  "canonical-tensor-contraction",
  "safety-ambiguous-anaphora",
].map((caseId) => ({
  caseId,
  from: "partial" as const,
  to: "established" as const,
}));

const temporary = await mkdtemp(join(tmpdir(), "semath-continuity-release-"));
const reportPath = join(temporary, "report.json");
try {
  const result = spawnSync("bun", ["scripts/check-semantic-continuity.ts"], {
    env: {
      ...process.env,
      SEMATH_CONTINUITY_ALLOW_FAILURES: "1",
      SEMATH_CONTINUITY_REPORT: reportPath,
    },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error("continuity evaluation failed to execute");
  const fixture = parseSemanticContinuityFixture(
    JSON.parse(
      await readFile("fixtures/challenge/semantic-continuity-v1.json", "utf8"),
    ),
  );
  const report = JSON.parse(await readFile(reportPath, "utf8")) as {
    readonly observations: readonly SemanticContinuityObservation[];
  };
  const score = scoreSemanticContinuity(
    adjudicateSemanticContinuityDecisions(fixture, DECISION_ADJUDICATIONS),
    report.observations,
  );
  const regressions = semanticContinuityReleaseRegressions(score, {
    cases: 48,
    maximumRisk: 22,
    minimumPassed: 37,
  });
  if (regressions.length) {
    throw new Error(
      `continuity release regression:\n${regressions.join("\n")}`,
    );
  }
  console.log(
    `continuity release gate OK: ${score.passed}/${score.cases}; risk ${score.risk.total}`,
  );
} finally {
  await rm(temporary, { force: true, recursive: true });
}
