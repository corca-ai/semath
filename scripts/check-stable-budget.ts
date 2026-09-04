import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  medianSample,
  isReleasePerformanceHost,
  retainedRssBudgetBytes,
  timingBudget,
} from "./performance-budget-policy";

if (!isReleasePerformanceHost()) {
  throw new Error("stable release performance requires x86_64 Linux; use bun run budget for host diagnostics");
}

const SAMPLE_COUNT = 5;
const configurations = [
  { documentCount: 60, deltaRuns: 30 },
  // With only ten edits, the nearest-rank p95 is the single maximum sample.
  // Thirty keeps the release p95 definition consistent at both project sizes.
  { documentCount: 500, deltaRuns: 30 },
] as const;
const temporaryDirectory = await mkdtemp(join(tmpdir(), "semath-stable-budget-"));

try {
  for (const configuration of configurations) {
    const reports = [];
    for (let sample = 0; sample < SAMPLE_COUNT; sample += 1) {
      const reportPath = join(
        temporaryDirectory,
        `documents-${configuration.documentCount}-sample-${sample + 1}.json`,
      );
      runSample(configuration.documentCount, configuration.deltaRuns, reportPath);
      reports.push(JSON.parse(await readFile(reportPath, "utf8")) as BudgetReport);
    }
    const retainedSamples = reports.map((report) => report.retainedRssGrowthBytes);
    const retainedMedian = medianSample(retainedSamples);
    const retainedBudget = retainedRssBudgetBytes(configuration.documentCount);
    const timing = timingBudget(configuration.documentCount, true);
    const coldSamples = reports.map((report) => report.coldMs);
    const deltaSamples = reports.map((report) => report.deltaP95Ms);
    const semanticDeltaSamples = reports.map((report) => report.semanticDeltaMs);
    const querySamples = reports.map((report) =>
      Math.max(...Object.values(report.queryP95ByKind)),
    );
    const summary = {
      coldBudgetMs: timing.coldMs,
      coldMedianMs: medianSample(coldSamples),
      coldSamplesMs: coldSamples,
      deltaP95BudgetMs: timing.deltaP95Ms,
      deltaP95MedianMs: medianSample(deltaSamples),
      deltaP95SamplesMs: deltaSamples,
      documentCount: configuration.documentCount + 1,
      peakRssMedianBytes: medianSample(reports.map((report) => report.peakRssGrowthBytes)),
      postDisposeRssMedianBytes: medianSample(
        reports.map((report) => report.postDisposeRssGrowthBytes),
      ),
      queryP95BudgetMs: timing.queryP95Ms,
      queryP95MedianMs: medianSample(querySamples),
      queryP95SamplesMs: querySamples,
      retainedRssBudgetBytes: retainedBudget,
      retainedRssMedianBytes: retainedMedian,
      retainedRssSamplesBytes: retainedSamples,
      semanticDeltaBudgetMs: timing.semanticDeltaMs,
      semanticDeltaMedianMs: medianSample(semanticDeltaSamples),
      semanticDeltaSamplesMs: semanticDeltaSamples,
    };
    console.log(`stable budget aggregate: ${JSON.stringify(summary)}`);
    if (retainedMedian > retainedBudget) {
      throw new Error(
        `stable budget retained RSS median ${retainedMedian}B exceeded ${retainedBudget}B`,
      );
    }
    assertWithin("cold start", summary.coldMedianMs, timing.coldMs);
    assertWithin("delta p95", summary.deltaP95MedianMs, timing.deltaP95Ms);
    assertWithin("semantic delta", summary.semanticDeltaMedianMs, timing.semanticDeltaMs);
    assertWithin("query p95", summary.queryP95MedianMs, timing.queryP95Ms);
  }
  console.log("stable budget OK");
} finally {
  await rm(temporaryDirectory, { force: true, recursive: true });
}

interface BudgetReport {
  readonly coldMs: number;
  readonly deltaP95Ms: number;
  readonly peakRssGrowthBytes: number;
  readonly postDisposeRssGrowthBytes: number;
  readonly queryP95ByKind: Readonly<Record<string, number>>;
  readonly retainedRssGrowthBytes: number;
  readonly semanticDeltaMs: number;
}

function assertWithin(label: string, measured: number, budget: number): void {
  if (measured > budget) {
    throw new Error(`stable budget ${label} median ${measured.toFixed(2)}ms exceeded ${budget}ms`);
  }
}

function runSample(documentCount: number, deltaRuns: number, reportPath: string): void {
  const environment = Object.fromEntries(
    Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined),
  );
  const result = Bun.spawnSync({
    cmd: [process.execPath, "scripts/check-budget.ts"],
    env: {
      ...environment,
      SEMATH_BUDGET_DELTA_RUNS: String(deltaRuns),
      SEMATH_BUDGET_DOCUMENTS: String(documentCount),
      SEMATH_BUDGET_REPORT: reportPath,
      SEMATH_BUDGET_RSS_GATE: "0",
      SEMATH_BUDGET_STABLE: "1",
      SEMATH_BUDGET_TIMING_GATE: "0",
    },
    stderr: "pipe",
    stdout: "pipe",
  });
  process.stdout.write(result.stdout);
  process.stderr.write(result.stderr);
  if (result.exitCode !== 0) {
    throw new Error(`stable budget sample failed with exit code ${result.exitCode}`);
  }
}
