import { describe, expect, test } from "bun:test";
import { buildPerformanceDocuments } from "./performance-fixtures";

describe("performance fixtures", () => {
  test.each([60, 500])(
    "builds one established high-fanout entity at %i project documents",
    (count) => {
      const document = buildPerformanceDocuments(count)[0]!;
      const occurrences = document.content.match(/\$z\$/gu) ?? [];
      expect(occurrences.length).toBe(count + 1);
      expect(document.content).toContain("Let $z$ denote");
      expect(document.queryOffset).toBeGreaterThan(document.content.indexOf("denote"));
    },
  );
});
