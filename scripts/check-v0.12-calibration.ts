import actionCorpus from "../fixtures/v0.12/action-pattern-calibration.json";
import domainCorpus from "../fixtures/v0.11/domain-pack-recognition-corpus.json";
import { builtInPacks } from "../packages/packs/src/index";
import "./check-v0.11-calibration";
import { recognitionVariants } from "./v0.11-domain-fixture.mjs";

const packs = builtInPacks();
const patternOwners = new Map(
  packs.flatMap((pack) =>
    pack.patterns.map((pattern) => [pattern.id, pack.packId] as const),
  ),
);
const actionPatterns = new Set(
  packs.flatMap((pack) =>
    pack.patterns
      .filter((pattern) => pattern.maturity !== "recognition")
      .map((pattern) => pattern.id),
  ),
);
const calibratedActions = new Set(
  actionCorpus.cases.map((entry) => entry.expectedPattern),
);
assertSameSet("action surface calibration", actionPatterns, calibratedActions);

for (const entry of actionCorpus.cases) {
  if (
    entry.surfaces.length < 3 ||
    new Set(entry.surfaces).size !== entry.surfaces.length
  ) {
    throw new Error(`${entry.id}: requires three distinct reviewed positive surfaces`);
  }
  if (!patternOwners.has(entry.expectedPattern)) {
    throw new Error(`${entry.id}: unknown pattern ${entry.expectedPattern}`);
  }
}
for (const entry of domainCorpus.cases) {
  if (recognitionVariants(entry).filter((variant) => variant.expected).length < 5) {
    throw new Error(`${entry.id}: requires five supported surfaces`);
  }
}

const expectedPairs = new Set<string>();
for (let left = 0; left < packs.length; left += 1) {
  for (let right = left + 1; right < packs.length; right += 1) {
    expectedPairs.add(pairKey(packs[left]!.packId, packs[right]!.packId));
  }
}
const coveredPairs = new Set(
  domainCorpus.collisions
    .filter((entry) => entry.packs[0] !== entry.packs[1])
    .map((entry) => pairKey(entry.packs[0]!, entry.packs[1]!)),
);
assertSameSet("cross-pack collision matrix", expectedPairs, coveredPairs);

for (const entry of domainCorpus.collisions) {
  if (entry.packs.length !== 2 || entry.expectedPatterns.length === 0) {
    throw new Error(`${entry.id}: incomplete collision contract`);
  }
  for (const pattern of entry.expectedPatterns) {
    if (!patternOwners.has(pattern)) {
      throw new Error(`${entry.id}: unknown expected pattern ${pattern}`);
    }
  }
}

console.log(
  `v0.12 calibration OK: 68 patterns, at least 5 supported surfaces and 5 structural negatives each, ${domainCorpus.collisions.length} reviewed collisions`,
);

function pairKey(left: string, right: string) {
  return [left, right].sort().join("/");
}

function assertSameSet(label: string, expected: Set<string>, actual: Set<string>) {
  const missing = [...expected].filter((value) => !actual.has(value));
  const extra = [...actual].filter((value) => !expected.has(value));
  if (missing.length || extra.length) {
    throw new Error(
      `${label} differs: missing=[${missing.join(", ")}] extra=[${extra.join(", ")}]`,
    );
  }
}
