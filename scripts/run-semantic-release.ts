import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

function run(command: string, args: string[]): void {
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed`);
}

function output(command: string, args: string[]): string {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || `${command} failed`);
  return result.stdout.trim();
}

function assertClean(): void {
  if (output("git", ["status", "--porcelain"])) {
    throw new Error("release qualification requires a committed, clean candidate");
  }
}

const reportPath = ".artifacts/conservative-release.json";
// Invalidate a prior success before any fallible step in a new attempt.
mkdirSync(".artifacts", { recursive: true });
writeFileSync(reportPath, `${JSON.stringify({ status: "incomplete" })}\n`);
// Qualification is repeatable and has no publication or GitHub side effects.
if (process.platform !== "linux" || process.arch !== "x64") {
  throw new Error("release qualification requires an x86_64 Linux host");
}
assertClean();
const candidate = output("git", ["rev-parse", "HEAD"]);
run("sh", ["scripts/build-wasm.sh"]);
run("git", ["diff", "--exit-code", "--", "lib/wasm"]);
run("bun", ["run", "check"]);
run("bun", ["run", "quality"]);
run("awiki", ["lint", "-r"]);
const packageDirectory = ".artifacts/conservative-package";
mkdirSync(packageDirectory, { recursive: true });
const packed = JSON.parse(output("npm", ["pack", "--json", "--pack-destination", packageDirectory]));
const packagePath = `${packageDirectory}/${packed[0].filename}`;
assertClean();
if (output("git", ["rev-parse", "HEAD"]) !== candidate) {
  throw new Error("candidate changed during qualification");
}
const packageMetadata = JSON.parse(readFileSync("package.json", "utf8"));
const digest = (path: string) => createHash("sha256").update(readFileSync(path)).digest("hex");
const report = {
  status: "passed",
  scope: "conservative-mathematical-document-analysis",
  candidate,
  packageVersion: packageMetadata.version,
  wasmtex: packageMetadata.dependencies.wasmtex,
  wasmSha256: digest("lib/wasm/semath_wasm_bg.wasm"),
  checksumManifestSha256: digest("lib/wasm/SHA256SUMS"),
  packagePath,
  packageSha256: digest(packagePath),
  completedAt: new Date().toISOString(),
  platform: process.platform,
  architecture: process.arch,
};
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(`conservative release qualified: ${candidate}; ${reportPath}`);
