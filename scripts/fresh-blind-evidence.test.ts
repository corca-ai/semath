import { describe, expect, test } from "bun:test";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  authoredMathFingerprints,
  authoredProseShingles,
  selectFreshBlindOccurrence,
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
    const scalarFormulaStart = [...content].findIndex(
      (character) => character === "\\",
    );
    expect(document.mathRootContentRanges[0]!.startOffset).toBe(formulaStart);
    // Public source ranges are UTF-16 offsets, so the astral prefix contributes
    // two code units even though it is one Unicode scalar.
    expect(formulaStart).toBe(scalarFormulaStart + 1);
    const occurrences = document.occurrences;
    if (!occurrences) throw new Error("missing occurrence syntax facts");
    const owners = occurrences.filter((occurrence) =>
      occurrence.selectionRange.startOffset <= x &&
      x < occurrence.selectionRange.endOffset
    );
    expect(owners.map((occurrence) =>
      content.slice(occurrence.range.startOffset, occurrence.range.endOffset)
    )).toContain("\\mathbf{\\hat{x}}_i");
    expect(owners).toHaveLength(1);
  });

  test("extracts real atoms and complete-application cursor edges", () => {
    const content = "The map is $f(x)+\\sin(y)$.";
    const snapshot = {
      documents: [{ content, fileId: "main", path: "main.md" }],
      id: "initial",
    };
    const [facts] = freshAuthoringSyntaxFactsForSelections([
      { scenarioId: "application", snapshot },
    ]);
    const occurrences = facts!.documents[0]!.occurrences!;
    const f = occurrences.find((occurrence) => occurrence.surface === "f")!;
    const sin = occurrences.find((occurrence) => occurrence.surface === "sin")!;
    expect(f.range).toEqual({
      startOffset: content.indexOf("f"),
      endOffset: content.indexOf("f") + 1,
    });
    expect(f.applicationEndOffset).toBe(content.indexOf("x)") + 2);
    expect(sin.applicationEndOffset).toBe(content.indexOf("y)") + 2);
    expect(content.slice(sin.range.startOffset, sin.range.endOffset)).toBe("\\sin");
  });

  test("keeps operator focus and nested script ownership syntax-backed", () => {
    const content = "The indexed operator is $\\sum_i x_i$.";
    const snapshot = {
      documents: [{ content, fileId: "main", path: "main.md" }],
      id: "initial",
    };
    const [facts] = freshAuthoringSyntaxFactsForSelections([
      { scenarioId: "operator-focus", snapshot },
    ]);
    const occurrences = facts!.documents[0]!.occurrences!;
    const sumStart = content.indexOf("\\sum");
    const firstIndex = content.indexOf("_i", sumStart) + 1;
    const xStart = content.indexOf("x_i");
    const secondIndex = content.indexOf("i", xStart);
    const sum = selectFreshBlindOccurrence(occurrences, sumStart + 1)!;
    const indexedX = selectFreshBlindOccurrence(occurrences, xStart)!;
    const index = selectFreshBlindOccurrence(occurrences, secondIndex)!;
    expect(content.slice(sum.range.startOffset, sum.range.endOffset)).toBe("\\sum");
    expect(selectFreshBlindOccurrence(occurrences, firstIndex)?.surface).toBe("i");
    expect(content.slice(indexedX.range.startOffset, indexedX.range.endOffset)).toBe("x_i");
    expect(index.surface).toBe("i");
    expect(index.range).toEqual({
      startOffset: secondIndex,
      endOffset: secondIndex + 1,
    });

    const namedContent = "The op is $\\operatorname{sin}_i(y)$.";
    const [namedFacts] = freshAuthoringSyntaxFactsForSelections([{
      scenarioId: "named-operator-focus",
      snapshot: {
        documents: [{
          content: namedContent,
          fileId: "main",
          path: "main.md",
        }],
        id: "initial",
      },
    }]);
    const namedOccurrences = namedFacts!.documents[0]!.occurrences!;
    const nameOffset = namedContent.indexOf("sin");
    const namedIndexOffset = namedContent.indexOf("_i") + 1;
    expect(selectFreshBlindOccurrence(namedOccurrences, nameOffset)?.surface)
      .toBe("\\operatorname{sin}_i");
    expect(selectFreshBlindOccurrence(namedOccurrences, namedIndexOffset)?.surface)
      .toBe("i");
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
