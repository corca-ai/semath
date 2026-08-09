import {
  LatexLanguageService,
  type LatexWorkspaceEdit,
  type NeutralCompletionItem,
  type NeutralHover,
  type NeutralLocation,
  type NeutralRange,
} from "wasmtex/lsp";
import {
  type JsonRpcMessage,
  pathFromUri,
  uriFromPath,
} from "wasmtex/lsp/server";
import { LatexSyntaxService } from "wasmtex/syntax";
import {
  SEMATH_PROTOCOL_VERSION,
  type ChangeEnvelope,
  type DocumentLanguage,
  type Location,
  type ProjectChange,
  type ProjectDocument,
  type ProjectSnapshot,
  type QueryEnvelope,
  type QueryResult,
  type SemanticDiagnostic,
  type SemanticEditProposal,
  type SourceRange,
} from "../../protocol/src/index";
import { adaptWasmtexDocument } from "../../wasmtex-adapter/src/index";
import { SemathWorkerEngine } from "../../worker/src/index";

export type { JsonRpcMessage } from "wasmtex/lsp/server";

export interface SemathEngineLike {
  apply(changes: ChangeEnvelope): unknown;
  dispose?(): void;
  query(envelope: QueryEnvelope): QueryResult;
  reset(snapshot: ProjectSnapshot): unknown;
}

export interface SemathLspServerOptions {
  epoch?: string;
  latexService?: LatexLanguageService;
  projectId?: string;
  semath: SemathEngineLike;
  syntaxService?: LatexSyntaxService;
}

export type SendMessage = (message: JsonRpcMessage) => void;

export interface SemathLspRuntimeStats {
  documents: number;
  inventoryVersion: number;
  syntax: {
    documents: number;
    parseCount: number;
  };
}

interface LspPosition {
  character: number;
  line: number;
}

interface LspRange {
  end: LspPosition;
  start: LspPosition;
}

interface TextDocumentPositionParams {
  position: LspPosition;
  textDocument: { uri: string };
}

interface DocumentState {
  document: ProjectDocument;
  uri: string;
}

const severity = { error: 1, warning: 2, hint: 4 } as const;

export class SemathLspServer {
  private readonly semath: SemathEngineLike;
  private readonly syntax: LatexSyntaxService;
  private readonly latex: LatexLanguageService;
  private readonly epoch: string;
  private readonly projectId: string;
  private readonly documents = new Map<string, DocumentState>();
  private readonly fileIdsByUri = new Map<string, string>();
  private readonly cancelled = new Set<number | string>();
  private readonly publishedUris = new Set<string>();
  private inventoryVersion = 0;
  private analysisGeneration = 0;
  private initializedProject = false;

  constructor(
    private readonly send: SendMessage,
    options: SemathLspServerOptions,
  ) {
    this.semath = options.semath;
    this.syntax =
      options.syntaxService ??
      options.latexService?.getSyntaxService() ??
      new LatexSyntaxService();
    this.latex =
      options.latexService ??
      new LatexLanguageService({ syntaxService: this.syntax });
    if (this.latex.getSyntaxService() !== this.syntax) {
      throw new Error(
        "latexService and syntaxService must share one syntax runtime",
      );
    }
    this.epoch = options.epoch ?? "semath-lsp-session";
    this.projectId = options.projectId ?? "semath-lsp";
  }

