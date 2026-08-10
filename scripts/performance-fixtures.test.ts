import { describe, expect, test } from "bun:test";
import {
  buildPerformanceDocuments,
  editPerformanceDocument,
  PERFORMANCE_FIXTURE_FAMILIES,
} from "./performance-fixtures";

describe("full-path performance fixtures", () => {
  test("cover every declared notation family deterministically", () => {
    const first = buildPerformanceDocuments(PERFORMANCE_FIXTURE_FAMILIES.length * 2);
    const second = buildPerformanceDocuments(PERFORMANCE_FIXTURE_FAMILIES.length * 2);

    expect(first).toEqual(second);
    expect(new Set(first.map((document) => document.family))).toEqual(
      new Set(PERFORMANCE_FIXTURE_FAMILIES),
    );
    for (const document of first) {
      expect(document.content[document.queryOffset]).toBe("p");
      expect(document.content).toContain("$");
    }
  });

  test("changes one leaf without moving its query target", () => {
    const source = buildPerformanceDocuments(1)[0]!;
    const edited = editPerformanceDocument(source, 3);

    expect(edited.fileId).toBe(source.fileId);
    expect(edited.documentVersion).toBe(2);
    expect(edited.queryOffset).toBe(source.queryOffset);
    expect(edited.content.slice(0, source.content.length)).toBe(source.content);
  });
});
