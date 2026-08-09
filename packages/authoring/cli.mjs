#!/usr/bin/env bun

import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import {
  compareScorecards,
  findForbiddenRuntimeBranches,
  packagePackAssets,
  projectValidatedPack,
  scaffoldPackWorkspace,
} from "./src/index.ts";
import {
  findCorpusDuplicates,
  observeQualityRun,
  parseCorpus,
  parseQualityManifest,
  planQualityRun,
  scoreQuality,
  explainQualityCase,
} from "../evaluation/src/index.ts";

const [command, ...args] = process.argv.slice(2);
try {
  switch (command) {
    case "init":
      await initialize(args);
      break;
    case "validate":
      await validateCommand(args);
      break;
    case "scaffold":
      await scaffoldCommand(args);
      break;
    case "score":
      await scoreCommand(args, false);
      break;
    case "explain":
      await scoreCommand(args, true);
      break;
    case "compare":
      await compareCommand(args);
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
  const workspace = scaffoldPackWorkspace(projectValidatedPack(JSON.parse(source)));
  await writeNew(join(target, "pack.json"), source);
  await writeNew(join(target, "corpus.json"), pretty(workspace.corpus));
  await writeNew(join(target, "manifest.json"), pretty(workspace.manifest));
  console.log(`initialized ${packId} in ${target}`);
  console.log("Review the generated positive and refusal seeds before claiming probe maturity.");
}

async function validateCommand(paths) {
  if (!paths.length) fail("validate requires one or more pack JSON files");
  const wasm = await loadWasm();
  const sources = await readSources(paths);
  const report = inspect(wasm, sources);
  printCompilerReport(report);
  assertCompilerClean(report);
}

async function scaffoldCommand([packPath, directory]) {
  if (!packPath || !directory) fail("scaffold requires <pack.json> <directory>");
  const wasm = await loadWasm();
  const [source] = await readSources([packPath]);
  const report = inspect(wasm, [source]);
  printCompilerReport(report);
  assertCompilerClean(report);
  const workspace = scaffoldPackWorkspace(projectValidatedPack(JSON.parse(source.source)));
  const target = resolve(directory);
  await mkdir(target, { recursive: true });
  await writeNew(join(target, "corpus.json"), pretty(workspace.corpus));
  await writeNew(join(target, "manifest.json"), pretty(workspace.manifest));
  console.log(`scaffolded ${workspace.corpus.cases.length} reviewed starting cells in ${target}`);
}

async function scoreCommand(args, explain) {
  const [workspacePath, packId, caseId] = args;
  if (!workspacePath || !packId || (explain && !caseId)) {
    fail(explain
      ? "explain requires <workspace-or-manifest> <pack-id> <case-id>"
      : "score requires <workspace-or-manifest> <pack-id>");
  }
  const { corpora, manifest } = await loadWorkspace(workspacePath);
  const scoped = scopePack(manifest, corpora, packId);
  const duplicateFailures = findCorpusDuplicates([...scoped.corpora.values()]);
  if (duplicateFailures.length) {
    fail(duplicateFailures.join("\n"));
  }
  const plan = planQualityRun(scoped.manifest, scoped.corpora);
  const results = await runWasm(plan);
  if (explain) {
    const index = plan.planned.findIndex(
      (item) => !item.generatedFrom && item.case.id === caseId,
    );
    if (index < 0) fail(`${caseId}: case not found in ${packId}`);
    console.log(pretty(explainQualityCase(plan.planned[index], results[index])).trim());
    return;
  }
  const observations = observeQualityRun(plan, results);
  const scorecard = scoreQuality(scoped.manifest, scoped.corpora, observations);
  console.log(pretty(scorecard).trim());
  if (scorecard.failures.length) fail("focused scorecard did not pass");
}

async function compareCommand([baselinePath, candidatePath]) {
  if (!baselinePath || !candidatePath) {
    fail("compare requires <baseline-scorecard.json> <candidate-scorecard.json>");
  }
  const [baseline, candidate] = await Promise.all([
    readJson(baselinePath),
    readJson(candidatePath),
  ]);
  const comparison = compareScorecards(baseline, candidate);
  console.log(pretty(comparison).trim());
  if (comparison.regressions.length) fail("candidate contains metric regressions");
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
  const payload = new TextEncoder().encode(JSON.stringify({ schemaVersion: 1, sources }));
  return JSON.parse(new TextDecoder().decode(wasm.inspectPackCatalog(payload)));
}

async function runWasm(plan) {
  const wasm = await loadWasm();
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const engine = new wasm.SemathEngine();
  try {
    engine.resetProject(encoder.encode(JSON.stringify(plan.snapshot)));
    return plan.queries.map((query) =>
      JSON.parse(decoder.decode(engine.query(encoder.encode(JSON.stringify(query))))),
    );
  } finally {
    engine.free();
  }
}

async function loadWorkspace(path) {
  const manifestPath = path.endsWith(".json")
    ? resolve(path)
    : resolve(path, "manifest.json");
  const manifest = parseQualityManifest(await readJson(manifestPath));
  const corpora = new Map();
  for (const suite of manifest.suites) {
    const source = await readJson(resolve(dirname(manifestPath), suite.path));
    corpora.set(suite.id, parseCorpus(source, suite));
  }
  return { corpora, manifest };
}

function scopePack(manifest, corpora, packId) {
  const support = manifest.packs.find((pack) => pack.packId === packId);
  if (!support) fail(`${packId}: support declaration not found`);
  const suiteIds = new Set(
    Object.values(support.capabilities).flatMap((capability) => capability.suiteIds),
  );
  const suites = manifest.suites.filter(
    (suite) => suite.kind === "law" && suite.packId === packId && suiteIds.has(suite.id),
  );
  if (!suites.length) fail(`${packId}: no law corpus suites are owned by this pack`);
  return {
    corpora: new Map(suites.map((suite) => [suite.id, corpora.get(suite.id)])),
    manifest: {
      ...manifest,
      foundationSuites: [],
      packs: [support],
      suites,
    },
  };
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
  console.log(
    `compiler OK: ${report.packs.length} pack(s), ${report.forms.length} canonical form(s), ${report.diagnostics.length} diagnostic(s)`,
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
  scaffold <pack.json> <directory>
  score <workspace-or-manifest> <pack-id>
  explain <workspace-or-manifest> <pack-id> <case-id>
  compare <baseline-scorecard.json> <candidate-scorecard.json>
  package <output.json> <pack.json...>
  audit-runtime <pack.json...>`);
}
