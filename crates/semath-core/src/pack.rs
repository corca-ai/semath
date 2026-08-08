use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{FormulaConstraint, FormulaParameter, FormulaSideCondition};

pub const PACK_SCHEMA_VERSION: u32 = 2;
const MAX_PACK_BYTES: usize = 256 * 1024;
const MAX_ACTIVATION_PATTERNS: usize = 128;
const MAX_PATTERN_RULES: usize = 256;
const MAX_REGEX_BYTES: usize = 512;

const BUILTIN_PACK_SOURCES: &[(&str, &str)] = &[
    (
        "linear-algebra",
        include_str!("../../../packs/linear-algebra/v1.json"),
    ),
    (
        "probability",
        include_str!("../../../packs/probability/v1.json"),
    ),
    (
        "calculus-analysis",
        include_str!("../../../packs/calculus-analysis/v1.json"),
    ),
    (
        "optimization-ml",
        include_str!("../../../packs/optimization-ml/v1.json"),
    ),
    (
        "discrete-math",
        include_str!("../../../packs/discrete-math/v1.json"),
    ),
];

static IDENTIFIER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$").unwrap());
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[a-z0-9.-]+)?$").unwrap());

static BUILTIN_PACKS: LazyLock<Vec<DomainPack>> = LazyLock::new(|| {
    let packs = BUILTIN_PACK_SOURCES
        .iter()
        .map(|(expected_id, source)| {
            let pack = load_pack(source)
                .unwrap_or_else(|error| panic!("invalid built-in pack {expected_id}: {error}"));
            assert_eq!(
                pack.pack_id, *expected_id,
                "built-in pack registration must match its declared ID"
            );
            pack
        })
        .collect::<Vec<_>>();
    validate_catalog(&packs).expect("built-in pack catalog must be coherent");
    packs
});

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum PackMaturity {
    Recognition,
    Completion,
    Diagnostic,
    Rewrite,
}

