import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.12/action-pattern-calibration.json";
import {
  assertActionPatternResults,
  buildActionPatternFixture,
} from "./v0.12-action-fixture.mjs";

describe("v0.12 action pattern fixture", () => {
  test("builds five positive surfaces and five structural negatives", () => {
    const { expectations, fixture } = buildActionPatternFixture(corpus);
    expect(fixture.snapshot.documents).toHaveLength(9);
    expect(fixture.queries).toHaveLength(9 * 11);
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
