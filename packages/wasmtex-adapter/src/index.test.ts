import { describe, expect, test } from "bun:test";
import { LatexSyntaxService } from "wasmtex/syntax";
import { createProjectSnapshot } from "./index";

describe("wasmtex adapter", () => {
  test("keeps wasmtex UTF-16 ranges without translating them", () => {
    const region = {
      closed: true,
      contentRange: { endOffset: 9, startOffset: 6 },
      delimiter: "$",
      fullRange: { endOffset: 10, startOffset: 5 },
    };
    const snapshot = createProjectSnapshot({
      documents: [
        {
          content: "😀 한 $x_i$",
          language: "markdown",
          syntax: {
            documentVersion: 2,
            fileId: "f1",
            includes: [
              {
                path: "chapter",
                source: { range: { endOffset: 4, startOffset: 1 } },
              },
            ],
            mathRegions: [region],
            path: "main.md",
            schemaVersion: 2,
          },
        },
      ],
      epoch: "p:1",
      inventoryVersion: 4,
      projectId: "p",
    });
    expect(snapshot.documents[0]?.mathRegions?.[0]).toEqual(region);
    expect(snapshot.documents[0]?.includes).toEqual([
      { path: "chapter", sourceRange: { endOffset: 4, startOffset: 1 } },
    ]);
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
    const regions = snapshot.documents[0]!.mathRegions!;
    expect(
      regions.map((region) =>
        content.slice(region.contentRange.startOffset, region.contentRange.endOffset),
      ),
    ).toEqual(["\\nabla f", "\\forall x \\in S, P(x)", "x + {"]);
    expect(regions.at(-1)?.closed).toBe(false);
  });
});
