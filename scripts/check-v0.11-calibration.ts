import domainCorpus from "../fixtures/v0.11/domain-pack-recognition-corpus.json";
import probabilityCorpus from "../fixtures/v0.6/probability-formula-corpus.json";
import rewriteCorpus from "../fixtures/v0.7/formula-rewrite-corpus.json";
import formulaGolden from "../fixtures/v0.4/formula-intelligence.golden.json";
import actionGolden from "../fixtures/v0.11/action-capable-patterns.golden.json";
import { builtInPacks } from "../packages/packs/src/index";

const packs = builtInPacks();
const patterns = packs.flatMap((pack) =>
  pack.patterns.map((pattern) => ({ pack, pattern })),
);
if (packs.length !== 5 || patterns.length !== 68) {
  throw new Error(`catalog size changed: ${packs.length} packs, ${patterns.length} patterns`);
}

for (const { pack, pattern } of patterns) {
  const references = new Set(pack.references.map((reference) => reference.id));
  if (
    !pattern.topic ||
    !pattern.description ||
    !pattern.descriptionKey ||
    pattern.references.length === 0 ||
    pattern.references.some((reference) => !references.has(reference))
  ) {
    throw new Error(`${pack.packId}/${pattern.id}: incomplete calibration metadata`);
  }
}

const recognitionCatalog = new Set(
  patterns
    .filter(({ pattern }) => pattern.maturity === "recognition")
    .map(({ pattern }) => pattern.id),
);
const recognitionCorpus = new Set(
  domainCorpus.cases.map((entry) => entry.expectedPattern),
);
assertSameSet("recognition corpus", recognitionCatalog, recognitionCorpus);
if (domainCorpus.falsePositiveBudget !== 0) {
  throw new Error("domain recognition false-positive budget must remain zero");
}

const actionCatalog = new Set(
  patterns
    .filter(({ pattern }) => pattern.maturity !== "recognition")
    .map(({ pattern }) => pattern.id),
);
const actionCoverage = new Set<string>();
for (const result of [...formulaGolden.results, ...actionGolden.results]) {
  if (result.value.kind !== "formulaRecognitions") continue;
  for (const recognition of result.value.recognitions) {
    actionCoverage.add(recognition.patternId);
  }
}
for (const entry of probabilityCorpus.cases) {
  for (const pattern of entry.expectedPatterns) actionCoverage.add(pattern);
}
assertSameSet("action-capable pattern corpus", actionCatalog, actionCoverage);
if (probabilityCorpus.falsePositiveBudget !== 0 || rewriteCorpus.falsePositiveBudget !== 0) {
  throw new Error("supported edit corpora must keep a zero false-positive budget");
}
const rewriteCatalog = new Set(packs.flatMap((pack) => pack.rewrites.map((rule) => rule.id)));
const rewriteCoverage = new Set(rewriteCorpus.cases.flatMap((entry) => entry.expectedRules));
assertSameSet("rewrite corpus", rewriteCatalog, rewriteCoverage);

for (const pack of packs) {
  const recognition = pack.patterns.filter(
    (pattern) => pattern.maturity === "recognition",
  ).length;
  console.log(
    `${pack.packId}: ${pack.patterns.length} entries (${recognition} recognition-only, ${pack.patterns.length - recognition} action-capable)`,
  );
}
console.log("v0.11 calibration OK: 68 entries, zero known false links/edits");

function assertSameSet(label: string, expected: Set<string>, actual: Set<string>) {
  const missing = [...expected].filter((value) => !actual.has(value));
  const extra = [...actual].filter((value) => !expected.has(value));
  if (missing.length || extra.length) {
    throw new Error(
      `${label} differs: missing=[${missing.join(", ")}] extra=[${extra.join(", ")}]`,
    );
  }
}
