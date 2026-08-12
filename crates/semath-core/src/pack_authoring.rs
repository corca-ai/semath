use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::canonical::canonical_template;
use crate::domain_signature::{
    compile_collision_atlas, compile_domain_signatures, expression_shape_key,
};
use crate::pack::{
    DomainPack, LAW_ARCHETYPES, PackActivationRule, PackCapabilities, PackConcept,
    PackConditionKind, PackKind, PackLaw, PackLawCondition, PackLawRole, PackReference,
    PackValidationError, authored_law_archetypes, compile_pack, validate_catalog,
};

pub const PACK_AUTHORING_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackSource {
    pub path: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackAuthoringRequest {
    pub schema_version: u32,
    pub sources: Vec<PackSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackAuthoringDiagnostic {
    pub code: String,
    pub severity: String,
    pub file: String,
    pub json_path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackCanonicalForm {
    pub pack_id: String,
    pub law_id: String,
    pub form_index: usize,
    pub source: String,
    pub canonical: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackAuthoringSummary {
    pub pack_id: String,
    pub pack_version: String,
    pub concepts: usize,
    pub laws: usize,
    pub quantity_kinds: usize,
    pub units: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackDomainTerm {
    pub text: String,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackDomainSignature {
    pub pack_id: String,
    pub pack_version: String,
    pub title: String,
    pub pack_kind: String,
    pub terms: Vec<PackDomainTerm>,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
    pub structural_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackLawCollision {
    pub left_relation_id: String,
    pub right_relation_id: String,
    pub structural_key: String,
    pub distinguishing_evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackArchetypeReport {
    pub archetype_id: String,
    pub parameter_slots: Vec<String>,
    pub matching_laws: Vec<String>,
    pub adopted_laws: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackAuthoringReport {
    pub schema_version: u32,
    pub diagnostics: Vec<PackAuthoringDiagnostic>,
    pub forms: Vec<PackCanonicalForm>,
    pub packs: Vec<PackAuthoringSummary>,
    pub signatures: Vec<PackDomainSignature>,
    pub collisions: Vec<PackLawCollision>,
    pub archetypes: Vec<PackArchetypeReport>,
}

pub fn inspect_pack_catalog(request: PackAuthoringRequest) -> PackAuthoringReport {
    let mut diagnostics = Vec::new();
    if request.schema_version != PACK_AUTHORING_SCHEMA_VERSION {
        diagnostics.push(PackAuthoringDiagnostic {
            code: "request.unsupported-schema".into(),
            severity: "error".into(),
            file: "request".into(),
            json_path: "schemaVersion".into(),
            message: format!(
                "unsupported schema {}; expected {PACK_AUTHORING_SCHEMA_VERSION}",
                request.schema_version
            ),
            entity_id: None,
        });
    }
    let mut compiled = Vec::new();
    for source in &request.sources {
        match compile_pack(&source.source) {
            Ok(pack) => compiled.push((source.path.clone(), pack)),
            Err(error) => diagnostics.push(compile_diagnostic(&source.path, error)),
        }
    }
    if diagnostics.iter().all(|item| item.severity != "error") {
        let packs = compiled
            .iter()
            .map(|(_, pack)| pack.clone())
            .collect::<Vec<_>>();
        if let Err(error) = validate_catalog(&packs) {
            let (file, path) = catalog_location(&compiled, &error.path);
            let entity_id = diagnostic_entity(&error);
            diagnostics.push(PackAuthoringDiagnostic {
                code: diagnostic_code(&error).into(),
                severity: "error".into(),
                file,
                json_path: path,
                message: error.message,
                entity_id,
            });
        }
    }
    for (path, pack) in &compiled {
        audit_pack(path, pack, &mut diagnostics);
    }
    diagnostics.sort_by(|left, right| {
        (&left.severity, &left.file, &left.json_path, &left.code).cmp(&(
            &right.severity,
            &right.file,
            &right.json_path,
            &right.code,
        ))
    });
    let forms = compiled
        .iter()
        .flat_map(|(_, pack)| {
            pack.laws.iter().flat_map(move |law| {
                law.relations()
                    .enumerate()
                    .map(move |(form_index, source)| PackCanonicalForm {
                        pack_id: pack.pack_id.clone(),
                        law_id: law.id.clone(),
                        form_index,
                        canonical: canonical_template(source),
                        source: source.to_owned(),
                    })
            })
        })
        .collect();
    let catalog = compiled
        .iter()
        .map(|(_, pack)| pack.clone())
        .collect::<Vec<_>>();
    let signatures = compile_domain_signatures(&catalog)
        .into_iter()
        .map(|signature| PackDomainSignature {
            pack_id: signature.pack_id,
            pack_version: signature.pack_version,
            title: signature.title,
            pack_kind: match signature.pack_kind {
                PackKind::Capability => "capability",
                PackKind::Field => "field",
                PackKind::Application => "application",
            }
            .into(),
            terms: signature
                .terms
                .into_iter()
                .map(|term| PackDomainTerm {
                    text: term.text,
                    source: term.source,
                })
                .collect(),
            dependencies: signature.dependencies,
            capabilities: signature.capabilities,
            structural_keys: signature.structural_keys,
        })
        .collect();
    let collisions = compile_collision_atlas(&catalog)
        .into_iter()
        .map(|collision| PackLawCollision {
            left_relation_id: format!("{}:{}", collision.left_pack_id, collision.left_law_id),
            right_relation_id: format!("{}:{}", collision.right_pack_id, collision.right_law_id),
            structural_key: collision.structural_key,
            distinguishing_evidence: collision.distinguishing_evidence,
        })
        .collect();
    let archetypes = compile_archetype_report(&compiled, &request.sources);
    let packs = compiled
        .into_iter()
        .map(|(_, pack)| PackAuthoringSummary {
            pack_id: pack.pack_id,
            pack_version: pack.pack_version,
            concepts: pack.concepts.len(),
            laws: pack.laws.len(),
            quantity_kinds: pack.quantity_kinds.len(),
            units: pack.units.len(),
        })
        .collect();
    PackAuthoringReport {
        schema_version: PACK_AUTHORING_SCHEMA_VERSION,
        diagnostics,
        forms,
        packs,
        signatures,
        collisions,
        archetypes,
    }
}

fn compile_archetype_report(
    compiled: &[(String, DomainPack)],
    sources: &[PackSource],
) -> Vec<PackArchetypeReport> {
    let adopted = compiled
        .iter()
        .flat_map(|(path, pack)| {
            sources
                .iter()
                .find(|source| source.path == *path)
                .into_iter()
                .flat_map(move |source| {
                    authored_law_archetypes(&source.source)
                        .into_iter()
                        .map(move |use_| {
                            (
                                use_.archetype_id,
                                format!("{}:{}", pack.pack_id, use_.law_id),
                            )
                        })
                })
        })
        .fold(
            BTreeMap::<String, BTreeSet<String>>::new(),
            |mut by_id, (id, law)| {
                by_id.entry(id).or_default().insert(law);
                by_id
            },
        );
    LAW_ARCHETYPES
        .iter()
        .map(|archetype| {
            let role_names = archetype
                .slots
                .iter()
                .enumerate()
                .map(|(index, slot)| (*slot, format!("slot{index}")))
                .collect::<BTreeMap<_, _>>();
            let relation = archetype
                .slots
                .iter()
                .fold(archetype.canonical_relation.to_owned(), |relation, slot| {
                    relation.replace(&format!("{{{slot}}}"), &role_names[slot])
                });
            let placeholders = role_names.values().cloned().collect::<BTreeSet<_>>();
            let structural_key =
                expression_shape_key(&canonical_template_expression(&relation), &placeholders);
            let matching_laws = compiled
                .iter()
                .flat_map(|(_, pack)| {
                    pack.laws.iter().filter_map(|law| {
                        let roles = law
                            .roles
                            .iter()
                            .map(|role| role.id.clone())
                            .collect::<BTreeSet<_>>();
                        law.relations()
                            .any(|relation| {
                                expression_shape_key(
                                    &canonical_template_expression(relation),
                                    &roles,
                                ) == structural_key
                            })
                            .then(|| format!("{}:{}", pack.pack_id, law.id))
                    })
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            PackArchetypeReport {
                archetype_id: archetype.id.into(),
                parameter_slots: archetype.slots.iter().map(|slot| (*slot).into()).collect(),
                matching_laws,
                adopted_laws: adopted
                    .get(archetype.id)
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect(),
            }
        })
        .collect()
}

fn canonical_template_expression(source: &str) -> crate::canonical::SemanticExpr {
    crate::canonical::lower_template(source)
}

pub fn inspect_pack_catalog_json(payload: &[u8]) -> Result<Vec<u8>, serde_json::Error> {
    let request = serde_json::from_slice(payload)?;
    serde_json::to_vec(&inspect_pack_catalog(request))
}

pub fn pack_template(pack_id: &str) -> Result<String, PackValidationError> {
    if !is_identifier(pack_id) {
        return Err(PackValidationError {
            path: "packId".into(),
            message: "must be a lowercase kebab-case identifier".into(),
        });
    }
    let title = pack_id
        .split('-')
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    let reference_id = format!("{pack_id}-reference");
    let pack = DomainPack {
        schema_version: crate::pack::PACK_SCHEMA_VERSION,
        pack_id: pack_id.into(),
        pack_version: "0.1.0".into(),
        pack_kind: PackKind::Field,
        namespace: pack_id.into(),
        title: title.clone(),
        description: format!("Typed semantic support for {title}."),
        dependencies: Vec::new(),
        capabilities: PackCapabilities {
            provides: vec![
                format!("{pack_id}:law-relations"),
                "semath:formula-recognition".into(),
            ],
            requires: Vec::new(),
        },
        concepts: vec![
            template_concept("output", "Output", &reference_id),
            template_concept("coefficient", "Coefficient", &reference_id),
            template_concept("input", "Input", &reference_id),
        ],
        quantity_kinds: Vec::new(),
        units: Vec::new(),
        laws: vec![PackLaw {
            id: "scaled-output".into(),
            title: "Scaled output".into(),
            description: "The output equals a scalar coefficient times the input.".into(),
            canonical_relation: "output = coefficient input".into(),
            representations: Vec::new(),
            roles: vec![
                template_role(pack_id, "output", "Output value."),
                template_role(pack_id, "coefficient", "Scalar coefficient."),
                template_role(pack_id, "input", "Input value."),
            ],
            conditions: vec![PackLawCondition {
                id: "scalar-values".into(),
                kind: PackConditionKind::ShapeCompatible,
                subjects: vec!["coefficient".into(), "input".into(), "output".into()],
                label: "The coefficient, input, and output are scalar.".into(),
                evidence_phrases: Vec::new(),
            }],
            activation_phrases: Vec::new(),
            references: vec![reference_id.clone()],
        }],
        activation_rules: vec![PackActivationRule {
            id: "field-vocabulary".into(),
            topic: "foundations".into(),
            phrases: vec![title.to_ascii_lowercase()],
            structures: Vec::new(),
            references: vec![reference_id.clone()],
        }],
        roles: Vec::new(),
        operators: Vec::new(),
        references: vec![PackReference {
            id: reference_id,
            title: format!("{title} reference"),
            citation: "Replace with an authoritative domain reference.".into(),
            url: None,
        }],
    };
    validate_catalog(std::slice::from_ref(&pack))?;
    Ok(serde_json::to_string_pretty(&pack).unwrap() + "\n")
}

fn template_concept(id: &str, title: &str, reference: &str) -> PackConcept {
    PackConcept {
        id: id.into(),
        concept_kind: "entity".into(),
        title: title.into(),
        description: format!("The {title} role in the domain relation."),
        aliases: Vec::new(),
        parents: Vec::new(),
        references: vec![reference.into()],
    }
}

fn template_role(pack_id: &str, id: &str, description: &str) -> PackLawRole {
    PackLawRole {
        id: id.into(),
        concept: format!("{pack_id}:{id}"),
        quantity: None,
        description: description.into(),
        shape: Some("scalar".into()),
        notation: Vec::new(),
        variadic: false,
    }
}

fn audit_pack(path: &str, pack: &DomainPack, diagnostics: &mut Vec<PackAuthoringDiagnostic>) {
    let mut used_references = BTreeSet::new();
    for reference in pack
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
        .chain(pack.operators.iter().flat_map(|value| &value.references))
    {
        used_references.insert(reference);
    }
    for (index, reference) in pack.references.iter().enumerate() {
        if !used_references.contains(&reference.id) {
            diagnostics.push(warning(
                "reference.unused",
                path,
                format!("references[{index}]"),
                format!("reference {} is not used", reference.id),
                Some(reference.id.clone()),
            ));
        }
    }
    let concept_kinds = pack
        .concepts
        .iter()
        .map(|concept| {
            (
                format!("{}:{}", pack.namespace, concept.id),
                concept.concept_kind.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (law_index, law) in pack.laws.iter().enumerate() {
        let mut canonical_forms = BTreeMap::<String, usize>::new();
        for (form_index, form) in law.relations().enumerate() {
            let form_path = if form_index == 0 {
                format!("laws[{law_index}].canonicalRelation")
            } else {
                format!("laws[{law_index}].representations[{}]", form_index - 1)
            };
            let canonical = canonical_template(form);
            if let Some(first) = canonical_forms.insert(canonical.clone(), form_index) {
                diagnostics.push(warning(
                    "form.duplicate-canonical",
                    path,
                    form_path.clone(),
                    format!("duplicates canonical form {first}: {canonical}"),
                    Some(law.id.clone()),
                ));
            }
            if canonical.contains("unknown(") {
                diagnostics.push(error_diagnostic(
                    "form.unknown-lowering",
                    path,
                    form_path,
                    "semantic form contains an unsupported canonical fragment".into(),
                    Some(law.id.clone()),
                ));
            }
        }
        for (role_index, role) in law.roles.iter().enumerate() {
            if concept_kinds
                .get(&role.concept)
                .is_some_and(|kind| matches!(*kind, "relation" | "system"))
            {
                diagnostics.push(error_diagnostic(
                    "constraint.impossible-role-kind",
                    path,
                    format!("laws[{law_index}].roles[{role_index}].concept"),
                    format!("{} cannot bind a value role", role.concept),
                    Some(role.id.clone()),
                ));
            }
        }
    }
}

fn compile_diagnostic(path: &str, error: PackValidationError) -> PackAuthoringDiagnostic {
    let entity_id = diagnostic_entity(&error);
    PackAuthoringDiagnostic {
        code: diagnostic_code(&error).into(),
        severity: "error".into(),
        file: path.into(),
        json_path: error.path,
        message: error.message,
        entity_id,
    }
}

fn diagnostic_entity(error: &PackValidationError) -> Option<String> {
    [
        "duplicate id ",
        "missing capability ",
        "unknown concept ",
        "unknown pack ",
        "unknown parent concept ",
        "unknown reference ",
        "unknown unit ",
    ]
    .iter()
    .find_map(|prefix| error.message.strip_prefix(prefix).map(str::to_owned))
    .or_else(|| {
        error
            .message
            .strip_prefix("unit ")
            .and_then(|message| message.split_whitespace().next())
            .map(str::to_owned)
    })
}

fn diagnostic_code(error: &PackValidationError) -> &'static str {
    if error
        .message
        .contains("duplicates another canonical law form")
        || error
            .message
            .contains("duplicates the archetype-expanded canonical law form")
    {
        "form.duplicate-canonical"
    } else if error.message.contains("unknown field") {
        "schema.unknown-field"
    } else if error.message.contains("duplicate id") {
        "schema.duplicate-id"
    } else if error.message.contains("dependency cycle") {
        "dependency.cycle"
    } else if error.message.contains("missing capability")
        || error.message.contains("capabilities required")
    {
        "dependency.capability"
    } else if error.message.contains("unknown pack") {
        "dependency.unknown-pack"
    } else if error.message.contains("unknown concept")
        || error.message.contains("unknown parent concept")
    {
        "reference.unknown-concept"
    } else if error.message.contains("unknown reference") {
        "reference.unknown-reference"
    } else if error.message.contains("unknown unit") {
        "reference.unknown-unit"
    } else if error.message.contains("incompatible dimension") {
        "constraint.unit-dimension"
    } else {
        "schema.invalid"
    }
}

fn catalog_location(compiled: &[(String, DomainPack)], path: &str) -> (String, String) {
    let Some(rest) = path.strip_prefix("packs[") else {
        return ("catalog".into(), path.into());
    };
    let Some((index, suffix)) = rest.split_once(']') else {
        return ("catalog".into(), path.into());
    };
    let Some((file, _)) = index
        .parse::<usize>()
        .ok()
        .and_then(|index| compiled.get(index))
    else {
        return ("catalog".into(), path.into());
    };
    (file.clone(), suffix.trim_start_matches('.').into())
}

fn warning(
    code: &str,
    file: &str,
    json_path: String,
    message: String,
    entity_id: Option<String>,
) -> PackAuthoringDiagnostic {
    PackAuthoringDiagnostic {
        code: code.into(),
        severity: "warning".into(),
        file: file.into(),
        json_path,
        message,
        entity_id,
    }
}

fn error_diagnostic(
    code: &str,
    file: &str,
    json_path: String,
    message: String,
    entity_id: Option<String>,
) -> PackAuthoringDiagnostic {
    PackAuthoringDiagnostic {
        code: code.into(),
        severity: "error".into(),
        file: file.into(),
        json_path,
        message,
        entity_id,
    }
}

fn is_identifier(value: &str) -> bool {
    let mut parts = value.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    valid_id_part(first, true) && parts.all(|part| valid_id_part(part, false))
}

fn valid_id_part(value: &str, first_part: bool) -> bool {
    !value.is_empty()
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit() && (!first_part || index > 0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::{built_in_pack_sources, built_in_packs};

    #[test]
    fn inspects_the_authoritative_catalog_and_canonical_forms() {
        let report = inspect_pack_catalog(PackAuthoringRequest {
            schema_version: PACK_AUTHORING_SCHEMA_VERSION,
            sources: built_in_pack_sources()
                .iter()
                .map(|(pack_id, source)| PackSource {
                    path: format!("packs/{pack_id}/v1.json"),
                    source: (*source).into(),
                })
                .collect(),
        });
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != "error"),
            "{:?}",
            report.diagnostics
        );
        let expected_forms = built_in_packs()
            .iter()
            .flat_map(|pack| &pack.laws)
            .map(|law| 1 + law.representations.len())
            .sum::<usize>();
        assert_eq!(report.forms.len(), expected_forms);
        assert!(report.forms.iter().all(|form| !form.canonical.is_empty()));
        assert_eq!(report.signatures.len(), built_in_packs().len());
        assert!(
            report
                .signatures
                .iter()
                .all(|signature| !signature.terms.is_empty())
        );
        assert!(report.archetypes.iter().all(|archetype| {
            archetype.matching_laws.len() >= 2
                && !archetype.adopted_laws.is_empty()
                && archetype
                    .adopted_laws
                    .iter()
                    .all(|law| archetype.matching_laws.contains(law))
                && archetype
                    .matching_laws
                    .iter()
                    .filter_map(|law| law.split_once(':').map(|(pack, _)| pack))
                    .collect::<BTreeSet<_>>()
                    .len()
                    >= 2
        }));
        assert!(report.collisions.iter().any(|collision| {
            collision.left_relation_id != collision.right_relation_id
                && collision
                    .distinguishing_evidence
                    .iter()
                    .any(|value| value == "domain")
        }));
    }

    #[test]
    fn creates_a_typed_pack_without_a_rust_registry_edit() {
        let source = pack_template("fluid-dynamics").unwrap();
        let report = inspect_pack_catalog(PackAuthoringRequest {
            schema_version: PACK_AUTHORING_SCHEMA_VERSION,
            sources: vec![PackSource {
                path: "fluid-dynamics/v1.json".into(),
                source,
            }],
        });
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        assert_eq!(report.packs[0].pack_id, "fluid-dynamics");
        assert_eq!(report.forms[0].law_id, "scaled-output");
    }

    #[test]
    fn reports_precise_schema_catalog_and_hygiene_failures() {
        let mut malformed = serde_json::to_value(
            serde_json::from_str::<DomainPack>(&pack_template("sample-field").unwrap()).unwrap(),
        )
        .unwrap();
        malformed["extra"] = serde_json::json!(true);
        let report = inspect_pack_catalog(PackAuthoringRequest {
            schema_version: PACK_AUTHORING_SCHEMA_VERSION,
            sources: vec![PackSource {
                path: "sample.json".into(),
                source: serde_json::to_string(&malformed).unwrap(),
            }],
        });
        assert_eq!(report.diagnostics[0].code, "schema.unknown-field");
        assert_eq!(report.diagnostics[0].file, "sample.json");
        assert_eq!(report.diagnostics[0].json_path, "extra");

        let mut duplicate =
            serde_json::from_str::<DomainPack>(&pack_template("sample-field").unwrap()).unwrap();
        duplicate.laws[0]
            .representations
            .push("output=(coefficient input)".into());
        duplicate.references.push(PackReference {
            id: "unused-source".into(),
            title: "Unused".into(),
            citation: "Unused".into(),
            url: None,
        });
        let report = inspect_pack_catalog(PackAuthoringRequest {
            schema_version: PACK_AUTHORING_SCHEMA_VERSION,
            sources: vec![PackSource {
                path: "sample.json".into(),
                source: serde_json::to_string(&duplicate).unwrap(),
            }],
        });
        let codes = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("form.duplicate-canonical"));
        assert!(codes.contains("reference.unused"));
    }
}
