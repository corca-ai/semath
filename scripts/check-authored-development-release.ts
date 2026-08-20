import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  authoredReleaseResultRegressions,
  authoredScenarioReviewPayload,
  parseAuthoredScientificFixture,
  parseAuthoredReleaseReport,
} from "../packages/evaluation/src/index";

const temporary = await mkdtemp(join(tmpdir(), "semath-authored-development-"));
const reportPath = join(temporary, "report.json");
try {
  const fixture = parseAuthoredScientificFixture(
    JSON.parse(
      await readFile(
        "fixtures/challenge/document-reasoning-development-v1.json",
        "utf8",
      ),
    ),
  );
  for (const scenario of fixture.scenarios) {
    const digest = createHash("sha256")
      .update(authoredScenarioReviewPayload(fixture, scenario.id))
      .digest("hex");
    if (scenario.review.finalDigest !== digest) {
      throw new Error(`${scenario.id}: final development review digest is stale`);
    }
  }
  const result = spawnSync("bun", ["scripts/check-authored-scientific.ts"], {
    env: {
      ...process.env,
      SEMATH_AUTHORED_ALLOW_FAILURES: "1",
      SEMATH_AUTHORED_FIXTURE:
        "fixtures/challenge/document-reasoning-development-v1.json",
      SEMATH_AUTHORED_REPORT: reportPath,
      SEMATH_AUTHORED_SPLIT: "development",
    },
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error("development evaluation failed to execute");
  const authoring = spawnSync(
    "bun",
    ["scripts/check-math-authoring-development.ts"],
    {
      env: { ...process.env, SEMATH_AUTHORED_SKIP_BUILD: "1" },
      stdio: "inherit",
    },
  );
  if (authoring.status !== 0) {
    throw new Error("exact math authoring development gate failed");
  }
  const report = parseAuthoredReleaseReport(
    JSON.parse(await readFile(reportPath, "utf8")),
  );
  const regressions = authoredReleaseResultRegressions(report, {
    cases: 166,
    maximumRisk: 116,
    minimumPassed: 108,
  });
  if (regressions.length) {
    throw new Error(`development release regression:\n${regressions.join("\n")}`);
  }
  const { score } = report;
  console.log(
    `development release gate OK: ${score.passed}/${score.cases}; risk ${score.risk.total}`,
  );
} finally {
  await rm(temporary, { force: true, recursive: true });
}
