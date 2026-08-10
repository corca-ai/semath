import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import {
  CHALLENGE_LAYERS,
  CHALLENGE_METRICS,
  findChallengeFixtureLeaks,
  parseChallengeCorpus,
  scoreChallenge,
  type ChallengeCase,
  type ChallengeObservation,
  type DevelopmentFixtureCase,
} from "./challenge";

function cases(): ChallengeCase[] {
  return Array.from({ length: 48 }, (_, index) => ({
    cursor: { fileId: "main", needle: "$x$" },
    documents: [{ content: "$x$", fileId: "main", path: "main.tex" }],
    expectation: index % 2 === 0 ? { symbol: "x" } : { excludedRelationId: "wrong" },
    id: `case-${index}`,
    metric: CHALLENGE_METRICS[index % CHALLENGE_METRICS.length]!,
    outcome: index % 2 === 0 ? "positive" : "refusal",
    owner: CHALLENGE_LAYERS[Math.floor(index / 2) % CHALLENGE_LAYERS.length]!,
    variationTags: [`variation-${index}`, `boundary-pair:pair-${Math.floor(index / 2)}`],
  }));
}

function source(value: readonly ChallengeCase[]) {
  return { cases: value, schemaVersion: 2 };
}

describe("independent recognition challenge", () => {
  test("keeps the checked-in holdout strict and coverage-complete", async () => {
    const fixture: unknown = JSON.parse(
      await readFile(
        new URL("../../../fixtures/challenge/recognition-v2.json", import.meta.url),
        "utf8",
      ),
    );
    const challenge = parseChallengeCorpus(fixture);
    expect(challenge.cases).toHaveLength(48);

    const development: DevelopmentFixtureCase[] = [];
    const glob = new Bun.Glob("{corpus,foundation}/*.json");
    for await (const path of glob.scan({
      cwd: new URL("../../../fixtures", import.meta.url).pathname,
      onlyFiles: true,
    })) {
      const suite: unknown = JSON.parse(
        await readFile(new URL(`../../../fixtures/${path}`, import.meta.url), "utf8"),
      );
      development.push(...parseDevelopmentCases(suite));
    }
    expect(findChallengeFixtureLeaks(challenge.cases, development)).toEqual([]);
  });

  test("parses a strict frozen matrix with every layer, outcome, and metric", () => {
    expect(parseChallengeCorpus(source(cases())).cases).toHaveLength(48);
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
    expect(scorecard.passed).toBe(47);
    expect(scorecard.failures).toEqual(["case-0: missing symbol x"]);
    expect(scorecard.layers.binding).toEqual({ passed: 7, total: 8 });
    expect(scorecard.outcomes.refusal).toEqual({ passed: 24, total: 24 });
  });

  test("rejects ambiguous cursors and incomplete coverage", () => {
    const values = cases();
    values[0] = {
      ...values[0]!,
      cursor: { fileId: "main", needle: "x" },
      documents: [{ content: "$x+x$", fileId: "main", path: "main.tex" }],
    };
    expect(() => parseChallengeCorpus(source(values))).toThrow("must occur exactly once");
    expect(() => parseChallengeCorpus(source(cases().slice(0, 47)))).toThrow("at least 48");
  });

  test("requires complete semantic boundary pairs and rejects development-fixture reuse", () => {
    const values = cases();
    values[0] = { ...values[0]!, variationTags: ["boundary-pair:unpaired"] };
    expect(() => parseChallengeCorpus(source(values))).toThrow("incomplete boundary pair unpaired");

    const challenge = cases().slice(0, 1);
    expect(findChallengeFixtureLeaks(challenge, challenge)).toEqual([
      "case-0: duplicate fixture id",
      "case-0: duplicate fixture source",
    ]);
  });
});

function parseDevelopmentCases(value: unknown): DevelopmentFixtureCase[] {
  if (!isRecord(value) || !Array.isArray(value.cases)) return [];
  return value.cases.flatMap((item) => {
    if (!isRecord(item) || typeof item.id !== "string" || !Array.isArray(item.documents)) {
      return [];
    }
    const documents = item.documents.flatMap((document) =>
      isRecord(document) &&
      typeof document.content === "string" &&
      typeof document.fileId === "string" &&
      typeof document.path === "string"
        ? [{ content: document.content, fileId: document.fileId, path: document.path }]
        : [],
    );
    return documents.length === item.documents.length ? [{ documents, id: item.id }] : [];
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
