import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";
import {
  parseEquivalenceChallenge,
  scoreEquivalenceChallenge,
  selectEquivalenceObservation,
} from "./equivalence-challenge";

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
      decisionDomain: item.decisionDomain,
      problemCount: 0,
      relationIds: item.expectedRelationId ? [item.expectedRelationId] : [],
    }));
    observations[0] = { ...observations[0]!, relationIds: [] };
    const score = scoreEquivalenceChallenge(challenge, observations);
    expect(score.firstFailure).toContain(challenge.cases[0]!.id);
    expect(score.passed).toBe(23);
  });

  test("keeps cursor-entity and selected-formula decisions separate", () => {
    const formula = selectEquivalenceObservation(
      "formula-case",
      "selected-formula",
      { decision: "established", problemCount: 0, relationIds: [] },
      { decision: "partial", problemCount: 0, relationIds: ["circuits:ohm-law"] },
    );
    expect(formula).toEqual({
      caseId: "formula-case",
      decision: "partial",
      decisionDomain: "selected-formula",
      problemCount: 0,
      relationIds: ["circuits:ohm-law"],
    });

    const entity = selectEquivalenceObservation(
      "entity-case",
      "cursor-entity",
      { decision: "established", problemCount: 0, relationIds: [] },
      { decision: "unsupported", problemCount: 0, relationIds: ["linear-algebra:matrix-vector-product"] },
    );
    expect(entity).toEqual({
      caseId: "entity-case",
      decision: "established",
      decisionDomain: "cursor-entity",
      problemCount: 0,
      relationIds: [],
    });
  });

  test("rejects unknown observations and empty relation expectations", () => {
    const challenge = parseEquivalenceChallenge(fixture);
    const observations = challenge.cases.map((item) => ({
      caseId: item.id,
      decision: item.expectedDecision,
      decisionDomain: item.decisionDomain,
      problemCount: 0,
      relationIds: item.expectedRelationId ? [item.expectedRelationId] : [],
    }));
    const score = scoreEquivalenceChallenge(challenge, [
      ...observations,
      {
        caseId: "unknown-case",
        decision: "partial",
        decisionDomain: "selected-formula",
        problemCount: 0,
        relationIds: [],
      },
    ]);
    expect(score.failures).toContain("unknown-case: unexpected observation");

    const invalid = structuredClone(fixture);
    invalid.cases[0].expectedRelationId = " ";
    expect(() => parseEquivalenceChallenge(invalid)).toThrow(
      "expectedRelationId: must be non-empty text",
    );
  });
});
