import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";

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

// The fresh blind engine run is deliberately last. A failed pre-blind gate does
// not spend the sealed fixture, and ordinary CI never invokes this orchestrator.
run("bun", ["run", "check"]);
run("bun", ["run", "quality"]);
run("bun", ["run", "authored:development:release"]);
run("awiki", ["lint", "-r"]);
run("sh", ["scripts/build-wasm.sh"]);
run("sha256sum", ["-c", "SHA256SUMS"], { cwd: "lib/wasm" });
run("bun", ["run", "package:smoke"]);
run("bun", ["run", "continuity"]);
run("bun", ["run", "authored:historical"]);
run("bun", ["scripts/check-fresh-blind-fixture.ts"], {
  env: { SEMATH_FRESH_BLIND_FIXTURE: fixture },
});
run("bun", ["scripts/run-fresh-blind-release.ts"], {
  env: {
    SEMATH_FRESH_BLIND_FIXTURE: fixture,
    SEMATH_FRESH_BLIND_RECEIPT: receipt,
  },
});

function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`${name} must be set explicitly`);
  return value;
}

function output(command: string, args: readonly string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || `${command} failed`);
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
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed`);
}
