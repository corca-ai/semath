import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import {
  assertFreshBlindLinuxX64,
  assertFreshBlindWorkflowBoundary,
  freshBlindWorkflowBoundaryFromEnvironment,
} from "./fresh-blind-workflow-boundary";

export const SEMANTIC_RELEASE_STEPS = [
  "check",
  "quality",
  "authored-development",
  "math-authoring-public",
  "docs",
  "wasm-build",
  "wasm-checksum",
  "wasm-committed",
  "worktree-clean",
  "package-smoke",
  "continuity",
  "authored-historical",
  "fresh-static-validation",
  "identity-recheck",
  "native-build",
  "retained-package",
  "preblind-manifest",
] as const;

export const SEMANTIC_RELEASE_SPEND_STEPS = [
  "global-reservation",
  "reservation-identity",
  "fresh-engine",
] as const;

export function assertSemanticReleaseStepPlan(
  preflight: readonly string[] = SEMANTIC_RELEASE_STEPS,
  spend: readonly string[] = SEMANTIC_RELEASE_SPEND_STEPS,
): void {
  const fresh = spend.indexOf("fresh-engine");
  if (fresh !== spend.length - 1) {
    throw new Error("fresh engine must be the final semantic release step");
  }
  if (
    spend.indexOf("global-reservation") !== 0 ||
    spend.indexOf("reservation-identity") !== 1
  ) {
    throw new Error(
      "global reservation and identity must immediately precede the fresh engine",
    );
  }
  for (const required of [
    "wasm-build",
    "wasm-checksum",
    "wasm-committed",
    "worktree-clean",
    "math-authoring-public",
    "fresh-static-validation",
    "identity-recheck",
    "preblind-manifest",
  ]) {
    const index = preflight.indexOf(required);
    if (index < 0) {
      throw new Error(`${required} must precede the fresh engine`);
    }
  }
  if (
    preflight.indexOf("wasm-build") > preflight.indexOf("wasm-checksum") ||
    preflight.indexOf("wasm-checksum") > preflight.indexOf("wasm-committed") ||
    preflight.indexOf("wasm-committed") > preflight.indexOf("worktree-clean")
  ) {
    throw new Error(
      "WASM build, checksum, commit, and clean checks are misordered",
    );
  }
  if (
    preflight.indexOf("identity-recheck") > preflight.indexOf("native-build") ||
    preflight.indexOf("native-build") > preflight.indexOf("retained-package") ||
    preflight.indexOf("retained-package") >
      preflight.indexOf("preblind-manifest") ||
    preflight.at(-1) !== "preblind-manifest"
  ) {
    throw new Error(
      "the pre-blind manifest must seal the final frozen artifacts",
    );
  }
}

if (import.meta.main) runSemanticRelease();

function runSemanticRelease(): void {
  assertFreshBlindLinuxX64();
  assertSemanticReleaseStepPlan();
  const fixture = required("SEMATH_FRESH_BLIND_FIXTURE");
  const receipt = required("SEMATH_FRESH_BLIND_RECEIPT");
  const releaseId = required("SEMATH_RELEASE_ID");
  const candidateSha = required("SEMATH_CANDIDATE_SHA");
  const confirmation = required("SEMATH_RELEASE_CONFIRMATION");
  const phase = required("SEMATH_RELEASE_PHASE");
  if (!/^v0\.[1-9][0-9]*$/u.test(releaseId)) {
    throw new Error("SEMATH_RELEASE_ID must be an exact semantic release id");
  }
  if (
    !/^fixtures\/challenge\/document-reasoning-fresh-v[0-9]{3}\.json$/u.test(
      fixture,
    )
  ) {
    throw new Error(
      "fresh blind fixture must use the reviewed challenge namespace",
    );
  }
  if (!/^[0-9a-f]{40}$/u.test(candidateSha)) {
    throw new Error("SEMATH_CANDIDATE_SHA must be a full lowercase commit SHA");
  }
  if (confirmation !== `spend-once:${releaseId}:${candidateSha}`) {
    throw new Error(
      "SEMATH_RELEASE_CONFIRMATION does not match the frozen execution",
    );
  }
  assertFreshBlindWorkflowBoundary(
    freshBlindWorkflowBoundaryFromEnvironment(candidateSha),
  );
  if (existsSync(receipt)) {
    throw new Error(`fresh blind receipt already exists: ${receipt}`);
  }
  assertIdentity(candidateSha);

  if (phase === "preflight") {
    runSemanticReleasePreflight(fixture, releaseId, candidateSha);
    return;
  }
  if (phase === "execute") {
    runSemanticReleaseExecution(fixture, receipt, releaseId, candidateSha);
    return;
  }
  throw new Error("SEMATH_RELEASE_PHASE must be preflight or execute");
}