  async handle(message: JsonRpcMessage): Promise<void> {
    if (!message.method) return;
    const { id, method, params } = message;
    try {
      switch (method) {
        case "initialize":
          this.respond(id, {
            capabilities: capabilities(),
            serverInfo: { name: "semath" },
          });
          return;
        case "initialized":
          return;
        case "shutdown":
          this.respond(id, null);
          return;
        case "exit":
          this.dispose();
          return;
        case "$/cancelRequest": {
          const requestId = params?.id;
          if (typeof requestId === "number" || typeof requestId === "string") {
            this.cancelled.add(requestId);
          }
          return;
        }
        case "textDocument/didOpen":
          this.didOpen(params);
          return;
        case "textDocument/didChange":
          this.didChange(params);
          return;
        case "textDocument/didClose":
          this.didClose(params);
          return;
        case "workspace/didRenameFiles":
          this.didRenameFiles(params);
          return;
        case "textDocument/selectionRange":
          this.respond(id, this.selectionRanges(params));
          return;
        case "textDocument/hover":
          this.respond(
            id,
            this.hover(params as unknown as TextDocumentPositionParams),
          );
          return;
        case "textDocument/definition":
          this.respond(id, this.locations(params, "definition"));
          return;
        case "textDocument/references":
          this.respond(id, this.locations(params, "references"));
          return;
        case "textDocument/prepareRename":
          this.respond(
            id,
            this.prepareRename(params as unknown as TextDocumentPositionParams),
          );
          return;
        case "textDocument/rename":
          this.respond(id, this.rename(params));
          return;
        case "textDocument/completion":
          this.respond(
            id,
            await this.completion(
              params as unknown as TextDocumentPositionParams,
              id,
            ),
          );
          return;
        case "semath/semanticView":
          this.respond(id, this.queryAt(params).value);
          return;
        case "semath/runtimeStats":
          this.respond(id, this.getRuntimeStats());
          return;
        default:
          if (id != null)
            this.respondError(id, -32601, `Unknown method: ${method}`);
      }
    } catch (error) {
      if (id == null) return;
      const detail = error instanceof Error ? error.message : String(error);
      this.respondError(id, -32603, `Internal error: ${detail}`);
    }
  }

  dispose(): void {
    this.semath.dispose?.();
  }

  /** Observable invariants for calibration and host diagnostics. */
  getRuntimeStats(): SemathLspRuntimeStats {
    return {
      documents: this.documents.size,
      inventoryVersion: this.inventoryVersion,
      syntax: this.syntax.getStats(),
    };
  }

  private didOpen(params: Record<string, unknown> | undefined): void {
    const item = (params?.textDocument ?? {}) as {
      languageId?: string;
      text?: string;
      uri: string;
      version?: number;
    };
    this.upsert(
      item.uri,
      item.text ?? "",
      item.version ?? 0,
      languageOf(item.uri, item.languageId),
    );
  }

  private didChange(params: Record<string, unknown> | undefined): void {
    const item = (params?.textDocument ?? {}) as {
      uri: string;
      version?: number;
    };
    const changes = (params?.contentChanges ?? []) as { text: string }[];
    if (!changes.length) return;
    const state = this.stateForUri(item.uri);
    const version = Math.max(
      item.version ?? state.document.documentVersion + 1,
      state.document.documentVersion + 1,
    );
    this.upsert(
      item.uri,
      changes.at(-1)!.text,
      version,
      state.document.language,
    );
  }

  private didClose(params: Record<string, unknown> | undefined): void {
    const uri = ((params?.textDocument ?? {}) as { uri?: string }).uri;
    if (!uri) return;
    const fileId = this.fileIdsByUri.get(uri);
    if (!fileId) return;
    const state = this.documents.get(fileId)!;
    if (state.document.language === "bibtex")
      this.latex.removeFile(state.document.path);
    else this.latex.removeDocument(fileId);
    this.documents.delete(fileId);
    this.fileIdsByUri.delete(uri);
    this.apply([{ fileId, kind: "remove" }]);
    this.publishDiagnostics();
  }

  private didRenameFiles(params: Record<string, unknown> | undefined): void {
    const files = (params?.files ?? []) as { newUri: string; oldUri: string }[];
    const changes: ProjectChange[] = [];
    for (const file of files) {
      const fileId = this.fileIdsByUri.get(file.oldUri);
      if (!fileId) continue;
      const state = this.documents.get(fileId)!;
      const path = pathFromUri(file.newUri);
      if (state.document.language === "bibtex") {
        this.latex.removeFile(state.document.path);
        this.latex.updateFile(path, state.document.content);
      } else {
        this.latex.moveDocument(fileId, path);
      }
      state.document = { ...state.document, path };
      state.uri = file.newUri;
      this.fileIdsByUri.delete(file.oldUri);
      this.fileIdsByUri.set(file.newUri, fileId);
      changes.push({ fileId, kind: "path-change", path });
    }
    if (changes.length) {
      this.apply(changes);
      this.publishDiagnostics();
    }
  }

