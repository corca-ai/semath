import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  authoredHistoricalReleaseRegressions,
  parseAuthoredScientificFixture,
  type AuthoredScientificObservation,
  type AuthoredScientificScorecard,
} from "../packages/evaluation/src/index";

const fixturePath = "fixtures/challenge/document-reasoning-holdout-v1.json";
const temporary = await mkdtemp(join(tmpdir(), "semath-authored-historical-"));
const reportPath = join(temporary, "report.json");
try {
  const result = spawnSync("bun", ["scripts/check-authored-scientific.ts"], {
    env: {
      ...process.env,
      SEMATH_AUTHORED_ALLOW_FAILURES: "1",
      SEMATH_AUTHORED_FIXTURE: fixturePath,
      SEMATH_AUTHORED_REPORT: reportPath,
      SEMATH_AUTHORED_SPLIT: "holdout",
    },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error("historical evaluation failed to execute");
  const fixture = parseAuthoredScientificFixture(
    JSON.parse(await readFile(fixturePath, "utf8")),
  );
  const report = JSON.parse(await readFile(reportPath, "utf8")) as {
    readonly results: readonly {
      readonly observations: readonly AuthoredScientificObservation[];
      readonly score: AuthoredScientificScorecard;
    }[];
  };
  const resultReport = report.results[0];
  if (!resultReport || report.results.length !== 1) {
    throw new Error("historical evaluation must produce exactly one score");
  }
  const regressions = authoredHistoricalReleaseRegressions(
    fixture,
    resultReport.observations,
    resultReport.score,
    {
      approvedConservativeDecisionIds: ["CA-HO-06-probe"],
      approvedCursorBoundaryIdentityIds: ["CA-HO-05-probe"],
      approvedFalseEstablishments: [
        {
          caseId: "FMH-027-06-probe-stage-6-retracted-volume-flow",
          causes: ["decision", "excluded-relation", "proof-grounding"],
        },
      ],
      approvedFormulaDecisionDomainIds: [
        "CA-HO-06-probe-revised-prime-notation",
        "FMH-027-01-probe-water-volume-flow",
        "probability-holdout-structural-conditional-expectation-001-probe-event-set-operations",
      ],
      approvedSourceGroundedNavigationRecoveries: [
        {
          caseId: "OPTML-HO-03-probe",
          definition: {
            fileId: "specs/nonlinear_solver.md",
            needle: "\\varepsilon",
            occurrence: 0,
          },
          references: [
            {
              fileId: "specs/nonlinear_solver.md",
              needle: "\\varepsilon",
              occurrence: 0,
            },
            {
              fileId: "specs/nonlinear_solver.md",
              needle: "\\varepsilon",
              occurrence: 1,
            },
            {
              fileId: "specs/nonlinear_solver.md",
              needle: "\\varepsilon",
              occurrence: 2,
            },
          ],
          symbol: "\\varepsilon",
          symbolOccurrence: {
            fileId: "specs/nonlinear_solver.md",
            needle: "\\varepsilon",
            occurrence: 2,
          },
        },
      ],
      cases: 97,
      maximumMissedCoverage: 78,
      maximumNavigationOrIdentity: 54,
      maximumRisk: 720,
      minimumPassed: 6,
    },
  );
  if (regressions.length) {
    throw new Error(
      `historical release regression:\n${regressions.join("\n")}`,
    );
  }
  console.log(
    `historical release gate OK: ${resultReport.score.passed}/${resultReport.score.cases}; raw risk ${resultReport.score.risk.total}; reviewed legacy formula-domain adjudications 3; formula-boundary identity adjudications 1; source-grounded navigation recoveries 1; conservative proof adjudications 1`,
  );
} finally {
  await rm(temporary, { force: true, recursive: true });
}