function runSemanticReleasePreflight(
  fixture: string,
  releaseId: string,
  candidateSha: string,
): void {
  // The fresh blind engine run is deliberately last. A failed pre-blind gate
  // does not spend the sealed fixture, and ordinary CI never invokes it.
  run("bun", ["run", "check"]);
  run("bun", ["run", "quality"]);
  run("bun", ["run", "authored:development:release"]);
  run("bun", ["run", "math-authoring:development"]);
  run("awiki", ["lint", "-r"]);
  run("sh", ["scripts/build-wasm.sh"]);
  run("sha256sum", ["-c", "SHA256SUMS"], { cwd: "lib/wasm" });
  run("git", ["diff", "--exit-code", "--", "lib/wasm"]);
  assertCleanWorktree();
  run("bun", ["run", "package:smoke"]);
  run("bun", ["run", "continuity:release"]);
  run("bun", ["run", "authored:historical:release"]);
  run("bun", ["scripts/check-fresh-blind-fixture.ts"], {
    env: {
      SEMATH_FRESH_BLIND_FIXTURE: fixture,
      SEMATH_RELEASE_ID: releaseId,
    },
  });
  assertIdentity(candidateSha);
  run("cargo", ["build", "--quiet", "--locked", "-p", "semath-native"]);
  run("bun", ["scripts/fresh-blind-preflight-manifest.ts"], {
    env: {
      SEMATH_CANDIDATE_SHA: candidateSha,
      SEMATH_FRESH_BLIND_FIXTURE: fixture,
      SEMATH_RELEASE_ID: releaseId,
    },
  });
}

function runSemanticReleaseExecution(
  fixture: string,
  receipt: string,
  releaseId: string,
  candidateSha: string,
): void {
  const reservation = required("SEMATH_FRESH_BLIND_RESERVATION");
  run("bun", ["scripts/check-fresh-blind-reservation.ts"], {
    env: {
      SEMATH_CANDIDATE_SHA: candidateSha,
      SEMATH_FRESH_BLIND_FIXTURE: fixture,
      SEMATH_FRESH_BLIND_RESERVATION: reservation,
      SEMATH_RELEASE_ID: releaseId,
    },
  });
  assertIdentity(candidateSha);
  run("bun", ["scripts/run-fresh-blind-release.ts"], {
    env: {
      SEMATH_CANDIDATE_SHA: candidateSha,
      SEMATH_FRESH_BLIND_FIXTURE: fixture,
      SEMATH_FRESH_BLIND_RECEIPT: receipt,
      SEMATH_FRESH_BLIND_RESERVATION: reservation,
      SEMATH_FRESH_BLIND_PREFLIGHT_MANIFEST: required(
        "SEMATH_FRESH_BLIND_PREFLIGHT_MANIFEST",
      ),
      SEMATH_RELEASE_ID: releaseId,
    },
  });
}

function assertIdentity(candidateSha: string): void {
  if (output("git", ["rev-parse", "HEAD"]) !== candidateSha) {
    throw new Error("semantic release candidate SHA does not match HEAD");
  }
  assertCleanWorktree();
}

function assertCleanWorktree(): void {
  if (output("git", ["status", "--porcelain"])) {
    throw new Error("semantic release requires a clean worktree");
  }
}

function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return value;
}

function output(command: string, args: readonly string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0)
    throw new Error(result.stderr || `${command} failed`);
  return result.stdout.trim();
}

function run(
  command: string,
  args: readonly string[],
  options: {
    readonly cwd?: string;
    readonly env?: Readonly<Record<string, string>>;
  } = {},
): void {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")} failed`);
}
