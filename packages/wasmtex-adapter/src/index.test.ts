import { describe, expect, test } from "bun:test";
import { LatexSyntaxService } from "wasmtex/syntax";
import { adaptWasmtexDocument, createProjectSnapshot } from "./index";

describe("wasmtex adapter", () => {
  test("rejects an incompatible syntax schema before crossing the ABI", () => {
    const syntax = new LatexSyntaxService().upsert({
      fileId: "schema",
      path: "schema.tex",
      content: "$x$",
      documentVersion: 1,
      language: "latex",
    });
    expect(() =>
      adaptWasmtexDocument({
        content: "$x$",
        language: "latex",
        syntax: { ...syntax, schemaVersion: 3 } as unknown as typeof syntax,
      }),
    ).toThrow("expected 7");
  });

  test("keeps wasmtex UTF-16 ranges without translating them", () => {
    const content = "😀 한 $x_i$";
    const syntax = new LatexSyntaxService().upsert({
      fileId: "f1",
      path: "main.md",
      content,
      documentVersion: 2,
      language: "markdown",
    });
    const region = syntax.mathRegions[0]!;
    const snapshot = createProjectSnapshot({
      documents: [
        {
          content,
          language: "markdown",
          syntax,
        },
      ],
      epoch: "p:1",
      inventoryVersion: 4,
      projectId: "p",
    });
    expect(snapshot.documents[0]?.mathRoots?.[0]).toMatchObject({
      contentRange: region.contentRange,
      delimiter: region.delimiter,
      fullRange: region.fullRange,
      state: "complete",
    });
    expect(snapshot.documents[0]?.includes).toEqual([]);
    expect(snapshot.documents[0]?.macros).toEqual([]);
  });

  test("preserves neutral citation annotations without interpreting them", () => {
    const content = "Prior work \\parencite{study} might define $A$.";
    const syntax = new LatexSyntaxService().upsert({
      fileId: "citation",
      path: "main.tex",
      content,
      documentVersion: 1,
      language: "latex",
    });
    const document = adaptWasmtexDocument({ content, language: "latex", syntax });

    expect(document.proseAnnotations).toEqual(syntax.proseAnnotations);
    expect(document.proseAnnotations[0]).toMatchObject({
      kind: "citation",
      name: "parencite",
      state: "complete",
    });
  });

  test("keeps only wasmtex-approved real-world math regions", () => {
    const content = [
      "% $comment$",
      "\\verb|$verbatim$|",
      "\\iffalse $false$ \\else $\\nabla f$ \\fi",
      "한글 😀 é $\\forall x \\in S, P(x)$\r",
      "unfinished $x + {",
    ].join("\n");
    const syntax = new LatexSyntaxService().upsert({
      fileId: "real-world",
      path: "main.tex",
      content,
      documentVersion: 3,
      language: "latex",
    });
    const snapshot = createProjectSnapshot({
      documents: [{ content, language: "latex", syntax }],
      epoch: "real-world:3",
      inventoryVersion: 3,
      mainFileId: "real-world",
      projectId: "real-world",
    });
    const regions = snapshot.documents[0]!.mathRoots;
    expect(
      regions.map((region) =>
        content.slice(region.contentRange.startOffset, region.contentRange.endOffset),
      ),
    ).toEqual(["\\nabla f", "\\forall x \\in S, P(x)", "x + {"]);
    expect(regions.at(-1)?.state).toBe("incomplete");
  });

  test("preserves bounded macro provenance from the shared syntax snapshot", () => {
    const content = "\\newcommand{\\vect}[1]{\\mathbf{#1}} $\\vect{x}$";
    const syntax = new LatexSyntaxService().upsert({
      fileId: "macros",
      path: "main.tex",
      content,
      documentVersion: 1,
      language: "latex",
    });
    const snapshot = createProjectSnapshot({
      documents: [{ content, language: "latex", syntax }],
      epoch: "macros:1",
      inventoryVersion: 1,
      projectId: "macros",
    });

    expect(snapshot.documents[0]?.macros).toEqual(syntax.macros);
    expect(snapshot.documents[0]?.macros?.some((macro) => macro.kind === "call")).toBe(
      true,
    );
    expect(
      snapshot.documents[0]?.macros?.find(
        (macro) => macro.kind === "call" && macro.name === "vect",
      )?.expansion,
    ).toMatchObject({
        surface: "\\mathbf{x}",
    });
  });
});
