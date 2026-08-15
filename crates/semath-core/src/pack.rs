use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PACK_SCHEMA_VERSION: u32 = 11;
const MAX_PACK_BYTES: usize = 256 * 1024;

include!(concat!(env!("OUT_DIR"), "/pack_catalog.rs"));

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
    pub aliases: Vec<String>,
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
    pub canonical_relation: String,
    #[serde(default)]
    pub representations: Vec<String>,
    pub roles: Vec<PackLawRole>,
    #[serde(default)]
    pub conditions: Vec<PackLawCondition>,
    #[serde(default)]
    pub activation_phrases: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackLawArchetypeUse {
    id: String,
    slots: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthoredLawArchetype {
    pub law_id: String,
    pub archetype_id: String,
    pub slots: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LawArchetype {
    pub id: &'static str,
    pub canonical_relation: &'static str,
    pub slots: &'static [&'static str],
}

pub(crate) const LAW_ARCHETYPES: &[LawArchetype] = &[
    LawArchetype {
        id: "binary-product",
        canonical_relation: "{result} = {left-factor} {right-factor}",
        slots: &["result", "left-factor", "right-factor"],
    },
    LawArchetype {
        id: "ternary-product",
        canonical_relation: "{result} = {first-factor} {second-factor} {third-factor}",
        slots: &["result", "first-factor", "second-factor", "third-factor"],
    },
    LawArchetype {
        id: "reciprocal",
        canonical_relation: "{result} = 1 / {denominator}",
        slots: &["result", "denominator"],
    },
    LawArchetype {
        id: "negative-gradient-transport",
        canonical_relation: "{flux} = -{coefficient} \\nabla {field}",
        slots: &["flux", "coefficient", "field"],
    },
    LawArchetype {
        id: "ternary-product-ratio",
        canonical_relation: "{result} = {first-factor} {second-factor} {third-factor} / {denominator}",
        slots: &[
            "result",
            "first-factor",
            "second-factor",
            "third-factor",
            "denominator",
        ],
    },
];

impl PackLaw {
    pub fn relations(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.canonical_relation.as_str())
            .chain(self.representations.iter().map(String::as_str))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackLawCondition {
    pub id: String,
    pub kind: PackConditionKind,
    pub subjects: Vec<String>,
    pub label: String,
    #[serde(default)]
    pub operator_property: Option<PackOperatorProperty>,
    #[serde(default)]
    pub evidence_phrases: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackConditionKind {
    Assumption,
    Differentiable,
    DomainMembership,
    MapsBetween,
    OperatorProperty,
    Positive,
    RankCompatible,
    SameContext,
    ShapeCompatible,
    SignConvention,
    Uniform,
}

impl PackConditionKind {
    fn valid_arity(self, arity: usize) -> bool {
        match self {
            Self::Differentiable | Self::RankCompatible => arity == 2,
            Self::MapsBetween => arity == 3,
            Self::OperatorProperty => arity == 1,
            Self::DomainMembership | Self::Positive | Self::Uniform => arity >= 1,
            Self::SameContext | Self::ShapeCompatible => arity >= 2,
            Self::Assumption | Self::SignConvention => arity >= 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackOperatorProperty {
    Adjoint,
    Bilinear,
    Gradient,
    Hessian,
    InnerProduct,
    Jacobian,
    Linear,
    Norm,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackLawRole {
    pub id: String,
    pub concept: String,
    #[serde(default)]
    pub quantity: Option<String>,
    pub description: String,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub notation: Vec<String>,
    #[serde(default, skip_serializing_if = "RoleSourceProjection::is_expression")]
    pub source_projection: RoleSourceProjection,
    #[serde(default)]
    pub variadic: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RoleSourceProjection {
    #[default]
    Expression,
    Head,
}

impl RoleSourceProjection {
    fn is_expression(value: &Self) -> bool {
        *value == Self::Expression
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackActivationRule {
    pub id: String,
    pub topic: String,
    #[serde(default)]
    pub phrases: Vec<String>,
    #[serde(default)]
    pub structures: Vec<PackActivationStructure>,
    pub references: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PackActivationStructure {
    Calculus,
    Discrete,
    Optimization,
    Probability,
    RealCoordinateSpace,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackVocabularyEntry {
    pub id: String,
    pub topic: String,
    pub description: String,
    #[serde(default)]
    pub notation: Vec<String>,
    #[serde(default)]
    pub operand_concepts: Vec<String>,
    #[serde(default)]
    pub result_concept: Option<String>,
    #[serde(default)]
    pub result_shape: Option<String>,
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
    let mut value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|cause| error("pack", format!("invalid JSON: {cause}")))?;
    expand_law_archetypes(&mut value)?;
    let expanded = serde_json::to_string(&value)
        .map_err(|cause| error("pack", format!("cannot encode expanded pack: {cause}")))?;
    let mut deserializer = serde_json::Deserializer::from_str(&expanded);
    let pack =
        serde_path_to_error::deserialize::<_, DomainPack>(&mut deserializer).map_err(|cause| {
            let path = cause.path().to_string();
            error(
                if path == "." { "pack".into() } else { path },
                format!("invalid schema: {}", cause.inner()),
            )
        })?;
    validate_pack(&pack)?;
    Ok(pack)
}

fn expand_law_archetypes(value: &mut serde_json::Value) -> Result<(), PackValidationError> {
    let Some(laws) = value
        .get_mut("laws")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };
    for (law_index, law) in laws.iter_mut().enumerate() {
        let Some(object) = law.as_object_mut() else {
            continue;
        };
        let Some(authored) = object.remove("archetype") else {
            continue;
        };
        let path = format!("laws[{law_index}].archetype");
        if object.contains_key("canonicalRelation") {
            return Err(error(
                path,
                "must replace canonicalRelation rather than coexist with it",
            ));
        }
        let authored: PackLawArchetypeUse = serde_json::from_value(authored)
            .map_err(|cause| error(path.clone(), format!("invalid archetype use: {cause}")))?;
        let Some(archetype) = LAW_ARCHETYPES
            .iter()
            .find(|archetype| archetype.id == authored.id)
        else {
            return Err(error(path, format!("unknown archetype {}", authored.id)));
        };
        let expected = archetype.slots.iter().copied().collect::<BTreeSet<_>>();
        let actual = authored
            .slots
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(error(
                format!("laws[{law_index}].archetype.slots"),
                format!("must bind exactly [{}]", archetype.slots.join(", ")),
            ));
        }
        let role_ids = object
            .get("roles")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|role| role.get("id").and_then(serde_json::Value::as_str))
            .collect::<BTreeSet<_>>();
        let bindings = authored
            .slots
            .values()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if bindings.len() != authored.slots.len() || bindings != role_ids {
            return Err(error(
                format!("laws[{law_index}].archetype.slots"),
                "must bind every law role exactly once",
            ));
        }
        let mut relation = archetype.canonical_relation.to_owned();
        for slot in archetype.slots {
            let role = &authored.slots[*slot];
            validate_id(&format!("laws[{law_index}].archetype.slots.{slot}"), role)?;
            relation = relation.replace(&format!("{{{slot}}}"), role);
        }
        let canonical = crate::canonical::canonical_template(&relation);
        if object
            .get("representations")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|representation| crate::canonical::canonical_template(representation) == canonical)
        {
            return Err(error(
                format!("laws[{law_index}].representations"),
                "duplicates the archetype-expanded canonical law form",
            ));
        }
        object.insert(
            "canonicalRelation".into(),
            serde_json::Value::String(relation),
        );
    }
    Ok(())
}

pub(crate) fn authored_law_archetypes(source: &str) -> Vec<AuthoredLawArchetype> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    value
        .get("laws")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|law| {
            let law_id = law.get("id")?.as_str()?.to_owned();
            let authored =
                serde_json::from_value::<PackLawArchetypeUse>(law.get("archetype")?.clone())
                    .ok()?;
            Some(AuthoredLawArchetype {
                law_id,
                archetype_id: authored.id,
                slots: authored.slots,
            })
        })
        .collect()
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
    validate_entries(pack)?;
    validate_links(pack, &references)?;
    validate_laws(pack)?;
    validate_quantities(pack)?;
    Ok(())
}

pub fn validate_catalog(packs: &[DomainPack]) -> Result<(), PackValidationError> {
    let ids = unique_ids("packs", packs.iter().map(|pack| pack.pack_id.as_str()))?;
    unique_ids(
        "namespaces",
        packs.iter().map(|pack| pack.namespace.as_str()),
    )?;
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
    let units = packs
        .iter()
        .flat_map(|pack| {
            pack.units
                .iter()
                .map(move |unit| (format!("{}:{}", pack.namespace, unit.id), unit))
        })
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
        let declared_requirements = pack
            .dependencies
            .iter()
            .flat_map(|dependency| dependency.required_capabilities.iter().cloned())
            .collect::<BTreeSet<_>>();
        let pack_requirements = pack
            .capabilities
            .requires
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if declared_requirements != pack_requirements {
            return Err(error(
                format!("packs[{pack_index}].capabilities.requires"),
                "must equal the capabilities required from declared dependencies",
            ));
        }
        for (concept_index, concept) in pack.concepts.iter().enumerate() {
            for parent in &concept.parents {
                if !concepts.contains(parent) {
                    return Err(error(
                        format!("packs[{pack_index}].concepts[{concept_index}].parents"),
                        format!("unknown parent concept {parent}"),
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
                if let Some(quantity) = &role.quantity
                    && !concepts.contains(quantity)
                {
                    return Err(error(
                        format!(
                            "packs[{pack_index}].laws[{law_index}].roles[{role_index}].quantity"
                        ),
                        format!("unknown quantity {quantity}"),
                    ));
                }
            }
        }
        for (quantity_index, quantity) in pack.quantity_kinds.iter().enumerate() {
            let Some(default_unit) = &quantity.default_unit else {
                continue;
            };
            let path = format!("packs[{pack_index}].quantityKinds[{quantity_index}].defaultUnit");
            let Some(unit) = units.get(default_unit) else {
                return Err(error(path, format!("unknown unit {default_unit}")));
            };
            if !same_dimension(&quantity.dimension, &unit.dimension) {
                return Err(error(
                    path,
                    format!("unit {default_unit} has an incompatible dimension"),
                ));
            }
        }
    }
    dependency_cycles(packs, &ids)
}

fn same_dimension(left: &[PackDimensionExponent], right: &[PackDimensionExponent]) -> bool {
    let left = left
        .iter()
        .filter(|value| value.numerator != 0)
        .map(|value| (value.base.as_str(), (value.numerator, value.denominator)))
        .collect::<BTreeMap<_, _>>();
    let right = right
        .iter()
        .filter(|value| value.numerator != 0)
        .map(|value| (value.base.as_str(), (value.numerator, value.denominator)))
        .collect::<BTreeMap<_, _>>();
    left.len() == right.len()
        && left
            .iter()
            .all(|(base, (left_numerator, left_denominator))| {
                right
                    .get(base)
                    .is_some_and(|(right_numerator, right_denominator)| {
                        i64::from(*left_numerator) * i64::from(*right_denominator)
                            == i64::from(*right_numerator) * i64::from(*left_denominator)
                    })
            })
}

pub fn built_in_packs() -> &'static [DomainPack] {
    &BUILTIN_PACKS
}

#[cfg(test)]
pub(crate) fn built_in_pack_sources() -> &'static [(&'static str, &'static str)] {
    BUILTIN_PACK_SOURCES
}

fn validate_laws(pack: &DomainPack) -> Result<(), PackValidationError> {
    unique_ids("laws", pack.laws.iter().map(|law| law.id.as_str()))?;
    for (law_index, law) in pack.laws.iter().enumerate() {
        let path = format!("laws[{law_index}]");
        validate_id(&format!("{path}.id"), &law.id)?;
        require_text(&format!("{path}.title"), &law.title)?;
        require_text(&format!("{path}.description"), &law.description)?;
        require_text(
            &format!("{path}.canonicalRelation"),
            &law.canonical_relation,
        )?;
        unique_ids(
            &format!("{path}.roles"),
            law.roles.iter().map(|role| role.id.as_str()),
        )?;
        let role_ids = law
            .roles
            .iter()
            .map(|role| role.id.as_str())
            .collect::<BTreeSet<_>>();
        unique_ids(
            &format!("{path}.conditions"),
            law.conditions.iter().map(|condition| condition.id.as_str()),
        )?;
        for (condition_index, condition) in law.conditions.iter().enumerate() {
            let condition_path = format!("{path}.conditions[{condition_index}]");
            validate_id(&format!("{condition_path}.id"), &condition.id)?;
            require_text(&format!("{condition_path}.label"), &condition.label)?;
            for (phrase_index, phrase) in condition.evidence_phrases.iter().enumerate() {
                require_text(
                    &format!("{condition_path}.evidencePhrases[{phrase_index}]"),
                    phrase,
                )?;
            }
            if !condition.kind.valid_arity(condition.subjects.len()) {
                return Err(error(
                    format!("{condition_path}.subjects"),
                    format!(
                        "invalid arity {} for {:?}",
                        condition.subjects.len(),
                        condition.kind
                    ),
                ));
            }
            if (condition.kind == PackConditionKind::OperatorProperty)
                != condition.operator_property.is_some()
            {
                return Err(error(
                    format!("{condition_path}.operatorProperty"),
                    "must be present exactly for an operator-property condition",
                ));
            }
            let subjects = unique_ids(
                &format!("{condition_path}.subjects"),
                condition.subjects.iter().map(String::as_str),
            )?;
            if let Some(subject) = subjects
                .iter()
                .find(|subject| !role_ids.contains(**subject))
            {
                return Err(error(
                    format!("{condition_path}.subjects"),
                    format!("unknown law role {subject}"),
                ));
            }
        }
        for (role_index, role) in law.roles.iter().enumerate() {
            if role.shape.as_deref().is_some_and(|shape| {
                !matches!(
                    shape,
                    "function" | "scalar" | "vector" | "matrix" | "tensor"
                )
            }) {
                return Err(error(
                    format!("{path}.roles[{role_index}].shape"),
                    "must be function, scalar, vector, matrix, or tensor",
                ));
            }
        }
        for (form_index, form) in law.relations().enumerate() {
            let form_path = if form_index == 0 {
                format!("{path}.canonicalRelation")
            } else {
                format!("{path}.representations[{}]", form_index - 1)
            };
            require_text(&form_path, form)?;
            for role in &law.roles {
                if !form.contains(&role.id) {
                    return Err(error(
                        form_path.clone(),
                        format!("does not bind role {}", role.id),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_entries(pack: &DomainPack) -> Result<(), PackValidationError> {
    let concepts = unique_ids(
        "concepts",
        pack.concepts.iter().map(|entry| entry.id.as_str()),
    )?;
    let mut concept_targets = concepts.clone();
    concept_targets.extend(pack.quantity_kinds.iter().map(|entry| entry.id.as_str()));
    unique_ids(
        "activationRules",
        pack.activation_rules.iter().map(|entry| entry.id.as_str()),
    )?;
    unique_ids("roles", pack.roles.iter().map(|entry| entry.id.as_str()))?;
    unique_ids(
        "operators",
        pack.operators.iter().map(|entry| entry.id.as_str()),
    )?;
    for (index, concept) in pack.concepts.iter().enumerate() {
        let path = format!("concepts[{index}]");
        if !matches!(
            concept.concept_kind.as_str(),
            "entity" | "operator" | "quantity" | "relation" | "system"
        ) {
            return Err(error(
                format!("{path}.conceptKind"),
                "must be entity, operator, quantity, relation, or system",
            ));
        }
        require_text(&format!("{path}.title"), &concept.title)?;
        require_text(&format!("{path}.description"), &concept.description)?;
        for (alias_index, alias) in concept.aliases.iter().enumerate() {
            require_text(&format!("{path}.aliases[{alias_index}]"), alias)?;
        }
    }
    for capability in pack
        .capabilities
        .provides
        .iter()
        .chain(&pack.capabilities.requires)
        .chain(
            pack.dependencies
                .iter()
                .flat_map(|dependency| &dependency.required_capabilities),
        )
    {
        validate_qualified_id("capabilities", capability)?;
    }
    for (index, rule) in pack.activation_rules.iter().enumerate() {
        let path = format!("activationRules[{index}]");
        require_text(&format!("{path}.topic"), &rule.topic)?;
        if rule.phrases.is_empty() && rule.structures.is_empty() {
            return Err(error(
                format!("{path}.evidence"),
                "must contain an activation phrase or structural kind",
            ));
        }
        if rule.phrases.iter().any(|pattern| pattern.trim().is_empty()) {
            return Err(error(
                format!("{path}.phrases"),
                "must contain nonempty activation phrases",
            ));
        }
    }
    for (collection, entries) in [("roles", &pack.roles), ("operators", &pack.operators)] {
        for (index, entry) in entries.iter().enumerate() {
            if !concepts.contains(entry.id.as_str()) {
                return Err(error(
                    format!("{collection}[{index}].id"),
                    format!("has no concept {}:{}", pack.namespace, entry.id),
                ));
            }
            require_text(&format!("{collection}[{index}].topic"), &entry.topic)?;
            require_text(
                &format!("{collection}[{index}].description"),
                &entry.description,
            )?;
            if collection == "roles"
                && (!entry.operand_concepts.is_empty()
                    || entry.result_concept.is_some()
                    || entry.result_shape.is_some())
            {
                return Err(error(
                    format!("{collection}[{index}].signature"),
                    "operator signatures are allowed only on operators",
                ));
            }
            for (concept_index, concept) in entry.operand_concepts.iter().enumerate() {
                validate_pack_concept_reference(
                    pack,
                    &concept_targets,
                    &format!("{collection}[{index}].operandConcepts[{concept_index}]"),
                    concept,
                )?;
            }
            if let Some(concept) = &entry.result_concept {
                validate_pack_concept_reference(
                    pack,
                    &concept_targets,
                    &format!("{collection}[{index}].resultConcept"),
                    concept,
                )?;
            }
            if let Some(shape) = &entry.result_shape
                && !matches!(shape.as_str(), "scalar" | "vector" | "matrix" | "tensor")
            {
                return Err(error(
                    format!("{collection}[{index}].resultShape"),
                    "must be scalar, vector, matrix, or tensor",
                ));
            }
        }
    }
    Ok(())
}

fn validate_pack_concept_reference(
    pack: &DomainPack,
    concepts: &HashSet<&str>,
    path: &str,
    concept: &str,
) -> Result<(), PackValidationError> {
    let Some((namespace, id)) = concept.split_once(':') else {
        return Err(error(path, "must be a qualified concept ID"));
    };
    if namespace != pack.namespace || !concepts.contains(id) {
        return Err(error(path, format!("unknown concept {concept}")));
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

fn validate_qualified_id(path: &str, value: &str) -> Result<(), PackValidationError> {
    let Some((namespace, local)) = value.split_once(':') else {
        return Err(error(path, "must be a namespace-qualified identifier"));
    };
    validate_id(path, namespace)?;
    validate_id(path, local)
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
    use super::{
        PACK_SCHEMA_VERSION, built_in_packs, compile_pack, validate_catalog, validate_pack,
    };

    #[test]
    fn compiles_the_single_current_schema_and_catalog() {
        assert_eq!(PACK_SCHEMA_VERSION, 11);
        assert_eq!(built_in_packs().len(), 14);
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
    fn validates_typed_operator_signatures_against_pack_concepts() {
        let pack = built_in_packs()
            .iter()
            .find(|pack| pack.pack_id == "discrete-math")
            .unwrap();
        let cardinality = pack
            .operators
            .iter()
            .find(|operator| operator.id == "cardinality")
            .unwrap();
        assert_eq!(cardinality.operand_concepts, ["discrete-math:set"]);
        assert_eq!(
            cardinality.result_concept.as_deref(),
            Some("discrete-math:cardinality")
        );
        assert_eq!(cardinality.result_shape.as_deref(), Some("scalar"));

        let mut unknown = pack.clone();
        unknown.operators[0].operand_concepts = vec!["discrete-math:missing".into()];
        let error = validate_pack(&unknown).unwrap_err();
        assert!(error.path.ends_with("operators[0].operandConcepts[0]"));
        assert!(error.message.contains("unknown concept"));

        let mut role_signature = pack.clone();
        role_signature.roles[0].result_shape = Some("scalar".into());
        let error = validate_pack(&role_signature).unwrap_err();
        assert!(error.path.ends_with("roles[0].signature"));
    }

    #[test]
    fn compiles_closed_typed_space_operator_and_rank_conditions() {
        let source = super::built_in_pack_sources()
            .iter()
            .find(|(pack_id, _)| *pack_id == "linear-algebra")
            .unwrap()
            .1;
        let mut value = serde_json::from_str::<serde_json::Value>(source).unwrap();
        value["laws"][0]["conditions"] = serde_json::json!([
            {
                "id": "mapping",
                "kind": "maps-between",
                "subjects": ["operator", "vector", "result"],
                "label": "The operator maps the input space to the result space."
            },
            {
                "id": "linear",
                "kind": "operator-property",
                "subjects": ["operator"],
                "label": "The operator is linear.",
                "operatorProperty": "linear"
            },
            {
                "id": "rank",
                "kind": "rank-compatible",
                "subjects": ["operator", "result"],
                "label": "The rank is compatible with the result extent."
            }
        ]);
        let pack = compile_pack(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(pack.laws[0].conditions.len(), 3);
        assert_eq!(
            pack.laws[0].conditions[1].operator_property,
            Some(super::PackOperatorProperty::Linear)
        );

        value["laws"][0]["conditions"][1]
            .as_object_mut()
            .unwrap()
            .remove("operatorProperty");
        let error = compile_pack(&serde_json::to_string(&value).unwrap()).unwrap_err();
        assert!(error.path.ends_with("operatorProperty"));
    }

    #[test]
    fn retains_reviewed_condition_phrase_families_declaratively() {
        for (pack_id, law_id, required) in [
            (
                "circuits",
                "kirchhoff-current-law",
                &[
                    "junction convention",
                    "currents directed into the junction",
                    "currents directed away",
                ][..],
            ),
            (
                "signals-systems",
                "wave-speed-relation",
                &["same-phase"][..],
            ),
            (
                "electromagnetism",
                "electric-potential-energy",
                &["potential relative to", "region held at potential"][..],
            ),
        ] {
            let condition = &built_in_packs()
                .iter()
                .find(|pack| pack.pack_id == pack_id)
                .unwrap()
                .laws
                .iter()
                .find(|law| law.id == law_id)
                .unwrap()
                .conditions[0];
            assert!(required.iter().all(|phrase| {
                condition
                    .evidence_phrases
                    .iter()
                    .any(|candidate| candidate == phrase)
            }));
        }
    }

    #[test]
    fn kcl_activation_requires_circuit_specific_language() {
        let law = built_in_packs()
            .iter()
            .find(|pack| pack.pack_id == "circuits")
            .unwrap()
            .laws
            .iter()
            .find(|law| law.id == "kirchhoff-current-law")
            .unwrap();
        assert!(law.activation_phrases.iter().any(|phrase| phrase == "kcl"));
        assert!(
            law.activation_phrases
                .iter()
                .all(|phrase| phrase == "kcl" || phrase.split_whitespace().count() >= 2)
        );
        assert!(
            !law.activation_phrases
                .iter()
                .any(|phrase| phrase == "balance")
        );
    }

    #[test]
    fn rejects_legacy_and_incoherent_law_conditions() {
        let pack = built_in_packs()
            .iter()
            .find(|pack| !pack.laws.is_empty())
            .unwrap();
        let mut legacy = serde_json::to_value(pack).unwrap();
        legacy["laws"][0]["conditions"] = serde_json::json!(["legacy free-form condition"]);
        assert!(compile_pack(&serde_json::to_string(&legacy).unwrap()).is_err());

        let mut unknown_role = pack.clone();
        unknown_role.laws[0].conditions[0].subjects =
            vec!["function".into(), "missing-role".into()];
        let error = validate_pack(&unknown_role).unwrap_err();
        assert!(error.message.contains("unknown law role"));

        let mut wrong_arity = pack.clone();
        wrong_arity.laws[0].conditions[0].subjects.truncate(1);
        let error = validate_pack(&wrong_arity).unwrap_err();
        assert!(error.message.contains("invalid arity"));
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

    #[test]
    fn archetypes_expand_once_into_the_existing_law_ir() {
        let source = super::built_in_pack_sources()
            .iter()
            .find(|(pack_id, _)| *pack_id == "circuits")
            .unwrap()
            .1;
        let pack = compile_pack(source).unwrap();
        let law = pack.laws.iter().find(|law| law.id == "ohm-law").unwrap();
        assert_eq!(law.canonical_relation, "voltage = resistance current");
        assert!(!serde_json::to_string(law).unwrap().contains("archetype"));

        let mut invalid = serde_json::from_str::<serde_json::Value>(source).unwrap();
        invalid["laws"][0]["archetype"]["slots"]
            .as_object_mut()
            .unwrap()
            .remove("result");
        let error = compile_pack(&serde_json::to_string(&invalid).unwrap()).unwrap_err();
        assert!(error.message.contains("must bind exactly"));

        let mut duplicate = serde_json::from_str::<serde_json::Value>(source).unwrap();
        duplicate["laws"][0]["representations"] =
            serde_json::json!(["voltage = resistance current"]);
        let error = compile_pack(&serde_json::to_string(&duplicate).unwrap()).unwrap_err();
        assert!(error.message.contains("archetype-expanded canonical"));

        let mut unknown = serde_json::from_str::<serde_json::Value>(source).unwrap();
        unknown["laws"][0]["archetype"]["id"] = serde_json::json!("unknown-archetype");
        let error = compile_pack(&serde_json::to_string(&unknown).unwrap()).unwrap_err();
        assert!(error.message.contains("unknown archetype"));
    }

    #[test]
    fn authored_and_manually_expanded_archetype_laws_compile_identically() {
        for (_, source) in super::built_in_pack_sources() {
            let compiled = compile_pack(source).unwrap();
            let mut expanded_source = serde_json::from_str::<serde_json::Value>(source).unwrap();
            let Some(laws) = expanded_source["laws"].as_array_mut() else {
                continue;
            };
            for law in laws {
                if law.get("archetype").is_none() {
                    continue;
                }
                let law_id = law["id"].as_str().unwrap();
                let canonical = compiled
                    .laws
                    .iter()
                    .find(|candidate| candidate.id == law_id)
                    .unwrap()
                    .canonical_relation
                    .clone();
                law.as_object_mut().unwrap().remove("archetype");
                law["canonicalRelation"] = serde_json::Value::String(canonical);
            }
            assert_eq!(
                compiled,
                compile_pack(&serde_json::to_string(&expanded_source).unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn rejects_unknown_and_dimensionally_wrong_default_units() {
        let mut packs = built_in_packs().to_vec();
        let quantities = packs
            .iter_mut()
            .find(|pack| pack.pack_id == "quantities-units")
            .unwrap();
        quantities.quantity_kinds[0].default_unit = Some("quantities-units:missing".into());
        let error = validate_catalog(&packs).unwrap_err();
        assert!(error.path.ends_with("quantityKinds[0].defaultUnit"));
        assert!(error.message.contains("unknown unit"));

        let mut packs = built_in_packs().to_vec();
        let quantities = packs
            .iter_mut()
            .find(|pack| pack.pack_id == "quantities-units")
            .unwrap();
        quantities.quantity_kinds[0].default_unit = Some("quantities-units:second".into());
        let error = validate_catalog(&packs).unwrap_err();
        assert!(error.path.ends_with("quantityKinds[0].defaultUnit"));
        assert!(error.message.contains("incompatible dimension"));
    }
}
