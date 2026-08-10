import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import {
  CHALLENGE_LAYERS,
  CHALLENGE_METRICS,
  parseChallengeCorpus,
  scoreChallenge,
  type ChallengeCase,
  type ChallengeObservation,
} from "./challenge";

function cases(): ChallengeCase[] {
  return Array.from({ length: 24 }, (_, index) => ({
    cursor: { fileId: "main", needle: "$x$" },
    documents: [{ content: "$x$", fileId: "main", path: "main.tex" }],
    expectation: index % 2 === 0 ? { symbol: "x" } : { excludedRelationId: "wrong" },
    id: `case-${index}`,
    metric: CHALLENGE_METRICS[index % CHALLENGE_METRICS.length]!,
    outcome: index % 2 === 0 ? "positive" : "refusal",
    owner: CHALLENGE_LAYERS[Math.floor(index / 2) % CHALLENGE_LAYERS.length]!,
    variationTags: [`variation-${index}`],
  }));
}

function source(value: readonly ChallengeCase[]) {
  return { cases: value, schemaVersion: 1 };
}

describe("independent recognition challenge", () => {
  test("keeps the checked-in holdout strict and coverage-complete", async () => {
    const fixture: unknown = JSON.parse(
      await readFile(
        new URL("../../../fixtures/challenge/recognition-v1.json", import.meta.url),
        "utf8",
      ),
    );
    expect(parseChallengeCorpus(fixture).cases).toHaveLength(24);
  });

  test("parses a strict frozen matrix with every layer, outcome, and metric", () => {
    expect(parseChallengeCorpus(source(cases())).cases).toHaveLength(24);
    expect(() =>
      parseChallengeCorpus({ ...source(cases()), compatibilityMode: true }),
    ).toThrow("unknown field compatibilityMode");
  });

  test("scores behavior by owner, metric, and outcome without blending failures", () => {
    const corpus = parseChallengeCorpus(source(cases()));
    const observations: ChallengeObservation[] = corpus.cases.map((item) => ({
      assumptionValues: [],
      candidates: [],
      caseId: item.id,
      conceptIds: [],
      definitions: [],
      relationIds: item.outcome === "refusal" ? [] : ["unrelated"],
      shapes: [],
      status: "partial",
      symbols: item.outcome === "positive" ? ["x"] : [],
    }));
    observations[0] = { ...observations[0]!, symbols: [] };
    const scorecard = scoreChallenge(corpus, observations);
    expect(scorecard.passed).toBe(23);
    expect(scorecard.failures).toEqual(["case-0: missing symbol x"]);
    expect(scorecard.layers.binding).toEqual({ passed: 3, total: 4 });
    expect(scorecard.outcomes.refusal).toEqual({ passed: 12, total: 12 });
  });

  test("rejects ambiguous cursors and incomplete coverage", () => {
    const values = cases();
    values[0] = {
      ...values[0]!,
      cursor: { fileId: "main", needle: "x" },
      documents: [{ content: "$x+x$", fileId: "main", path: "main.tex" }],
    };
    expect(() => parseChallengeCorpus(source(values))).toThrow("must occur exactly once");
    expect(() => parseChallengeCorpus(source(cases().slice(0, 23)))).toThrow("at least 24");
  });
});
