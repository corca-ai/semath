export const SEMATH_PROTOCOL_VERSION = 4 as const;

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

export interface ProjectSourceRef {
  fileId: string;
  path: string;
  range: SourceRange;
}

export interface ProjectMacro {
  definitions: readonly ProjectSourceRef[];
  expansion: {
    depth: number;
    editable: boolean;
    inputRange?: SourceRange;
    status:
      "cycle" | "expanded" | "not-applicable" | "truncated" | "unresolved";
    surface?: string;
  };
  kind: "call" | "definition";
  name: string;
  source: ProjectSourceRef;
}

export interface ProjectDocument {
  content: string;
  documentVersion: number;
  fileId: string;
  language: DocumentLanguage;
  includes: readonly ProjectInclude[];
  macros: readonly ProjectMacro[];
  mathRegions: readonly MathRegion[];
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
  | { fileId: string; kind: "semanticView"; offset: number }
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
    };

export interface QueryEnvelope {
  analysisGeneration: number;
  documentVersion: number;
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  query: SemathQuery;
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

export interface DimensionExponentInfo {
  base: string;
  denominator: number;
  numerator: number;
}

export interface PhysicalDimensionInfo {
  display: string;
  exponents: readonly DimensionExponentInfo[];
}

export interface QuantityInfo {
  derivedFrom: readonly string[];
  dimension: PhysicalDimensionInfo;
  display: string;
  evidence: Evidence;
  quantityKind?: string;
  quantityKindId?: string;
  symbol: string;
  unit?: string;
  unitId?: string;
}

export interface SymbolInfo {
  definitions: readonly DefinitionInfo[];
  diagnostics: readonly SemanticDiagnostic[];
  location: Location;
  quantities?: readonly QuantityInfo[];
  roles?: readonly RoleInfo[];
  semanticId?: SemanticSymbolId;
  shapes: readonly ShapeInfo[];
  symbol: string;
  truncated: boolean;
}

export interface RoleInfo {
  conceptId: string;
  description: string;
  evidence: Evidence;
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

export interface ConceptInfo {
  conceptId: string;
  description: string;
  evidence: Evidence;
  label: string;
}

export interface AssumptionInfo {
  evidence: Evidence;
  kind: string;
  subjects?: readonly string[];
  value: string;
}

export type SemanticClaimStatus =
  "certain" | "supported" | "speculative" | "conflicting";

export interface SemanticClaimInfo {
  claimId: string;
  conflicts: readonly string[];
  evidence: readonly Evidence[];
  predicate: string;
  status: SemanticClaimStatus;
  value: string;
}

export interface RelationRoleInfo {
  conceptId?: string;
  label: string;
  role: string;
  symbol: string;
}

export interface RelationInfo {
  conditions: readonly string[];
  description: string;
  evidence: readonly Evidence[];
  range: SourceRange;
  relationId: string;
  roles: readonly RelationRoleInfo[];
  title: string;
}

export interface SemanticContextInfo {
  assumptions?: readonly AssumptionInfo[];
  claims: readonly SemanticClaimInfo[];
  concepts: readonly ConceptInfo[];
  relations: readonly RelationInfo[];
  quantities: readonly QuantityInfo[];
  semanticId?: SemanticSymbolId;
  symbol?: string;
  truncated: boolean;
}

export interface SemanticDiagnostic {
  code: string;
  evidence: readonly Evidence[];
  explanation: string;
  message: string;
  range: SourceRange;
  severity: "error" | "hint" | "warning";
}

export interface SemanticConstraint {
  concepts?: readonly string[];
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

export interface LawBinding {
  constraint: SemanticConstraint;
  evidence: Evidence;
  parameter: string;
  symbol: string;
}

export interface LawConditionInfo {
  kind: string;
  label: string;
  status: "required";
}

export interface LawRecognition {
  bindings: readonly LawBinding[];
  evidence: readonly Evidence[];
  conditions: readonly LawConditionInfo[];
  description: string;
  descriptionKey: string;
  maturity: "completion" | "diagnostic" | "recognition" | "rewrite";
  packId: string;
  packVersion: string;
  lawId: string;
  range: SourceRange;
  rank: number;
  relation?: RelationInfo;
  result: SemanticConstraint;
  status: "condition-missing" | "recognized" | "verified";
  title: string;
}

export interface RenamePreparation {
  placeholder?: string;
  range?: SourceRange;
  rejection?: string;
}

export interface SemanticViewInfo {
  context: SemanticContextInfo;
  declarations: readonly Location[];
  diagnostics: readonly SemanticDiagnostic[];
  domains: readonly DomainActivation[];
  refusal?: string;
  status:
    "ambiguous" | "conflicting" | "established" | "partial" | "unsupported";
  summary: string;
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
  | { kind: "semanticView"; view: SemanticViewInfo }
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
    };

export interface QueryResult {
  analysisGeneration: number;
  documentVersion: number;
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  value: QueryValue;
}

export interface AnalysisStats {
  analyzedDocuments: number;
  constraints: number;
  lawRulesVisited: number;
  recognizedLaws: number;
  semanticNodes: number;
  totalDocuments: number;
}

export interface UpdateResult {
  analysisGeneration: number;
  analyzedFileIds: readonly string[];
  changedFileIds: readonly string[];
  epoch: string;
  inventoryVersion: number;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
  stats: AnalysisStats;
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
