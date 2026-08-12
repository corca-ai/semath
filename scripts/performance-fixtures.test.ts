import { describe, expect, test } from "bun:test";
import {
  buildPerformanceDocuments,
  performanceEntityFanout,
} from "./performance-fixtures";

describe("performance fixtures", () => {
  test.each([60, 500])(
    "builds one established high-fanout entity at %i project documents",
    (count) => {
      const documents = buildPerformanceDocuments(count);
      const document = documents[0]!;
      const occurrences = documents.flatMap(
        (candidate) => candidate.content.match(/\$z\$/gu) ?? [],
      );
      expect(occurrences.length).toBe(performanceEntityFanout(count) + 1);
      expect(document.content).toContain("Let $z$ denote");
      expect(document.queryOffset).toBeGreaterThan(document.content.indexOf("denote"));
    },
  );
});
