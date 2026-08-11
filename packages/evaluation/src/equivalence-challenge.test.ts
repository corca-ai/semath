import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";
import { parseEquivalenceChallenge, scoreEquivalenceChallenge } from "./equivalence-challenge";

describe("guarded-equivalence challenge", () => {
  const fixture = JSON.parse(readFileSync(new URL("../../../fixtures/challenge/equivalence-v1.json", import.meta.url), "utf8"));

  test("freezes balanced independently-authored families", () => {
    const challenge = parseEquivalenceChallenge(fixture);
    expect(challenge.cases).toHaveLength(24);
    expect(new Set(challenge.cases.map((item) => item.family)).size).toBe(6);
  });

  test("reports the first deterministic divergence", () => {
    const challenge = parseEquivalenceChallenge(fixture);
    const observations = challenge.cases.map((item) => ({
      caseId: item.id,
      decision: item.expectedDecision,
      problemCount: 0,
      relationId: item.expectedRelationId,
    }));
    observations[0] = { ...observations[0]!, relationId: null };
    const score = scoreEquivalenceChallenge(challenge, observations);
    expect(score.firstFailure).toContain(challenge.cases[0]!.id);
    expect(score.passed).toBe(23);
  });
});