  private upsert(
    uri: string,
    content: string,
    documentVersion: number,
    language: DocumentLanguage,
  ): void {
    const fileId = this.fileIdsByUri.get(uri) ?? uri;
    const path = pathFromUri(uri);
    let document: ProjectDocument;
    if (language === "bibtex") {
      this.latex.updateFile(path, content);
      document = {
        content,
        documentVersion,
        fileId,
        includes: [],
        language,
        macros: [],
        mathRegions: [],
        path,
      };
    } else {
      const syntax = this.latex.updateDocument({
        content,
        documentVersion,
        fileId,
        language,
        path,
      });
      document = adaptWasmtexDocument({ content, language, syntax });
    }
    this.fileIdsByUri.set(uri, fileId);
    this.documents.set(fileId, { document, uri });
    this.apply([{ document, kind: "upsert" }]);
    this.publishDiagnostics();
  }

  private apply(changes: readonly ProjectChange[]): void {
    this.inventoryVersion++;
    this.analysisGeneration++;
    if (!this.initializedProject) {
      this.semath.reset(this.snapshot());
      this.initializedProject = true;
      return;
    }
    this.semath.apply({
      analysisGeneration: this.analysisGeneration,
      changes,
      epoch: this.epoch,
      inventoryVersion: this.inventoryVersion,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    });
  }

  private snapshot(): ProjectSnapshot {
    return {
      documents: [...this.documents.values()].map((state) => state.document),
      epoch: this.epoch,
      inventoryVersion: this.inventoryVersion,
      projectId: this.projectId,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
    };
  }

  private selectionRanges(
    params: Record<string, unknown> | undefined,
  ): object[] {
    const uri = ((params?.textDocument ?? {}) as { uri: string }).uri;
    const positions = (params?.positions ?? []) as LspPosition[];
    return positions.map((position) => {
      const result = this.queryPosition(uri, position, "selection");
      const ranges =
        result.value.kind === "selection" ? result.value.ranges : [];
      let parent: object | undefined;
      for (const range of [...ranges].reverse()) {
        parent = {
          ...(parent ? { parent } : {}),
          range: this.lspRange(uri, range),
        };
      }
      return parent ?? { range: { start: position, end: position } };
    });
  }

  private hover(params: TextDocumentPositionParams): object | null {
    const result = this.queryPosition(
      params.textDocument.uri,
      params.position,
      "semanticView",
    );
    if (result.value.kind === "semanticView") {
      const value = result.value.view;
      const lines = [
        `**${value.summary}**`,
        value.symbol?.shapes[0]?.display ?? "",
        ...(value.symbol?.roles ?? []).map((role) => role.description),
        ...(value.symbol?.definitions ?? []).map((definition) => definition.description),
        ...value.context.relations.map((relation) => relation.description),
        value.refusal ?? "",
      ].filter(Boolean);
      if (lines.length)
        return { contents: { kind: "markdown", value: lines.join("\n\n") } };
    }
    const { path, line, column } = this.locate(params);
    return mapHover(this.latex.getHover(path, line, column));
  }

  private locations(
    params: Record<string, unknown> | undefined,
    kind: "definition" | "references",
  ): object[] | object | null {
    const position = params as unknown as TextDocumentPositionParams;
    const result = this.queryPosition(
      position.textDocument.uri,
      position.position,
      kind,
    );
    const semantic =
      result.value.kind === "locations"
        ? result.value.locations.map((item) => this.location(item))
        : [];
    if (semantic.length) return kind === "definition" ? semantic[0]! : semantic;
    const { path, line, column } = this.locate(position);
    if (kind === "definition") {
      const fallback = this.latex.getDefinition(path, line, column);
      return fallback ? mapLocation(fallback) : null;
    }
    return this.latex.getReferences(path, line, column).map(mapLocation);
  }

