import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.11/domain-pack-recognition-corpus.json";
import {
  assertDomainPackResults,
  buildDomainPackFixture,
} from "./v0.11-domain-fixture.mjs";

describe("v0.11 domain fixture", () => {
  test("builds positive, authority, and unfinished checks for every entry", () => {
    const { expectations, fixture } = buildDomainPackFixture(corpus);
    expect(fixture.snapshot.documents).toHaveLength(
      Math.ceil(corpus.cases.length / 4),
    );
    expect(fixture.queries).toHaveLength(corpus.cases.length * 4);
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
    expect(() => assertDomainPackResults(results, expectations)).toThrow(
      `${entry.id}: recognition-only entry exposed formulaCompletion`,
    );
  });
});
