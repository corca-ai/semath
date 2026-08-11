import {
  SEMATH_PROTOCOL_VERSION,
  type DocumentLanguage,
  type ProjectDocument,
  type ProjectSnapshot,
} from "../../protocol/src/index";
import {
  assertLatexSyntaxSchemaVersion,
  type LatexFileSyntax,
} from "wasmtex/syntax";

export interface SourceDocument {
  content: string;
  language: DocumentLanguage;
  syntax: LatexFileSyntax;
}

export function adaptWasmtexDocument(source: SourceDocument): ProjectDocument {
  assertLatexSyntaxSchemaVersion(source.syntax);
  return {
    content: source.content,
    language: source.language,
    ...source.syntax,
  };
}

export function adaptNonLatexDocument(input: {
  content: string;
  documentVersion: number;
  fileId: string;
  language: "bibtex";
  path: string;
}): ProjectDocument {
  return {
    ...input,
    declarations: [],
    includes: [],
    macros: [],
    mathRoots: [],
    nodes: [],
    proseAnnotations: [],
    schemaVersion: 7,
    scopes: [
      {
        kind: "document",
        parent: null,
        range: {
          startOffset: 0,
          endOffset: input.content.length,
        },
        state: "complete",
      },
    ],
    visibleProse: [],
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