  private prepareRename(params: TextDocumentPositionParams): object | null {
    const result = this.queryPosition(
      params.textDocument.uri,
      params.position,
      "prepareRename",
    );
    if (result.value.kind === "renamePreparation" && result.value.range) {
      return {
        placeholder: result.value.placeholder,
        range: this.lspRange(params.textDocument.uri, result.value.range),
      };
    }
    const { path, line, column } = this.locate(params);
    const symbol = this.latex
      .getProjectIndex()
      .findSymbolAt(path, line, column);
    if (!symbol) return null;
    const occurrence = this.latex
      .getProjectIndex()
      .findAllOccurrences(symbol.name, symbol.type)
      .find(
        (item) =>
          item.filePath === path &&
          item.line === line &&
          column >= item.column &&
          column <= item.column + item.length,
      );
    if (!occurrence) return null;
    return {
      placeholder: symbol.name,
      range: {
        end: {
          character: occurrence.column - 1 + occurrence.length,
          line: occurrence.line - 1,
        },
        start: { character: occurrence.column - 1, line: occurrence.line - 1 },
      },
    };
  }

  private rename(params: Record<string, unknown> | undefined): object | null {
    const position = params as unknown as TextDocumentPositionParams & {
      newName?: string;
    };
    const result = this.queryPosition(
      position.textDocument.uri,
      position.position,
      "rename",
      {
        newName: position.newName ?? "",
      },
    );
    if (result.value.kind === "editProposal" && result.value.proposal) {
      return this.workspaceEdit(result.value.proposal);
    }
    const { path, line, column } = this.locate(position);
    return mapWorkspaceEdit(
      this.latex.getRenameEdits(path, line, column, position.newName ?? ""),
    );
  }

  private async completion(
    params: TextDocumentPositionParams,
    id: JsonRpcMessage["id"],
  ): Promise<object> {
    const { path, line, column } = this.locate(params);
    const thisServer = this;
    const token = {
      get isCancellationRequested() {
        return id != null && thisServer.cancelled.has(id);
      },
    };
    const latex = await this.latex.getCompletionResultAsync(
      path,
      line,
      column,
      token,
    );
    const latexItems = latex.items.map((item) =>
      mapCompletion(item, params.position),
    );
    return {
      isIncomplete: latex.isIncomplete,
      items: dedupe(latexItems),
    };
  }

  private queryAt(
    params: Record<string, unknown> | undefined,
  ): QueryResult {
    const position = params as unknown as TextDocumentPositionParams;
    return this.queryPosition(
      position.textDocument.uri,
      position.position,
      "semanticView",
    );
  }

  private queryPosition(
    uri: string,
    position: LspPosition,
    kind:
      | "definition"
      | "prepareRename"
      | "references"
      | "rename"
      | "selection"
      | "semanticView",
    extra: Record<string, unknown> = {},
  ): QueryResult {
    const state = this.stateForUri(uri);
    const fileId = state.document.fileId;
    const offset = offsetAt(state.document.content, position);
    switch (kind) {
      case "definition":
        return this.query(state, { fileId, kind, offset });
      case "semanticView":
        return this.query(state, { fileId, kind, offset });
      case "prepareRename":
        return this.query(state, { fileId, kind, offset });
      case "references":
        return this.query(state, { fileId, kind, offset });
      case "rename":
        return this.query(state, {
          fileId,
          kind,
          newName: String(extra.newName ?? ""),
          offset,
        });
      case "selection":
        return this.query(state, { fileId, kind, offset });
    }
  }

