import { describe, expect, test } from "bun:test";
import type { Corpus, CorpusCase } from "./model";
import {
  findCorpusDuplicates,
  generateGlobalRefusalCorpus,
  generateLawDiversityCorpus,
  parseSyntheticDiversitySpec,
  validateFixtureSource,
} from "./synthetic";

const profile = {
  batch: "source-batch",
  mutationFamily: "affirmative",
  projectTopology: "single-document",
  proseFamily: "let-singular",
  semanticSkeleton: "product-equality",
  syntaxStructure: "inline-math",
} as const;

const positive: CorpusCase = {
  cursor: { fileId: "main", needle: "F=ma" },
  diversity: profile,
  documents: [{
    content: "Let $F$ be force, $m$ mass, and $a$ acceleration. $F=ma$",
    fileId: "main",
    path: "main.md",
  }],
  expectation: "established",
  expectedRoles: { acceleration: "a", force: "F", mass: "m" },
  id: "positive-source",
  lawId: "newton-second-law",
  variationTags: ["english-declarations", "conventional-notation"],
};

const refusal: CorpusCase = {
  cursor: { fileId: "main", needle: "F=m+a" },
  diversity: { ...profile, mutationFamily: "wrong-operator" },
  documents: [{
    content: "Let $F$ be force, $m$ mass, and $a$ acceleration. $F=m+a$",
    fileId: "main",
    path: "main.md",
  }],
  expectation: "refused",
  id: "refusal-source",
  lawId: "newton-second-law",
  refusalCategory: "wrong-operator",
  variationTags: ["wrong-operator"],
};

function spec() {
  return parseSyntheticDiversitySpec({
    batches: ["cardinality", "prose", "presentation", "scope"],
    casesPerLaw: 16,
    globalCases: 8,
    mutationFamilies: [
      "affirmative",
      "wrong-operator",
      "wrong-sign",
      "wrong-factor",
      "missing-term",
      "wrong-role",
    ],
    positiveCasesPerLaw: 10,
    projectTopologies: [
      "single-document",
      "context-file",
      "definitions-file",
      "nested-section",
    ],
    proseFamilies: [
      "let-singular",
      "let-paired",
      "let-series",
      "respectively",
      "where-clause",
      "we-write",
      "symbol-list",
      "role-first",
    ],
    schemaVersion: 1,
    syntaxStructures: [
      "inline-math",
      "display-math",
      "equation-environment",
      "aligned-environment",
    ],
  });
}

describe("synthetic corpus planning", () => {
  test("is deterministic, balanced, and leaves source cases immutable", () => {
    const source = [positive, refusal];
    const first = generateLawDiversityCorpus("mechanics-diversity", source, spec());
    const second = generateLawDiversityCorpus("mechanics-diversity", source, spec());
    expect(first).toEqual(second);
    expect(first.cases).toHaveLength(16);
    expect(first.cases.filter((item) => item.expectation === "established")).toHaveLength(10);
    expect(new Set(first.cases.map((item) => item.diversity.proseFamily)).size).toBe(8);
    expect(first.cases.some((item) => item.cursor.edge === "after")).toBe(true);
    expect(positive.documents[0]!.content).not.toContain("presentation-only");
  });

  test("creates an independent law-free unknown and collision suite", () => {
    const corpus = generateGlobalRefusalCorpus("global-adversarial", spec());
    expect(corpus.cases).toHaveLength(8);
    expect(corpus.cases.every((item) => !("lawId" in item))).toBe(true);
    expect(new Set(corpus.cases.flatMap((item) =>
      "refusalCategory" in item ? [item.refusalCategory] : [],
    ))).toEqual(
      new Set(["cross-pack-symbol-collision", "unknown-domain-relation"]),
    );
  });

  test("rejects normalized duplicates and malformed source fixtures", () => {
    const duplicate = { ...positive, id: "duplicate-source" };
    const corpus: Corpus = {
      cases: [positive, duplicate],
      domain: "duplicate-test",
      schemaVersion: 2,
    };
    expect(findCorpusDuplicates([corpus])).toContain(
      "duplicate-test/duplicate-source: duplicates duplicate-test/positive-source",
    );
    expect(validateFixtureSource("$x")).toContain(
      "has an unmatched math dollar delimiter",
    );
    expect(validateFixtureSource("\\begin{aligned}x\\end{equation}")).toContain(
      "has mismatched TeX environments",
    );
  });
});
