import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import {
  parseAuthoredScientificFixture,
  parseStemBreadthManifest,
  scoreStemBreadth,
  validateStemBreadthBenchmark,
  type AuthoredFirstLossRecord,
} from "../packages/evaluation/src/index";

const manifest = parseStemBreadthManifest(
  JSON.parse(
    await readFile("fixtures/development/stem-breadth-v1.json", "utf8"),
  ),
);
const fixture = parseAuthoredScientificFixture(
  JSON.parse(
    await readFile(manifest.sourcePolicy.developmentFixturePath, "utf8"),
  ),
);
const validation = validateStemBreadthBenchmark(manifest, fixture);

console.log(
  `STEM breadth fixture OK: ${validation.measuredCells}/50 measured cells, ` +
    `${validation.commissionedGaps} commissioned gaps, ` +
    `${validation.referencedProbes} reviewed probes`,
);

const output = process.env.SEMATH_STEM_BREADTH_REPORT;
if (output) {
  const temporary = await mkdtemp(join(tmpdir(), "semath-stem-breadth-"));
  const authoredReport = join(temporary, "authored.json");
  try {
    const result = spawnSync("bun", ["scripts/check-authored-scientific.ts"], {
      env: {
        ...process.env,
        SEMATH_AUTHORED_ALLOW_FAILURES: "1",
        SEMATH_AUTHORED_FIXTURE:
          manifest.sourcePolicy.developmentFixturePath,
        SEMATH_AUTHORED_REPORT: authoredReport,
        SEMATH_AUTHORED_SPLIT: "development",
      },
      stdio: "inherit",
    });
    if (result.status !== 0) {
      throw new Error("STEM breadth evaluation failed to execute");
    }
    const report = JSON.parse(await readFile(authoredReport, "utf8")) as {
      readonly results: readonly {
        readonly firstLoss: readonly AuthoredFirstLossRecord[];
      }[];
    };
    const firstLoss = report.results[0]?.firstLoss;
    if (!firstLoss || report.results.length !== 1) {
      throw new Error(
        "STEM breadth evaluation requires one development observation set",
      );
    }
    const score = scoreStemBreadth(manifest, firstLoss);
    await mkdir(dirname(output), { recursive: true });
    await writeFile(
      output,
      `${JSON.stringify({ baseline: manifest.baseline, score, validation }, null, 2)}\n`,
    );
    console.log(
      `STEM breadth baseline: ${score.uniqueProbes.passed}/` +
        `${score.uniqueProbes.cases} unique probes; report ${output}`,
    );
    for (const [field, count] of Object.entries(score.fields)) {
      console.log(`  field ${field}: ${count.passed}/${count.cases}`);
    }
    for (const [capability, count] of Object.entries(score.capabilities)) {
      console.log(
        `  capability ${capability}: ${count.passed}/${count.cases}`,
      );
    }
  } finally {
    await rm(temporary, { force: true, recursive: true });
  }
}
