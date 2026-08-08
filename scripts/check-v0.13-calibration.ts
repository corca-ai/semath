import actionCorpus from "../fixtures/v0.12/action-pattern-calibration.json";
import domainCorpus from "../fixtures/v0.11/domain-pack-recognition-corpus.json";
import { builtInPacks } from "../packages/packs/src/index";
import "./check-v0.12-calibration";
import { recognitionVariants } from "./v0.11-domain-fixture.mjs";
import { actionPatternVariants } from "./v0.12-action-fixture.mjs";

const domainByPattern = new Map(
  domainCorpus.cases.map((entry) => [entry.expectedPattern, entry] as const),
);
const actionByPattern = new Map(
  actionCorpus.cases.map((entry) => [entry.expectedPattern, entry] as const),
);
const scorecards = builtInPacks().map((pack) => {
  let positives = 0;
  let refusals = 0;
  for (const pattern of pack.patterns) {
    const domain = domainByPattern.get(pattern.id);
    const action = actionByPattern.get(pattern.id);
    if (Boolean(domain) === Boolean(action)) {
      throw new Error(`${pattern.id}: expected exactly one calibration owner`);
    }
    const variants = domain
      ? recognitionVariants(domain)
      : actionPatternVariants(action!);
    const supported = variants.filter((variant) => variant.expected);
    const rejected = variants.filter((variant) => !variant.expected);
    if (supported.length < 6 || rejected.length < 6) {
      throw new Error(
        `${pattern.id}: requires at least six contextual positives and six refusals`,
      );
    }
    if (!supported.some((variant) => variant.id.includes("unicode"))) {
      throw new Error(`${pattern.id}: missing Unicode/CRLF context coverage`);
    }
    if (!rejected.some((variant) => variant.id.includes("mutation"))) {
      throw new Error(`${pattern.id}: missing generated mutation refusal`);
    }
    positives += supported.length;
    refusals += rejected.length;
  }
  return {
    packId: pack.packId,
    patterns: pack.patterns.length,
    positives,
    refusals,
  };
});

if (scorecards.reduce((total, score) => total + score.patterns, 0) !== 68) {
  throw new Error("v0.13 must retain exactly 68 patterns");
}

for (const score of scorecards) {
  console.log(
    `v0.13 pack scorecard: ${score.packId} ${score.patterns} patterns, ${score.positives} positives, ${score.refusals} refusals`,
  );
}
