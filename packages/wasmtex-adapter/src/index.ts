import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type ProjectSnapshot,
} from "../../protocol/src/index";
import type { LatexFileSyntax } from "wasmtex/syntax";

export interface SourceDocument {
  content: string;
  language: DocumentLanguage;
  syntax: LatexFileSyntax;
}

export function adaptWasmtexDocument(source: SourceDocument): ProjectDocument {
  return {
    content: source.content,
    documentVersion: source.syntax.documentVersion,
    fileId: source.syntax.fileId,
    includes: source.syntax.includes.map((include) => ({
      path: include.path,
      sourceRange: include.source.range,
    })),
    language: source.language,
    macros: source.syntax.macros,
    mathRegions: source.syntax.mathRegions,
    path: source.syntax.path,
  };
}

export function createProjectSnapshot(input: {
  documents: readonly SourceDocument[];
  epoch: string;
  inventoryVersion: number;
  mainFileId?: string;
  projectId: string;
}): ProjectSnapshot {
  return {
    documents: input.documents.map(adaptWasmtexDocument),
    epoch: input.epoch,
    inventoryVersion: input.inventoryVersion,
    ...(input.mainFileId ? { mainFileId: input.mainFileId } : {}),
    projectId: input.projectId,
    protocolVersion: SEMATH_PROTOCOL_VERSION,
  };
}
