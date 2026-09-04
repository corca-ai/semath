import type {
  LatexDocumentSyntaxSnapshot,
  LatexInclude,
  LatexMacroEvent,
} from "wasmtex/syntax";

export const SEMATH_PROTOCOL_VERSION = 18 as const;
export const WASMTEX_SYNTAX_SCHEMA_VERSION = 8 as const;
export const MATH_INTERPRETATION_HYPOTHESIS_LIMIT = 16 as const;

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

export type FormulaDisposition =
  | "established"
  | "partial"
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

export type FormulaRequirementInfo =
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
  | "structural-alternative";

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
  | "derived-evidence";

export interface MathInterpretationEvidenceInfo {
  evidence: Evidence;
  provenance: MathInterpretationEvidenceProvenance;
  role: "supporting" | "contradicting";
  sourceAnchors: readonly MathInterpretationEvidenceSourceAnchorInfo[];
}

export interface MathInterpretationEvidenceReferenceInfo {
  evidence: Evidence;
  sourceAnchors: readonly MathInterpretationEvidenceSourceAnchorInfo[];
}

export interface MathInterpretationEvidenceSourceAnchorInfo {
  documentVersion: number;
  generation: "authored" | "generated";
  lifecycle: "current" | "retracted";
  location: Location;
  scopePath: readonly number[];
}

export interface MathInterpretationConditionInfo {
  conditionId: string;
  evidence: readonly MathInterpretationEvidenceReferenceInfo[];
  kind: LawConditionInfo["kind"];
  label: string;
  operatorProperty?: LawConditionInfo["operatorProperty"];
  status: LawConditionInfo["status"];
  subjects: readonly string[];
}

export interface MathInterpretationDomainRelevanceInfo {
  evidence: readonly MathInterpretationEvidenceReferenceInfo[];
  support: DomainSupportTier;
}

export interface MathInterpretationAlternativeInfo {
  alternativeId: string;
  evidence: readonly MathInterpretationEvidenceReferenceInfo[];
  label: string;
  range: SourceRange;
  relevance?: MathInterpretationDomainRelevanceInfo;
}

export type MathInterpretationRequirementInfo =
  | {
      evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      kind: "declaration";
      occurrenceId: SourceOccurrenceId;
      requirementId: string;
      symbol: string;
    }
  | {
      constraint: SemanticConstraint;
      evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      kind: "role-declaration";
      parameter: string;
      requirementId: string;
      symbol: string;
    }
  | {
      condition: MathInterpretationConditionInfo;
      kind: "condition";
      requirementId: string;
    }
  | {
      alternatives: readonly MathInterpretationAlternativeInfo[];
      evidence: readonly MathInterpretationEvidenceReferenceInfo[];
      kind: "disambiguation";
      requirementId: string;
    };

export type MathInterpretationOrderingReasonKind =
  | "explicit-evidence"
  | "typed-evidence"
  | "derived-evidence"
  | "domain-relevance"
  | "stable-source-order";

