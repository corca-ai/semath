use serde::{Deserialize, Serialize};

use crate::semantic_index::{EntityId, NotationComponent, SourceOccurrenceId};

pub const PROTOCOL_VERSION: u32 = 5;
pub const WASMTEX_SYNTAX_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start_offset: u32,
    pub end_offset: u32,
}

impl SourceRange {
    pub fn contains(&self, offset: u32) -> bool {
        self.start_offset <= offset && offset < self.end_offset
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MathRegion {
    pub full_range: SourceRange,
    pub content_range: SourceRange,
    pub delimiter: String,
    pub closed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocument {
    pub schema_version: u32,
    pub file_id: String,
    pub path: String,
    pub language: DocumentLanguage,
    pub content: String,
    pub document_version: u64,
    pub nodes: Vec<NotationNode>,
    pub math_roots: Vec<MathRoot>,
    pub visible_prose: Vec<VisibleProseSpan>,
    pub scopes: Vec<SyntaxScope>,
    pub declarations: Vec<StructuralDeclaration>,
    #[cfg(test)]
    #[serde(default)]
    pub math_regions: Vec<MathRegion>,
    pub macros: Vec<ProjectMacro>,
    pub includes: Vec<ProjectInclude>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SyntaxState {
    Complete,
    Incomplete,
    Ambiguous,
    Opaque,
    Cyclic,
    Truncated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotationNodeKind {
    Token,
    Sequence,
    Group,
    Command,
    Script,
    Delimiter,
    Alignment,
    Environment,
    Modifier,
    Style,
    NamedOperator,
    Opaque,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotationNodeRanges {
    pub full: SourceRange,
    pub command: Option<SourceRange>,
    pub name: Option<SourceRange>,
    pub nucleus: Option<SourceRange>,
    pub editable: Option<SourceRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxProvenance {
    pub origin: String,
    pub source: ProjectSourceRef,
    pub call_site: Option<ProjectSourceRef>,
    #[serde(default)]
    pub definitions: Vec<ProjectSourceRef>,
    pub editable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotationArgument {
    pub node: u32,
    pub role: String,
    pub syntax: String,
    pub range: SourceRange,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotationNode {
    pub kind: NotationNodeKind,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub ranges: NotationNodeRanges,
    pub state: SyntaxState,
    pub name: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub arguments: Vec<NotationArgument>,
    pub math_class: Option<String>,
    pub provenance: Option<SyntaxProvenance>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MathRoot {
    pub node: u32,
    pub delimiter: String,
    pub full_range: SourceRange,
    pub content_range: SourceRange,
    pub state: MathRootState,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MathRootState {
    Complete,
    Incomplete,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisibleProseSpan {
    pub range: SourceRange,
    pub state: CompleteSyntaxState,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CompleteSyntaxState {
    Complete,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxScope {
    pub kind: String,
    pub parent: Option<u32>,
    pub range: SourceRange,
    pub state: MathRootState,
    pub name: Option<String>,
    pub level: Option<String>,
    pub source: Option<ProjectSourceRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum StructuralDeclaration {
    Class {
        name: String,
        options: String,
        source: ProjectSourceRef,
    },
    Package {
        name: String,
        options: String,
        source: ProjectSourceRef,
    },
    Environment {
        name: String,
        source: ProjectSourceRef,
    },
    Macro {
        name: String,
        parameters: Option<u32>,
        optional_default: Option<String>,
        body: Option<String>,
        body_source: Option<ProjectSourceRef>,
        source: ProjectSourceRef,
        state: Option<MathRootState>,
    },
    Operator {
        name: String,
        surface: String,
        limits: bool,
        source: ProjectSourceRef,
        name_source: ProjectSourceRef,
        surface_source: ProjectSourceRef,
        state: MathRootState,
    },
    PairedDelimiter {
        name: String,
        left: String,
        right: String,
        source: ProjectSourceRef,
        name_source: ProjectSourceRef,
        state: MathRootState,
    },
    Glossary {
        key: String,
        options: Vec<StructuralField>,
        fields: Vec<StructuralField>,
        source: ProjectSourceRef,
        key_source: ProjectSourceRef,
        state: MathRootState,
    },
    Acronym {
        key: String,
        short: String,
        long: String,
        options: Vec<StructuralField>,
        source: ProjectSourceRef,
        key_source: ProjectSourceRef,
        short_source: ProjectSourceRef,
        long_source: ProjectSourceRef,
        state: MathRootState,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuralField {
    pub name: String,
    pub value: String,
    pub source: ProjectSourceRef,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMacro {
    pub kind: ProjectMacroKind,
    pub name: String,
    pub source: ProjectSourceRef,
    pub definitions: Vec<ProjectSourceRef>,
    pub expansion: ProjectMacroExpansion,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectMacroKind {
    Definition,
    Call,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceRef {
    pub file_id: String,
    pub path: String,
    pub range: SourceRange,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMacroExpansion {
    pub status: ProjectMacroExpansionStatus,
    pub depth: u32,
    pub editable: bool,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub input_range: Option<SourceRange>,
    #[serde(default)]
    pub notation: Option<GeneratedNotationTree>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedNotationTree {
    pub nodes: Vec<GeneratedNotationNode>,
    pub root: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedNotationNode {
    pub kind: NotationNodeKind,
    pub children: Vec<u32>,
    pub state: SyntaxState,
    pub name: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub arguments: Vec<GeneratedNotationArgument>,
    pub math_class: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedNotationArgument {
    pub node: u32,
    pub role: String,
    pub syntax: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectMacroExpansionStatus {
    NotApplicable,
    Unresolved,
    Expanded,
    Cycle,
    Truncated,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInclude {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub source: ProjectSourceRef,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DocumentLanguage {
    Latex,
    Markdown,
    Bibtex,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub project_id: String,
    pub main_file_id: Option<String>,
    pub documents: Vec<ProjectDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshotMetadata {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub project_id: String,
    pub main_file_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectChange {
    Upsert {
        document: Box<ProjectDocument>,
    },
    PathChange {
        #[serde(rename = "fileId")]
        file_id: String,
        path: String,
    },
    Remove {
        #[serde(rename = "fileId")]
        file_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeEnvelope {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub analysis_generation: u64,
    pub changes: Vec<ProjectChange>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Query {
    Selection {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    SemanticView {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    Definition {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    References {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    PrepareRename {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    Rename {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
        #[serde(rename = "newName")]
        new_name: String,
    },
    Diagnostics {
        #[serde(rename = "fileId")]
        file_id: String,
    },
    ExplainDiagnostic {
        #[serde(rename = "fileId")]
        file_id: String,
        code: String,
        offset: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryEnvelope {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub document_version: u64,
    pub analysis_generation: u64,
    pub query: Query,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub file_id: String,
    pub path: String,
    pub range: SourceRange,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub rule_id: String,
    pub kind: String,
    pub strength: String,
    pub source_ranges: Vec<SourceRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfo {
    pub symbol: String,
    pub description: String,
    pub location: Location,
    pub evidence: Evidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<EntityId>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub symbol: String,
    pub occurrence_id: SourceOccurrenceId,
    pub notation: Vec<NotationComponent>,
    pub source_notation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<EntityId>,
    pub location: Location,
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: Vec<ShapeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quantities: Vec<QuantityInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleInfo>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub symbol: String,
    pub concept_id: String,
    pub description: String,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DomainActivation {
    pub pack_id: String,
    pub pack_version: String,
    pub title: String,
    pub strength: String,
    pub scope_kind: String,
    pub scope_range: SourceRange,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConceptInfo {
    pub concept_id: String,
    pub label: String,
    pub description: String,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssumptionInfo {
    pub kind: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticClaimStatus {
    Certain,
    Supported,
    Speculative,
    Conflicting,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticClaimInfo {
    pub claim_id: String,
    pub predicate: String,
    pub value: String,
    pub status: SemanticClaimStatus,
    pub evidence: Vec<Evidence>,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticCandidateStatus {
    Conflicting,
    Rejected,
    Supported,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticCandidateInfo {
    pub candidate_id: String,
    pub family: String,
    pub interpretation: String,
    pub status: SemanticCandidateStatus,
    pub range: SourceRange,
    pub supporting_claim_ids: Vec<String>,
    pub rejecting_claim_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelationRoleInfo {
    pub role: String,
    pub label: String,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concept_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelationInfo {
    pub relation_id: String,
    pub title: String,
    pub description: String,
    pub roles: Vec<RelationRoleInfo>,
    pub conditions: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub range: SourceRange,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticContextInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<EntityId>,
    pub concepts: Vec<ConceptInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<AssumptionInfo>,
    pub claims: Vec<SemanticClaimInfo>,
    pub candidates: Vec<SemanticCandidateInfo>,
    pub relations: Vec<RelationInfo>,
    pub quantities: Vec<QuantityInfo>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DimensionExponentInfo {
    pub base: String,
    pub numerator: i32,
    pub denominator: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDimensionInfo {
    pub exponents: Vec<DimensionExponentInfo>,
    pub display: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuantityInfo {
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity_kind_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub dimension: PhysicalDimensionInfo,
    pub display: String,
    pub evidence: Evidence,
    #[serde(default)]
    pub derived_from: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShapeInfo {
    pub symbol: String,
    pub kind: String,
    pub dimensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinements: Vec<String>,
    pub display: String,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub explanation: String,
    pub range: SourceRange,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticConstraint {
    pub kind: SemanticConstraintKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinements: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticConstraintKind {
    Distribution,
    Event,
    Expression,
    Function,
    Graph,
    Index,
    Matrix,
    Proposition,
    RandomVariable,
    Scalar,
    Set,
    Tensor,
    Vector,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LawBinding {
    pub parameter: String,
    pub symbol: String,
    pub constraint: SemanticConstraint,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LawConditionInfo {
    pub condition_id: String,
    pub kind: ScientificConstraintKind,
    pub subjects: Vec<String>,
    pub label: String,
    pub status: ConstraintStatus,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScientificConstraintKind {
    Assumption,
    Differentiable,
    DomainMembership,
    Positive,
    SameContext,
    ShapeCompatible,
    SignConvention,
    Uniform,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConstraintStatus {
    Conflicting,
    Required,
    Unsupported,
    Verified,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LawRecognition {
    pub law_id: String,
    pub title: String,
    pub description: String,
    pub description_key: String,
    pub maturity: String,
    pub status: String,
    pub pack_id: String,
    pub pack_version: String,
    pub range: SourceRange,
    pub bindings: Vec<LawBinding>,
    pub result: SemanticConstraint,
    pub conditions: Vec<LawConditionInfo>,
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<RelationInfo>,
    pub rank: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EquationNode {
    pub kind: String,
    pub label: Option<String>,
    pub range: SourceRange,
    pub children: Vec<EquationNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreparation {
    pub range: Option<SourceRange>,
    pub placeholder: Option<String>,
    pub rejection: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticViewInfo {
    pub status: String,
    pub summary: String,
    pub symbol: Option<SymbolInfo>,
    pub context: SemanticContextInfo,
    pub declarations: Vec<Location>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub domains: Vec<DomainActivation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTextEdit {
    pub range: SourceRange,
    pub expected_text: String,
    pub replacement_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEditFile {
    pub file_id: String,
    pub path: String,
    pub document_version: u64,
    pub edits: Vec<SemanticTextEdit>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEditProposal {
    pub title: String,
    pub safety: String,
    pub evidence: Vec<Evidence>,
    pub files: Vec<SemanticEditFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QueryValue {
    Selection {
        ranges: Vec<SourceRange>,
    },
    SemanticView {
        view: Box<SemanticViewInfo>,
    },
    Locations {
        locations: Vec<Location>,
    },
    RenamePreparation {
        range: Option<SourceRange>,
        placeholder: Option<String>,
        rejection: Option<String>,
    },
    EditProposal {
        proposal: Option<SemanticEditProposal>,
        rejection: Option<String>,
    },
    Diagnostics {
        diagnostics: Vec<SemanticDiagnostic>,
    },
    DiagnosticExplanation {
        diagnostic: Option<SemanticDiagnostic>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub document_version: u64,
    pub analysis_generation: u64,
    pub value: QueryValue,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub protocol_version: u32,
    pub epoch: String,
    pub inventory_version: u64,
    pub analysis_generation: u64,
    pub changed_file_ids: Vec<String>,
    pub analyzed_file_ids: Vec<String>,
    pub stats: AnalysisStats,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStats {
    pub analyzed_documents: u32,
    pub total_documents: u32,
    pub recognized_laws: u32,
    pub semantic_nodes: u32,
    pub constraints: u32,
    pub law_rules_visited: u32,
    pub semantic_occurrences: u32,
    pub semantic_entities: u32,
    pub semantic_claims: u32,
    pub semantic_evidence: u32,
    pub semantic_dependency_edges: u32,
    pub invalidated_semantic_claims: u32,
    pub semantic_candidates: u32,
}

#[cfg(test)]
mod tests {
    use super::{Evidence, PhysicalDimensionInfo, QuantityInfo};

    #[test]
    fn quantity_wire_contract_keeps_an_empty_derivation_array() {
        let quantity = QuantityInfo {
            symbol: "F".into(),
            quantity_kind_id: None,
            quantity_kind: Some("Force".into()),
            unit_id: None,
            unit: Some("N".into()),
            dimension: PhysicalDimensionInfo {
                exponents: Vec::new(),
                display: "1".into(),
            },
            display: "Force · N".into(),
            evidence: Evidence {
                rule_id: "test/quantity".into(),
                kind: "explicit-prose".into(),
                strength: "strong".into(),
                source_ranges: Vec::new(),
            },
            derived_from: Vec::new(),
        };

        let value = serde_json::to_value(quantity).unwrap();
        assert_eq!(value["derivedFrom"], serde_json::json!([]));
    }
}
