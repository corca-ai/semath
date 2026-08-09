import { describe, expect, test } from "bun:test";
import {
  assertSyntheticProseResults,
  buildSyntheticProseFixture,
  parseSyntheticProseCorpus,
} from "./synthetic-prose-corpus";

const corpus = parseSyntheticProseCorpus(
  {
    schemaVersion: 1,
    domain: "english-prose",
    cases: [
      {
        id: "recognized",
        topic: "let definition",
        purpose: "recognition",
        language: "latex",
        annotatedSource:
          "Let $x$ denote the state vector. We later update $<<CURSOR>>x$.",
        expectedDefinitions: [
          {
            description: "the state vector",
            ruleId: "english-let-definition",
          },
        ],
      },
      {
        id: "coverage",
        topic: "unsupported definition",
        purpose: "coverage",
        language: "markdown",
        annotatedSource: "Call $z$ the latent code. We decode $<<CURSOR>>z$.",
        expectedDefinitions: [],
      },
    ],
  },
  "english.json",
);

describe("synthetic prose corpus", () => {
  test("validates exactly one annotated cursor", () => {
    expect(() =>
      parseSyntheticProseCorpus(
        {
          schemaVersion: 1,
          domain: "bad",
          cases: [
            {
              id: "missing-marker",
              topic: "invalid",
              purpose: "refusal",
              language: "latex",
              annotatedSource: "Use $x$.",
              expectedDefinitions: [],
            },
          ],
        },
        "bad.json",
      ),
    ).toThrow("must contain one cursor marker");
  });

  test("removes annotations and points hover at the following symbol", () => {
    const { fixture } = buildSyntheticProseFixture([corpus]);
    const document = fixture.snapshot.documents[0]!;
    const query = fixture.queries[0]!.query;
    expect(document.content).not.toContain("<<CURSOR>>");
    expect(document.content.slice(query.offset, query.offset + 1)).toBe("x");
  });

  test("asserts stable definition descriptions and evidence rules", () => {
    const { expectations } = buildSyntheticProseFixture([corpus]);
    expect(
      assertSyntheticProseResults(
        [
          {
            value: {
              kind: "hover",
              definitions: [
                {
                  description: "the state vector",
                  evidence: { ruleId: "english-let-definition" },
                },
              ],
            },
          },
          { value: { kind: "hover", definitions: [] } },
        ],
        expectations,
      ),
    ).toEqual({
      cases: 2,
      recognition: 1,
      refusals: 0,
      coverageTargets: 1,
      supportedCoverageTargets: 0,
      semanticCoveragePercent: 50,
    });
  });
});