  private query(
    state: DocumentState,
    query: QueryEnvelope["query"],
  ): QueryResult {
    return this.semath.query({
      analysisGeneration: this.analysisGeneration,
      documentVersion: state.document.documentVersion,
      epoch: this.epoch,
      inventoryVersion: this.inventoryVersion,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query,
    });
  }

  private publishDiagnostics(): void {
    const byUri = new Map<string, object[]>();
    for (const state of this.documents.values()) {
      const result = this.query(state, {
        fileId: state.document.fileId,
        kind: "diagnostics",
      });
      const diagnostics =
        result.value.kind === "diagnostics" ? result.value.diagnostics : [];
      byUri.set(
        state.uri,
        diagnostics.map((item) => this.diagnostic(state.uri, item)),
      );
    }
    for (const item of this.latex.getDiagnostics()) {
      const state = [...this.documents.values()].find(
        (candidate) => candidate.document.path === item.file,
      );
      const uri = state?.uri ?? uriFromPath(item.file);
      const list = byUri.get(uri) ?? [];
      list.push({
        code: item.code,
        message: item.message,
        range: {
          end: { character: item.endColumn - 1, line: item.line - 1 },
          start: { character: item.column - 1, line: item.line - 1 },
        },
        severity:
          item.severity === "error" ? 1 : item.severity === "warning" ? 2 : 3,
        source: "wasmtex",
      });
      byUri.set(uri, list);
    }
    const targets = new Set([...this.publishedUris, ...byUri.keys()]);
    this.publishedUris.clear();
    for (const uri of byUri.keys()) this.publishedUris.add(uri);
    for (const uri of targets) {
      this.send({
        jsonrpc: "2.0",
        method: "textDocument/publishDiagnostics",
        params: { diagnostics: byUri.get(uri) ?? [], uri },
      });
    }
  }

  private diagnostic(uri: string, item: SemanticDiagnostic): object {
    return {
      code: item.code,
      message: item.message,
      range: this.lspRange(uri, item.range),
      severity: severity[item.severity],
      source: "semath",
    };
  }

  private workspaceEdit(proposal: SemanticEditProposal): object {
    const changes: Record<string, object[]> = {};
    for (const file of proposal.files) {
      const state = this.documents.get(file.fileId);
      const uri = state?.uri ?? uriFromPath(file.path);
      changes[uri] = file.edits.map((edit) => ({
        newText: edit.replacementText,
        range: this.lspRange(uri, edit.range),
      }));
    }
    return { changes };
  }

  private location(item: Location): object {
    const state = this.documents.get(item.fileId);
    return {
      range: this.lspRange(state?.uri ?? uriFromPath(item.path), item.range),
      uri: state?.uri ?? uriFromPath(item.path),
    };
  }

  private lspRange(uri: string, range: SourceRange): LspRange {
    const state = this.stateForUri(uri);
    return {
      end: positionAt(state.document.content, range.endOffset),
      start: positionAt(state.document.content, range.startOffset),
    };
  }

  private locate(params: TextDocumentPositionParams): {
    column: number;
    line: number;
    path: string;
  } {
    return {
      column: params.position.character + 1,
      line: params.position.line + 1,
      path: this.stateForUri(params.textDocument.uri).document.path,
    };
  }

  private stateForUri(uri: string): DocumentState {
    const fileId = this.fileIdsByUri.get(uri);
    const state = fileId ? this.documents.get(fileId) : undefined;
    if (!state) throw new Error(`document is not open: ${uri}`);
    return state;
  }

  private respond(id: JsonRpcMessage["id"], result: unknown): void {
    if (id == null) return;
    if (this.cancelled.delete(id)) return;
    this.send({ id, jsonrpc: "2.0", result });
  }

  private respondError(
    id: number | string,
    code: number,
    message: string,
  ): void {
    if (this.cancelled.delete(id)) return;
    this.send({ error: { code, message }, id, jsonrpc: "2.0" });
  }
}

