import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import {
  CHALLENGE_LAYERS,
  CHALLENGE_METRICS,
  findChallengeFixtureLeaks,
  parseChallengeCorpus,
  parseChallengeV3,
  parseChallengeV4,
  scoreChallenge,
  type ChallengeCase,
  type ChallengeCorpus,
  type ChallengeObservation,
  type DevelopmentFixtureCase,
} from "./challenge";

function cases(): ChallengeCase[] {
  return Array.from({ length: 48 }, (_, index) => ({
    cursor: { fileId: "main", needle: "$x$" },
    documents: [{ content: "$x$", fileId: "main", path: "main.tex" }],
    expectation:
      index % 2 === 0 ? { symbol: "x" } : { excludedRelationId: "wrong" },
    id: `case-${index}`,
    metric: CHALLENGE_METRICS[index % CHALLENGE_METRICS.length]!,
    outcome: index % 2 === 0 ? "positive" : "refusal",
    owner: CHALLENGE_LAYERS[Math.floor(index / 2) % CHALLENGE_LAYERS.length]!,
    variationTags: [
      `variation-${index}`,
      `boundary-pair:pair-${Math.floor(index / 2)}`,
    ],
  }));
}

function source(value: readonly ChallengeCase[]) {
  return { cases: value, schemaVersion: 2 };
}

function observation(item: ChallengeCase): ChallengeObservation {
  return {
    assumptionValues: [],
    candidates: [],
    caseId: item.id,
    conceptIds: [],
    definitions: [],
    problemCount: 0,
    reasonKinds: [],
    relationIds: [],
    shapes: [],
    sourceGrounded: false,
    symbols: item.expectation.symbol ? [item.expectation.symbol] : [],
  };
}

