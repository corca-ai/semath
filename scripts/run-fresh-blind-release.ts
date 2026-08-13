import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import {
  freshBlindSafetyGateFailed,
  freshBlindSafetySummary,
} from "../packages/evaluation/src/fresh-blind-release";
import {
  type AuthoredScientificObservation,
  type AuthoredScientificScorecard,
} from "../packages/evaluation/src/index";
import { loadFreshBlindEvidence, sha256 } from "./fresh-blind-evidence";
import {
  finalizeFreshBlindReceipt,
  reserveFreshBlindReceipt,
  type FreshBlindReleaseReceipt,
} from "./fresh-blind-receipt";

const fixturePath = requiredPath("SEMATH_FRESH_BLIND_FIXTURE");
const receiptPath = requiredPath("SEMATH_FRESH_BLIND_RECEIPT");

// Everything before this boundary is validation or compilation. The semantic
// engine has not seen the fresh fixture yet.
const evidence = await loadFreshBlindEvidence(fixturePath);
run("cargo", ["build", "--quiet", "--locked", "-p", "semath-native"]);
const provenance = {
  nativeSha256: sha256(await readFile("target/debug/semath-native")),
  semathCommit: output("git", ["rev-parse", "HEAD"]),
  wasmSha256: sha256(await readFile("lib/wasm/semath_wasm_bg.wasm")),
  wasmtexCommit: pinnedWasmtexCommit(
    JSON.parse(await readFile("package.json", "utf8")) as PackageManifest,
  ),
};
const started: FreshBlindReleaseReceipt = {
  fixture: {
    id: evidence.release.release.id,
    seal: evidence.release.release.seal,
  },
  provenance,
  schemaVersion: 1,
  startedAt: new Date().toISOString(),
  status: "started",
};
const temporary = await mkdtemp(join(tmpdir(), "semath-fresh-blind-release-"));
const evaluationPath = join(temporary, "evaluation.json");
const lifecyclePath = join(temporary, "lifecycle.json");
let receiptReserved = false;
let terminalReceiptWritten = false;
let completedEvaluation:
  | {
      readonly bytes: Uint8Array;
      readonly result: EvaluationReport["results"][number];
      readonly safety: ReturnType<typeof freshBlindSafetySummary>;
    }
  | undefined;
try {
  await reserveFreshBlindReceipt(receiptPath, started);
  receiptReserved = true;
  run("bun", ["scripts/check-authored-scientific.ts"], {
    SEMATH_AUTHORED_ALLOW_FAILURES: "1",
    SEMATH_AUTHORED_FIXTURE: evidence.path,
    SEMATH_AUTHORED_REPORT: evaluationPath,
    SEMATH_AUTHORED_SPLIT: "holdout",
    SEMATH_AUTHORED_SKIP_BUILD: "1",
  });
  const evaluationBytes = await readFile(evaluationPath);
  const evaluation = JSON.parse(evaluationBytes.toString()) as EvaluationReport;
  const result = evaluation.results[0];
  if (!result || evaluation.results.length !== 1) {
    throw new Error("fresh blind evaluation must produce exactly one result");
  }
  completedEvaluation = {
    bytes: evaluationBytes,
    result,
    safety: freshBlindSafetySummary(evidence.release.fixture, result.observations),
  };
  run("bun", ["scripts/check-fresh-blind-lifecycle.ts"], {
    SEMATH_FRESH_BLIND_FIXTURE: evidence.path,
    SEMATH_FRESH_BLIND_LIFECYCLE_REPORT: lifecyclePath,
  });
  const lifecycleBytes = await readFile(lifecyclePath);
  if (!completedEvaluation) {
    throw new Error("fresh blind evaluation evidence is unavailable");
  }
  const {
    bytes: finalEvaluationBytes,
    result: finalResult,
    safety,
  } = completedEvaluation;
  const safetyFailed = freshBlindSafetyGateFailed(safety);
  const completed: FreshBlindReleaseReceipt = {
    ...started,
    artifacts: {
      evaluationSha256: sha256(finalEvaluationBytes),
      lifecycleSha256: sha256(lifecycleBytes),
    },
    completedAt: new Date().toISOString(),
    result: {
      firstLossAtlas: finalResult.firstLossAtlas,
      lifecycle: JSON.parse(lifecycleBytes.toString()) as unknown,
      score: {
        cases: finalResult.score.cases,
        passed: finalResult.score.passed,
        risk: finalResult.score.risk,
      },
      safety,
      validation: evidence.summary,
    },
    status: safetyFailed ? "safety-failed" : "completed",
  };
  await finalizeFreshBlindReceipt(receiptPath, completed);
  terminalReceiptWritten = true;
  if (safetyFailed) {
    throw new Error(
      "fresh blind safety gate failed; coverage misses remain visible in the receipt",
    );
  }
  console.log(
    `fresh blind release recorded: ${finalResult.score.passed}/${finalResult.score.cases}; ` +
      `receipt ${receiptPath}`,
  );
} catch (error) {
  if (receiptReserved && !terminalReceiptWritten) {
    const failed: FreshBlindReleaseReceipt = {
      ...started,
      ...(completedEvaluation
        ? {
            artifacts: {
              evaluationSha256: sha256(completedEvaluation.bytes),
            },
            result: {
              firstLossAtlas: completedEvaluation.result.firstLossAtlas,
              lifecycle: null,
              score: {
                cases: completedEvaluation.result.score.cases,
                passed: completedEvaluation.result.score.passed,
                risk: completedEvaluation.result.score.risk,
              },
              safety: completedEvaluation.safety,
              validation: evidence.summary,
            },
          }
        : {}),
      completedAt: new Date().toISOString(),
      error: error instanceof Error ? error.message : String(error),
      status: "execution-error",
    };
    await finalizeFreshBlindReceipt(receiptPath, failed);
  }
  throw error;
} finally {
  await rm(temporary, { recursive: true });
}

interface EvaluationReport {
  readonly results: readonly {
    readonly firstLossAtlas: unknown;
    readonly observations: readonly AuthoredScientificObservation[];
    readonly score: AuthoredScientificScorecard;
  }[];
}

interface PackageManifest {
  readonly dependencies?: Readonly<Record<string, string>>;
}

function requiredPath(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return isAbsolute(value) ? value : resolve(process.cwd(), value);
}

function pinnedWasmtexCommit(manifest: PackageManifest): string {
  const dependency = manifest.dependencies?.wasmtex;
  const commit = dependency?.match(/#([0-9a-f]{40})$/u)?.[1];
  if (!commit) throw new Error("package.json must pin wasmtex to a full commit");
  return commit;
}

function output(command: string, args: readonly string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(result.stderr || `${command} failed`);
  }
  return result.stdout.trim();
}

function run(
  command: string,
  args: readonly string[],
  environment: Readonly<Record<string, string>> = {},
): void {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    env: { ...process.env, ...environment },
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed`);
}
