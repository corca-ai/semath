import type {
  LatexDocumentSyntaxSnapshot,
  LatexInclude,
  LatexMacroEvent,
} from "wasmtex/syntax";

export const SEMATH_PROTOCOL_VERSION = 13 as const;
export const WASMTEX_SYNTAX_SCHEMA_VERSION = 8 as const;

export type DocumentLanguage = "bibtex" | "latex" | "markdown";

export interface SourceRange {
  endOffset: number;
  startOffset: number;
}

export interface ProjectDocument extends LatexDocumentSyntaxSnapshot {
  content: string;
  documentVersion: number;
  fileId: string;
  language: DocumentLanguage;
  includes: readonly LatexInclude[];
  macros: readonly LatexMacroEvent[];
  path: string;
  schemaVersion: typeof WASMTEX_SYNTAX_SCHEMA_VERSION;
}

export interface ProjectSnapshot {
  documents: readonly ProjectDocument[];
  epoch: string;
  inventoryVersion: number;
  mainFileId?: string;
  projectId: string;
  protocolVersion: typeof SEMATH_PROTOCOL_VERSION;
}

export type ProjectSnapshotMetadata = Omit<ProjectSnapshot, "documents">;

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
  entityId?: EntityId;
  evidence: Evidence;
  location: Location;
  symbol: string;
}

/** One real source occurrence in a specific document revision. */
export interface SourceOccurrenceId {
  documentVersion: number;
  fileId: string;
  localId: number;
}

/** A scoped semantic entity anchored to source evidence. */
export interface EntityId {
  anchor: SourceOccurrenceId;
  componentId: string;
  kind: string;
  scopePath: readonly number[];
}

export type NotationComponent =
  | { kind: "identifier"; value: string }
  | { kind: "named-surface"; value: string }
  | { kind: "modifier"; name: string }
  | { kind: "style"; name: string }
  | { kind: "subscript"; base: string; index: string }
  | { kind: "superscript" }
  | { kind: "argument"; role: string }
  | { kind: "delimiter"; value: string };

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
  entityId?: EntityId;
  location: Location;
  notation: readonly NotationComponent[];
  occurrenceId: SourceOccurrenceId;
  sourceNotation: string;
  quantities?: readonly QuantityInfo[];
  roles?: readonly RoleInfo[];
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

export type DomainSupportTier = "explicit" | "supported" | "tentative";

export interface DomainRelevance {
  evidence: readonly Evidence[];
  support: DomainSupportTier;
}

export interface DomainActivation {
  evidence: readonly Evidence[];
  packId: string;
  packVersion: string;
  scopeKind: "document" | "equation" | "section";
  scopeRange: SourceRange;
  support: DomainSupportTier;
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

export interface SemanticCandidateInfo {
  candidateId: string;
  family: string;
  interpretation: string;
  status: "conflicting" | "rejected" | "supported" | "unresolved";
  range: SourceRange;
  supportingClaimIds: readonly string[];
  rejectingClaimIds: readonly string[];
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
  candidates: readonly SemanticCandidateInfo[];
  concepts: readonly ConceptInfo[];
  relations: readonly RelationInfo[];
  quantities: readonly QuantityInfo[];
  entityId?: EntityId;
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
  proof: "typed" | "derived" | "asserted" | "candidate";
  symbol: string;
}

export interface LawConditionInfo {
  conditionId: string;
  evidence: readonly Evidence[];
  kind:
    | "assumption"
    | "differentiable"
    | "domain-membership"
    | "nonzero"
    | "positive"
    | "same-context"
    | "shape-compatible"
    | "sign-convention"
    | "uniform";
  label: string;
  status: "conflicting" | "required" | "unsupported" | "verified";
  subjects: readonly string[];
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
  relevance?: DomainRelevance;
  relation?: RelationInfo;
  result: SemanticConstraint;
  status: "condition-missing" | "conflicting" | "recognized" | "verified";
  title: string;
}

export interface RenamePreparation {
  placeholder?: string;
  range?: SourceRange;
  rejection?: string;
}

export interface SemanticViewInfo {
  context: SemanticContextInfo;
  decision: MeaningDecision;
  declarations: readonly Location[];
  diagnostics: readonly SemanticDiagnostic[];
  domains: readonly DomainActivation[];
  symbol?: SymbolInfo;
  truncated: boolean;
}

export type MeaningDecision =
  | {
      status: "established";
      meaning: MeaningConclusion;
      reasons: readonly DecisionReason[];
    }
  | {
      status: "partial";
      meaning: MeaningConclusion;
      facts: readonly MeaningFact[];
      requirements: readonly MeaningRequirement[];
      reasons: readonly DecisionReason[];
    }
  | {
      status: "ambiguous";
      alternatives: readonly MeaningAlternative[];
      reasons: readonly DecisionReason[];
    }
  | {
      status: "conflicting";
      conflicts: readonly MeaningConflict[];
      reasons: readonly DecisionReason[];
    }
  | {
      status: "unsupported";
      reasons: readonly DecisionReason[];
    };

export interface MeaningConclusion {
  label: string;
  relationId: string | null;
}

export type DecisionReasonKind =
  | "proof"
  | "uncertainty"
  | "engine-limit"
  | "source-conflict";

export interface DecisionReason {
  evidence: readonly Evidence[];
  kind: DecisionReasonKind;
  label: string;
}

export interface MeaningFact {
  evidence: readonly Evidence[];
  factId: string;
  label: string;
}

export interface MeaningRequirement {
  evidence: readonly Evidence[];
  label: string;
  requirementId: string;
  subjects: readonly string[];
}

export interface MeaningAlternative {
  alternativeId: string;
  evidence: readonly Evidence[];
  label: string;
  range: SourceRange;
  relevance?: DomainRelevance;
}

export interface MeaningConflict {
  conflictId: string;
  evidence: readonly Evidence[];
  label: string;
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
  packFrontierCandidates: number;
  packLatentCandidates: number;
  packLatentFallbacks: number;
  domainHypotheses: number;
  domainEvidence: number;
  equivalenceStates: number;
  equivalenceGuardChecks: number;
  recognizedLaws: number;
  semanticNodes: number;
  semanticOccurrences: number;
  semanticEntities: number;
  semanticClaims: number;
  semanticEvidence: number;
  semanticDependencyEdges: number;
  invalidatedSemanticClaims: number;
  semanticCandidates: number;
  semanticConstraintWork: number;
  semanticDerivedClaims: number;
  semanticConstraintTruncated: boolean;
  proseClauses: number;
  proseConstructionCandidates: number;
  proseMatcherWork: number;
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
