import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.12/realistic-mixed-project.json";
import {
  assertRealisticProjectResults,
  buildRealisticProjectFixture,
} from "./v0.12-realistic-project-fixture.mjs";

describe("v0.12 realistic project fixture", () => {
  test("builds nested includes and mixed-domain queries deterministically", () => {
    const { expectations, fixture } = buildRealisticProjectFixture(corpus);
    expect(fixture.snapshot.documents).toHaveLength(7);
    expect(
      fixture.snapshot.documents.reduce(
        (count, document) => count + document.includes.length,
        0,
      ),
    ).toBe(5);
    expect(fixture.queries).toHaveLength(corpus.targets.length);
    expect(expectations).toEqual(corpus.targets);
  });

  test("reports a divergent target by its scenario id", () => {
    expect(() =>
      assertRealisticProjectResults(
        [{ value: { kind: "formulaRecognitions", recognitions: [] } }],
        [corpus.targets[0]!],
      ),
    ).toThrow("linear-inspection");
  });
});
