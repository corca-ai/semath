import { readFile } from "node:fs/promises";
import {
  parseSpentHoldoutRegistry,
  validateSpentHoldoutIsolation,
} from "../packages/evaluation/src/index";
import {
  freshSpentHoldoutLineage,
  loadFreshBlindEvidence,
} from "./fresh-blind-evidence";

const path = process.env.SEMATH_FRESH_BLIND_FIXTURE;
if (!path) {
  throw new Error(
    "SEMATH_FRESH_BLIND_FIXTURE must name the sealed fixture explicitly",
  );
}

const evidence = await loadFreshBlindEvidence(path);
const releaseId = process.env.SEMATH_RELEASE_ID;
if (!releaseId?.trim()) {
  throw new Error("SEMATH_RELEASE_ID must be set explicitly");
}
if (evidence.release.release.id !== releaseId) {
  throw new Error(
    `fresh blind fixture id ${evidence.release.release.id} does not match ${releaseId}`,
  );
}
const spentRegistry = parseSpentHoldoutRegistry(
  JSON.parse(
    await readFile(
      "fixtures/challenge/spent-holdout-registry-v1.json",
      "utf8",
    ),
  ),
);
const spentIsolation = validateSpentHoldoutIsolation(
  spentRegistry,
  freshSpentHoldoutLineage(evidence.release),
);
console.log(
  `fresh blind fixture OK: ${evidence.summary.scenarios} scenarios, ` +
    `${evidence.summary.probes} probes, ${evidence.summary.laws} laws, ` +
    `max prose similarity ${evidence.summary.maximumProseSimilarity.toFixed(3)}, ` +
    `${spentIsolation.spentReleases} spent releases isolated`,
);
