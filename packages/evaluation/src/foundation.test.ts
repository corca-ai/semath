import { describe, expect, test } from "bun:test";
import {
  parseFoundationCorpus,
  scoreFoundation,
  type FoundationObservation,
} from "./foundation";
import type { FoundationSuiteConfig } from "./model";

const suite: FoundationSuiteConfig = {
  capability: "shape-quantity-unit",
  id: "quantities-foundation",
  minimumCases: 1,
  packId: "quantities-units",
  path: "foundation/quantities-units.json",
  requiredDimensions: ["notation", "constraints"],
  tier: "evaluated",
};

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
    expect(parseFoundationCorpus(corpusValue(), suite).cases[0]).toMatchObject({
      id: "velocity-declaration",
      expectation: { quantityKindId: "velocity", symbol: "v" },
    });
  });

  test("scores semantic output and reports missing evidence by case", () => {
    const corpus = parseFoundationCorpus(corpusValue(), suite);
    const complete: FoundationObservation = {
      caseId: "velocity-declaration",
      conceptIds: [],
      diagnosticCodes: [],
      dimensions: ["length*time^-1"],
      quantityKindIds: ["velocity"],
      relationIds: [],
      suiteId: suite.id,
      symbols: ["v"],
      unitIds: [],
    };
    const dimensions = new Map([
      ["notation", ["unit-explicit"]],
      ["constraints", ["shape"]],
    ]);
    expect(scoreFoundation(suite, corpus, [complete], dimensions)).toMatchObject({
      failures: [],
      passed: 1,
    });

    const incomplete = { ...complete, quantityKindIds: [] };
    expect(scoreFoundation(suite, corpus, [incomplete], dimensions).failures).toContain(
      "quantities-foundation/velocity-declaration: missing quantity velocity",
    );
  });

  test("refuses schema drift and ambiguous cursor needles", () => {
    expect(() =>
      parseFoundationCorpus({ ...corpusValue(), rollout: "live" }, suite),
    ).toThrow("unknown fields: rollout");
    const duplicateNeedle = corpusValue();
    duplicateNeedle.cases[0]!.documents[0]!.content = "$v$ and $v$";
    expect(() => parseFoundationCorpus(duplicateNeedle, suite)).toThrow(
      "must occur exactly once",
    );
  });
});