export interface MathInterpretationOrderingReason {
  evidence: readonly MathInterpretationEvidenceReferenceInfo[];
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
  | "discriminator-set-capped"
  | "engine-limit"
  | "generated-source"
  | "retracted-source";

export interface MathInterpretationAnalysisLimitInfo {
  evidence: readonly MathInterpretationEvidenceReferenceInfo[];
  kind: MathInterpretationAnalysisLimitKind;
}

export interface MathInterpretationCandidateCapInfo {
  candidateCountBeforeCap: number;
  preCapSemanticKeyDigest: string;
}

export function parseMathInterpretationCandidateCapInfo(
  value: unknown,
): MathInterpretationCandidateCapInfo {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("candidateCap must be an object");
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (
    keys.length !== 2 ||
    keys[0] !== "candidateCountBeforeCap" ||
    keys[1] !== "preCapSemanticKeyDigest"
  ) {
    throw new TypeError("candidateCap must contain exactly the protocol-17 fields");
  }
  const count = record.candidateCountBeforeCap;
  const digest = record.preCapSemanticKeyDigest;
  if (
    typeof count !== "number" ||
    !Number.isSafeInteger(count) ||
    count > 0xffff_ffff ||
    count <= MATH_INTERPRETATION_HYPOTHESIS_LIMIT
  ) {
    throw new TypeError(
      "candidateCountBeforeCap must be a u32 integer above the hypothesis limit",
    );
  }
  if (typeof digest !== "string" || !/^[0-9a-f]{64}$/u.test(digest)) {
    throw new TypeError(
      "preCapSemanticKeyDigest must be 64 lowercase hexadecimal characters",
    );
  }
  return {
    candidateCountBeforeCap: count,
    preCapSemanticKeyDigest: digest,
  };
}

export interface MathInterpretationPreCapSourceIdentity {
  documentVersion: number;
  generation: "authored" | "generated";
  lifecycle: "current" | "retracted";
  location: Location;
}

export interface MathInterpretationPreCapBindingKey {
  parameter: string;
  symbol: string;
}

export interface MathInterpretationPreCapConditionKey {
  conditionId: string;
  status: LawConditionInfo["status"];
}

export interface MathInterpretationPreCapEvidenceKey {
  provenance: MathInterpretationEvidenceProvenance;
  role: MathInterpretationEvidenceInfo["role"];
  sourceAnchors: readonly MathInterpretationPreCapSourceIdentity[];
}

export interface MathInterpretationPreCapSemanticKey {
  bindings: readonly MathInterpretationPreCapBindingKey[];
  conditions: readonly MathInterpretationPreCapConditionKey[];
  evidence: readonly MathInterpretationPreCapEvidenceKey[];
  formulaSource: MathInterpretationPreCapSourceIdentity;
  kind: MathInterpretationKind;
  label: string;
  relationId: string | null;
  support: MathInterpretationSupportTier;
}

/**
 * Canonical protocol-17 input to `SHA-256`: recursively sorted object keys,
 * canonicalized binding/condition/evidence arrays, then a lexically sorted
 * JSON array of distinct semantic-key JSON strings. Duplicate derivation paths
 * and opaque IDs therefore cannot create a cap. Encode the returned string as
 * UTF-8 before hashing and publish the lowercase 64-character hexadecimal
 * digest.
 */
export function canonicalMathInterpretationPreCapPayload(
  values: readonly MathInterpretationPreCapSemanticKey[],
): string {
  const keys = [
    ...new Set(values.map(canonicalMathInterpretationSemanticKey)),
  ].sort(compareUtf8);
  return JSON.stringify(keys);
}

export async function mathInterpretationPreCapSemanticKeyDigest(
  values: readonly MathInterpretationPreCapSemanticKey[],
): Promise<string> {
  const payload = canonicalMathInterpretationPreCapPayload(values);
  const digest = await globalThis.crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(payload),
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

function canonicalMathInterpretationSemanticKey(
  value: MathInterpretationPreCapSemanticKey,
): string {
  const bindings = value.bindings
    .map(({ parameter, symbol }) => ({ parameter, symbol }))
    .sort(stableJsonOrder);
  const conditions = value.conditions
    .map(({ conditionId, status }) => ({ conditionId, status }))
    .sort(stableJsonOrder);
  const evidence = value.evidence
    .map(({ provenance, role, sourceAnchors }) => ({
      provenance,
      role,
      sourceAnchors: sourceAnchors
        .map(({ documentVersion, generation, lifecycle, location }) => ({
          documentVersion,
          generation,
          lifecycle,
          location,
        }))
        .sort(stableJsonOrder),
    }))
    .sort(stableJsonOrder);
  const { documentVersion, generation, lifecycle, location } = value.formulaSource;
  return stableJson({
    bindings,
    conditions,
    evidence,
    formulaSource: { documentVersion, generation, lifecycle, location },
    kind: value.kind,
    label: value.label,
    relationId: value.relationId,
    support: value.support,
  });
}

function stableJsonOrder(left: unknown, right: unknown): number {
  return compareUtf8(stableJson(left), stableJson(right));
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value)
      .filter(([, child]) => child !== undefined)
      .sort(([left], [right]) => compareUtf8(left, right))
      .map(([key, child]) => `${JSON.stringify(key)}:${stableJson(child)}`)
      .join(",")}}`;
  }
  const serialized = JSON.stringify(value);
  if (serialized === undefined) {
    throw new TypeError("pre-cap semantic keys must contain JSON values");
  }
  return serialized;
}

function compareUtf8(left: string, right: string): number {
  const encoder = new TextEncoder();
  const leftBytes = encoder.encode(left);
  const rightBytes = encoder.encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] ?? 0) - (rightBytes[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

export interface MathInterpretationSetInfo {
  analysisLimits: readonly MathInterpretationAnalysisLimitInfo[];
  candidateCap?: MathInterpretationCandidateCapInfo;
  exhaustiveness: "bounded-open-world";
  hypotheses: readonly MathInterpretationHypothesisInfo[];
  missingDiscriminators: readonly MathInterpretationRequirementInfo[];
  truncated: boolean;
}

export interface FormulaAnalysisInfo {
  approximation?: MathApproximationInfo;
  claimEvidence: readonly MathClaimEvidenceLinkInfo[];
  conditions: readonly LawConditionInfo[];
  disposition: FormulaDisposition;
  equationLinks: readonly MathEquationLinkInfo[];
  formula?: MathFormulaAnchorInfo;
  lifecycle: MathSourceLifecycleInfo;
  interpretations: MathInterpretationSetInfo;
  notationOccurrences: readonly MathNotationOccurrenceInfo[];
  requirements: readonly MathInterpretationRequirementInfo[];
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
  formulaAnalysis: FormulaAnalysisInfo;
  context: SemanticContextInfo;
  decision: MeaningDecision;
  declarations: readonly Location[];
  diagnostics: readonly SemanticDiagnostic[];
  domains: readonly DomainActivation[];
  /** Rust `Option` serializes an absent symbol as `null` on the JSON wire. */
  symbol?: SymbolInfo | null;
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
