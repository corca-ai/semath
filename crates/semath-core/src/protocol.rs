use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

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
    pub file_id: String,
    pub path: String,
    pub language: DocumentLanguage,
    pub content: String,
    pub document_version: u64,
    #[serde(default)]
    pub math_regions: Vec<MathRegion>,
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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectChange {
    Upsert {
        document: ProjectDocument,
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
    EquationTree {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    Hover {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    SymbolInfo {
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
    FormulaRecognition {
        #[serde(rename = "fileId")]
        file_id: String,
        offset: u32,
    },
    FormulaCompletion {
        #[serde(rename = "fileId")]
        file_id: String,
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
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub symbol: String,
    pub location: Location,
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: Vec<ShapeInfo>,
    pub formulas: Vec<FormulaRecognition>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShapeInfo {
    pub symbol: String,
    pub kind: String,
    pub dimensions: Vec<String>,
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
pub struct FormulaConstraint {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refinements: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaParameter {
    pub id: String,
    pub constraint: FormulaConstraint,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaSideCondition {
    pub kind: String,
    pub left: String,
    pub right: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaPattern {
    pub schema_version: u32,
    pub pack_id: String,
    pub pack_version: String,
    pub id: String,
    pub title: String,
    pub matcher: String,
    pub parameters: Vec<FormulaParameter>,
    pub result: FormulaConstraint,
    pub side_conditions: Vec<FormulaSideCondition>,
    pub generation_template: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaBinding {
    pub parameter: String,
    pub symbol: String,
    pub constraint: FormulaConstraint,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaRecognition {
    pub pattern_id: String,
    pub title: String,
    pub pack_id: String,
    pub pack_version: String,
    pub range: SourceRange,
    pub bindings: Vec<FormulaBinding>,
    pub result: FormulaConstraint,
    pub evidence: Vec<Evidence>,
    pub rank: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormulaCompletion {
    pub pattern_id: String,
    pub title: String,
    pub detail: String,
    pub rank: u32,
    pub proposal: SemanticEditProposal,
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
    EquationTree {
        tree: Option<EquationNode>,
    },
    Hover {
        symbol: Option<String>,
        #[serde(rename = "equationKind")]
        equation_kind: Option<String>,
        definitions: Vec<DefinitionInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        shape: Option<ShapeInfo>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        formulas: Vec<FormulaRecognition>,
    },
    SymbolInfo {
        info: Option<SymbolInfo>,
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
    FormulaRecognitions {
        recognitions: Vec<FormulaRecognition>,
    },
    FormulaCompletions {
        completions: Vec<FormulaCompletion>,
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
}
