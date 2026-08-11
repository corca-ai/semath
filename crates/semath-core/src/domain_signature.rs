use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::canonical::{SemanticExpr, SemanticExprKind, lower_template};
use crate::equivalence::compile_guarded_forms;
use crate::law::unify_all;
use crate::pack::{DomainPack, PackKind, built_in_packs};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomainTerm {
    pub(crate) text: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomainSignature {
    pub(crate) pack_id: String,
    pub(crate) pack_version: String,
    pub(crate) title: String,
    pub(crate) pack_kind: PackKind,
    pub(crate) terms: Vec<DomainTerm>,
    pub(crate) dependencies: Vec<String>,
    pub(crate) capabilities: Vec<String>,
    pub(crate) structural_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LawCollision {
    pub(crate) left_pack_id: String,
    pub(crate) left_law_id: String,
    pub(crate) right_pack_id: String,
    pub(crate) right_law_id: String,
    pub(crate) structural_key: String,
    pub(crate) distinguishing_evidence: Vec<String>,
}

pub(crate) fn compile_domain_signatures(packs: &[DomainPack]) -> Vec<DomainSignature> {
    let mut signatures = packs
        .iter()
        .map(|pack| {
            let mut terms = BTreeMap::<String, String>::new();
            add_term(&mut terms, &pack.title, "pack-title");
            for rule in &pack.activation_rules {
                for phrase in &rule.phrases {
                    add_term(&mut terms, phrase, &format!("activation/{}", rule.id));
                }
            }
            for concept in &pack.concepts {
                add_term(
                    &mut terms,
                    &concept.title,
                    &format!("concept/{}", concept.id),
                );
                for alias in &concept.aliases {
                    add_term(&mut terms, alias, &format!("concept/{}/alias", concept.id));
                }
            }
            for quantity in &pack.quantity_kinds {
                add_term(
                    &mut terms,
                    &quantity.title,
                    &format!("quantity/{}", quantity.id),
                );
                for alias in &quantity.aliases {
                    add_term(
                        &mut terms,
                        alias,
                        &format!("quantity/{}/alias", quantity.id),
                    );
                }
            }
            for unit in &pack.units {
                add_term(&mut terms, &unit.id, &format!("unit/{}", unit.id));
                for alias in &unit.aliases {
                    add_term(&mut terms, alias, &format!("unit/{}/alias", unit.id));
                }
            }
            for entry in pack.roles.iter().chain(&pack.operators) {
                add_term(
                    &mut terms,
                    &entry.topic,
                    &format!("vocabulary/{}", entry.id),
                );
            }
            for law in &pack.laws {
                add_term(&mut terms, &law.title, &format!("law/{}", law.id));
                for role in &law.roles {
                    if let Some((_, concept)) = role.concept.rsplit_once(':') {
                        add_term(
                            &mut terms,
                            concept,
                            &format!("law/{}/role/{}", law.id, role.id),
                        );
                    }
                }
            }
            DomainSignature {
                pack_id: pack.pack_id.clone(),
                pack_version: pack.pack_version.clone(),
                title: pack.title.clone(),
                pack_kind: pack.pack_kind,
                terms: terms
                    .into_iter()
                    .map(|(text, source)| DomainTerm { text, source })
                    .collect(),
                dependencies: pack
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.pack_id.clone())
                    .collect(),
                capabilities: pack
                    .capabilities
                    .provides
                    .iter()
                    .chain(&pack.capabilities.requires)
                    .cloned()
                    .collect(),
                structural_keys: pack
                    .laws
                    .iter()
                    .flat_map(|law| {
                        let placeholders = law
                            .roles
                            .iter()
                            .map(|role| role.id.clone())
                            .collect::<BTreeSet<_>>();
                        let scalars = law
                            .roles
                            .iter()
                            .filter(|role| role.shape.as_deref() == Some("scalar"))
                            .map(|role| role.id.clone())
                            .collect::<BTreeSet<_>>();
                        law.relations().flat_map(move |relation| {
                            let placeholders = placeholders.clone();
                            compile_guarded_forms(lower_template(relation), &scalars)
                                .into_iter()
                                .map(move |form| {
                                    expression_shape_key(&form.expression, &placeholders)
                                })
                        })
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    signatures.sort_by(|left, right| left.pack_id.cmp(&right.pack_id));
    signatures
}

pub(crate) fn compile_collision_atlas(packs: &[DomainPack]) -> Vec<LawCollision> {
    #[derive(Clone)]
    struct FormOwner {
        pack_index: usize,
        law_index: usize,
        pack_id: String,
        law_id: String,
    }

    let mut by_key = BTreeMap::<String, Vec<FormOwner>>::new();
    for (pack_index, pack) in packs.iter().enumerate() {
        for (law_index, law) in pack.laws.iter().enumerate() {
            let placeholders = law
                .roles
                .iter()
                .map(|role| role.id.clone())
                .collect::<BTreeSet<_>>();
            let scalar_placeholders = law
                .roles
                .iter()
                .filter(|role| role.shape.as_deref() == Some("scalar"))
                .map(|role| role.id.clone())
                .collect::<BTreeSet<_>>();
            let keys = law
                .relations()
                .flat_map(|relation| {
                    compile_guarded_forms(lower_template(relation), &scalar_placeholders)
                })
                .map(|form| expression_shape_key(&form.expression, &placeholders))
                .collect::<BTreeSet<_>>();
            for key in keys {
                by_key.entry(key).or_default().push(FormOwner {
                    pack_index,
                    law_index,
                    pack_id: pack.pack_id.clone(),
                    law_id: law.id.clone(),
                });
            }
        }
    }

    let mut collisions = Vec::new();
    for (structural_key, owners) in by_key {
        for left_index in 0..owners.len() {
            for right_index in left_index + 1..owners.len() {
                let left = &owners[left_index];
                let right = &owners[right_index];
                if left.pack_id == right.pack_id && left.law_id == right.law_id {
                    continue;
                }
                let left_law = &packs[left.pack_index].laws[left.law_index];
                let right_law = &packs[right.pack_index].laws[right.law_index];
                collisions.push(LawCollision {
                    left_pack_id: left.pack_id.clone(),
                    left_law_id: left.law_id.clone(),
                    right_pack_id: right.pack_id.clone(),
                    right_law_id: right.law_id.clone(),
                    structural_key: structural_key.clone(),
                    distinguishing_evidence: distinguishing_evidence(left_law, right_law),
                });
            }
        }
    }

    #[derive(Clone)]
    struct FormPattern {
        owner: FormOwner,
        expression: SemanticExpr,
        placeholders: BTreeSet<String>,
    }
    let patterns = packs
        .iter()
        .enumerate()
        .flat_map(|(pack_index, pack)| {
            pack.laws
                .iter()
                .enumerate()
                .flat_map(move |(law_index, law)| {
                    let placeholders = law
                        .roles
                        .iter()
                        .map(|role| role.id.clone())
                        .collect::<BTreeSet<_>>();
                    let scalars = law
                        .roles
                        .iter()
                        .filter(|role| role.shape.as_deref() == Some("scalar"))
                        .map(|role| role.id.clone())
                        .collect::<BTreeSet<_>>();
                    let owner = FormOwner {
                        pack_index,
                        law_index,
                        pack_id: pack.pack_id.clone(),
                        law_id: law.id.clone(),
                    };
                    law.relations().flat_map(move |relation| {
                        let owner = owner.clone();
                        let placeholders = placeholders.clone();
                        compile_guarded_forms(lower_template(relation), &scalars)
                            .into_iter()
                            .map(move |form| FormPattern {
                                owner: owner.clone(),
                                expression: form.expression,
                                placeholders: placeholders.clone(),
                            })
                    })
                })
        })
        .collect::<Vec<_>>();
    for left_index in 0..patterns.len() {
        for right_index in left_index + 1..patterns.len() {
            let left = &patterns[left_index];
            let right = &patterns[right_index];
            if left.owner.pack_id == right.owner.pack_id && left.owner.law_id == right.owner.law_id
            {
                continue;
            }
            let overlaps = !unify_all(
                &left.expression,
                &right.expression,
                &left.placeholders,
                &BTreeMap::new(),
            )
            .is_empty()
                || !unify_all(
                    &right.expression,
                    &left.expression,
                    &right.placeholders,
                    &BTreeMap::new(),
                )
                .is_empty();
            if !overlaps {
                continue;
            }
            let mut keys = [
                expression_shape_key(&left.expression, &left.placeholders),
                expression_shape_key(&right.expression, &right.placeholders),
            ];
            keys.sort();
            collisions.push(LawCollision {
                left_pack_id: left.owner.pack_id.clone(),
                left_law_id: left.owner.law_id.clone(),
                right_pack_id: right.owner.pack_id.clone(),
                right_law_id: right.owner.law_id.clone(),
                structural_key: format!("overlap({},{})", keys[0], keys[1]),
                distinguishing_evidence: distinguishing_evidence(
                    &packs[left.owner.pack_index].laws[left.owner.law_index],
                    &packs[right.owner.pack_index].laws[right.owner.law_index],
                ),
            });
        }
    }
    collisions.sort_by(|left, right| {
        (
            &left.left_pack_id,
            &left.left_law_id,
            &left.right_pack_id,
            &left.right_law_id,
            &left.structural_key,
        )
            .cmp(&(
                &right.left_pack_id,
                &right.left_law_id,
                &right.right_pack_id,
                &right.right_law_id,
                &right.structural_key,
            ))
    });
    collisions.dedup();
    collisions
}

fn add_term(terms: &mut BTreeMap<String, String>, value: &str, source: &str) {
    let normalized = normalize_domain_text(value);
    if normalized.len() < 3 {
        return;
    }
    terms
        .entry(normalized)
        .and_modify(|current| {
            if source < current.as_str() {
                *current = source.to_owned();
            }
        })
        .or_insert_with(|| source.to_owned());
}

pub(crate) fn normalize_domain_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace('-', " ")
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn contains_domain_term(normalized_text: &str, term: &str) -> bool {
    normalized_text == term
        || normalized_text.starts_with(&format!("{term} "))
        || normalized_text.ends_with(&format!(" {term}"))
        || normalized_text.contains(&format!(" {term} "))
}

pub(crate) fn is_capability_pack(pack_id: &str) -> bool {
    static CAPABILITY_PACKS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
        built_in_packs()
            .iter()
            .filter(|pack| pack.pack_kind == PackKind::Capability)
            .map(|pack| pack.pack_id.clone())
            .collect()
    });
    CAPABILITY_PACKS.contains(pack_id)
}

pub(crate) fn laws_share_collision(
    left_pack_id: &str,
    left_law_id: &str,
    right_pack_id: &str,
    right_law_id: &str,
) -> bool {
    static COLLISION_PAIRS: LazyLock<BTreeSet<(String, String)>> = LazyLock::new(|| {
        compile_collision_atlas(built_in_packs())
            .into_iter()
            .map(|collision| {
                let mut pair = [
                    format!("{}:{}", collision.left_pack_id, collision.left_law_id),
                    format!("{}:{}", collision.right_pack_id, collision.right_law_id),
                ];
                pair.sort();
                (pair[0].clone(), pair[1].clone())
            })
            .collect()
    });
    let mut pair = [
        format!("{left_pack_id}:{left_law_id}"),
        format!("{right_pack_id}:{right_law_id}"),
    ];
    pair.sort();
    COLLISION_PAIRS.contains(&(pair[0].clone(), pair[1].clone()))
}

fn expression_shape_key(expression: &SemanticExpr, placeholders: &BTreeSet<String>) -> String {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => {
            if placeholders.contains(symbol) {
                "$".into()
            } else {
                format!("symbol({symbol})")
            }
        }
        SemanticExprKind::Number(value) => format!("number({value})"),
        SemanticExprKind::Sum(items) => list_key("sum", items, placeholders),
        SemanticExprKind::Product(items) => list_key("product", items, placeholders),
        SemanticExprKind::Dot(left, right) => binary_key("dot", left, right, placeholders),
        SemanticExprKind::Cross(left, right) => binary_key("cross", left, right, placeholders),
        SemanticExprKind::Fraction(left, right) => {
            binary_key("fraction", left, right, placeholders)
        }
        SemanticExprKind::Power(left, right) => binary_key("power", left, right, placeholders),
        SemanticExprKind::Negate(inner) => {
            format!("negate({})", expression_shape_key(inner, placeholders))
        }
        SemanticExprKind::Derivative {
            expression,
            variable,
            order,
        } => format!(
            "derivative({},{},{order})",
            expression_shape_key(expression, placeholders),
            if placeholders.contains(variable) {
                "$"
            } else {
                variable
            }
        ),
        SemanticExprKind::Relation {
            operator,
            left,
            right,
        } => binary_key(operator, left, right, placeholders),
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => {
            let operator = if placeholders.contains(operator) {
                "$"
            } else {
                operator
            };
            format!(
                "apply({operator},{})",
                list_key("args", arguments, placeholders)
            )
        }
        SemanticExprKind::Unknown(value) => format!("unknown({value})"),
    }
}

fn list_key(name: &str, items: &[SemanticExpr], placeholders: &BTreeSet<String>) -> String {
    format!(
        "{name}({})",
        items
            .iter()
            .map(|item| expression_shape_key(item, placeholders))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn binary_key(
    name: &str,
    left: &SemanticExpr,
    right: &SemanticExpr,
    placeholders: &BTreeSet<String>,
) -> String {
    format!(
        "{name}({},{})",
        expression_shape_key(left, placeholders),
        expression_shape_key(right, placeholders)
    )
}

fn distinguishing_evidence(
    left: &crate::pack::PackLaw,
    right: &crate::pack::PackLaw,
) -> Vec<String> {
    let mut evidence = vec!["domain".to_owned()];
    let left_concepts = left
        .roles
        .iter()
        .map(|role| role.concept.as_str())
        .collect::<BTreeSet<_>>();
    let right_concepts = right
        .roles
        .iter()
        .map(|role| role.concept.as_str())
        .collect::<BTreeSet<_>>();
    if left_concepts != right_concepts {
        evidence.push("concept".into());
    }
    let role_projection = |law: &crate::pack::PackLaw| {
        law.roles
            .iter()
            .map(|role| (role.quantity.clone(), role.shape.clone(), role.variadic))
            .collect::<Vec<_>>()
    };
    let left_projection = role_projection(left);
    let right_projection = role_projection(right);
    if left_projection
        .iter()
        .map(|item| &item.0)
        .collect::<Vec<_>>()
        != right_projection
            .iter()
            .map(|item| &item.0)
            .collect::<Vec<_>>()
    {
        evidence.push("quantity".into());
    }
    if left_projection
        .iter()
        .map(|item| &item.1)
        .collect::<Vec<_>>()
        != right_projection
            .iter()
            .map(|item| &item.1)
            .collect::<Vec<_>>()
    {
        evidence.push("shape".into());
    }
    if left.conditions != right.conditions {
        evidence.push("condition".into());
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::{compile_collision_atlas, compile_domain_signatures, contains_domain_term};
    use crate::pack::built_in_packs;

    #[test]
    fn derives_reviewed_terms_without_pack_specific_runtime_code() {
        let signatures = compile_domain_signatures(built_in_packs());
        let circuits = signatures
            .iter()
            .find(|signature| signature.pack_id == "circuits")
            .unwrap();
        assert!(
            circuits
                .terms
                .iter()
                .any(|term| term.text == "electric current")
        );
        assert!(
            circuits
                .dependencies
                .contains(&"quantities-units".to_owned())
        );
        assert!(
            circuits
                .capabilities
                .contains(&"semath:formula-recognition".to_owned())
        );
        assert!(!circuits.structural_keys.is_empty());
        assert!(contains_domain_term(
            "we study electric current in a circuit",
            "electric current"
        ));
        assert!(!contains_domain_term("currentness", "current"));
    }

    #[test]
    fn finds_cross_pack_collisions_across_guarded_forms() {
        let collisions = compile_collision_atlas(built_in_packs());
        assert!(collisions.iter().any(|collision| {
            collision.left_pack_id != collision.right_pack_id
                && collision
                    .distinguishing_evidence
                    .contains(&"domain".to_owned())
        }));
        assert_eq!(
            collisions,
            compile_collision_atlas(built_in_packs()),
            "the atlas must be deterministic"
        );
    }
}
