import { createHash } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import {
  type Corpus,
  parseQualityManifest,
} from "../packages/evaluation/src/index";
import { materializeEngineeringCorpora } from "./generate-engineering-corpus";
import { materializeSyntheticCorpora } from "./generate-synthetic-corpus";

const root = new URL("../", import.meta.url);
const ledgerPath = new URL("fixtures/corpus-materialization.json", root);
const manifest = parseQualityManifest(JSON.parse(
  await readFile(new URL("fixtures/corpus-manifest.json", root), "utf8"),
));
const groups = await Promise.all([
  materializeSyntheticCorpora(),
  materializeEngineeringCorpora(),
]);
const generated = new Map<string, Corpus>();
for (const group of groups) {
  for (const [path, corpus] of group) {
    if (generated.has(path)) {
      throw new Error(`${path}: multiple materializers own the same output`);
    }
    generated.set(path, corpus);
  }
}
const suites = manifest.materializedSuiteIds.map((id) => {
  const suite = manifest.suites.find((candidate) => candidate.id === id);
  if (!suite) throw new Error(`${id}: materialized suite is not declared`);
  const corpus = generated.get(suite.path);
  if (!corpus) throw new Error(`${id}: no materializer owns ${suite.path}`);
  const canonical = JSON.stringify(corpus);
  return {
    bytes: Buffer.byteLength(canonical),
    cases: corpus.cases.length,
    id,
    path: suite.path,
    sha256: createHash("sha256").update(canonical).digest("hex"),
  };
});

for (const path of generated.keys()) {
  if (!suites.some((suite) => suite.path === path)) {
    throw new Error(`${path}: materializer output is missing from the manifest`);
  }
}
for (const suite of suites) {
  const tracked = new URL(`fixtures/${suite.path}`, root);
  if (await stat(tracked).then(() => true, () => false)) {
    throw new Error(`${suite.path}: deterministic output must not be tracked`);
  }
}

const ledger = {
  schemaVersion: 1,
  suites,
  totalBytes: suites.reduce((sum, suite) => sum + suite.bytes, 0),
  totalCases: suites.reduce((sum, suite) => sum + suite.cases, 0),
};
const serialized = `${JSON.stringify(ledger, null, 2)}\n`;
if (process.argv.includes("--update")) {
  await writeFile(ledgerPath, serialized);
  console.log(`updated corpus materialization ledger: ${suites.length} suites`);
} else {
  const current = await readFile(ledgerPath, "utf8").catch(() => "");
  if (current !== serialized) {
    throw new Error(
      "corpus materialization ledger is stale; run bun run corpus:materialization:update",
    );
  }
  console.log(
    `corpus materialization OK: ${suites.length} suites, ${ledger.totalCases} reproducible cases, no tracked expansions`,
  );
}
