import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";
import {
  findLatexNotationPath,
  LatexSyntaxService,
  type LatexFileSyntax,
} from "wasmtex/syntax";
import {
  generateNotationFuzzSources,
  matchesNotationExpectation,
  notationArenaFailures,
  notationCoverageGaps,
  parseNotationConformanceCorpus,
} from "./notation-conformance";

const fixtureUrl = new URL(
  "../../fixtures/notation-conformance.json",
  import.meta.url,
);

describe("notation conformance", () => {
  test("covers the reviewed matrix and every permanent structural regression", async () => {
    const corpus = parseNotationConformanceCorpus(
      JSON.parse(await readFile(fixtureUrl, "utf8")),
    );
    expect(notationCoverageGaps(corpus)).toEqual([]);

    for (const item of corpus.cases) {
      const syntax = syntaxFor(item.content, item.language);
      const cursor = item.content.indexOf(item.cursor.needle) + item.cursor.offset;
      const path = findLatexNotationPath(syntax, cursor).map(
        (nodeId) => syntax.nodes[nodeId]!,
      );

      expect(path.length, item.id).toBeGreaterThan(0);
      expect(
        path.some((node) => matchesNotationExpectation(node, item.expectedAncestor)),
        `${item.id}: expected ancestor ${JSON.stringify(item.expectedAncestor)} in ${JSON.stringify(
          path.map(nodeSummary),
        )}`,
      ).toBe(true);
      if (item.forbiddenAncestor) {
        expect(
          path.some((node) => matchesNotationExpectation(node, item.forbiddenAncestor!)),
          `${item.id}: forbidden ancestor was selected`,
        ).toBe(false);
      }
      expect(notationArenaFailures(syntax.nodes, item.content.length), item.id).toEqual([]);
    }
  });

  test("keeps clean and incremental notation snapshots identical", async () => {
    const corpus = parseNotationConformanceCorpus(
      JSON.parse(await readFile(fixtureUrl, "utf8")),
    );
    for (const item of corpus.cases) {
      const service = new LatexSyntaxService();
      service.upsert({
        fileId: "case",
        path: item.language === "markdown" ? "case.md" : "case.tex",
        content: "$placeholder$",
        documentVersion: 1,
        language: item.language,
      });
      const incremental = service.upsert({
        fileId: "case",
        path: item.language === "markdown" ? "case.md" : "case.tex",
        content: item.content,
        documentVersion: 2,
        language: item.language,
      });
      expect(incremental, item.id).toEqual(syntaxFor(item.content, item.language, 2));
    }
  });

  test("survives deterministic bounded malformed, Unicode, nesting, and command fuzz", () => {
    const first = generateNotationFuzzSources(0x5e_aa_17, 160);
    expect(generateNotationFuzzSources(0x5e_aa_17, 160)).toEqual(first);
    expect(new Set(first).size).toBeGreaterThan(120);

    for (const [index, content] of first.entries()) {
      const syntax = syntaxFor(content, "latex");
      expect(syntax.nodes.length, `fuzz case ${index}`).toBeLessThanOrEqual(10_000);
      expect(notationArenaFailures(syntax.nodes, content.length), `fuzz case ${index}`).toEqual(
        [],
      );
      for (const root of syntax.mathRoots) {
        const offset = Math.min(
          root.contentRange.endOffset,
          root.contentRange.startOffset + 1,
        );
        expect(findLatexNotationPath(syntax, offset).length).toBeGreaterThan(0);
      }
    }
  });

  test("rejects corpus drift instead of silently accepting unknown fields", () => {
    expect(() =>
      parseNotationConformanceCorpus({
        schemaVersion: 1,
        requiredCoverage: {},
        cases: [],
        compatibilityMode: true,
      }),
    ).toThrow("unknown compatibilityMode");
  });

  test("retracts generated shapes through the explicit invalidated closure", () => {
    const service = new LatexSyntaxService();
    service.reset({
      documents: [
        {
          fileId: "caller",
          path: "caller.tex",
          content: "$\\estimate{x}$",
          documentVersion: 1,
        },
        {
          fileId: "defs",
          path: "defs.tex",
          content: "\\newcommand{\\estimate}[1]{\\hat{#1}}",
          documentVersion: 1,
        },
        {
          fileId: "unrelated",
          path: "unrelated.tex",
          content: "$z$",
          documentVersion: 1,
        },
      ],
    });
    expect(expandedCall(service.getFile("caller"))?.kind).toBe("modifier");

    service.remove("defs");

    expect(expandedCall(service.getFile("caller"))).toBeUndefined();
    expect(service.getInvalidatedFiles().map((syntax) => syntax.fileId)).toEqual([
      "caller",
    ]);
  });
});

function syntaxFor(
  content: string,
  language: "latex" | "markdown",
  documentVersion = 1,
): LatexFileSyntax {
  return new LatexSyntaxService().upsert({
    fileId: "case",
    path: language === "markdown" ? "case.md" : "case.tex",
    content,
    documentVersion,
    language,
  });
}

function nodeSummary(node: LatexFileSyntax["nodes"][number]) {
  return {
    kind: node.kind,
    name: node.name,
    origin: node.provenance?.origin ?? "source",
    state: node.state,
    text: node.text,
  };
}

function expandedCall(syntax: LatexFileSyntax | null) {
  return syntax?.nodes.find((node) => node.provenance?.origin === "expansion");
}
