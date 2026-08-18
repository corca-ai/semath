import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";

// The fresh blind engine run is deliberately last. A failed pre-blind gate does
// not spend the sealed fixture, and ordinary CI never invokes this orchestrator.
export function semanticReleaseSteps(
  fixture: string,
  receipt: string,
): readonly SemanticReleaseStep[] {
  return [
    command("bun", ["run", "check"]),
    command("bun", ["run", "quality"]),
    command("bun", ["run", "authored:development:release"]),
    command("awiki", ["lint", "-r"]),
    command("sh", ["scripts/build-wasm.sh"]),
    command("sha256sum", ["-c", "SHA256SUMS"], { cwd: "lib/wasm" }),
    { kind: "assert-committed-wasm-artifacts-match-head" },
    command("bun", ["run", "package:smoke"]),
    command("bun", ["run", "continuity:release"]),
    command("bun", ["run", "authored:historical:release"]),
    command("bun", ["scripts/check-fresh-blind-fixture.ts"], {
      env: { SEMATH_FRESH_BLIND_FIXTURE: fixture },
    }),
    { kind: "assert-clean-release-worktree" },
    command("bun", ["scripts/run-fresh-blind-release.ts"], {
      env: {
        SEMATH_FRESH_BLIND_FIXTURE: fixture,
        SEMATH_FRESH_BLIND_RECEIPT: receipt,
      },
    }),
  ];
}

export function assertCommittedWasmArtifactsMatchHead(
  changedPaths: string,
  untrackedPaths = "",
): void {
  const changed = [changedPaths, untrackedPaths]
    .map((paths) => paths.trim())
    .filter(Boolean)
    .join("\n");
  if (changed) {
    throw new Error(
      `release WASM artifacts differ from HEAD after the x86_64 build:\n${changed}`,
    );
  }
}

export function assertCleanReleaseWorktree(changedPaths: string): void {
  const changed = changedPaths.trim();
  if (changed) {
    throw new Error(
      `semantic release worktree changed before the fresh blind boundary:\n${changed}`,
    );
  }
}

function main(): void {
  if (process.platform !== "linux" || process.arch !== "x64") {
    throw new Error("semantic releases must run on a separate x86_64 Linux host");
  }
  const fixture = required("SEMATH_FRESH_BLIND_FIXTURE");
  const receipt = required("SEMATH_FRESH_BLIND_RECEIPT");
  if (existsSync(receipt)) {
    throw new Error(`fresh blind receipt already exists: ${receipt}`);
  }
  if (output("git", ["status", "--porcelain"])) {
    throw new Error("semantic release requires a clean worktree");
  }

  for (const step of semanticReleaseSteps(fixture, receipt)) {
    if (step.kind === "assert-committed-wasm-artifacts-match-head") {
      assertCommittedWasmArtifactsMatchHead(
        output("git", ["diff", "--name-only", "HEAD", "--", "lib/wasm"]),
        output("git", ["ls-files", "--others", "--", "lib/wasm"]),
      );
    } else if (step.kind === "assert-clean-release-worktree") {
      assertCleanReleaseWorktree(
        output("git", ["status", "--porcelain", "--untracked-files=all"]),
      );
    } else {
      run(step.command, step.args, step.options);
    }
  }
}

interface CommandOptions {
  readonly cwd?: string;
  readonly env?: Readonly<Record<string, string>>;
}

type SemanticReleaseStep =
  | {
      readonly kind: "command";
      readonly command: string;
      readonly args: readonly string[];
      readonly options: CommandOptions;
    }
  | { readonly kind: "assert-clean-release-worktree" }
  | { readonly kind: "assert-committed-wasm-artifacts-match-head" };

function command(
  executable: string,
  args: readonly string[],
  options: CommandOptions = {},
): SemanticReleaseStep {
  return { args, command: executable, kind: "command", options };
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
  options: CommandOptions = {},
): void {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")} failed`);
}

if (import.meta.main) main();
