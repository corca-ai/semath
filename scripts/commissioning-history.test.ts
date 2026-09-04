import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";

const history = await readFile(
  new URL("../docs/commissioning-history.md", import.meta.url),
  "utf8",
);

describe("commissioning development history", () => {
  test("accounts for every v0.40 through v0.94 attempt exactly once", () => {
    const versions = [...history.matchAll(/^\| v0\.(\d+) \|/gmu)].map(
      (match) => Number(match[1]),
    );
    expect(versions).toEqual(
      Array.from({ length: 55 }, (_, index) => index + 40),
    );
  });

  test("separates development reuse from final holdout freshness", () => {
    expect(history).toContain(
      "Failed and abandoned artifacts are development evidence",
    );
    expect(history).toContain("Freshness applies at final qualification");
    expect(history).toContain(
      "Do not key product behavior to a historical release ID",
    );
  });
});
