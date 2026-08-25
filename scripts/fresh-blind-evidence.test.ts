import { describe, expect, test } from "bun:test";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  authoredMathFingerprints,
  authoredProseShingles,
  spentHoldoutProfile,
} from "../packages/evaluation/src/index";
import {
  freshAuthoringSyntaxFactsForSelections,
  isApprovedReferenceFixturePath,
  sha256,
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

  test("extracts neutral composite owners without collapsing to their nucleus", () => {
    const content = "😀 The value is $\\mathbf{\\hat{x}}_i=0$.";
    const snapshot = {
      documents: [{ content, fileId: "main", path: "main.tex" }],
      id: "initial",
    };
    const [facts] = freshAuthoringSyntaxFactsForSelections([
      { scenarioId: "composite", snapshot },
    ]);
    const document = facts!.documents[0]!;
    const x = content.indexOf("x");
    const formulaStart = content.indexOf("\\mathbf");
    expect(document.mathRootContentRanges[0]!.startOffset).toBe(formulaStart);
    expect(formulaStart).toBeGreaterThan(
      [...content].findIndex((character) => character === "\\"),
    );
    const compositeOccurrences = document.compositeOccurrences;
    if (!compositeOccurrences) throw new Error("missing composite syntax facts");
    const owners = compositeOccurrences.filter((occurrence) =>
      occurrence.selectionRange.startOffset <= x &&
      x < occurrence.selectionRange.endOffset
    );
    expect(owners.map((occurrence) =>
      content.slice(occurrence.range.startOffset, occurrence.range.endOffset)
    )).toContain("\\mathbf{\\hat{x}}_i");
  });

  test("versions the exact document, math, and prose lineage algorithms", () => {
    const content =
      "The calibrated relation follows the reviewed balance.\n\\[\na+b=c\n\\]\n";
    const service = new LatexSyntaxService();
    service.reset({
      documents: [
        {
          content,
          documentVersion: 1,
          fileId: "golden",
          language: "latex",
          path: "golden.tex",
        },
      ],
    });
    const syntax = service.getFile("golden");
    if (!syntax) throw new Error("missing golden lineage syntax");
    expect(
      spentHoldoutProfile(
        "golden",
        [sha256(content)],
        {
          id: "golden",
          mathFingerprints: authoredMathFingerprints(syntax),
          proseShingles: authoredProseShingles(content, syntax),
        },
        sha256,
      ),
    ).toEqual({
      documentSha256: [
        "6248391d046e9a9515135aa8aad41765f862cf08c0d85f46f93d4507b1a5d231",
      ],
      id: "golden",
      mathFingerprintSha256: [
        "ef3c1507993477a8b3c835da762f890cda1e42d5e5fa21f3a3f3c78864d0f8e3",
      ],
      proseShingleSha256: [
        "3c4aaa44f2b26cfaa92ab5f9b1b7fad5b8a3c2af91306774279ec7469f0f6c7f",
        "884b72842636eeeee44e09cab625db17a96c27d9791c45f62900ce70f952ae10",
        "f766f0011895d872ccd10d420eb45380cb4cec8299d21609c1dd6dc9ecd233ae",
      ],
    });
  });
});
