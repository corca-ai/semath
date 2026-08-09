use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PACK_SCHEMA_VERSION: u32 = 4;
const MAX_PACK_BYTES: usize = 256 * 1024;

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
    (
        "quantities-units",
        include_str!("../../../packs/quantities-units/v1.json"),
    ),
    (
        "classical-mechanics",
        include_str!("../../../packs/classical-mechanics/v1.json"),
    ),
    ("circuits", include_str!("../../../packs/circuits/v1.json")),
    (
        "control-systems",
        include_str!("../../../packs/control-systems/v1.json"),
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
            let pack = compile_pack(source)
                .unwrap_or_else(|error| panic!("invalid built-in pack {expected_id}: {error}"));
            assert_eq!(&pack.pack_id, expected_id);
            pack
        })
        .collect::<Vec<_>>();
    validate_catalog(&packs).expect("built-in pack catalog must be coherent");
    packs
});

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainPack {
    pub schema_version: u32,
    pub pack_id: String,
    pub pack_version: String,
    pub pack_kind: PackKind,
    pub namespace: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<PackDependency>,
    pub capabilities: PackCapabilities,
    #[serde(default)]
    pub concepts: Vec<PackConcept>,
    #[serde(default)]
    pub quantity_kinds: Vec<PackQuantityKind>,
    #[serde(default)]
    pub units: Vec<PackUnit>,
    #[serde(default)]
    pub laws: Vec<PackLaw>,
    #[serde(default)]
    pub activation_rules: Vec<PackActivationRule>,
    #[serde(default)]
    pub roles: Vec<PackVocabularyEntry>,
    #[serde(default)]
    pub operators: Vec<PackVocabularyEntry>,
    pub references: Vec<PackReference>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    Capability,
    Field,
    Application,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackDependency {
    pub pack_id: String,
    pub version_major: u32,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackCapabilities {
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackConcept {
    pub id: String,
    pub concept_kind: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub parents: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackDimensionExponent {
    pub base: String,
    pub numerator: i32,
    pub denominator: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackRational {
    pub numerator: i64,
    pub denominator: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackQuantityKind {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub dimension: Vec<PackDimensionExponent>,
    #[serde(default)]
    pub default_unit: Option<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackUnit {
    pub id: String,
    pub symbol: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub dimension: Vec<PackDimensionExponent>,
    pub scale: PackRational,
    #[serde(default)]
    pub offset: Option<PackRational>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackLaw {
    pub id: String,
    pub title: String,
    pub description: String,
    pub semantic_forms: Vec<String>,
    pub roles: Vec<PackLawRole>,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub activation_phrases: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackLawRole {
    pub id: String,
    pub concept: String,
    pub description: String,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub notation: Vec<String>,
    #[serde(default)]
    pub variadic: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackActivationRule {
    pub id: String,
    pub topic: String,
    pub patterns: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackVocabularyEntry {
    pub id: String,
    pub topic: String,
    pub description: String,
    #[serde(default)]
    pub notation: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackReference {
    pub id: String,
    pub title: String,
    pub citation: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{path}: {message}")]
pub struct PackValidationError {
    pub path: String,
    pub message: String,
}

pub fn compile_pack(source: &str) -> Result<DomainPack, PackValidationError> {
    if source.len() > MAX_PACK_BYTES {
        return Err(error("pack", "source exceeds the 256 KiB limit"));
    }
    let pack = serde_json::from_str::<DomainPack>(source)
        .map_err(|cause| error("pack", format!("invalid schema: {cause}")))?;
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
    validate_id("namespace", &pack.namespace)?;
    if !VERSION.is_match(&pack.pack_version) {
        return Err(error("packVersion", "must be semantic version x.y.z"));
    }
    require_text("title", &pack.title)?;
    require_text("description", &pack.description)?;
    if pack.references.is_empty() {
        return Err(error("references", "must not be empty"));
    }
    let references = unique_ids(
        "references",
        pack.references
            .iter()
            .map(|reference| reference.id.as_str()),
    )?;
    validate_links(pack, &references)?;
    validate_laws(pack)?;
    validate_quantities(pack)?;
    Ok(())
}

pub fn validate_catalog(packs: &[DomainPack]) -> Result<(), PackValidationError> {
    let ids = unique_ids("packs", packs.iter().map(|pack| pack.pack_id.as_str()))?;
    let concepts = packs
        .iter()
        .flat_map(|pack| {
            pack.concepts
                .iter()
                .map(move |concept| format!("{}:{}", pack.namespace, concept.id))
                .chain(
                    pack.quantity_kinds
                        .iter()
                        .map(move |quantity| format!("{}:{}", pack.namespace, quantity.id)),
                )
        })
        .collect::<BTreeSet<_>>();
    let by_id = packs
        .iter()
        .map(|pack| (pack.pack_id.as_str(), pack))
        .collect::<BTreeMap<_, _>>();
    for (pack_index, pack) in packs.iter().enumerate() {
        for (dependency_index, dependency) in pack.dependencies.iter().enumerate() {
            let path = format!("packs[{pack_index}].dependencies[{dependency_index}]");
            let Some(target) = by_id.get(dependency.pack_id.as_str()) else {
                return Err(error(path, format!("unknown pack {}", dependency.pack_id)));
            };
            let actual_major = target
                .pack_version
                .split('.')
                .next()
                .and_then(|major| major.parse::<u32>().ok())
                .unwrap_or_default();
            if actual_major != dependency.version_major {
                return Err(error(path, "dependency major version does not match"));
            }
            for capability in &dependency.required_capabilities {
                if !target.capabilities.provides.contains(capability) {
                    return Err(error(
                        path.clone(),
                        format!("missing capability {capability}"),
                    ));
                }
            }
        }
        for (law_index, law) in pack.laws.iter().enumerate() {
            for (role_index, role) in law.roles.iter().enumerate() {
                if !concepts.contains(&role.concept) {
                    return Err(error(
                        format!(
                            "packs[{pack_index}].laws[{law_index}].roles[{role_index}].concept"
                        ),
                        format!("unknown concept {}", role.concept),
                    ));
                }
            }
        }
    }
    dependency_cycles(packs, &ids)
}

pub fn built_in_packs() -> &'static [DomainPack] {
    &BUILTIN_PACKS
}

fn validate_laws(pack: &DomainPack) -> Result<(), PackValidationError> {
    unique_ids("laws", pack.laws.iter().map(|law| law.id.as_str()))?;
    for (law_index, law) in pack.laws.iter().enumerate() {
        let path = format!("laws[{law_index}]");
        validate_id(&format!("{path}.id"), &law.id)?;
        require_text(&format!("{path}.title"), &law.title)?;
        require_text(&format!("{path}.description"), &law.description)?;
        if law.semantic_forms.is_empty() {
            return Err(error(format!("{path}.semanticForms"), "must not be empty"));
        }
        unique_ids(
            &format!("{path}.roles"),
            law.roles.iter().map(|role| role.id.as_str()),
        )?;
        for (role_index, role) in law.roles.iter().enumerate() {
            if role
                .shape
                .as_deref()
                .is_some_and(|shape| !matches!(shape, "scalar" | "vector" | "matrix" | "tensor"))
            {
                return Err(error(
                    format!("{path}.roles[{role_index}].shape"),
                    "must be scalar, vector, matrix, or tensor",
                ));
            }
        }
        for (form_index, form) in law.semantic_forms.iter().enumerate() {
            require_text(&format!("{path}.semanticForms[{form_index}]"), form)?;
            for role in &law.roles {
                if !form.contains(&role.id) {
                    return Err(error(
                        format!("{path}.semanticForms[{form_index}]"),
                        format!("does not bind role {}", role.id),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_quantities(pack: &DomainPack) -> Result<(), PackValidationError> {
    unique_ids(
        "quantityKinds",
        pack.quantity_kinds
            .iter()
            .map(|quantity| quantity.id.as_str()),
    )?;
    unique_ids("units", pack.units.iter().map(|unit| unit.id.as_str()))?;
    for (index, quantity) in pack.quantity_kinds.iter().enumerate() {
        validate_dimension(
            &format!("quantityKinds[{index}].dimension"),
            &quantity.dimension,
        )?;
    }
    for (index, unit) in pack.units.iter().enumerate() {
        validate_dimension(&format!("units[{index}].dimension"), &unit.dimension)?;
        if unit.scale.denominator == 0
            || unit
                .offset
                .as_ref()
                .is_some_and(|value| value.denominator == 0)
        {
            return Err(error(
                format!("units[{index}]"),
                "rational denominator must be positive",
            ));
        }
    }
    Ok(())
}

fn validate_dimension(
    path: &str,
    dimension: &[PackDimensionExponent],
) -> Result<(), PackValidationError> {
    let mut bases = HashSet::new();
    for exponent in dimension {
        validate_id(path, &exponent.base)?;
        if exponent.denominator == 0 {
            return Err(error(path, "dimension denominator must be positive"));
        }
        if !bases.insert(&exponent.base) {
            return Err(error(path, format!("duplicate base {}", exponent.base)));
        }
    }
    Ok(())
}

fn validate_links(
    pack: &DomainPack,
    references: &HashSet<&str>,
) -> Result<(), PackValidationError> {
    let links = pack
        .concepts
        .iter()
        .flat_map(|value| &value.references)
        .chain(
            pack.quantity_kinds
                .iter()
                .flat_map(|value| &value.references),
        )
        .chain(pack.units.iter().flat_map(|value| &value.references))
        .chain(pack.laws.iter().flat_map(|value| &value.references))
        .chain(
            pack.activation_rules
                .iter()
                .flat_map(|value| &value.references),
        )
        .chain(pack.roles.iter().flat_map(|value| &value.references))
        .chain(pack.operators.iter().flat_map(|value| &value.references));
    for reference in links {
        if !references.contains(reference.as_str()) {
            return Err(error(
                "references",
                format!("unknown reference {reference}"),
            ));
        }
    }
    Ok(())
}

fn dependency_cycles(packs: &[DomainPack], ids: &HashSet<&str>) -> Result<(), PackValidationError> {
    fn visit<'a>(
        id: &'a str,
        packs: &'a [DomainPack],
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        let cycle = packs
            .iter()
            .find(|pack| pack.pack_id == id)
            .is_some_and(|pack| {
                pack.dependencies
                    .iter()
                    .any(|dependency| visit(&dependency.pack_id, packs, visiting, visited))
            });
        visiting.remove(id);
        visited.insert(id);
        cycle
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for id in ids {
        if visit(id, packs, &mut visiting, &mut visited) {
            return Err(error("dependencies", "dependency cycle"));
        }
    }
    Ok(())
}

fn unique_ids<'a>(
    path: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> Result<HashSet<&'a str>, PackValidationError> {
    let mut ids = HashSet::new();
    for value in values {
        validate_id(path, value)?;
        if !ids.insert(value) {
            return Err(error(path, format!("duplicate id {value}")));
        }
    }
    Ok(ids)
}

fn validate_id(path: &str, value: &str) -> Result<(), PackValidationError> {
    if IDENTIFIER.is_match(value) {
        Ok(())
    } else {
        Err(error(path, "must be a lowercase kebab-case identifier"))
    }
}

fn require_text(path: &str, value: &str) -> Result<(), PackValidationError> {
    if value.trim().is_empty() {
        Err(error(path, "must not be empty"))
    } else {
        Ok(())
    }
}

fn error(path: impl Into<String>, message: impl Into<String>) -> PackValidationError {
    PackValidationError {
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PACK_SCHEMA_VERSION, built_in_packs, compile_pack, validate_catalog};

    #[test]
    fn compiles_the_single_current_schema_and_catalog() {
        assert_eq!(PACK_SCHEMA_VERSION, 4);
        assert_eq!(built_in_packs().len(), 9);
        validate_catalog(built_in_packs()).unwrap();
    }

    #[test]
    fn rejects_unknown_fields_instead_of_preserving_legacy_schema() {
        let mut source = serde_json::to_value(&built_in_packs()[0]).unwrap();
        source["patterns"] = serde_json::json!([]);
        let error = compile_pack(&serde_json::to_string(&source).unwrap()).unwrap_err();
        assert!(error.message.contains("unknown field"));
    }

    #[test]
    fn compilation_is_deterministic() {
        for pack in built_in_packs() {
            let source = serde_json::to_string(pack).unwrap();
            assert_eq!(
                compile_pack(&source).unwrap(),
                compile_pack(&source).unwrap()
            );
        }
    }
}
