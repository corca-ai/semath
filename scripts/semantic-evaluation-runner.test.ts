import { describe, expect, test } from "bun:test";
import { semanticEvaluationCursorOffset } from "./semantic-evaluation-runner";

describe("semantic evaluation cursor offsets", () => {
  test("treats edges as the boundaries of the complete reviewed needle", () => {
    const content = "The estimate is $s$.";
    expect(
      semanticEvaluationCursorOffset(content, {
        edge: "before",
        fileId: "main",
        needle: "estimate is $s",
      }),
    ).toBe(content.indexOf("estimate is $s"));
    expect(
      semanticEvaluationCursorOffset(content, {
        edge: "after",
        fileId: "main",
        needle: "estimate is $s",
      }),
    ).toBe(content.indexOf("estimate is $s") + "estimate is $s".length);
  });

  test("uses an explicit relative offset for probes inside a surface", () => {
    const content = "Use $\\hat y$.";
    expect(
      semanticEvaluationCursorOffset(content, {
        fileId: "main",
        needle: "\\hat y",
        offset: 5,
      }),
    ).toBe(content.indexOf("\\hat y") + 5);
  });

  test("rejects missing and non-unique needles", () => {
    expect(() =>
      semanticEvaluationCursorOffset("$x$ and $x$", {
        fileId: "main",
        needle: "x",
      }),
    ).toThrow("unique or select a valid occurrence");
    expect(() =>
      semanticEvaluationCursorOffset("$x$", {
        fileId: "main",
        needle: "y",
      }),
    ).toThrow("unique or select a valid occurrence");
  });

  test("selects a reviewed occurrence without rewriting the source", () => {
    const content = "$x$ and $x$";
    expect(
      semanticEvaluationCursorOffset(content, {
        fileId: "main",
        needle: "x",
        occurrence: 1,
        offset: 1,
      }),
    ).toBe(content.lastIndexOf("x") + 1);
  });
});
