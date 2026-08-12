import { loadFreshBlindEvidence } from "./fresh-blind-evidence";

const path = process.env.SEMATH_FRESH_BLIND_FIXTURE;
if (!path) {
  throw new Error(
    "SEMATH_FRESH_BLIND_FIXTURE must name the sealed fixture explicitly",
  );
}

const evidence = await loadFreshBlindEvidence(path);
console.log(
  `fresh blind fixture OK: ${evidence.summary.scenarios} scenarios, ` +
    `${evidence.summary.probes} probes, ${evidence.summary.laws} laws, ` +
    `max prose similarity ${evidence.summary.maximumProseSimilarity.toFixed(3)}`,
);
