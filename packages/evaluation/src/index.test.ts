import { describe, expect, test } from "bun:test";
import {
  checkPackConformance,
  type Corpus,
  type CorpusCase,
  type EstablishedCorpusCase,
  type LawRefusalCorpusCase,
  type CaseObservation,
  parseCorpus,
  parseQualityManifest,
  planMetamorphicCases,
  evidenceIsSourceLinked,
  normalizeSymbol,
  rolesMatch,
  scoreQuality,
  summarizePack,
} from "./index";

describe("observation helpers", () => {
  test("matches canonical roles through either relation roles or concept ids", () => {
    expect(
      rolesMatch(
        [
          { role: "left", conceptId: "mechanics:force", symbol: "\\mathbf{F}" },
          { role: "mass", conceptId: "quantities-units:mass", symbol: "m" },
        ],
        { force: "F", mass: "m" },
        undefined,
      ),
    ).toBe(true);
  });

  test("matches variadic roles as a multiset without depending on order", () => {
    expect(
      rolesMatch(
        [
          { role: "branch-current", symbol: "i_2" },
          { role: "branch-current", symbol: "i_1" },
        ],
        { first: "i_1", second: "i_2" },
        undefined,
      ),
    ).toBe(true);
  });

  test("normalizes macros and presentation commands without mutating input", () => {
    const macros = [{ definition: "x_m", name: "\\state" }] as const;
    expect(normalizeSymbol("\\mathbf{\\state}", macros)).toBe("x_m");
    expect(macros).toEqual([{ definition: "x_m", name: "\\state" }]);
  });

  test("requires every evidence item and a condition to be source-linked", () => {
    expect(evidenceIsSourceLinked([{ sourceRanges: [{}] }], [{}])).toBe(true);
    expect(evidenceIsSourceLinked([{ sourceRanges: [] }], [{}])).toBe(false);
    expect(evidenceIsSourceLinked([{ sourceRanges: [{}] }], [])).toBe(false);
  });
});

function manifestValue() {
  const unsupported = { maturity: "unsupported", suiteIds: [] };
  return {
    schemaVersion: 3,
    thresholds: {
      evidenceIntegrity: 100,
      lawPrecision: 99,
      lawRecall: 95,
      refusalPreservation: 100,
      roleAccuracy: 100,
    },
    dimensions: [
      { id: "notation", tags: ["renamed"] },
      { id: "prose", tags: ["declared"] },
      { id: "roles", tags: ["role-conflict"] },
      { id: "constraints", tags: ["shape"] },
      { id: "project", tags: ["multi-file"] },
      { id: "mutation", tags: ["wrong-operator"] },
    ],
    metamorphic: {
      casesPerLaw: 1,
      transforms: ["neutral-prose", "trailing-comment", "document-order"],
    },
    packs: [
      {
        capabilities: {
          "concept-vocabulary": { maturity: "evaluated", suiteIds: [] },
          "declarations-roles": unsupported,
          "shape-quantity-unit": unsupported,
          "law-recognition": { maturity: "evaluated", suiteIds: ["mechanics"] },
          "diagnostics-refusal": unsupported,
          "project-macro": unsupported,
          "navigation-explanation": unsupported,
        },
        packId: "classical-mechanics",
      },
      {
        capabilities: {
          "concept-vocabulary": { maturity: "evaluated", suiteIds: [] },
          "declarations-roles": unsupported,
          "shape-quantity-unit": unsupported,
          "law-recognition": unsupported,
          "diagnostics-refusal": unsupported,
          "project-macro": unsupported,
          "navigation-explanation": unsupported,
        },
        packId: "quantities-units",
      },
    ],
    foundationSuites: [],
    suites: [
      {
        id: "mechanics",
        kind: "law",
        minimumPositiveCasesPerLaw: 30,
        minimumRefusalCasesPerLaw: 20,
        packId: "classical-mechanics",
        path: "mechanics.json",
        requiredDimensions: [
          "notation",
          "prose",
          "roles",
          "constraints",
          "project",
          "mutation",
        ],
        requiredDiversity: {
          maximumProfileShare: 1,
          minimumDistinct: {
            mutationFamily: 1,
            projectTopology: 1,
            proseFamily: 1,
            semanticSkeleton: 1,
            syntaxStructure: 1,
          },
        },
        tier: "evaluated",
      },
    ],
  };
}

