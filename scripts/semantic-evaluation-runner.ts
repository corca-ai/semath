import { spawnSync } from "node:child_process";
import { LatexSyntaxService } from "wasmtex/syntax";
import type { CorpusDocument } from "./testing/foundation";
import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type QueryEnvelope,
  type QueryResult,
} from "../packages/protocol/src/index";
import { adaptWasmtexDocument } from "../packages/wasmtex-adapter/src/index";

export interface SemanticEvaluationCase {
  readonly cursor: {
    readonly edge?: "after" | "before";
    readonly fileId: string;
    readonly needle: string;
    readonly occurrence?: number;
    readonly offset?: number;
  };
  readonly documents: readonly CorpusDocument[];
  readonly id: string;
}

export function semanticEvaluationCursorOffset(
  content: string,
  cursor: SemanticEvaluationCase["cursor"],
): number {
  const matches: number[] = [];
  for (
    let offset = content.indexOf(cursor.needle);
    offset >= 0;
    offset = content.indexOf(
      cursor.needle,
      offset + Math.max(cursor.needle.length, 1),
    )
  ) {
    matches.push(offset);
  }
  const first = matches[cursor.occurrence ?? 0];
  if (
    first === undefined ||
    (matches.length > 1 && cursor.occurrence === undefined)
  ) {
    throw new Error(
      "cursor needle must be unique or select a valid occurrence",
    );
  }
  if (cursor.offset !== undefined) return first + cursor.offset;
  return cursor.edge === "after" ? first + cursor.needle.length : first;
}

export function runSemanticEvaluation(
  cases: readonly SemanticEvaluationCase[],
  epoch: string,
): QueryResult[] {
  const documents: ProjectDocument[] = [];
  const queries: QueryEnvelope[] = [];
  for (const item of cases) {
    const prefix = `${item.id}/`;
    const inputs = item.documents.map((document) => ({
      ...document,
      fileId: prefix + document.fileId,
      path: prefix + document.path,
    }));
    const syntax = new LatexSyntaxService();
    syntax.reset({
      documents: inputs.map((document) => ({
        ...document,
        documentVersion: 1,
      })),
    });
    for (const input of inputs) {
      const snapshot = syntax.getFile(input.fileId);
      if (!snapshot)
        throw new Error(`${item.id}: missing syntax for ${input.fileId}`);
      documents.push(
        adaptWasmtexDocument({
          content: input.content,
          language: languageOf(input.path),
          syntax: snapshot,
        }),
      );
    }
    const cursorDocument = inputs.find(
      (document) => document.fileId === prefix + item.cursor.fileId,
    );
    if (!cursorDocument) throw new Error(`${item.id}: unknown cursor document`);
    let offset: number;
    try {
      offset = semanticEvaluationCursorOffset(
        cursorDocument.content,
        item.cursor,
      );
    } catch {
      throw new Error(`${item.id}: ambiguous cursor needle`);
    }
    queries.push({
      analysisGeneration: 0,
      documentVersion: 1,
      epoch,
      inventoryVersion: 1,
      protocolVersion: SEMATH_PROTOCOL_VERSION,
      query: {
        fileId: prefix + item.cursor.fileId,
        kind: "semanticView",
        offset,
      },
    });
  }

  const native = spawnSync(
    "cargo",
    ["run", "--quiet", "--locked", "-p", "semath-native"],
    {
      encoding: "utf8",
      input: JSON.stringify({
        queries,
        snapshot: {
          documents,
          epoch,
          inventoryVersion: 1,
          projectId: epoch,
          protocolVersion: SEMATH_PROTOCOL_VERSION,
        },
      }),
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (native.status !== 0) {
    throw new Error(native.stderr || `native ${epoch} evaluation failed`);
  }
  const results: unknown = JSON.parse(native.stdout);
  if (!Array.isArray(results) || results.length !== cases.length) {
    throw new Error(
      `native ${epoch} evaluation returned ${Array.isArray(results) ? results.length : "invalid"}/${cases.length}`,
    );
  }
  return results as QueryResult[];
}

function languageOf(path: string): DocumentLanguage {
  return /\.md$/iu.test(path) ? "markdown" : "latex";
}
