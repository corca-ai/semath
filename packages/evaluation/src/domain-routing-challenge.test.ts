import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  DOMAIN_CHALLENGE_FAMILIES,
  parseDomainRoutingChallenge,
  scoreDomainRoutingChallenge,
} from "./domain-routing-challenge";

const fixture = parseDomainRoutingChallenge(
  JSON.parse(
    readFileSync(new URL("../../../fixtures/challenge/domain-routing-v1.json", import.meta.url), "utf8"),
  ),
);

describe("scoped-domain challenge", () => {
  test("freezes independently authored document families and collision cells", () => {
    expect(fixture.cases).toHaveLength(30);
    for (const family of DOMAIN_CHALLENGE_FAMILIES) {
      expect(fixture.cases.filter((item) => item.family === family).length).toBeGreaterThanOrEqual(4);
    }
    expect(fixture.baseline.protocolVersion).toBe(10);
    expect(fixture.reviewedCollisionComponents.every((item) => item.reason.length > 20)).toBe(true);
  });

  test("scores ordered tiers, decisions, and Problems policy independently", () => {
    const observations = fixture.cases.map((item) => ({
      caseId: item.id,
      decision: item.expectedDecision,
      domains: item.expectedDomains,
      problemCount: item.expectedProblems,
    }));
    expect(scoreDomainRoutingChallenge(fixture, observations)).toMatchObject({
      failures: [],
      passed: 30,
    });
    const changed = observations.map((observation, index) =>
      index === 0 ? { ...observation, domains: [], problemCount: 1 } : observation,
    );
    const score = scoreDomainRoutingChallenge(fixture, changed);
    expect(score.passed).toBe(29);
    expect(score.failures).toHaveLength(2);
  });
});