const diversity = {
  batch: "test-batch",
  mutationFamily: "affirmative",
  projectTopology: "single-document",
  proseFamily: "let-singular",
  semanticSkeleton: "product-equality",
  syntaxStructure: "inline-math",
} as const;

function corpusCase(
  overrides: Partial<EstablishedCorpusCase> = {},
): EstablishedCorpusCase {
  return {
    cursor: { fileId: "main", needle: "F=ma" },
    documents: [
      {
        content: "Let F be force, m mass, and a acceleration. $F=ma$",
        fileId: "main",
        path: "main.md",
      },
    ],
    diversity,
    expectation: "established",
    expectedRoles: { force: "F", mass: "m" },
    id: "positive",
    lawId: "newton-second-law",
    variationTags: ["renamed", "declared"],
    ...overrides,
  };
}

function refusalCase(
  overrides: Partial<LawRefusalCorpusCase> = {},
): LawRefusalCorpusCase {
  const { expectedRoles: _expectedRoles, ...positive } = corpusCase();
  return {
    ...positive,
    diversity: { ...diversity, mutationFamily: "wrong-operator" },
    expectation: "refused",
    id: "negative",
    refusalCategory: "wrong-operator",
    variationTags: ["wrong-operator"],
    ...overrides,
  };
}

function corpus(cases: readonly CorpusCase[]): Corpus {
  return { cases, domain: "mechanics", schemaVersion: 2 };
}

function passingMetamorphicObservations(
  manifest: ReturnType<typeof parseQualityManifest>,
  corpora: ReadonlyMap<string, Corpus>,
): CaseObservation[] {
  return planMetamorphicCases(manifest, corpora).map((planned) => {
    const established = planned.case.expectation === "established";
    return {
      caseId: planned.case.id,
      evidenceIntegrity: established,
      establishedLawIds: established && "lawId" in planned.case
        ? [planned.case.lawId]
        : [],
      generatedFrom: {
        caseId: planned.sourceCaseId,
        transform: planned.transform,
      },
      rolesCorrect: established,
      status: established ? "established" : "unsupported",
      suiteId: planned.suiteId,
      targetPresent: established,
    };
  });
}

describe("quality manifest", () => {
  test("parses capability maturity, suite ownership, dimensions, and thresholds", () => {
    const manifest = parseQualityManifest(manifestValue());
    expect(manifest.suites[0]).toMatchObject({
      id: "mechanics",
      packId: "classical-mechanics",
      tier: "evaluated",
    });
    expect(manifest.packs.map((pack) => pack.capabilities["law-recognition"].maturity)).toEqual([
      "evaluated",
      "unsupported",
    ]);
  });

  test("rejects unsafe fixture paths and unknown policy fields", () => {
    const unsafe = manifestValue();
    unsafe.suites[0]!.path = "../mechanics.json";
    expect(() => parseQualityManifest(unsafe)).toThrow("safe relative JSON path");

    const unknown = { ...manifestValue(), rollout: "live" };
    expect(() => parseQualityManifest(unknown)).toThrow("unknown fields: rollout");
  });

  test("rejects duplicate identities and unknown dimensions before I/O", () => {
    const duplicate = manifestValue();
    duplicate.suites.push({ ...duplicate.suites[0]! });
    expect(() => parseQualityManifest(duplicate)).toThrow("duplicate value mechanics");

    const unknownDimension = manifestValue();
    unknownDimension.suites[0]!.requiredDimensions.push("frames");
    expect(() => parseQualityManifest(unknownDimension)).toThrow(
      "unknown dimension frames",
    );
  });
});

