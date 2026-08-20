import type {
  LatexDocumentSyntaxSnapshot,
  LatexInclude,
  LatexMacroEvent,
} from "wasmtex/syntax";

export const SEMATH_PROTOCOL_VERSION = 16 as const;
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
  | {
      fileId: string;
      includeDeclaration?: boolean;
      kind: "references";
      offset: number;
    }
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
    | "maps-between"
    | "nonzero"
    | "operator-property"
    | "positive"
    | "rank-compatible"
    | "same-context"
    | "shape-compatible"
    | "sign-convention"
    | "uniform";
  label: string;
  operatorProperty?:
    | "adjoint"
    | "bilinear"
    | "gradient"
    | "hessian"
    | "inner-product"
    | "jacobian"
    | "linear"
    | "norm";
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

export type ConventionalRequirementInfo =
  | {
      constraint: SemanticConstraint;
      evidence: readonly Evidence[];
      kind: "role-declaration";
      parameter: string;
      requirementId: string;
      symbol: string;
    }
  | {
      condition: LawConditionInfo;
      kind: "condition";
      requirementId: string;
    };

export interface ConventionalCandidateInfo {
  bindings: readonly LawBinding[];
  candidateId: string;
  disposition: "conventional-candidate";
  evidence: readonly Evidence[];
  lawId: string;
  packId: string;
  packVersion: string;
  relation: RelationInfo;
  relevance: DomainRelevance;
  requirements: readonly ConventionalRequirementInfo[];
  title: string;
}

export type MathAuthoringDisposition =
  | "established"
  | "partial"
  | "conventional"
  | "ambiguous"
  | "conflicting"
  | "unsupported"
  | "engine-limited";

export interface MathSourceLifecycleInfo {
  capped: boolean;
  documentVersion: number;
  editable: boolean;
  engineLimited: boolean;
  freshness: "current";
  generation: "authored" | "generated";
  retracted: boolean;
}

export interface MathFormulaAnchorInfo {
  documentVersion: number;
  location: Location;
  provenance?: readonly SourceRange[];
  scopePath: readonly number[];
  sourceNotation: string;
}

export type MathAuthoringRequirementInfo =
  | {
      evidence: readonly Evidence[];
      kind: "declaration";
      occurrenceId: SourceOccurrenceId;
      requirementId: string;
      symbol: string;
    }
  | {
      constraint: SemanticConstraint;
      evidence: readonly Evidence[];
      kind: "role-declaration";
      parameter: string;
      requirementId: string;
      symbol: string;
    }
  | {
      condition: LawConditionInfo;
      kind: "condition";
      requirementId: string;
    }
  | {
      alternatives: readonly MeaningAlternative[];
      evidence: readonly Evidence[];
      kind: "disambiguation";
      requirementId: string;
    };

export interface MathEquationLinkInfo {
  evidence: readonly Evidence[];
  kind: "derived-law" | "shared-entity";
  linkId: string;
  sharedEntities: readonly EntityId[];
  source: MathFormulaAnchorInfo;
  target: MathFormulaAnchorInfo;
}

export interface MathApproximationInfo {
  evidence: readonly Evidence[];
  exactness: "approximate";
  relatedFactIds?: readonly string[];
  relationRange: SourceRange;
}

export interface MathClaimEvidenceLinkInfo {
  claim: Location;
  claimId: string;
  evidence: readonly Evidence[];
  modality: "asserted" | "cited" | "hedged" | "hypothetical" | "quoted";
  polarity: "negative" | "positive";
  strengthCeiling: "asserted" | "qualified" | "unusable";
  supportingClaimIds: readonly string[];
  supportingFormulas: readonly MathFormulaAnchorInfo[];
}

export interface MathNotationOccurrenceInfo {
  entityId: EntityId;
  location: Location;
  occurrenceId: SourceOccurrenceId;
  scopePath: readonly number[];
  sourceNotation: string;
}

export type MathInterpretationKind =
  | "source-meaning"
  | "typed-law"
  | "scoped-domain"
  | "structural-alternative"
  | "reviewed-convention";

