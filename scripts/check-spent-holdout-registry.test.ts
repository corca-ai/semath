import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";
import { parseSpentHoldoutRegistry } from "../packages/evaluation/src/index";

describe("repository spent holdout registry", () => {
  test("contains every terminal semantic execution through v0.43", async () => {
    const registry = parseSpentHoldoutRegistry(
      JSON.parse(
        await readFile(
          "fixtures/challenge/spent-holdout-registry-v1.json",
          "utf8",
        ),
      ) as unknown,
    );
    expect(registry.entries.map((entry) => entry.lineage.releaseId)).toEqual([
      "v0.38",
      "v0.39",
      "v0.40",
      "v0.41",
      "v0.42",
      "v0.43",
    ]);
  });
});