export async function createSemathLspServer(
  send: SendMessage,
  options: Omit<SemathLspServerOptions, "semath"> = {},
): Promise<SemathLspServer> {
  const wasm = await import("../../../lib/wasm/semath_wasm.js");
  const semath = await SemathWorkerEngine.create(async () => ({
    SemathEngine: wasm.SemathEngine,
    default: wasm.default,
  }));
  return new SemathLspServer(send, { ...options, semath });
}

function languageOf(uri: string, languageId?: string): DocumentLanguage {
  if (languageId === "markdown" || /\.md$/i.test(uri)) return "markdown";
  if (languageId === "bibtex" || /\.bib$/i.test(uri)) return "bibtex";
  return "latex";
}

function lineStarts(content: string): number[] {
  const starts = [0];
  for (let index = 0; index < content.length; index++) {
    if (content.charCodeAt(index) === 10) starts.push(index + 1);
  }
  return starts;
}

export function offsetAt(content: string, position: LspPosition): number {
  const starts = lineStarts(content);
  const line = Math.max(0, Math.min(position.line, starts.length - 1));
  const start = starts[line]!;
  const next = starts[line + 1] ?? content.length;
  return Math.max(start, Math.min(start + position.character, next));
}

export function positionAt(content: string, offset: number): LspPosition {
  const starts = lineStarts(content);
  const clamped = Math.max(0, Math.min(offset, content.length));
  let line = 0;
  while (line + 1 < starts.length && starts[line + 1]! <= clamped) line++;
  return { character: clamped - starts[line]!, line };
}

function mapRange(range: NeutralRange): LspRange {
  return {
    end: { character: range.endColumn - 1, line: range.endLine - 1 },
    start: { character: range.startColumn - 1, line: range.startLine - 1 },
  };
}

function mapHover(hover: NeutralHover | null): object | null {
  return hover
    ? {
        contents: { kind: "markdown", value: hover.contents.join("\n\n") },
        range: mapRange(hover.range),
      }
    : null;
}

function mapLocation(location: NeutralLocation): object {
  return { range: mapRange(location.range), uri: uriFromPath(location.file) };
}

function mapWorkspaceEdit(
  edit: LatexWorkspaceEdit | null | undefined,
): object | null {
  if (!edit) return null;
  const changes: Record<string, object[]> = {};
  for (const item of edit.edits) {
    const uri = uriFromPath(item.file);
    (changes[uri] ??= []).push({
      newText: item.newText,
      range: {
        end: {
          character: item.range.endColumn - 1,
          line: item.range.endLineNumber - 1,
        },
        start: {
          character: item.range.startColumn - 1,
          line: item.range.startLineNumber - 1,
        },
      },
    });
  }
  return { changes };
}

function mapCompletion(
  item: NeutralCompletionItem,
  position: LspPosition,
): object {
  return {
    detail: item.detail,
    documentation: item.documentation,
    insertTextFormat: item.snippet ? 2 : 1,
    kind: item.kind === "reference" ? 18 : item.kind === "file" ? 17 : 3,
    label: item.label,
    textEdit: {
      newText: item.insertText,
      range: item.replacementRange
        ? mapRange(item.replacementRange)
        : {
            end: position,
            start: {
              character: Math.max(0, position.character - item.replaceLength),
              line: position.line,
            },
          },
    },
  };
}

function dedupe(items: object[]): object[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const candidate = item as {
      label?: string;
      textEdit?: { newText?: string };
    };
    const key = `${candidate.label ?? ""}\u0000${candidate.textEdit?.newText ?? ""}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function capabilities(): object {
  return {
    completionProvider: { triggerCharacters: ["\\", "{", "[", ",", "=", "@"] },
    definitionProvider: true,
    hoverProvider: true,
    referencesProvider: true,
    renameProvider: { prepareProvider: true },
    selectionRangeProvider: true,
    textDocumentSync: 1,
    workspace: {
      fileOperations: {
        didRename: { filters: [{ pattern: { glob: "**/*" } }] },
      },
    },
  };
}
