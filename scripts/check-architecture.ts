import { join, relative } from "node:path";

const ROOT = join(import.meta.dir, "..");

async function sourceFiles(): Promise<readonly string[]> {
  const files: string[] = [];
  for (const root of ["crates", "packages", "scripts"]) {
    const glob = new Bun.Glob("**/*");
    for await (const path of glob.scan({ cwd: join(ROOT, root), onlyFiles: true })) {
      if (/\.(?:rs|ts|mjs)$/u.test(path)) files.push(join(ROOT, root, path));
    }
  }
  return files.sort();
}

function fail(message: string): never {
  throw new Error(`architecture gate: ${message}`);
}

const packageJson = await Bun.file(join(ROOT, "package.json")).json();
const dependencyNames = Object.keys(packageJson.dependencies ?? {});
if (dependencyNames.some((name) => name.includes("cortex"))) {
  fail("Semath must not depend on CorTeX");
}
if (dependencyNames.filter((name) => name === "wasmtex").length !== 1) {
  fail("wasmtex must be the single structural frontend dependency");
}

let adapterDefinitions = 0;
for (const file of await sourceFiles()) {
  const path = relative(ROOT, file);
  const source = await Bun.file(file).text();
  if (/\b(?:from|import\s*\(|require\s*\()\s*["'][^"']*cortex/iu.test(source)) {
    fail(`${path} imports CorTeX`);
  }
  if (
    path !== "scripts/check-architecture.ts" &&
    !/\.(?:test|spec)\.(?:ts|rs)$/u.test(path) &&
    source.includes("compatibilityMode")
  ) {
    fail(`${path} contains a compatibility runtime`);
  }
  adapterDefinitions += source.match(/function\s+adaptWasmtexDocument\s*\(/gu)?.length ?? 0;
}
if (adapterDefinitions !== 1) {
  fail(`expected one wasmtex adapter, found ${adapterDefinitions}`);
}

const engine = await Bun.file(
  join(ROOT, "crates/semath-core/src/engine.rs"),
).text();
if (engine.includes("SemanticFactStore") || /\bfacts:\s*HashMap</u.test(engine)) {
  fail("a parallel project semantic fact store remains");
}
if (!/#\[cfg\(test\)\]\s+let parsed = if document\.nodes\.is_empty\(\)/u.test(engine)) {
  fail("the raw-TeX parser exception is not visibly test-only");
}

const parser = await Bun.file(
  join(ROOT, "crates/semath-core/src/parser.rs"),
).text();
if (!/#\[cfg\(test\)\]\s+pub\(crate\) fn parse_regions/u.test(parser)) {
  fail("raw-TeX region parsing is not test-only");
}

console.log("architecture OK: dependency direction, singular adapter, and one project authority");
