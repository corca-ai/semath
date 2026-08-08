import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.11/domain-pack-recognition-corpus.json";
import {
  assertDomainPackResults,
  buildDomainPackFixture,
  recognitionVariants,
} from "./v0.11-domain-fixture.mjs";

describe("v0.11 domain fixture", () => {
  test("builds five positive surfaces and five structural negatives for every entry", () => {
    const { expectations, fixture } = buildDomainPackFixture(corpus);
    expect(fixture.snapshot.documents).toHaveLength(
      corpus.cases.length * 2 + corpus.collisions.length,
    );
    expect(
      recognitionVariants(corpus.cases[0]).filter((variant) => variant.expected),
    ).toHaveLength(6);
    const caseQueries = corpus.cases.reduce(
      (total, entry) =>
        total +
        recognitionVariants(entry).reduce(
          (count, variant) => count + (variant.expected ? 3 : 1),
          0,
        ),
      0,
    );
    expect(fixture.queries).toHaveLength(
      caseQueries + corpus.collisions.length,
    );
    expect(expectations).toHaveLength(fixture.queries.length);
  });

  test("reports unsafe action authority with the owning entry", () => {
    const entry = corpus.cases[0]!;
    const { expectations } = buildDomainPackFixture({ cases: [entry] });
    const results = [
      {
        value: {
          kind: "formulaRecognitions",
          recognitions: [{ patternId: entry.expectedPattern }],
        },
      },
      { value: { kind: "formulaCompletions", completions: [{}] } },
      { value: { kind: "formulaRewrites", rewrites: [] } },
      { value: { kind: "formulaRecognitions", recognitions: [] } },
    ];
    expect(() =>
      assertDomainPackResults(results, expectations.slice(0, 4)),
    ).toThrow(
      `${entry.id}/positive-inline: recognition-only entry exposed formulaCompletion`,
    );
  });
});