export type MathInterpretationSupportTier =
  | "explicit"
  | "derived"
  | "supported"
  | "tentative"
  | "contradicted";

export type MathInterpretationEvidenceProvenance =
  | "explicit-declaration"
  | "typed-structure"
  | "natural-language-extraction"
  | "domain-context"
  | "reviewed-convention"
  | "derived-evidence";

export interface MathInterpretationEvidenceInfo {
  evidence: Evidence;
  provenance: MathInterpretationEvidenceProvenance;
  role: "supporting" | "contradicting";
}

export type MathInterpretationOrderingReasonKind =
  | "explicit-evidence"
  | "typed-evidence"
  | "derived-evidence"
  | "domain-relevance"
  | "reviewed-convention"
  | "stable-source-order";

export interface MathInterpretationOrderingReason {
  evidence: readonly Evidence[];
  kind: MathInterpretationOrderingReasonKind;
}

export interface MathInterpretationHypothesisInfo {
  bindings: readonly LawBinding[];
  conditions: readonly LawConditionInfo[];
  evidence: readonly MathInterpretationEvidenceInfo[];
  formula?: MathFormulaAnchorInfo;
  hypothesisId: string;
  kind: MathInterpretationKind;
  label: string;
  location: Location;
  documentVersion: number;
  missingDiscriminatorIds: readonly string[];
  orderingReasons: readonly MathInterpretationOrderingReason[];
  range: SourceRange;
  rank: number;
  relation?: RelationInfo;
  scopePath: readonly number[];
  support: MathInterpretationSupportTier;
}

export type MathInterpretationAnalysisLimitKind =
  | "candidate-set-capped"
  | "evidence-truncated"
  | "engine-limit"
  | "generated-source"
  | "retracted-source";

export interface MathInterpretationAnalysisLimitInfo {
  evidence: readonly Evidence[];
  kind: MathInterpretationAnalysisLimitKind;
}

export interface MathInterpretationSetInfo {
  analysisLimits: readonly MathInterpretationAnalysisLimitInfo[];
  exhaustiveness: "bounded-open-world";
  hypotheses: readonly MathInterpretationHypothesisInfo[];
  missingDiscriminators: readonly MathAuthoringRequirementInfo[];
  truncated: boolean;
}

export interface MathAuthoringContext {
  approximation?: MathApproximationInfo;
  claimEvidence: readonly MathClaimEvidenceLinkInfo[];
  conditions: readonly LawConditionInfo[];
  conventionalCandidates?: readonly ConventionalCandidateInfo[];
  disposition: MathAuthoringDisposition;
  equationLinks: readonly MathEquationLinkInfo[];
  formula?: MathFormulaAnchorInfo;
  lifecycle: MathSourceLifecycleInfo;
  interpretations: MathInterpretationSetInfo;
  notationOccurrences: readonly MathNotationOccurrenceInfo[];
  requirements: readonly MathAuthoringRequirementInfo[];
  truncated: boolean;
}

export type EntitySurfaceRefusalKind =
  | "unsupported"
  | "ambiguous"
  | "conflicting"
  | "engine-limit"
  | "incomplete-source"
  | "non-editable"
  | "invalid-replacement"
  | "capture";

export interface EntitySurfaceRefusal {
  kind: EntitySurfaceRefusalKind;
  message: string;
}

export type EntitySurfaceAuthorization =
  | {
      entityId: EntityId;
      focusOccurrenceId: SourceOccurrenceId;
      status: "authorized";
    }
  | { reason: EntitySurfaceRefusal; status: "refused" };

export interface SemanticViewInfo {
  authoringContext: MathAuthoringContext;
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
  | {
      authorization: EntitySurfaceAuthorization;
      kind: "locations";
      locations: readonly Location[];
    }
  | {
      authorization: EntitySurfaceAuthorization;
      kind: "renamePreparation";
      placeholder?: string;
      range?: SourceRange;
    }
  | {
      authorization: EntitySurfaceAuthorization;
      kind: "editProposal";
      proposal?: SemanticEditProposal;
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
