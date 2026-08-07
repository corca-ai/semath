import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type ProjectSnapshot,
} from "../../protocol/src/index";

/** Structural subset of wasmtex/syntax kept separate from wasmtex's release cycle. */
export interface WasmtexFileSyntax {
  documentVersion: number;
  fileId: string;
  mathRegions: readonly {
    closed: boolean;
    contentRange: { endOffset: number; startOffset: number };
    delimiter: string;
    fullRange: { endOffset: number; startOffset: number };
  }[];
  path: string;
  schemaVersion: 1;
}

export interface SourceDocument {
  content: string;
  language: DocumentLanguage;
  syntax: WasmtexFileSyntax;
}

export function adaptWasmtexDocument(source: SourceDocument): ProjectDocument {
  return {
    content: source.content,
    documentVersion: source.syntax.documentVersion,
    fileId: source.syntax.fileId,
    language: source.language,
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