impl PackMaturity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recognition => "recognition",
            Self::Completion => "completion",
            Self::Diagnostic => "diagnostic",
            Self::Rewrite => "rewrite",
        }
    }

    pub fn allows_completion(self) -> bool {
        matches!(self, Self::Completion | Self::Rewrite)
    }

    pub fn allows_diagnostic(self) -> bool {
        matches!(self, Self::Diagnostic)
    }

    pub fn allows_rewrite(self) -> bool {
        matches!(self, Self::Rewrite)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DomainPack {
    pub schema_version: u32,
    pub pack_id: String,
    pub pack_version: String,
    pub title: String,
    pub description: String,
    pub activation_rules: Vec<PackActivationRule>,
    #[serde(default)]
    pub roles: Vec<PackVocabularyEntry>,
    #[serde(default)]
    pub operators: Vec<PackVocabularyEntry>,
    pub patterns: Vec<PackPattern>,
    #[serde(default)]
    pub rewrites: Vec<PackRewrite>,
    pub references: Vec<PackReference>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackActivationRule {
    pub id: String,
    pub topic: String,
    pub patterns: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackVocabularyEntry {
    pub id: String,
    pub topic: String,
    pub description: String,
    #[serde(default)]
    pub notation: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackPattern {
    #[serde(skip)]
    pub pack_id: String,
    #[serde(skip)]
    pub pack_version: String,
    pub id: String,
    pub topic: String,
    pub title: String,
    pub description: String,
    pub description_key: String,
    pub maturity: PackMaturity,
    pub matcher: PackMatcher,
    #[serde(default)]
    pub parameters: Vec<FormulaParameter>,
    pub result: FormulaConstraint,
    #[serde(default)]
    pub side_conditions: Vec<FormulaSideCondition>,
    #[serde(default)]
    pub condition_descriptions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_template: Option<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackMatcher {
    pub primitive: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackRewrite {
    pub id: String,
    pub topic: String,
    pub title: String,
    pub description: String,
    pub source_pattern: String,
    pub required_refinements: Vec<PackRequiredRefinement>,
    pub replacement_template: String,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackRequiredRefinement {
    pub parameter: String,
    pub refinement: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackReference {
    pub id: String,
    pub title: String,
    pub citation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    pub schema_version: u32,
    pub pack_id: String,
    pub pack_version: String,
    pub title: String,
    pub description: String,
    pub pattern_count: u32,
    pub rewrite_count: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{path}: {message}")]
pub struct PackValidationError {
    pub path: String,
    pub message: String,
}

pub fn load_pack(source: &str) -> Result<DomainPack, PackValidationError> {
    if source.len() > MAX_PACK_BYTES {
        return Err(error("pack", "source exceeds the 256 KiB limit"));
    }
    let mut pack = serde_json::from_str::<DomainPack>(source)
        .map_err(|cause| error("pack", format!("invalid JSON: {cause}")))?;
    for pattern in &mut pack.patterns {
        pattern.pack_id.clone_from(&pack.pack_id);
        pattern.pack_version.clone_from(&pack.pack_version);
    }
    validate_pack(&pack)?;
    Ok(pack)
}

pub fn validate_pack(pack: &DomainPack) -> Result<(), PackValidationError> {
    if pack.schema_version != PACK_SCHEMA_VERSION {
        return Err(error(
            "schemaVersion",
            format!(
                "unsupported schema {}; expected {PACK_SCHEMA_VERSION}",
                pack.schema_version
            ),
        ));
    }
    validate_id("packId", &pack.pack_id)?;
    if !VERSION.is_match(&pack.pack_version) {
        return Err(error("packVersion", "must be semantic version x.y.z"));
    }
    require_text("title", &pack.title)?;
    require_text("description", &pack.description)?;
    if pack.activation_rules.is_empty() {
        return Err(error("activationRules", "must not be empty"));
    }
    if pack.patterns.is_empty() {
        return Err(error("patterns", "must not be empty"));
    }
    if pack.patterns.len() > MAX_PATTERN_RULES {
        return Err(error("patterns", "exceeds the 256-entry limit"));
    }
    if pack.references.is_empty() {
        return Err(error("references", "must not be empty"));
    }

    let reference_ids = validate_references(pack)?;
    validate_activation_rules(pack, &reference_ids)?;
    validate_vocabulary("roles", &pack.roles, &reference_ids)?;
    validate_vocabulary("operators", &pack.operators, &reference_ids)?;
    validate_patterns(pack, &reference_ids)?;
    validate_rewrites(pack, &reference_ids)?;
    Ok(())
}

pub fn built_in_packs() -> &'static [DomainPack] {
    &BUILTIN_PACKS
}

pub fn built_in_pack_summaries() -> Vec<PackSummary> {
    BUILTIN_PACKS
        .iter()
        .map(|pack| PackSummary {
            schema_version: pack.schema_version,
            pack_id: pack.pack_id.clone(),
            pack_version: pack.pack_version.clone(),
            title: pack.title.clone(),
            description: pack.description.clone(),
            pattern_count: pack.patterns.len() as u32,
            rewrite_count: pack.rewrites.len() as u32,
        })
        .collect()
}

fn validate_catalog(packs: &[DomainPack]) -> Result<(), PackValidationError> {
    let mut ids = HashSet::new();
    for (index, pack) in packs.iter().enumerate() {
        if !ids.insert(pack.pack_id.as_str()) {
            return Err(error(
                format!("packs[{index}].packId"),
                format!("duplicate built-in pack {}", pack.pack_id),
            ));
        }
    }
    Ok(())
}

fn validate_references(pack: &DomainPack) -> Result<HashSet<&str>, PackValidationError> {
    let mut ids = HashSet::new();
    for (index, reference) in pack.references.iter().enumerate() {
        let path = format!("references[{index}]");
        validate_id(&format!("{path}.id"), &reference.id)?;
        require_text(&format!("{path}.title"), &reference.title)?;
        require_text(&format!("{path}.citation"), &reference.citation)?;
        if !ids.insert(reference.id.as_str()) {
            return Err(error(
                format!("{path}.id"),
                format!("duplicate reference {}", reference.id),
            ));
        }
    }
    Ok(ids)
}

fn validate_activation_rules(
    pack: &DomainPack,
    reference_ids: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    let mut ids = HashSet::new();
    let mut count = 0;
    for (index, rule) in pack.activation_rules.iter().enumerate() {
        let path = format!("activationRules[{index}]");
        validate_id(&format!("{path}.id"), &rule.id)?;
        require_text(&format!("{path}.topic"), &rule.topic)?;
        if !ids.insert(rule.id.as_str()) {
            return Err(error(
                format!("{path}.id"),
                format!("duplicate activation rule {}", rule.id),
            ));
        }
        if rule.patterns.is_empty()
            || rule
                .patterns
                .iter()
                .any(|pattern| pattern.trim().is_empty())
        {
            return Err(error(
                format!("{path}.patterns"),
                "must contain non-empty literals",
            ));
        }
        count += rule.patterns.len();
        validate_reference_links(&path, &rule.references, reference_ids)?;
    }
    if count > MAX_ACTIVATION_PATTERNS {
        return Err(error("activationRules", "exceeds the 128-literal limit"));
    }
    Ok(())
}

fn validate_vocabulary(
    category: &str,
    entries: &[PackVocabularyEntry],
    reference_ids: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    let mut ids = HashSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let path = format!("{category}[{index}]");
        validate_id(&format!("{path}.id"), &entry.id)?;
        require_text(&format!("{path}.topic"), &entry.topic)?;
        require_text(&format!("{path}.description"), &entry.description)?;
        if !ids.insert(entry.id.as_str()) {
            return Err(error(
                format!("{path}.id"),
                format!("duplicate {category} entry {}", entry.id),
            ));
        }
        validate_reference_links(&path, &entry.references, reference_ids)?;
    }
    Ok(())
}

fn validate_patterns(
    pack: &DomainPack,
    reference_ids: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    let mut ids = HashSet::new();
    let mut signatures = HashSet::new();
    for (index, pattern) in pack.patterns.iter().enumerate() {
        let path = format!("patterns[{index}]");
        validate_id(&format!("{path}.id"), &pattern.id)?;
        require_text(&format!("{path}.topic"), &pattern.topic)?;
        require_text(&format!("{path}.title"), &pattern.title)?;
        require_text(&format!("{path}.description"), &pattern.description)?;
        validate_id(&format!("{path}.descriptionKey"), &pattern.description_key)?;
        if !ids.insert(pattern.id.as_str()) {
            return Err(error(
                format!("{path}.id"),
                format!("duplicate pattern {}", pattern.id),
            ));
        }
        validate_matcher(&path, &pattern.matcher, pattern.parameters.len())?;
        let signature =
            serde_json::to_string(&(&pattern.matcher, &pattern.parameters, &pattern.result))
                .expect("pack signatures contain serializable data");
        if !signatures.insert(signature) {
            return Err(error(
                format!("{path}.matcher"),
                "duplicates another formula matcher",
            ));
        }
        validate_parameters(&path, &pattern.parameters)?;
        validate_constraint(&format!("{path}.result"), &pattern.result)?;
        validate_side_conditions(&path, pattern)?;
        validate_reference_links(&path, &pattern.references, reference_ids)?;

        if pattern.maturity.allows_completion() {
            let template = pattern.generation_template.as_deref().ok_or_else(|| {
                error(
                    format!("{path}.generationTemplate"),
                    "completion/rewrite maturity requires a template",
                )
            })?;
            validate_template(&path, template, &pattern.parameters, true)?;
        } else if pattern.generation_template.is_some() {
            return Err(error(
                format!("{path}.generationTemplate"),
                "recognition/diagnostic maturity cannot declare completion output",
            ));
        }
    }
    Ok(())
}

fn validate_matcher(
    path: &str,
    matcher: &PackMatcher,
    parameter_count: usize,
) -> Result<(), PackValidationError> {
    const PRIMITIVES: &[&str] = &[
        "binary-product",
        "conditional-probability",
        "event-probability",
        "expectation",
        "quadratic-form",
        "regex-captures",
        "transpose",
        "transposed-binary-product",
        "variance",
    ];
    if !PRIMITIVES.contains(&matcher.primitive.as_str()) {
        return Err(error(
            format!("{path}.matcher.primitive"),
            format!("unknown matcher primitive {}", matcher.primitive),
        ));
    }
    match (matcher.primitive.as_str(), matcher.expression.as_deref()) {
        ("regex-captures", Some(expression)) => {
            if expression.len() > MAX_REGEX_BYTES {
                return Err(error(
                    format!("{path}.matcher.expression"),
                    "regex exceeds the 512-byte limit",
                ));
            }
            let regex = Regex::new(expression).map_err(|cause| {
                error(
                    format!("{path}.matcher.expression"),
                    format!("invalid bounded regex: {cause}"),
                )
            })?;
            if regex.is_match("") {
                return Err(error(
                    format!("{path}.matcher.expression"),
                    "matcher must not accept an empty expression",
                ));
            }
            if regex.captures_len().saturating_sub(1) != parameter_count {
                return Err(error(
                    format!("{path}.matcher.expression"),
                    "capture count must equal parameter count",
                ));
            }
        }
        ("regex-captures", None) => {
            return Err(error(
                format!("{path}.matcher.expression"),
                "regex-captures requires an expression",
            ));
        }
        (_, Some(_)) => {
            return Err(error(
                format!("{path}.matcher.expression"),
                "only regex-captures accepts an expression",
            ));
        }
        (_, None) => {}
    }
    Ok(())
}

fn validate_parameters(
    path: &str,
    parameters: &[FormulaParameter],
) -> Result<(), PackValidationError> {
    let mut ids = HashSet::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let parameter_path = format!("{path}.parameters[{index}]");
        validate_id(&format!("{parameter_path}.id"), &parameter.id)?;
        if !ids.insert(parameter.id.as_str()) {
            return Err(error(
                format!("{parameter_path}.id"),
                format!("duplicate parameter {}", parameter.id),
            ));
        }
        validate_constraint(
            &format!("{parameter_path}.constraint"),
            &parameter.constraint,
        )?;
    }
    Ok(())
}

fn validate_constraint(
    path: &str,
    constraint: &FormulaConstraint,
) -> Result<(), PackValidationError> {
    const KINDS: &[&str] = &[
        "distribution",
        "event",
        "expression",
        "function",
        "graph",
        "index",
        "matrix",
        "proposition",
        "random-variable",
        "scalar",
        "set",
        "tensor",
        "vector",
    ];
    if !KINDS.contains(&constraint.kind.as_str()) {
        return Err(error(
            format!("{path}.kind"),
            format!("unknown constraint kind {}", constraint.kind),
        ));
    }
    if constraint
        .dimensions
        .iter()
        .any(|value| value.trim().is_empty())
        || constraint
            .refinements
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(error(path, "constraint values must not be blank"));
    }
    Ok(())
}

fn validate_side_conditions(path: &str, pattern: &PackPattern) -> Result<(), PackValidationError> {
    const CONDITIONS: &[&str] = &[
        "dimension-equality",
        "explicit-role",
        "positive-probability",
        "presentation-safe",
    ];
    if pattern.condition_descriptions.len() != pattern.side_conditions.len() {
        return Err(error(
            format!("{path}.conditionDescriptions"),
            "must contain one user-facing label per side condition",
        ));
    }
    for (index, condition) in pattern.side_conditions.iter().enumerate() {
        let condition_path = format!("{path}.sideConditions[{index}]");
        if !CONDITIONS.contains(&condition.kind.as_str()) {
            return Err(error(
                format!("{condition_path}.kind"),
                format!("unknown constraint primitive {}", condition.kind),
            ));
        }
        require_text(&format!("{condition_path}.left"), &condition.left)?;
        require_text(&format!("{condition_path}.right"), &condition.right)?;
        require_text(
            &format!("{path}.conditionDescriptions[{index}]"),
            &pattern.condition_descriptions[index],
        )?;
    }
    Ok(())
}

fn validate_rewrites(
    pack: &DomainPack,
    reference_ids: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    let patterns = pack
        .patterns
        .iter()
        .map(|pattern| (pattern.id.as_str(), pattern))
        .collect::<BTreeMap<_, _>>();
    let mut ids = HashSet::new();
    for (index, rewrite) in pack.rewrites.iter().enumerate() {
        let path = format!("rewrites[{index}]");
        validate_id(&format!("{path}.id"), &rewrite.id)?;
        require_text(&format!("{path}.topic"), &rewrite.topic)?;
        require_text(&format!("{path}.title"), &rewrite.title)?;
        require_text(&format!("{path}.description"), &rewrite.description)?;
        if !ids.insert(rewrite.id.as_str()) {
            return Err(error(
                format!("{path}.id"),
                format!("duplicate rewrite {}", rewrite.id),
            ));
        }
        let source = patterns
            .get(rewrite.source_pattern.as_str())
            .ok_or_else(|| {
                error(
                    format!("{path}.sourcePattern"),
                    format!("unknown source pattern {}", rewrite.source_pattern),
                )
            })?;
        if !source.maturity.allows_rewrite() {
            return Err(error(
                format!("{path}.sourcePattern"),
                "source pattern is not rewrite-mature",
            ));
        }
        if rewrite.required_refinements.is_empty() {
            return Err(error(
                format!("{path}.requiredRefinements"),
                "rewrite requires explicit side-condition evidence",
            ));
        }
        for (required_index, required) in rewrite.required_refinements.iter().enumerate() {
            let required_path = format!("{path}.requiredRefinements[{required_index}]");
            let Some(_parameter) = source
                .parameters
                .iter()
                .find(|parameter| parameter.id == required.parameter)
            else {
                return Err(error(
                    format!("{required_path}.parameter"),
                    format!("unknown parameter {}", required.parameter),
                ));
            };
            if required.refinement.trim().is_empty() {
                return Err(error(
                    format!("{required_path}.refinement"),
                    "required refinement must not be blank",
                ));
            }
        }
        validate_template(
            &path,
            &rewrite.replacement_template,
            &source.parameters,
            false,
        )?;
        validate_reference_links(&path, &rewrite.references, reference_ids)?;
    }
    Ok(())
}

fn validate_template(
    path: &str,
    template: &str,
    parameters: &[FormulaParameter],
    require_every_parameter: bool,
) -> Result<(), PackValidationError> {
    require_text(&format!("{path}.template"), template)?;
    let placeholders = template_placeholders(template).map_err(|message| error(path, message))?;
    let parameter_ids = parameters
        .iter()
        .map(|parameter| parameter.id.as_str())
        .collect::<HashSet<_>>();
    if let Some(unknown) = placeholders
        .iter()
        .find(|placeholder| !parameter_ids.contains(placeholder.as_str()))
    {
        return Err(error(
            path,
            format!("template references unknown parameter {unknown}"),
        ));
    }
    if require_every_parameter
        && let Some(missing) = parameter_ids
            .iter()
            .find(|parameter| !placeholders.contains(**parameter))
    {
        return Err(error(path, format!("template omits parameter {missing}")));
    }
    Ok(())
}

fn template_placeholders(template: &str) -> Result<HashSet<String>, String> {
    let mut values = HashSet::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .ok_or_else(|| "template has an unclosed placeholder".to_string())?;
        let value = &after[..end];
        if !IDENTIFIER.is_match(value) {
            return Err(format!("invalid template placeholder {value}"));
        }
        values.insert(value.to_string());
        rest = &after[end + 2..];
    }
    if rest.contains("}}") {
        return Err("template has an unmatched closing delimiter".into());
    }
    Ok(values)
}

fn validate_reference_links(
    path: &str,
    references: &[String],
    known: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    if references.is_empty() {
        return Err(error(
            format!("{path}.references"),
            "must cite at least one pack reference",
        ));
    }
    if let Some(reference) = references
        .iter()
        .find(|reference| !known.contains(reference.as_str()))
    {
        return Err(error(
            format!("{path}.references"),
            format!("unknown reference {reference}"),
        ));
    }
    Ok(())
}

fn validate_id(path: &str, value: &str) -> Result<(), PackValidationError> {
    if !IDENTIFIER.is_match(value) {
        return Err(error(path, "must be a lowercase kebab-case identifier"));
    }
    Ok(())
}

fn require_text(path: &str, value: &str) -> Result<(), PackValidationError> {
    if value.trim().is_empty() {
        return Err(error(path, "must not be blank"));
    }
    Ok(())
}

fn error(path: impl Into<String>, message: impl Into<String>) -> PackValidationError {
    PackValidationError {
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DomainPack, PACK_SCHEMA_VERSION, PackMaturity, built_in_pack_summaries, built_in_packs,
        load_pack,
    };

    #[test]
    fn loads_one_validated_catalog_for_every_capability() {
        let packs = built_in_packs();
        assert_eq!(packs.len(), 5);
        assert_eq!(built_in_pack_summaries()[0].pack_id, "linear-algebra");
        assert!(
            packs
                .iter()
                .flat_map(|pack| &pack.patterns)
                .any(|pattern| pattern.maturity == PackMaturity::Rewrite)
        );
    }

    #[test]
    fn reports_a_precise_path_for_an_unknown_primitive() {
        let mut pack = built_in_packs()[0].clone();
        pack.patterns[0].matcher.primitive = "run-user-code".into();
        let source = serde_json::to_string(&pack).unwrap();
        let error = load_pack(&source).unwrap_err();
        assert_eq!(error.path, "patterns[0].matcher.primitive");
        assert!(error.message.contains("unknown matcher primitive"));
    }

    #[test]
    fn recognition_only_entries_cannot_smuggle_an_edit_template() {
        let mut pack = built_in_packs()[0].clone();
        let pattern = &mut pack.patterns[0];
        pattern.maturity = PackMaturity::Recognition;
        let source = serde_json::to_string(&pack).unwrap();
        let error = load_pack(&source).unwrap_err();
        assert_eq!(error.path, "patterns[0].generationTemplate");
    }

    #[test]
    fn rejects_an_unknown_schema_before_exposing_metadata() {
        let source = format!(
            r#"{{"schemaVersion":{},"packId":"future"}}"#,
            PACK_SCHEMA_VERSION + 1
        );
        let error = load_pack(&source).unwrap_err();
        assert_eq!(error.path, "pack");
    }

    #[test]
    fn serialized_public_schema_round_trips() {
        let pack: DomainPack = built_in_packs()[0].clone();
        let encoded = serde_json::to_string(&pack).unwrap();
        assert_eq!(load_pack(&encoded).unwrap(), pack);
    }
}
