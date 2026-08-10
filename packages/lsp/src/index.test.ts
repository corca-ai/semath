import { describe, expect, test } from "bun:test";
import {
  createSemathLspServer,
  type JsonRpcMessage,
  offsetAt,
  positionAt,
} from "./index";

function response(messages: JsonRpcMessage[], id: number) {
  return messages.find((message) => message.id === id)?.result as any;
}

async function setup() {
  const messages: JsonRpcMessage[] = [];
  const server = await createSemathLspServer(
    (message) => messages.push(message),
    {
      epoch: "lsp-test",
      projectId: "lsp-test",
    },
  );
  return { messages, server };
}

describe("SemathLspServer", () => {
  test("advertises a native-editor language surface", async () => {
    const { messages, server } = await setup();
    await server.handle({ id: 1, method: "initialize", params: {} });

    expect(response(messages, 1).capabilities).toMatchObject({
      completionProvider: expect.any(Object),
      definitionProvider: true,
      hoverProvider: true,
      referencesProvider: true,
      renameProvider: { prepareProvider: true },
      selectionRangeProvider: true,
    });
    server.dispose();
  });

  test("serves selection, meaning-first semantic view, hover, and diagnostics", async () => {
    const { messages, server } = await setup();
    const uri = "file:///main.md";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "markdown",
          text: "Let $x$ denote the input.\nUse $x$.",
          uri,
          version: 1,
        },
      },
    });
    await server.handle({
      id: 2,
      method: "textDocument/selectionRange",
      params: { positions: [{ character: 5, line: 1 }], textDocument: { uri } },
    });
    await server.handle({
      id: 3,
      method: "textDocument/hover",
      params: { position: { character: 5, line: 1 }, textDocument: { uri } },
    });
    await server.handle({
      id: 4,
      method: "semath/semanticView",
      params: { position: { character: 5, line: 1 }, textDocument: { uri } },
    });

    expect(response(messages, 2)[0].range).toEqual({
      end: { character: 6, line: 1 },
      start: { character: 5, line: 1 },
    });
    expect(response(messages, 3).contents.value).toContain("the input");
    expect(response(messages, 4)).toMatchObject({
      kind: "semanticView",
      view: { symbol: { symbol: "x" }, status: "partial" },
    });
    expect(
      messages.some(
        (message) =>
          message.method === "textDocument/publishDiagnostics" &&
          (message.params as { uri?: string }).uri === uri,
      ),
    ).toBe(true);
    server.dispose();
  });

  test("keeps modifiers and named operators as compositional occurrences", async () => {
    const { messages, server } = await setup();
    const uri = "file:///notation.md";
    const content = [
      "Let $\\hat y$ denote the estimate.",
      "Compare plain $y$ with $\\hat y$ and $\\operatorname{ECE}$.",
    ].join("\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "markdown", text: content, uri, version: 1 },
      },
    });
    const plain = content.indexOf("$y$") + 1;
    const hat = content.lastIndexOf("\\hat y") + "\\hat ".length;
    const ece = content.lastIndexOf("ECE") + 1;
    for (const [id, offset] of [
      [61, plain],
      [62, hat],
      [63, ece],
    ] as const) {
      await server.handle({
        id,
        method: "semath/semanticView",
        params: { position: positionAt(content, offset), textDocument: { uri } },
      });
    }

    expect(response(messages, 61).view.symbol).not.toHaveProperty("entityId");
    expect(response(messages, 62).view.symbol).toMatchObject({
      entityId: expect.any(Object),
      notation: [
        { kind: "modifier", name: "hat" },
        { kind: "identifier", value: "y" },
      ],
      sourceNotation: "\\hat y",
      symbol: "y",
    });
    expect(response(messages, 63).view.symbol).toMatchObject({
      notation: [{ kind: "named-surface", value: "ECE" }],
      sourceNotation: "\\operatorname{ECE}",
      symbol: "ECE",
    });

    const surfaces = ["\\hat y", "\\operatorname{ECE}"] as const;
    let requestId = 100;
    for (const surface of surfaces) {
      const start = content.lastIndexOf(surface);
      for (let offset = start; offset <= start + surface.length; offset += 1) {
        const id = requestId++;
        await server.handle({
          id,
          method: "semath/semanticView",
          params: {
            position: positionAt(content, offset),
            textDocument: { uri },
          },
        });
        const symbol = response(messages, id).view.symbol;
        if (!symbol) {
          throw new Error(`missing ${surface} at offset ${offset - start}`);
        }
        expect(symbol).toMatchObject({
          location: {
            range: {
              endOffset: start + surface.length,
              startOffset: start,
            },
          },
          sourceNotation: surface,
        });
      }
    }
    const hatStart = content.lastIndexOf("\\hat y");
    await server.handle({
      id: 180,
      method: "textDocument/references",
      params: {
        context: { includeDeclaration: true },
        position: positionAt(content, hatStart),
        textDocument: { uri },
      },
    });
    const references = response(messages, 180);
    expect(references).toHaveLength(2);
    expect(new Set(references.map((location: unknown) => JSON.stringify(location))).size).toBe(
      2,
    );
    server.dispose();
  });

  test("addresses structural calculus operators and preserves indexed-family parts", async () => {
    const { messages, server } = await setup();
    const uri = "file:///calculus.tex";
    const content = "Use $\\int_0^1 g(t)\\,dt$ and the indexed family $x_i$.";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "latex", text: content, uri, version: 1 },
      },
    });
    await server.handle({
      id: 64,
      method: "semath/semanticView",
      params: {
        position: positionAt(content, content.indexOf("\\int") + "\\int".length),
        textDocument: { uri },
      },
    });
    await server.handle({
      id: 65,
      method: "semath/semanticView",
      params: {
        position: positionAt(content, content.indexOf("x_i")),
        textDocument: { uri },
      },
    });

    expect(response(messages, 64).view.context.candidates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ family: "binder", interpretation: "binder" }),
      ]),
    );
    expect(response(messages, 65).view.symbol.notation).toEqual(
      expect.arrayContaining([{ kind: "subscript", base: "x", index: "i" }]),
    );
    server.dispose();
  });

  test("maps three-way English declarations through hover and definition", async () => {
    const { messages, server } = await setup();
    const uri = "file:///declarations.md";
    const content = [
      "Let $x$, $y$, and $z$ denote the input, state, and output, respectively.",
      "The estimator updates $y$.",
    ].join("\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "markdown",
          text: content,
          uri,
          version: 1,
        },
      },
    });
    const use = content.lastIndexOf("$y$") + 1;
    await server.handle({
      id: 41,
      method: "textDocument/hover",
      params: { position: positionAt(content, use), textDocument: { uri } },
    });
    await server.handle({
      id: 42,
      method: "textDocument/definition",
      params: { position: positionAt(content, use), textDocument: { uri } },
    });

    expect(response(messages, 41).contents.value).toContain("state");
    expect(response(messages, 42)).toMatchObject({
      range: {
        start: positionAt(content, content.indexOf("$y$") + 1),
      },
      uri,
    });
    server.dispose();
  });

  test("ranks named-call candidates only from source-linked type evidence", async () => {
    const { messages, server } = await setup();
    const uri = "file:///candidates.md";
    const content = [
      "Let $\\operatorname{acc}$ denote an accuracy metric.",
      "$\\operatorname{acc}(B_m)$ and $\\operatorname{conf}(B_m)$.",
    ].join("\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "markdown", text: content, uri, version: 1 },
      },
    });
    for (const [id, needle] of [
      [181, "\\operatorname{acc}(B_m)"],
      [182, "\\operatorname{conf}(B_m)"],
    ] as const) {
      await server.handle({
        id,
        method: "semath/semanticView",
        params: {
          position: positionAt(content, content.lastIndexOf(needle) + 15),
          textDocument: { uri },
        },
      });
    }

    expect(response(messages, 181).view.context.candidates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          family: "application",
          interpretation: "application",
          status: "supported",
        }),
        expect.objectContaining({
          family: "juxtaposition",
          interpretation: "multiplication",
          status: "unresolved",
        }),
      ]),
    );
    expect(response(messages, 182).view.context.candidates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ family: "application", status: "unresolved" }),
        expect.objectContaining({ family: "juxtaposition", status: "unresolved" }),
      ]),
    );
    server.dispose();
  });

  test("binds prose acronyms to exact named surfaces and retracts stale evidence", async () => {
    const { messages, server } = await setup();
    const uri = "file:///acronym.md";
    const content = [
      "We report expected calibration error (ECE).",
      "$\\operatorname{ECE}=0$. Plain $ECE$ stays plain.",
    ].join("\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "markdown", text: content, uri, version: 1 },
      },
    });
    const named = content.indexOf("\\operatorname{ECE}") + 15;
    const plain = content.lastIndexOf("$ECE$") + 2;
    await server.handle({
      id: 190,
      method: "textDocument/definition",
      params: { position: positionAt(content, named), textDocument: { uri } },
    });
    await server.handle({
      id: 191,
      method: "semath/semanticView",
      params: { position: positionAt(content, named), textDocument: { uri } },
    });
    await server.handle({
      id: 192,
      method: "textDocument/definition",
      params: { position: positionAt(content, plain), textDocument: { uri } },
    });

    expect(response(messages, 190)).toMatchObject({
      range: {
        start: positionAt(content, content.indexOf("(ECE)") + 1),
      },
      uri,
    });
    expect(response(messages, 191)).toMatchObject({
      view: {
        status: "partial",
        summary: "expected calibration error",
        symbol: {
          sourceNotation: "\\operatorname{ECE}",
          symbol: "ECE",
        },
      },
    });
    expect(response(messages, 192)).toBeNull();

    const revised = content.replace(
      "expected calibration error (ECE)",
      "calibration quality",
    );
    await server.handle({
      method: "textDocument/didChange",
      params: {
        contentChanges: [{ text: revised }],
        textDocument: { uri, version: 2 },
      },
    });
    await server.handle({
      id: 193,
      method: "textDocument/definition",
      params: {
        position: positionAt(revised, revised.indexOf("\\operatorname{ECE}") + 15),
        textDocument: { uri },
      },
    });
    expect(response(messages, 193)).toBeNull();
    server.dispose();
  });

  test("shadows the same acronym by section instead of globally unioning strings", async () => {
    const { messages, server } = await setup();
    const uri = "file:///scoped-acronym.md";
    const content = [
      "# Metrics",
      "Expected calibration error (ECE) is reported.",
      "$\\operatorname{ECE}=0$.",
      "# Engineering",
      "Electrical computer engineering (ECE) is discussed.",
      "$\\operatorname{ECE}$ programs are compared.",
    ].join("\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "markdown", text: content, uri, version: 1 },
      },
    });
    for (const [id, offset] of [
      [194, content.indexOf("\\operatorname{ECE}") + 15],
      [195, content.lastIndexOf("\\operatorname{ECE}") + 15],
    ] as const) {
      await server.handle({
        id,
        method: "textDocument/definition",
        params: { position: positionAt(content, offset), textDocument: { uri } },
      });
    }
    expect(response(messages, 194).range.start).toEqual(
      positionAt(content, content.indexOf("(ECE)") + 1),
    );
    expect(response(messages, 195).range.start).toEqual(
      positionAt(content, content.lastIndexOf("(ECE)") + 1),
    );
    server.dispose();
  });

  test("lowers acronym, glossary, and named-operator declarations through one binding path", async () => {
    const cases = [
      {
        content:
          "\\newacronym{ece}{ECE}{expected calibration error}\n$\\operatorname{ECE}$",
        declaration: "{ECE}",
        declarationOffset: 1,
        cursorOffset: 15,
        languageId: "latex",
        use: "\\operatorname{ECE}",
      },
      {
        content:
          "\\newglossaryentry{ece}{name={ECE},description={expected calibration error}}\n$\\operatorname{ECE}$",
        declaration: "name={ECE}",
        declarationOffset: 0,
        cursorOffset: 15,
        languageId: "latex",
        use: "\\operatorname{ECE}",
      },
      {
        content: "\\DeclareMathOperator{\\ECE}{ECE}\n$\\ECE(x)$",
        declaration: "{ECE}",
        declarationOffset: 1,
        cursorOffset: 2,
        languageId: "latex",
        use: "\\ECE(x)",
      },
    ] as const;
    for (const [index, item] of cases.entries()) {
      const { messages, server } = await setup();
      const uri = `file:///structural-${index}.tex`;
      await server.handle({
        method: "textDocument/didOpen",
        params: {
          textDocument: {
            languageId: item.languageId,
            text: item.content,
            uri,
            version: 1,
          },
        },
      });
      await server.handle({
        id: 196 + index,
        method: "textDocument/definition",
        params: {
          position: positionAt(
            item.content,
            item.content.lastIndexOf(item.use) + item.cursorOffset,
          ),
          textDocument: { uri },
        },
      });
      expect(response(messages, 196 + index)).toMatchObject({
        range: {
          start: positionAt(
            item.content,
            item.content.indexOf(item.declaration) + item.declarationOffset,
          ),
        },
        uri,
      });
      server.dispose();
    }
  });

  test("keeps hypothetical, hedged, cited, and quoted acronym claims non-navigable", async () => {
    const claims = [
      "If ECE meant expected calibration error, continue.",
      "ECE might mean expected calibration error.",
      "According to the reference, ECE means expected calibration error.",
      'The phrase "ECE means expected calibration error" is quoted.',
    ];
    for (const [index, claim] of claims.entries()) {
      const { messages, server } = await setup();
      const uri = `file:///non-asserting-${index}.md`;
      const content = `${claim}\n$\\operatorname{ECE}=0$.`;
      await server.handle({
        method: "textDocument/didOpen",
        params: {
          textDocument: { languageId: "markdown", text: content, uri, version: 1 },
        },
      });
      await server.handle({
        id: 200 + index,
        method: "textDocument/definition",
        params: {
          position: positionAt(content, content.lastIndexOf("\\operatorname{ECE}") + 15),
          textDocument: { uri },
        },
      });
      expect(response(messages, 200 + index)).toBeNull();
      server.dispose();
    }
  });

  test("falls back to wasmtex for cross-file LaTeX navigation", async () => {
    const { messages, server } = await setup();
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "latex",
          text: "See \\ref{sec:intro}",
          uri: "file:///main.tex",
          version: 1,
        },
      },
    });
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "latex",
          text: "\\section{Intro}\\label{sec:intro}",
          uri: "file:///chapter.tex",
          version: 1,
        },
      },
    });
    await server.handle({
      id: 5,
      method: "textDocument/definition",
      params: {
        position: { character: 10, line: 0 },
        textDocument: { uri: "file:///main.tex" },
      },
    });

    expect(response(messages, 5)).toMatchObject({ uri: "file:///chapter.tex" });

    await server.handle({
      id: 51,
      method: "textDocument/completion",
      params: {
        position: { character: 4, line: 0 },
        textDocument: { uri: "file:///chapter.tex" },
      },
    });
    expect(response(messages, 51).items).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "\\section" })]),
    );
    expect(server.getRuntimeStats()).toMatchObject({
      documents: 2,
      inventoryVersion: 2,
      syntax: {
        documents: 2,
        notationNodes: expect.any(Number),
        parseCount: 2,
        snapshotBytes: expect.any(Number),
      },
    });

    await server.handle({
      id: 511,
      method: "textDocument/hover",
      params: {
        position: { character: 10, line: 0 },
        textDocument: { uri: "file:///main.tex" },
      },
    });
    expect(server.getRuntimeStats().syntax.parseCount).toBe(2);
    server.dispose();
  });

  test("keeps semantic navigation stable on both UTF-16 symbol edges", async () => {
    const { messages, server } = await setup();
    const uri = "file:///unicode.tex";
    const content = [
      "한글 😀 é",
      "Let $A$ denote an event of positive probability.",
      "$p = \\mathbb{P}(A)$",
    ].join("\r\n");
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "latex", text: content, uri, version: 1 },
      },
    });
    const start = content.lastIndexOf("A)");
    for (const [id, offset] of [
      [520, start],
      [521, start + 1],
    ] as const) {
      await server.handle({
        id,
        method: "textDocument/definition",
        params: { position: positionAt(content, offset), textDocument: { uri } },
      });
    }

    expect(response(messages, 520)).toEqual(response(messages, 521));
    expect(response(messages, 520)).toMatchObject({ uri });
    server.dispose();
  });

  test("refuses definitions that occur later in include expansion order", async () => {
    const { messages, server } = await setup();
    const mainUri = "file:///main.tex";
    const definitionsUri = "file:///definitions.tex";
    const main = "Before $x$.\n\\input{definitions}\nAfter $x$.";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "latex",
          text: main,
          uri: mainUri,
          version: 1,
        },
      },
    });
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "latex",
          text: "Let $x$ denote the included value.",
          uri: definitionsUri,
          version: 1,
        },
      },
    });
    await server.handle({
      id: 530,
      method: "textDocument/definition",
      params: {
        position: positionAt(main, main.indexOf("$x$") + 1),
        textDocument: { uri: mainUri },
      },
    });
    await server.handle({
      id: 531,
      method: "textDocument/definition",
      params: {
        position: positionAt(main, main.lastIndexOf("$x$") + 1),
        textDocument: { uri: mainUri },
      },
    });

    expect(response(messages, 530)).toBeNull();
    expect(response(messages, 531)).toMatchObject({ uri: definitionsUri });
    server.dispose();
  });

  test("returns recognized meaning with evidence and roles", async () => {
    const { messages, server } = await setup();
    const uri = "file:///probability.md";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "markdown",
          text: [
            "Let $A$ be an event.",
            "Let $B$ be an event.",
            "$A \\cap B$",
          ].join("\n"),
          uri,
          version: 1,
        },
      },
    });
    await server.handle({
      id: 52,
      method: "semath/semanticView",
      params: {
        position: { character: 2, line: 2 },
        textDocument: { uri },
      },
    });

    expect(response(messages, 52)).toMatchObject({
      kind: "semanticView",
      view: {
        context: {
          relations: [
            {
              relationId: "probability:event-intersection",
              roles: expect.arrayContaining([
                expect.objectContaining({ role: "left", symbol: "A" }),
                expect.objectContaining({ role: "right", symbol: "B" }),
              ]),
            },
          ],
        },
        status: "established",
      },
    });
    server.dispose();
  });

  test("suppresses a completion response cancelled through LSP", async () => {
    const { messages, server } = await setup();
    const uri = "file:///cancel.tex";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: { languageId: "latex", text: "\\fra", uri, version: 1 },
      },
    });
    const pending = server.handle({
      id: 53,
      method: "textDocument/completion",
      params: { position: { character: 4, line: 0 }, textDocument: { uri } },
    });
    await server.handle({ method: "$/cancelRequest", params: { id: 53 } });
    await pending;

    expect(messages.some((message) => message.id === 53)).toBe(false);
    server.dispose();
  });

  test("preserves stable identity through a file move and clears a closed document", async () => {
    const { messages, server } = await setup();
    const oldUri = "file:///old.md";
    const newUri = "file:///new.md";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "markdown",
          text: "Let $x$ denote a scalar.\n$x$",
          uri: oldUri,
          version: 1,
        },
      },
    });
    await server.handle({
      method: "workspace/didRenameFiles",
      params: { files: [{ newUri, oldUri }] },
    });
    await server.handle({
      id: 6,
      method: "textDocument/hover",
      params: {
        position: { character: 1, line: 1 },
        textDocument: { uri: newUri },
      },
    });
    expect(response(messages, 6).contents.value).toContain("Scalar");

    await server.handle({
      method: "textDocument/didClose",
      params: { textDocument: { uri: newUri } },
    });
    const published = messages.filter(
      (message) =>
        message.method === "textDocument/publishDiagnostics" &&
        (message.params as { uri?: string }).uri === newUri,
    );
    expect(
      (published.at(-1)!.params as { diagnostics: unknown[] }).diagnostics,
    ).toEqual([]);
    server.dispose();
  });

  test("reuses syntax trees across realistic query, edit, move, and close cycles", async () => {
    const { server } = await setup();
    const uris = ["file:///main.tex", "file:///chapter.tex", "file:///appendix.tex"];
    const contents = [
      "\\input{chapter}\nLet $x$ denote a vector. $\\lVert x \\rVert_2$",
      "\\input{appendix}\nLet $A$ and $B$ denote events. $\\Pr(A \\mid B)$",
      "한글 😀 é\n$\\forall x \\in S$",
    ];
    for (const [index, uri] of uris.entries()) {
      await server.handle({
        method: "textDocument/didOpen",
        params: {
          textDocument: {
            languageId: "latex",
            text: contents[index],
            uri,
            version: 1,
          },
        },
      });
    }
    expect(server.getRuntimeStats().syntax).toMatchObject({
      documents: 3,
      notationNodes: expect.any(Number),
      parseCount: 3,
      snapshotBytes: expect.any(Number),
    });

    for (let id = 700; id < 720; id += 2) {
      await server.handle({
        id,
        method: "textDocument/hover",
        params: { position: { character: 40, line: 0 }, textDocument: { uri: uris[0] } },
      });
      await server.handle({
        id: id + 1,
        method: "semath/semanticView",
        params: { position: { character: 40, line: 0 }, textDocument: { uri: uris[0] } },
      });
    }
    expect(server.getRuntimeStats().syntax.parseCount).toBe(3);

    await server.handle({
      method: "textDocument/didChange",
      params: {
        contentChanges: [{ text: `${contents[1]}\n% one edit` }],
        textDocument: { uri: uris[1], version: 2 },
      },
    });
    expect(server.getRuntimeStats().syntax.parseCount).toBe(4);

    const movedUri = "file:///chapters/probability.tex";
    await server.handle({
      method: "workspace/didRenameFiles",
      params: { files: [{ newUri: movedUri, oldUri: uris[1] }] },
    });
    // A move reparses only the moved document because relative includes depend
    // on its path; the other syntax trees remain reusable.
    expect(server.getRuntimeStats().syntax.parseCount).toBe(5);

    await server.handle({
      method: "textDocument/didClose",
      params: { textDocument: { uri: movedUri } },
    });
    expect(server.getRuntimeStats()).toMatchObject({
      documents: 2,
      syntax: { documents: 2, parseCount: 5 },
    });
    server.dispose();
  });
});

test("offset/position conversion is UTF-16 and clamps malformed positions", () => {
  const content = "한😀\nabc";
  expect(offsetAt(content, { character: 3, line: 0 })).toBe(3);
  expect(positionAt(content, 3)).toEqual({ character: 3, line: 0 });
  expect(offsetAt(content, { character: 99, line: 99 })).toBe(content.length);
  expect(positionAt(content, 99)).toEqual({ character: 3, line: 1 });
});
