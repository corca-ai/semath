import { describe, expect, test } from "bun:test";
import corpus from "../fixtures/v0.14/scientific-foundation.json";
import {
  assertScientificResults,
  buildScientificFixture,
  type ScientificCorpus,
} from "./v0.14-scientific-fixture";

describe("v0.14 scientific fixture", () => {
  test("builds one deterministic query for every semantic target", () => {
    const built = buildScientificFixture(corpus as ScientificCorpus);
    expect(built.fixture.queries).toHaveLength(corpus.targets.length);
    expect(built.expectations.map((target) => target.id)).toEqual(
      corpus.targets.map((target) => target.id),
    );
  });

  test("rejects a missing relation in a pure result assertion", () => {
    expect(() =>
      assertScientificResults(
        [
          {
            protocolVersion: 1,
            epoch: "test",
            inventoryVersion: 1,
            documentVersion: 1,
            analysisGeneration: 1,
            value: {
              kind: "semanticContext",
              context: {
                claims: [],
                concepts: [],
                quantities: [],
                relations: [],
                truncated: false,
              },
            },
          },
        ],
        [
          {
            id: "relation",
            fileId: "main",
            kind: "semanticContext",
            needle: "x",
            expectedRelation: "example:law",
          },
        ],
      ),
    ).toThrow("missing relation example:law");
  });
});
