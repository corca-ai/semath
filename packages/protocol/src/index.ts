export const SEMATH_PROTOCOL_VERSION = 1 as const;

export type DocumentLanguage = "bibtex" | "latex" | "markdown";

export interface SourceRange {
  endOffset: number;
  startOffset: number;
}

export interface MathRegion {
  closed: boolean;
  contentRange: SourceRange;
  delimiter: string;
  fullRange: SourceRange;
}

export interface ProjectDocument {
  content: string;
  documentVersion: number;
  fileId: string;
  language: DocumentLanguage;
  mathRegions?: readonly MathRegion[];
  path: string;
}

export interface ProjectSnapshot {
  documents: readonly ProjectDocument[];
  epoch: string;
  inventoryVersion: number;
  mainFileId?: string;
  projectId: string;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
}

export type ProjectChange =
  | { document: ProjectDocument; kind: "upsert" }
  | { fileId: string; kind: "path-change"; path: string }
  | { fileId: string; kind: "remove" };

export interface ChangeEnvelope {
  analysisGeneration: number;
  changes: readonly ProjectChange[];
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
}

export type SemathQuery =
  | { fileId: string; kind: "selection"; offset: number }
  | { fileId: string; kind: "equationTree"; offset: number }
  | { fileId: string; kind: "hover"; offset: number }
  | { fileId: string; kind: "symbolInfo"; offset: number }
  | { fileId: string; kind: "definition"; offset: number }
  | { fileId: string; kind: "references"; offset: number }
  | { fileId: string; kind: "prepareRename"; offset: number }
  | { fileId: string; kind: "rename"; newName: string; offset: number }
  | { fileId: string; kind: "diagnostics" }
  | {
      code: string;
      fileId: string;
      kind: "explainDiagnostic";
      offset: number;
    }
  | { fileId: string; kind: "formulaRecognition"; offset: number }
  | { fileId: string; kind: "formulaCompletion"; offset: number }
  | { fileId: string; kind: "domainEvidence"; offset: number };

export interface QueryEnvelope {
  analysisGeneration: number;
  documentVersion: number;
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  query: SemathQuery;
}

export interface EquationNode {
  children: readonly EquationNode[];
  kind: string;
  label?: string;
  range: SourceRange;
}

export interface Location {
  fileId: string;
  path: string;
  range: SourceRange;
}

export interface Evidence {
  kind: string;
  ruleId: string;
  sourceRanges: readonly SourceRange[];
  strength: string;
}

export interface DefinitionInfo {
  description: string;
  evidence: Evidence;
  location: Location;
  symbol: string;
}

export interface ShapeInfo {
  dimensions: readonly string[];
  display: string;
  evidence: Evidence;
  kind: "matrix" | "scalar" | "tensor" | "vector";
  refinements?: readonly string[];
  symbol: string;
}

export interface SymbolInfo {
  definitions: readonly DefinitionInfo[];
  diagnostics: readonly SemanticDiagnostic[];
  formulas: readonly FormulaRecognition[];
  location: Location;
  shapes: readonly ShapeInfo[];
  symbol: string;
  truncated: boolean;
}

export interface DomainActivation {
  evidence: readonly Evidence[];
  packId: string;
  packVersion: string;
  scopeKind: "document" | "equation" | "section";
  scopeRange: SourceRange;
  strength: "strong" | "weak";
  title: string;
}

export interface SemanticDiagnostic {
  code: string;
  evidence: readonly Evidence[];
  explanation: string;
  message: string;
  range: SourceRange;
  severity: "error" | "warning";
}

export interface FormulaConstraint {
  dimensions?: readonly string[];
  kind: "matrix" | "scalar" | "tensor" | "vector";
  refinements?: readonly string[];
}

export interface FormulaParameter {
  constraint: FormulaConstraint;
  id: string;
}

export interface FormulaSideCondition {
  kind: string;
  left: string;
  right: string;
}

export interface FormulaPattern {
  generationTemplate: string;
  id: string;
  matcher: string;
  packId: string;
  packVersion: string;
  parameters: readonly FormulaParameter[];
  result: FormulaConstraint;
  schemaVersion: number;
  sideConditions: readonly FormulaSideCondition[];
  title: string;
}

export interface FormulaBinding {
  constraint: FormulaConstraint;
  evidence: Evidence;
  parameter: string;
  symbol: string;
}

export interface FormulaRecognition {
  bindings: readonly FormulaBinding[];
  evidence: readonly Evidence[];
  packId: string;
  packVersion: string;
  patternId: string;
  range: SourceRange;
  rank: number;
  result: FormulaConstraint;
  title: string;
}

export interface FormulaCompletion {
  detail: string;
  patternId: string;
  proposal: SemanticEditProposal;
  rank: number;
  title: string;
}

export interface SemanticTextEdit {
  expectedText: string;
  range: SourceRange;
  replacementText: string;
}

export interface SemanticEditFile {
  documentVersion: number;
  edits: readonly SemanticTextEdit[];
  fileId: string;
  path: string;
}

export interface SemanticEditProposal {
  evidence: readonly Evidence[];
  files: readonly SemanticEditFile[];
  safety: "deterministic" | "review-required";
  title: string;
}

export type QueryValue =
  | { kind: "selection"; ranges: readonly SourceRange[] }
  | { kind: "equationTree"; tree?: EquationNode }
  | {
      definitions: readonly DefinitionInfo[];
      equationKind?: string;
      kind: "hover";
      formulas?: readonly FormulaRecognition[];
      shape?: ShapeInfo;
      symbol?: string;
    }
  | { info?: SymbolInfo; kind: "symbolInfo" }
  | { kind: "locations"; locations: readonly Location[] }
  | {
      kind: "renamePreparation";
      placeholder?: string;
      range?: SourceRange;
      rejection?: string;
    }
  | {
      kind: "editProposal";
      proposal?: SemanticEditProposal;
      rejection?: string;
    }
  | { diagnostics: readonly SemanticDiagnostic[]; kind: "diagnostics" }
  | {
      diagnostic?: SemanticDiagnostic;
      kind: "diagnosticExplanation";
    }
  | {
      kind: "formulaRecognitions";
      recognitions: readonly FormulaRecognition[];
    }
  | {
      completions: readonly FormulaCompletion[];
      kind: "formulaCompletions";
    }
  | {
      activations: readonly DomainActivation[];
      kind: "domainActivations";
      truncated: boolean;
    };

export interface QueryResult {
  analysisGeneration: number;
  documentVersion: number;
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  value: QueryValue;
}
