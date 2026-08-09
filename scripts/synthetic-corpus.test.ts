import { describe, expect, test } from "bun:test";
import {
  assertSyntheticFormulaResults,
  buildSyntheticFormulaFixture,
  parseSyntheticDomainCorpus,
  scoreSyntheticCorpora,
} from "./synthetic-corpus";

const corpus = parseSyntheticDomainCorpus(
  {
    schemaVersion: 1,
    domain: "example",
    cases: [
      {
        id: "recognized",
        topic: "example recognition",
        purpose: "recognition",
        language: "latex",
        source: "For a matrix, $\\det(M)$ is its determinant.",
        cursorNeedle: "\\det",
        expectedPatterns: ["determinant"],
      },
      {
        id: "coverage",
        topic: "unsupported example",
        purpose: "coverage",
        language: "markdown",
        source: "A future target is $u \\star v$.",
        cursorNeedle: "\\star",
        expectedPatterns: [],
      },
      {
        id: "refusal",
        topic: "near miss",
        purpose: "refusal",
        language: "latex",
        source: "This wrapper is intentionally unknown: $\\unknown{u}$.",
        cursorNeedle: "\\unknown",
        expectedPatterns: [],
      },
    ],
  },
  "example.json",
);

describe("synthetic corpus", () => {
  test("validates unique cursor needles and purpose contracts", () => {
    expect(() =>
      parseSyntheticDomainCorpus(
        {
          schemaVersion: 1,
          domain: "bad",
          cases: [
            {
              id: "ambiguous",
              topic: "ambiguous cursor",
              purpose: "recognition",
              language: "latex",
              source: "$x+x$",
              cursorNeedle: "x",
              expectedPatterns: [],
            },
          ],
        },
        "bad.json",
      ),
    ).toThrow("cursorNeedle must occur exactly once");

    expect(() =>
      parseSyntheticDomainCorpus(
        {
          schemaVersion: 1,
          domain: "overlap",
          cases: [
            {
              id: "domain-refusal-with-generic-fallback",
              topic: "overlapping domains",
              purpose: "refusal",
              language: "latex",
              source: "$x_{k+1}=x_k$",
              cursorNeedle: "x_{k+1}",
              expectedPatterns: ["linear-recurrence"],
            },
          ],
        },
        "overlap.json",
      ),
    ).not.toThrow();
  });

  test("builds one deterministic query per independent case", () => {
    const { expectations, fixture } = buildSyntheticFormulaFixture([corpus]);
    expect(fixture.snapshot.documents).toHaveLength(3);
    expect(fixture.queries).toHaveLength(3);
    expect(expectations.map((entry) => entry.case.id)).toEqual([
      "recognized",
      "coverage",
      "refusal",
    ]);
    expect(fixture.queries[0]?.query.offset).toBe(
      corpus.cases[0]!.source.indexOf("\\det") + 2,
    );
  });

  test("asserts exact recognitions and reports semantic coverage separately", () => {
    const { expectations } = buildSyntheticFormulaFixture([corpus]);
    const scorecards = assertSyntheticFormulaResults(
      [
        {
          value: {
            kind: "formulaRecognitions",
            recognitions: [{ patternId: "determinant" }],
          },
        },
        { value: { kind: "formulaRecognitions", recognitions: [] } },
        { value: { kind: "formulaRecognitions", recognitions: [] } },
      ],
      expectations,
    );
    expect(scorecards).toEqual([
      {
        domain: "example",
        cases: 3,
        recognition: 1,
        refusals: 1,
        coverageTargets: 1,
        supportedCoverageTargets: 0,
        semanticCoveragePercent: 50,
      },
    ]);
    expect(() =>
      assertSyntheticFormulaResults(
        [
          { value: { kind: "formulaRecognitions", recognitions: [] } },
          { value: { kind: "formulaRecognitions", recognitions: [] } },
          { value: { kind: "formulaRecognitions", recognitions: [] } },
        ],
        expectations,
      ),
    ).toThrow("expected [determinant], got []");
  });

  test("scores supported coverage targets without reclassifying their provenance", () => {
    const expectations = buildSyntheticFormulaFixture([corpus]).expectations;
    expectations[1]!.case.expectedPatterns.push("future-pattern");
    expect(scoreSyntheticCorpora(expectations)[0]).toMatchObject({
      coverageTargets: 1,
      supportedCoverageTargets: 1,
      semanticCoveragePercent: 100,
    });
  });
});
