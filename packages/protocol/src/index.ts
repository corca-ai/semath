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

export interface ProjectInclude {
  path: string;
  sourceRange: SourceRange;
}

export interface ProjectDocument {
  content: string;
  documentVersion: number;
  fileId: string;
  language: DocumentLanguage;
  includes?: readonly ProjectInclude[];
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
  | { fileId: string; kind: "formulaRewrite"; offset: number }
  | { fileId: string; kind: "domainEvidence"; offset: number }
  | { fileId: string; kind: "inspection"; offset: number };

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

export interface EquationNodeSummary {
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
  semanticId?: SemanticSymbolId;
  symbol: string;
}

/** Stable within one project snapshot; stable file identity survives path moves. */
export interface SemanticSymbolId {
  anchor: number;
  componentId: string;
  fileId: string;
  kind: string;
  scopePath: readonly number[];
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
  roles?: readonly RoleInfo[];
  semanticId?: SemanticSymbolId;
  shapes: readonly ShapeInfo[];
  symbol: string;
  truncated: boolean;
}

export interface RoleInfo {
  description: string;
  evidence: Evidence;
  role:
    | "distribution"
    | "event"
    | "function"
    | "index"
    | "operator"
    | "random-variable"
    | "set";
  symbol: string;
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
  severity: "error" | "hint" | "warning";
}

export interface FormulaConstraint {
  dimensions?: readonly string[];
  kind:
    | "distribution"
    | "event"
    | "expression"
    | "function"
    | "graph"
    | "index"
    | "matrix"
    | "proposition"
    | "random-variable"
    | "scalar"
    | "set"
    | "tensor"
    | "vector";
  refinements?: readonly string[];
}

export interface FormulaParameter {
  constraint: FormulaConstraint;
  id: string;
  optional?: boolean;
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

export interface FormulaConditionInfo {
  kind: string;
  label: string;
  status: "missing" | "verified";
}

export interface FormulaRecognition {
  bindings: readonly FormulaBinding[];
  evidence: readonly Evidence[];
  /** Additive v0.11 metadata; absent in protocol-v1 results from older engines. */
  conditions?: readonly FormulaConditionInfo[];
  description?: string;
  descriptionKey?: string;
  maturity?: "completion" | "diagnostic" | "recognition" | "rewrite";
  packId: string;
  packVersion: string;
  patternId: string;
  range: SourceRange;
  rank: number;
  result: FormulaConstraint;
  status?: "condition-missing" | "recognized" | "verified";
  title: string;
}

export interface FormulaCompletion {
  detail: string;
  patternId: string;
  proposal: SemanticEditProposal;
  rank: number;
  title: string;
}

export interface FormulaRewrite {
  detail: string;
  proposal: SemanticEditProposal;
  rank: number;
  ruleId: string;
  title: string;
}

export interface RenamePreparation {
  placeholder?: string;
  range?: SourceRange;
  rejection?: string;
}

export interface InspectionInfo {
  completions: readonly FormulaCompletion[];
  diagnostics: readonly SemanticDiagnostic[];
  domains: readonly DomainActivation[];
  equation?: EquationNode;
  recognitions: readonly FormulaRecognition[];
  references: readonly Location[];
  rename: RenamePreparation;
  rewrites: readonly FormulaRewrite[];
  selectionPath: readonly EquationNodeSummary[];
  symbol?: SymbolInfo;
  truncated: boolean;
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
      roles?: readonly RoleInfo[];
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
      kind: "formulaRewrites";
      rewrites: readonly FormulaRewrite[];
    }
  | {
      activations: readonly DomainActivation[];
      kind: "domainActivations";
      truncated: boolean;
    }
  | { inspection: InspectionInfo; kind: "inspection" };

export interface QueryResult {
  analysisGeneration: number;
  documentVersion: number;
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  value: QueryValue;
}

export type SemathWorkerPriority = "background" | "cursor" | "mutation";

export type SemathWorkerRequest =
  | {
      id: number;
      kind: "reset";
      priority?: SemathWorkerPriority;
      snapshot: ProjectSnapshot;
    }
  | {
      changes: ChangeEnvelope;
      id: number;
      kind: "change";
      priority?: SemathWorkerPriority;
    }
  | {
      envelope: QueryEnvelope;
      id: number;
      kind: "query";
      priority?: SemathWorkerPriority;
    }
  | { kind: "cancel"; requestId: number }
  | { id: number; kind: "dispose" };

export type SemathWorkerErrorCode =
  | "disposed"
  | "engine-failed"
  | "initialization-failed"
  | "runtime-failed"
  | "stale-generation";

export type SemathWorkerResponse =
  | { id: number; kind: "result"; result: unknown }
  | { id: number; kind: "cancelled" }
  | {
      error: {
        code: SemathWorkerErrorCode;
        message: string;
        recoverable: boolean;
      };
      id: number;
      kind: "error";
    }
  | { id: number; kind: "disposed" };