describe("corpus contract", () => {
  const suite = parseQualityManifest(manifestValue()).suites[0]!;

  test("accepts source-linked positive and categorized refusal cases", () => {
    const parsed = parseCorpus(
      {
        cases: [
          corpusCase(),
          refusalCase(),
        ],
        domain: "mechanics",
        schemaVersion: 2,
      },
      suite,
    );
    expect(parsed.cases).toHaveLength(2);
  });

  test("rejects an unowned domain and uncategorized refusal", () => {
    expect(() =>
      parseCorpus({ cases: [corpusCase()], domain: "other", schemaVersion: 2 }, suite),
    ).toThrow("must equal suite id mechanics");
    expect(() =>
      parseCorpus(
        {
          cases: [
            {
              ...corpusCase(),
              expectation: "refused",
              expectedRoles: undefined,
            },
          ],
          domain: "mechanics",
          schemaVersion: 2,
        },
        suite,
      ),
    ).toThrow("refused cases require a category");
  });

  test("rejects ambiguous cursors, unknown main files, and legacy macro fields", () => {
    const repeated = corpusCase({
      documents: [
        {
          content: "$F=ma$ and again $F=ma$",
          fileId: "main",
          path: "main.tex",
        },
      ],
    });
    expect(() =>
      parseCorpus(
        { cases: [repeated], domain: "mechanics", schemaVersion: 2 },
        suite,
      ),
    ).toThrow("must occur exactly once; found 2");

    expect(() =>
      parseCorpus(
        {
          cases: [{ ...corpusCase(), mainFileId: "missing" }],
          domain: "mechanics",
          schemaVersion: 2,
        },
        suite,
      ),
    ).toThrow("unknown document missing");

    expect(() =>
      parseCorpus(
        {
          cases: [{ ...corpusCase(), macros: [{ body: "F", name: "\\force" }] }],
          domain: "mechanics",
          schemaVersion: 2,
        },
        suite,
      ),
    ).toThrow("unknown fields: body");
  });
});

describe("metamorphic planning", () => {
  test("is deterministic, bounded per law, and does not mutate authored cases", () => {
    const manifest = parseQualityManifest(manifestValue());
    const source = corpusCase({
      documents: [
        ...corpusCase().documents,
        { content: "Definitions", fileId: "definitions", path: "definitions.md" },
      ],
    });
    const corpora = new Map([["mechanics", corpus([source])]]);
    const first = planMetamorphicCases(manifest, corpora);
    const second = planMetamorphicCases(manifest, corpora);
    expect(first).toEqual(second);
    expect(first.map((item) => item.transform)).toEqual([
      "neutral-prose",
      "trailing-comment",
      "document-order",
    ]);
    expect(source.documents[0]!.content).not.toContain("Context note");
    expect(first[2]!.case.documents.map((document) => document.fileId)).toEqual([
      "definitions",
      "main",
    ]);
  });

  test("skips a meaningless document-order variant for one-file cases", () => {
    const manifest = parseQualityManifest(manifestValue());
    const planned = planMetamorphicCases(
      manifest,
      new Map([["mechanics", corpus([corpusCase()])]]),
    );
    expect(planned.map((item) => item.transform)).toEqual([
      "neutral-prose",
      "trailing-comment",
    ]);
  });
});

