#!/usr/bin/env bun

import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import {
  findForbiddenRuntimeBranches,
  packagePackAssets,
} from "./src/index.ts";


const [command, ...args] = process.argv.slice(2);
try {
  switch (command) {
    case "init":
      await initialize(args);
      break;
    case "validate":
      await validateCommand(args);
      break;
    case "package":
      await packageCommand(args);
      break;
    case "audit-runtime":
      await auditRuntime(args);
      break;
    default:
      usage();
      process.exitCode = 2;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

async function initialize([directory, packId]) {
  if (!directory || !packId) fail("init requires <directory> <pack-id>");
  const target = resolve(directory);
  await mkdir(target, { recursive: true });
  const wasm = await loadWasm();
  const source = wasm.createPackTemplate(packId);
  const report = inspect(wasm, [{ path: "pack.json", source }]);
  assertCompilerClean(report);
  await writeNew(join(target, "pack.json"), source);
  console.log(`initialized ${packId} in ${target}`);
}

async function validateCommand(paths) {
  if (!paths.length) fail("validate requires one or more pack JSON files");
  const wasm = await loadWasm();
  const sources = await readSources(paths);
  const report = inspect(wasm, sources);
  printCompilerReport(report);
  assertCompilerClean(report);

}

async function packageCommand([outputPath, ...paths]) {
  if (!outputPath || !paths.length) {
    fail("package requires <output.json> <pack.json...>");
  }
  const wasm = await loadWasm();
  const sources = await readSources(paths);
  const report = inspect(wasm, sources);
  printCompilerReport(report);
  assertCompilerClean(report);
  await writeFile(resolve(outputPath), pretty(packagePackAssets(sources, report)));
  console.log(`packaged ${sources.length} pack(s) in ${resolve(outputPath)}`);
}

async function auditRuntime(paths) {
  if (!paths.length) fail("audit-runtime requires one or more pack JSON files");
  const packs = await Promise.all(paths.map(readJson));
  const forbidden = packs.flatMap((pack) => [
    pack.packId,
    ...pack.laws.map((law) => law.id),
  ]);
  const files = await collectRuntimeSources(process.cwd());
  const violations = findForbiddenRuntimeBranches(files, forbidden);
  for (const violation of violations) {
    console.error(
      `${violation.path}:${violation.line}: forbidden runtime decision on ${violation.id}: ${violation.sourceLine}`,
    );
  }
  if (violations.length) fail(`${violations.length} pack-specific runtime branch(es) found`);
  console.log(`runtime branch audit OK: ${forbidden.length} declarative IDs, ${files.length} source files`);
}

async function loadWasm() {
  const wasm = await import("../../lib/wasm/semath_wasm.js");
  const bytes = await readFile(new URL("../../lib/wasm/semath_wasm_bg.wasm", import.meta.url));
  await wasm.default({ module_or_path: bytes });
  if (
    typeof wasm.inspectPackCatalog !== "function" ||
    typeof wasm.createPackTemplate !== "function"
  ) {
    fail("installed Semath WASM does not expose the pack authoring contract");
  }
  return wasm;
}

function inspect(wasm, sources) {
  const payload = new TextEncoder().encode(JSON.stringify({ schemaVersion: 3, sources }));
  return JSON.parse(new TextDecoder().decode(wasm.inspectPackCatalog(payload)));
}

function printCompilerReport(report) {
  for (const diagnostic of report.diagnostics) {
    const entity = diagnostic.entityId ? ` [${diagnostic.entityId}]` : "";
    console.error(
      `${diagnostic.severity} ${diagnostic.file}:${diagnostic.jsonPath}${entity} ${diagnostic.code}: ${diagnostic.message}`,
    );
  }
  for (const form of report.forms) {
    console.log(`${form.packId}/${form.lawId}[${form.formIndex}]: ${form.canonical}`);
  }
  for (const archetype of report.archetypes) {
    console.log(
      `archetype ${archetype.archetypeId}: ${archetype.adoptedLaws.length}/${archetype.matchingLaws.length} matching laws adopted`,
    );
  }
  console.log(
    `compiler OK: ${report.packs.length} pack(s), ${report.signatures.length} domain signature(s), ${report.collisions.length} structural collision(s), ${report.forms.length} canonical form(s), ${report.diagnostics.length} diagnostic(s)`,
  );
}

function assertCompilerClean(report) {
  if (report.diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    fail("pack compiler rejected the catalog");
  }
}

async function readSources(paths) {
  return Promise.all(paths.map(async (path) => ({
    path,
    source: await readFile(resolve(path), "utf8"),
  })));
}

async function readJson(path) {
  return JSON.parse(await readFile(resolve(path), "utf8"));
}

async function writeNew(path, source) {
  await writeFile(path, source, { flag: "wx" });
}

async function collectRuntimeSources(root) {
  const includedRoots = ["crates", "packages"];
  const files = [];
  for (const included of includedRoots) {
    await walk(join(root, included), files, root);
  }
  return files;
}

async function walk(directory, files, root) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      await walk(path, files, root);
    } else if (/\.(?:rs|ts|mjs)$/u.test(entry.name)) {
      files.push({ path: relative(root, path), source: await readFile(path, "utf8") });
    }
  }
}

function pretty(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function fail(message) {
  throw new Error(message);
}

function usage() {
  console.error(`Usage: semath-pack <command>

  init <directory> <pack-id>
  validate <pack.json...>
  package <output.json> <pack.json...>
  audit-runtime <pack.json...>`);
}
