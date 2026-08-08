import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.12/action-pattern-calibration.json";
import {
  actionPatternVariants,
  assertActionPatternResults,
  buildActionPatternFixture,
} from "./v0.12-action-fixture.mjs";

describe("v0.12 action pattern fixture", () => {
  test("builds contextual positives and mutation refusals", () => {
    const { expectations, fixture } = buildActionPatternFixture(corpus);
    expect(fixture.snapshot.documents).toHaveLength(9);
    expect(actionPatternVariants(corpus.cases[0]).filter((item) => item.expected)).toHaveLength(7);
    expect(actionPatternVariants(corpus.cases[0]).filter((item) => !item.expected)).toHaveLength(6);
    expect(fixture.queries).toHaveLength(9 * 13);
    expect(expectations).toHaveLength(fixture.queries.length);
  });

  test("reports the owning pattern and surface", () => {
    const entry = corpus.cases[0]!;
    expect(() =>
      assertActionPatternResults(
        [{ value: { kind: "formulaRecognitions", recognitions: [] } }],
        [{ entry, variant: { expected: true, id: "positive-1" } }],
      ),
    ).toThrow(`${entry.id}/positive-1`);
  });
});
