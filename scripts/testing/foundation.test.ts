import { describe, expect, test } from "bun:test";
import {
  parseFoundationCorpus,
  scoreFoundation,
  type FoundationObservation,
} from "./foundation";
const suiteId = "quantities-foundation";

function corpusValue() {
  return {
    cases: [{
      cursor: { edge: "after", fileId: "main", needle: "$v$" },
      documents: [{ content: "Let $v$ be velocity.", fileId: "main", path: "main.tex" }],
      expectation: { dimension: "length*time^-1", quantityKindId: "velocity", symbol: "v" },
      id: "velocity-declaration",
      variationTags: ["unit-explicit", "shape"],
    }],
    domain: "quantities-foundation",
    schemaVersion: 1,
  };
}

describe("foundation corpus", () => {
  test("parses a strict, cursor-addressable non-law case", () => {
    expect(parseFoundationCorpus(corpusValue()).cases[0]).toMatchObject({
      id: "velocity-declaration",
      expectation: { quantityKindId: "velocity", symbol: "v" },
    });
  });

  test("scores semantic output and reports missing evidence by case", () => {
    const corpus = parseFoundationCorpus(corpusValue());
    const complete: FoundationObservation = {
      assumptions: [],
      caseId: "velocity-declaration",
      conceptIds: [],
      diagnosticCodes: [],
      definitions: [],
      dimensions: ["length*time^-1"],
      quantityKindIds: ["velocity"],
      relationIds: [],
      suiteId: suiteId,
      symbols: ["v"],
      unitIds: [],
    };
    expect(scoreFoundation(suiteId, corpus, [complete])).toMatchObject({
      failures: [],
      passed: 1,
    });

    const incomplete = { ...complete, quantityKindIds: [] };
    expect(scoreFoundation(suiteId, corpus, [incomplete]).failures).toContain(
      "quantities-foundation/velocity-declaration: missing quantity velocity",
    );
  });

  test("refuses schema drift and ambiguous cursor needles", () => {
    expect(() =>
      parseFoundationCorpus({ ...corpusValue(), rollout: "live" }),
    ).toThrow("unknown fields: rollout");
    const duplicateNeedle = corpusValue();
    duplicateNeedle.cases[0]!.documents[0]!.content = "$v$ and $v$";
    expect(() => parseFoundationCorpus(duplicateNeedle)).toThrow(
      "must occur exactly once",
    );
  });
});