describe("independent recognition challenge", () => {
  test("keeps the checked-in holdout strict and coverage-complete", async () => {
    const fixture: unknown = JSON.parse(
      await readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v2.json",
          import.meta.url,
        ),
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
        await readFile(
          new URL(`../../../fixtures/${path}`, import.meta.url),
          "utf8",
        ),
      );
      development.push(...parseDevelopmentCases(suite));
    }
    expect(findChallengeFixtureLeaks(challenge.cases, development)).toEqual([]);
  });

  test("composes every frozen case with explicit document and decision policy", async () => {
    const base: unknown = JSON.parse(
      await readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v2.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const profile: unknown = JSON.parse(
      await readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v3.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    const challenge = parseChallengeV3(base, profile);
    expect(challenge.schemaVersion).toBe(3);
    expect(challenge.cases).toHaveLength(48);
    expect(new Set(challenge.cases.map((item) => item.documentShape))).toEqual(
      new Set([
        "distant-prose",
        "macro-neighbor",
        "malformed-neighbor",
        "multi-equation",
        "project-neighbor",
        "sectioned",
      ]),
    );
    expect(challenge.cases.every((item) => item.decisionExpectation)).toBe(
      true,
    );
    expect(challenge.cases.every((item) => item.documents.length > 0)).toBe(
      true,
    );
  });

  test("composes the strict v4 authority overlay without changing v2 or v3", async () => {
    const [baseText, v3Text, v4Text] = await Promise.all([
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v2.json",
          import.meta.url,
        ),
        "utf8",
      ),
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v3.json",
          import.meta.url,
        ),
        "utf8",
      ),
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v4.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ]);
    const challenge = parseChallengeV4(
      baseText,
      v3Text,
      JSON.parse(v4Text),
    );
    expect(challenge.schemaVersion).toBe(4);
    expect(challenge.cases).toHaveLength(48);
    expect(challenge.cases.every((item) => item.decisionDomain)).toBe(true);
    expect(challenge.cases.every((item) => item.recognizedRelations)).toBe(
      true,
    );
    const baseRelations = new Set<string>(
      JSON.parse(baseText).cases.flatMap(
        (item: { id: string; expectation: { relationId?: string } }) =>
          item.expectation.relationId ? [item.id] : [],
      ),
    );
    const reviewedRelationCases = challenge.cases.filter((item) =>
      baseRelations.has(item.id),
    );
    expect(
      reviewedRelationCases.filter(
        (item) => item.expectation.relationId === undefined,
      ),
    ).toHaveLength(4);
    expect(
      reviewedRelationCases.filter(
        (item) => item.expectation.relationId !== undefined,
      ),
    ).toHaveLength(4);
    expect(JSON.parse(baseText).schemaVersion).toBe(2);
    expect(JSON.parse(v3Text).schemaVersion).toBe(3);
  });

  test("rejects incomplete, domain-mixed, and non-exact v4 policy", async () => {
    const [base, v3, v4Text] = await Promise.all([
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v2.json",
          import.meta.url,
        ),
        "utf8",
      ),
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v3.json",
          import.meta.url,
        ),
        "utf8",
      ),
      readFile(
        new URL(
          "../../../fixtures/challenge/recognition-v4.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ]);
    const complete: unknown = JSON.parse(v4Text);
    const missingDomain = structuredClone(complete);
    delete profileRecords(missingDomain)[0]!.decisionDomain;
    expect(() => parseChallengeV4(base, v3, missingDomain)).toThrow(
      "unknown or missing field decisionDomain",
    );

    const wrongAnchor = structuredClone(complete);
    profileRecords(wrongAnchor)[0]!.recognizedRelations = [
      {
        authority: "candidate",
        formulaAnchor: "cursor-formula",
        relationId: "test:relation",
        support: "supported",
      },
    ];
    expect(() => parseChallengeV4(base, v3, wrongAnchor)).toThrow(
      "formulaAnchor",
    );

    const invalidSupport = structuredClone(complete);
    profileRecords(invalidSupport)[0]!.recognizedRelations = [
      {
        authority: "candidate",
        formulaAnchor: "selected-formula",
        relationId: "test:relation",
        support: "certain",
      },
    ];
    expect(() => parseChallengeV4(base, v3, invalidSupport)).toThrow(
      "support",
    );

    const extra = structuredClone(complete);
    profileRecords(extra)[0]!.runtimeCaseId = "case-0";
    expect(() => parseChallengeV4(base, v3, extra)).toThrow(
      "unknown field runtimeCaseId",
    );

    const wrongDigest = structuredClone(complete);
    const wrongDigestRoot = recordFixture(wrongDigest, "v4 fixture");
    const digests = recordFixture(wrongDigestRoot.baseDigests, "baseDigests");
    digests.recognitionV2Sha256 = "0".repeat(64);
    expect(() => parseChallengeV4(base, v3, wrongDigest)).toThrow(
      "baseDigests.recognitionV2Sha256: must equal",
    );
    expect(() => parseChallengeV4(`${base} `, v3, complete)).toThrow(
      "baseDigests.recognitionV2Sha256: source digest mismatch",
    );

    const parsedBase = parseChallengeCorpus(JSON.parse(base));
    const excluded = parsedBase.cases.find(
      (item) => item.expectation.excludedRelationId,
    );
    if (!excluded) throw new Error("expected an excluded relation fixture");
    const wrongExcludedDomain = structuredClone(complete);
    const excludedProfile = profileRecords(wrongExcludedDomain).find(
      (profile) => profile.caseId === excluded.id,
    );
    if (!excludedProfile) throw new Error("missing excluded relation profile");
    excludedProfile.decisionDomain = "cursor-entity";
    expect(() => parseChallengeV4(base, v3, wrongExcludedDomain)).toThrow(
      "relation decisions must use selected-formula",
    );

    const tooManyBase: unknown = JSON.parse(base);
    const tooManyCases = fixtureCases(tooManyBase);
    recordFixture(tooManyBase, "v2 fixture").cases = [
      ...tooManyCases,
      { ...tooManyCases[0]!, id: "extra-v4-case" },
    ];
    expect(() =>
      parseChallengeV4(JSON.stringify(tooManyBase), v3, complete),
    ).toThrow("base: must contain exactly 48 cases");
  });

  test("scores v4 decisions in the declared domain and keeps relation authority exact", () => {
    const baseCases = cases();
    const corpus: ChallengeCorpus = {
      cases: baseCases.map(
        (item, index): ChallengeCase =>
          index === 0
            ? {
                ...item,
                decisionDomain: "selected-formula",
                decisionExpectation: {
                  meaning: "present",
                  problems: "source-conflict",
                  status: "conflicting",
                },
                expectation: { symbol: "x" },
                recognizedRelations: [
                  {
                    authority: "candidate",
                    formulaAnchor: "selected-formula",
                    relationId: "test:law",
                    support: "contradicted",
                  },
                ],
              }
            : {
                ...item,
                decisionDomain: "cursor-entity",
                decisionExpectation: {
                  meaning: "absent",
                  problems: "none",
                  status: "partial",
                },
                recognizedRelations: [],
              },
      ),
      schemaVersion: 4,
    };
    const observations = corpus.cases.map(
      (item): ChallengeObservation => ({
        ...observation(item),
        entityDecision:
          item.id === "case-0"
            ? {
                meaningLabel: "Wrong entity decision",
                meaningRelationId: "wrong:entity",
                problemCount: 0,
                reasonKinds: ["proof"],
                sourceGrounded: true,
                status: "established",
              }
            : {
                problemCount: 0,
                reasonKinds: ["uncertainty"],
                sourceGrounded: false,
                status: "partial",
              },
        formulaDecision: {
          meaningLabel: "Test law",
          meaningRelationId: "test:law",
          problemCount: 1,
          reasonKinds: ["source-conflict"],
          sourceGrounded: true,
          status: "conflicting",
        },
        recognizedRelations: [],
      }),
    );
    observations[0] = {
      ...observations[0]!,
      recognizedRelations: [
        {
          authority: "candidate",
          formulaAnchor: "selected-formula",
          relationId: "test:law",
          support: "contradicted",
        },
      ],
    };

    expect(scoreChallenge(corpus, observations).passed).toBe(48);

    observations[0] = {
      ...observations[0]!,
      relationIds: ["test:law"],
    };
    expect(scoreChallenge(corpus, observations).passed).toBe(48);

    observations[0] = {
      ...observations[0]!,
      formulaDecision: {
        ...observations[0]!.formulaDecision!,
        sourceGrounded: false,
      },
    };
    expect(scoreChallenge(corpus, observations).failures[0]).toContain(
      "source-grounded",
    );

    observations[0] = {
      ...observations[0]!,
      formulaDecision: {
        ...observations[0]!.formulaDecision!,
        sourceGrounded: true,
      },
      recognizedRelations: [
        {
          authority: "authoritative",
          formulaAnchor: "selected-formula",
          relationId: "test:law",
          support: "contradicted",
        },
      ],
    };
    expect(scoreChallenge(corpus, observations).failures[0]).toContain(
      "recognized relations",
    );
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
      ...observation(item),
      relationIds: item.outcome === "refusal" ? [] : ["unrelated"],
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

  test("scores decision, explanation, problem, and reason policy independently", () => {
    const base = cases();
    base[0] = {
      ...base[0]!,
      decisionExpectation: {
        meaning: "present",
        problems: "none",
        status: "established",
      },
    };
    const corpus = { cases: base, schemaVersion: 3 } as const;
    const observations = corpus.cases.map((item) => observation(item));
    observations[0] = {
      ...observations[0]!,
      meaningLabel: "Known relation",
      problemCount: 0,
      reasonKinds: ["proof"],
      sourceGrounded: true,
      status: "established",
      symbols: ["x"],
    };
    const scorecard = scoreChallenge(corpus, observations);
    expect(scorecard.decisions.established).toEqual({ passed: 1, total: 1 });
    expect(scorecard.explanation).toEqual({ passed: 1, total: 1 });
    expect(scorecard.problemPolicy.none).toEqual({ passed: 1, total: 1 });
    expect(scorecard.reasonIntegrity).toEqual({ passed: 1, total: 1 });
  });

  test("rejects ambiguous cursors and incomplete coverage", () => {
    const values = cases();
    values[0] = {
      ...values[0]!,
      cursor: { fileId: "main", needle: "x" },
      documents: [{ content: "$x+x$", fileId: "main", path: "main.tex" }],
    };
    expect(() => parseChallengeCorpus(source(values))).toThrow(
      "must occur exactly once",
    );
    expect(() => parseChallengeCorpus(source(cases().slice(0, 47)))).toThrow(
      "at least 48",
    );
  });

  test("requires complete semantic boundary pairs and rejects development-fixture reuse", () => {
    const values = cases();
    values[0] = { ...values[0]!, variationTags: ["boundary-pair:unpaired"] };
    expect(() => parseChallengeCorpus(source(values))).toThrow(
      "incomplete boundary pair unpaired",
    );

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
    if (
      !isRecord(item) ||
      typeof item.id !== "string" ||
      !Array.isArray(item.documents)
    ) {
      return [];
    }
    const documents = item.documents.flatMap((document) =>
      isRecord(document) &&
      typeof document.content === "string" &&
      typeof document.fileId === "string" &&
      typeof document.path === "string"
        ? [
            {
              content: document.content,
              fileId: document.fileId,
              path: document.path,
            },
          ]
        : [],
    );
    return documents.length === item.documents.length
      ? [{ documents, id: item.id }]
      : [];
  });
}

function recordFixture(
  value: unknown,
  path: string,
): Record<string, unknown> {
  if (!isRecord(value)) throw new Error(`${path}: expected an object`);
  return value;
}

function profileRecords(value: unknown): Record<string, unknown>[] {
  const root = recordFixture(value, "v4 fixture");
  if (!Array.isArray(root.profiles)) {
    throw new Error("v4 fixture: expected profiles");
  }
  return root.profiles.map((profile, index) =>
    recordFixture(profile, `v4 fixture profile ${index}`),
  );
}

function fixtureCases(value: unknown): Record<string, unknown>[] {
  const root = recordFixture(value, "v2 fixture");
  if (!Array.isArray(root.cases)) {
    throw new Error("v2 fixture: expected cases");
  }
  return root.cases.map((item, index) =>
    recordFixture(item, `v2 fixture case ${index}`),
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
