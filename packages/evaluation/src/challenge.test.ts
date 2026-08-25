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

interface V4TestProfile {
  caseId: string;
  decision: { meaning: string; problems: string; status: string };
  decisionDomain: string;
  recognizedRelations: {
    authority: string;
    formulaAnchor: string;
    relationId: string;
    support: string;
  }[];
  runtimeCaseId?: string;
}

function v4Profiles(value: readonly ChallengeCase[]): {
  baseSchemaVersion: number;
  profiles: V4TestProfile[];
  schemaVersion: number;
} {
  return {
    baseSchemaVersion: 3,
    profiles: value.map((item) => ({
      caseId: item.id,
      decision: {
        meaning: "absent",
        problems: "none",
        status: "partial",
      },
      decisionDomain: "cursor-entity",
      recognizedRelations: [],
    })),
    schemaVersion: 4,
  };
}

function v3Profiles(value: readonly ChallengeCase[]) {
  const shapes = [
    "distant-prose",
    "macro-neighbor",
    "malformed-neighbor",
    "multi-equation",
    "project-neighbor",
    "sectioned",
  ] as const;
  return {
    baseSchemaVersion: 2,
    profiles: value.map((item, index) => ({
      caseId: item.id,
      decision: {
        meaning: "absent",
        problems: index === 0 ? "source-conflict" : "none",
        status: [
          "conflicting",
          "established",
          "ambiguous",
          "unsupported",
          "partial",
        ][index % 5],
      },
      documentShape: shapes[index % shapes.length],
    })),
    schemaVersion: 3,
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
      JSON.parse(baseText),
      JSON.parse(v3Text),
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

  test("rejects incomplete, domain-mixed, and non-exact v4 policy", () => {
    const base = source(cases());
    const v3 = v3Profiles(cases());
    const complete = v4Profiles(cases());
    const missingDomain = structuredClone(complete);
    delete (missingDomain.profiles[0] as { decisionDomain?: string })
      .decisionDomain;
    expect(() => parseChallengeV4(base, v3, missingDomain)).toThrow(
      "unknown or missing field decisionDomain",
    );

    const wrongAnchor = structuredClone(complete);
    wrongAnchor.profiles[0]!.recognizedRelations = [
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
    invalidSupport.profiles[0]!.recognizedRelations = [
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
    extra.profiles[0] = {
      ...extra.profiles[0]!,
      runtimeCaseId: "case-0",
    };
    expect(() => parseChallengeV4(base, v3, extra)).toThrow(
      "unknown field runtimeCaseId",
    );
  });

  test("scores v4 decisions in the declared domain and keeps relation authority exact", () => {
    const baseCases = cases();
    baseCases[0] = {
      ...baseCases[0]!,
      expectation: { relationId: "test:law", symbol: "x" },
    };
    const base = source(baseCases);
    const v3 = v3Profiles(baseCases);
    const v4 = v4Profiles(baseCases);
    v4.profiles[0] = {
      caseId: "case-0",
      decision: {
        meaning: "present",
        problems: "source-conflict",
        status: "conflicting",
      },
      decisionDomain: "selected-formula",
      recognizedRelations: [
        {
          authority: "candidate",
          formulaAnchor: "selected-formula",
          relationId: "test:law",
          support: "contradicted",
        },
      ],
    };
    const corpus = parseChallengeV4(base, v3, v4);
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
    expect(scoreChallenge(corpus, observations).failures[0]).toContain(
      "recognized relations",
    );

    observations[0] = {
      ...observations[0]!,
      formulaDecision: {
        ...observations[0]!.formulaDecision!,
        sourceGrounded: false,
      },
      relationIds: [],
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
