import { describe, expect, test } from "bun:test";
import {
  freshAuthoringSyntaxFactsForSelections,
  isApprovedReferenceFixturePath,
} from "./fresh-blind-evidence";

describe("fresh blind reference isolation", () => {
  test("opens only public development namespaces", () => {
    expect(
      [
        "fixtures/corpus/generated-v1.json",
        "fixtures/development/evidence-graded-hypotheses-v1.json",
        "fixtures/foundation/scientific-prose-v1.json",
        "fixtures/challenge/document-reasoning-development-v1.json",
        "fixtures/challenge/math-authoring-oracle-source-v2.json",
        "fixtures/challenge/semantic-continuity-v1.json",
      ].every(isApprovedReferenceFixturePath),
    ).toBe(true);
  });

  test("rejects every historical or prospective blind namespace before I/O", () => {
    expect(
      [
        "fixtures/challenge/document-reasoning-holdout-v1.json",
        "fixtures/challenge/document-reasoning-fresh-v028.json",
        "fixtures/challenge/document-reasoning-fresh-v037.json",
        "fixtures/receipts/v0.37.json",
        "./fixtures/challenge/document-reasoning-fresh-sentinel.json",
      ].some(isApprovedReferenceFixturePath),
    ).toBe(false);
  });

  test("extracts one exact syntax-root inventory per selected snapshot", () => {
    const markdown = {
      documents: [{
        content: "Before.\n$$\nx=y\n$$\nAfter.",
        fileId: "main-md",
        path: "main.md",
      }],
      id: "initial",
    };
    const latex = {
      documents: [{
        content: "Before.\n\\[\nu=v\n\\]\nAfter.",
        fileId: "main-tex",
        path: "main.tex",
      }],
      id: "initial",
    };
    const facts = freshAuthoringSyntaxFactsForSelections([
      { scenarioId: "markdown", snapshot: markdown },
      { scenarioId: "markdown", snapshot: markdown },
      { scenarioId: "latex", snapshot: latex },
    ]);
    expect(facts).toHaveLength(2);
    for (const [scenarioId, content, notation] of [
      ["markdown", markdown.documents[0]!.content, "\nx=y\n"],
      ["latex", latex.documents[0]!.content, "\nu=v\n"],
    ] as const) {
      const document = facts.find((item) => item.scenarioId === scenarioId)!
        .documents[0]!;
      expect(document.mathRootContentRanges).toHaveLength(1);
      const range = document.mathRootContentRanges[0]!;
      expect(content.slice(range.startOffset, range.endOffset)).toBe(notation);
    }
  });
});