describe("multidimensional scorecard", () => {
  test("keeps law, role, evidence, refusal, variation, and metamorphic signals separate", () => {
    const value = manifestValue();
    value.suites[0]!.minimumPositiveCasesPerLaw = 1;
    value.suites[0]!.minimumRefusalCasesPerLaw = 1;
    value.suites[0]!.requiredDimensions = ["notation", "mutation"];
    const manifest = parseQualityManifest(value);
    const positive = corpusCase({ variationTags: ["renamed"] });
    const negative = refusalCase();
    const corpora = new Map([["mechanics", corpus([positive, negative])]]);
    const observations: CaseObservation[] = [
      {
        caseId: positive.id,
        evidenceIntegrity: true,
        establishedLawIds: [positive.lawId],
        rolesCorrect: true,
        status: "established",
        suiteId: "mechanics",
        targetPresent: true,
      },
      {
        caseId: negative.id,
        evidenceIntegrity: false,
        establishedLawIds: [],
        rolesCorrect: false,
        status: "unsupported",
        suiteId: "mechanics",
        targetPresent: false,
      },
      ...passingMetamorphicObservations(manifest, corpora),
    ];
    const scorecard = scoreQuality(
      manifest,
      corpora,
      observations,
    );
    expect(scorecard.failures).toEqual([]);
    expect(scorecard.laws[0]).toMatchObject({
      precision: { percent: 100 },
      recall: { percent: 100 },
      refusalPreservation: { percent: 100 },
      roleAccuracy: { percent: 100 },
    });
    expect(scorecard.coverage.map((score) => score.dimension)).toEqual([
      "mutation",
      "notation",
    ]);
    expect(scorecard.metamorphic.percent).toBe(100);
  });

  test("a broad suite cannot hide one weak law or broken evidence", () => {
    const value = manifestValue();
    value.suites[0]!.minimumPositiveCasesPerLaw = 1;
    value.suites[0]!.minimumRefusalCasesPerLaw = 1;
    value.suites[0]!.requiredDimensions = [];
    const manifest = parseQualityManifest(value);
    const positive = corpusCase();
    const negative = refusalCase();
    const scorecard = scoreQuality(
      manifest,
      new Map([["mechanics", corpus([positive, negative])]]),
      [
        {
          caseId: positive.id,
          evidenceIntegrity: false,
          establishedLawIds: [positive.lawId],
          rolesCorrect: true,
          status: "established",
          suiteId: "mechanics",
          targetPresent: true,
        },
        {
          caseId: negative.id,
          evidenceIntegrity: false,
          establishedLawIds: [],
          rolesCorrect: false,
          status: "unsupported",
          suiteId: "mechanics",
          targetPresent: false,
        },
      ],
    );
    expect(scorecard.failures).toContain(
      "mechanics/newton-second-law: evidenceIntegrity 0.0% is below 100%",
    );
  });
});

describe("pack conformance", () => {
  test("accepts explicit evaluated and vocabulary-only support", () => {
    const manifest = parseQualityManifest(manifestValue());
    const cases = [
      ...Array.from({ length: 30 }, (_, index) =>
        corpusCase({ id: `positive-${index}` }),
      ),
      ...Array.from({ length: 20 }, (_, index) =>
        refusalCase({ id: `negative-${index}` }),
      ),
    ];
    const report = checkPackConformance(
      manifest,
      [
        catalogEntry("classical-mechanics", ["newton-second-law"]),
        catalogEntry("quantities-units", []),
      ],
      new Map([["mechanics", corpus(cases)]]),
    );
    expect(report.failures).toEqual([]);
    expect(report.packs).toMatchObject([
      { authoredCases: 50, coveredLaws: 1, laws: 1, summary: "evaluated" },
      { authoredCases: 0, coveredLaws: 0, laws: 0, summary: "vocabulary-only" },
    ]);
  });

  test("reports unowned laws, dishonest capabilities, and unknown corpus targets", () => {
    const value = manifestValue();
    value.packs[1]!.capabilities["law-recognition"] = {
      maturity: "probe",
      suiteIds: ["mechanics"],
    };
    const manifest = parseQualityManifest(value);
    const report = checkPackConformance(
      manifest,
      [
        catalogEntry("classical-mechanics", [
          "newton-second-law",
          "kinetic-energy-definition",
        ]),
        catalogEntry("quantities-units", []),
      ],
      new Map([["mechanics", corpus([corpusCase({ lawId: "unknown-law" })])]]),
    );
    expect(report.failures).toEqual(
      expect.arrayContaining([
        "classical-mechanics/newton-second-law: no corpus coverage",
        "classical-mechanics/kinetic-energy-definition: no corpus coverage",
        "mechanics: corpus targets unknown law unknown-law",
        "quantities-units: probe law capability contains no laws",
      ]),
    );
  });

  test("summarizes only the Rust-owned pack identity and laws", () => {
    expect(
      summarizePack(
        {
          laws: [{ id: "ohm-law" }],
          packId: "circuits",
          schemaVersion: 4,
        },
        "circuits.json",
      ),
    ).toEqual({
      activationRules: 0,
      concepts: 0,
      lawIds: ["ohm-law"],
      operators: 0,
      packId: "circuits",
      quantityKinds: 0,
      roles: 0,
      units: 0,
    });
  });
});

function catalogEntry(packId: string, lawIds: readonly string[]) {
  return {
    activationRules: 1,
    concepts: 1,
    lawIds,
    operators: 0,
    packId,
    quantityKinds: 0,
    roles: 0,
    units: 0,
  };
}
