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
      codeActionProvider: { codeActionKinds: ["refactor.rewrite"] },
      completionProvider: expect.any(Object),
      definitionProvider: true,
      hoverProvider: true,
      referencesProvider: true,
      renameProvider: { prepareProvider: true },
      selectionRangeProvider: true,
    });
    server.dispose();
  });

  test("serves Semath selection, hover, inspection, and diagnostics over LSP", async () => {
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
      method: "semath/inspection",
      params: { position: { character: 5, line: 1 }, textDocument: { uri } },
    });

    expect(response(messages, 2)[0].range).toEqual({
      end: { character: 6, line: 1 },
      start: { character: 5, line: 1 },
    });
    expect(response(messages, 3).contents.value).toContain("the input");
    expect(response(messages, 4)).toMatchObject({
      kind: "inspection",
      inspection: { symbol: { symbol: "x" } },
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
    expect(server.getRuntimeStats()).toEqual({
      documents: 2,
      inventoryVersion: 2,
      syntax: { documents: 2, parseCount: 2 },
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

  test("returns semantic rewrites as reviewable code actions", async () => {
    const { messages, server } = await setup();
    const uri = "file:///probability.md";
    await server.handle({
      method: "textDocument/didOpen",
      params: {
        textDocument: {
          languageId: "markdown",
          text: [
            "Let $A$ denote an event of positive probability.",
            "Let $B$ denote an event of positive probability.",
            "$p = \\mathbb{P}(A \\mid B)$",
          ].join("\n"),
          uri,
          version: 1,
        },
      },
    });
    await server.handle({
      id: 52,
      method: "textDocument/codeAction",
      params: {
        range: {
          end: { character: 10, line: 2 },
          start: { character: 5, line: 2 },
        },
        textDocument: { uri },
      },
    });

    expect(response(messages, 52)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          isPreferred: false,
          kind: "refactor.rewrite",
          title: "Expand the conditional-probability definition",
        }),
      ]),
    );
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
});

test("offset/position conversion is UTF-16 and clamps malformed positions", () => {
  const content = "한😀\nabc";
  expect(offsetAt(content, { character: 3, line: 0 })).toBe(3);
  expect(positionAt(content, 3)).toEqual({ character: 3, line: 0 });
  expect(offsetAt(content, { character: 99, line: 99 })).toBe(content.length);
  expect(positionAt(content, 99)).toEqual({ character: 3, line: 1 });
});
