use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::pack::built_in_packs;

static PACK_ROLE_TERMS: LazyLock<Vec<(Vec<String>, String, u8)>> = LazyLock::new(|| {
    let mut terms = built_in_packs()
        .iter()
        .flat_map(|pack| {
            let mut entries = pack
                .roles
                .iter()
                .chain(&pack.operators)
                .map(|entry| {
                    (
                        entry.id.clone(),
                        format!("{}:{}", pack.namespace, entry.id),
                        0,
                    )
                })
                .collect::<Vec<_>>();
            for concept in &pack.concepts {
                let concept_id = format!("{}:{}", pack.namespace, concept.id);
                entries.push((concept.title.clone(), concept_id.clone(), 2));
                entries.extend(
                    concept
                        .aliases
                        .iter()
                        .cloned()
                        .map(|alias| (alias, concept_id.clone(), 2)),
                );
            }
            for kind in &pack.quantity_kinds {
                let concept_id = format!("{}:{}", pack.namespace, kind.id);
                entries.push((kind.id.replace('-', " "), concept_id.clone(), 1));
                entries.push((kind.title.clone(), concept_id.clone(), 1));
                entries.extend(
                    kind.aliases
                        .iter()
                        .cloned()
                        .map(|alias| (alias, concept_id.clone(), 1)),
                );
            }
            for law in &pack.laws {
                for role in &law.roles {
                    if let Some(concept) = role.concept.split(':').next_back() {
                        entries.push((concept.into(), role.concept.clone(), 2));
                    }
                }
            }
            entries
        })
        .filter(|(role, _, _)| {
            !matches!(
                role.trim().to_ascii_lowercase().as_str(),
                "matrix"
                    | "matrices"
                    | "scalar"
                    | "scalars"
                    | "tensor"
                    | "tensors"
                    | "vector"
                    | "vectors"
            )
        })
        .map(|(role, concept_id, priority)| {
            (
                role.to_ascii_lowercase()
                    .split(|character: char| !character.is_ascii_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                concept_id,
                priority,
            )
        })
        .collect::<Vec<_>>();
    terms.sort_by(|left, right| {
        right
            .0
            .len()
            .cmp(&left.0.len())
            .then(right.2.cmp(&left.2))
            .then(left.1.cmp(&right.1))
    });
    terms.dedup();
    terms
});

static PACK_CONCEPT_ANCESTORS: LazyLock<BTreeSet<(String, String)>> = LazyLock::new(|| {
    let parents = built_in_packs()
        .iter()
        .flat_map(|pack| {
            pack.concepts.iter().map(|concept| {
                (
                    format!("{}:{}", pack.namespace, concept.id),
                    concept.parents.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut ancestors = BTreeSet::new();
    for concept in parents.keys() {
        let mut frontier = parents.get(concept).cloned().unwrap_or_default();
        let mut visited = BTreeSet::new();
        while let Some(parent) = frontier.pop() {
            if !visited.insert(parent.clone()) {
                continue;
            }
            ancestors.insert((concept.clone(), parent.clone()));
            frontier.extend(parents.get(&parent).cloned().unwrap_or_default());
        }
    }
    ancestors
});

pub(crate) fn concepts_share_lineage(left: &str, right: &str) -> bool {
    left == right
        || PACK_CONCEPT_ANCESTORS.contains(&(left.to_owned(), right.to_owned()))
        || PACK_CONCEPT_ANCESTORS.contains(&(right.to_owned(), left.to_owned()))
}

pub(crate) fn classify_role(description: &str) -> Option<String> {
    let roles = classify_role_candidates(description);
    (roles.len() == 1).then(|| roles.into_iter().next().unwrap())
}

pub(crate) fn classify_role_candidates(description: &str) -> Vec<String> {
    let lower = description.to_ascii_lowercase();
    if let Some(coordinated) = lower.strip_prefix("both ") {
        let roles = coordinated
            .split(" and ")
            .flat_map(|part| part.split(" or "))
            .filter_map(classify_single_role)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !roles.is_empty() {
            return roles;
        }
    }
    classify_single_role(description).into_iter().collect()
}

fn classify_single_role(description: &str) -> Option<String> {
    let semantic_description = [" along ", " through ", " across ", " normal to "]
        .iter()
        .find_map(|separator| description.split_once(separator).map(|(head, _)| head))
        .unwrap_or(description);
    let normalized = semantic_description
        .to_ascii_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let words = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let first = words
        .iter()
        .copied()
        .find(|word| !matches!(*word, "a" | "an" | "the"));
    let last = words.last().copied();

    if let Some(role) = classified_pack_concept(&words) {
        Some(role)
    } else if contains_singular_or_plural(&words, "event") {
        Some("semath:event".into())
    } else if normalized.contains("probability distribution") || last == Some("distribution") {
        Some("semath:distribution".into())
    } else if ["set", "space", "domain", "codomain"]
        .iter()
        .any(|role| first.is_some_and(|word| singular_or_plural(word, role)))
        || ["set", "space", "domain", "codomain"]
            .iter()
            .any(|role| last.is_some_and(|word| singular_or_plural(word, role)))
    {
        Some("semath:set".into())
    } else if first == Some("index") || last == Some("index") {
        Some("semath:index".into())
    } else if contains_singular_or_plural(&words, "operator") {
        Some("semath:operator".into())
    } else if words.iter().any(|word| {
        ["function", "map", "mapping"]
            .iter()
            .any(|role| singular_or_plural(word, role))
    }) {
        Some("semath:function".into())
    } else {
        None
    }
}

fn classified_pack_concept(words: &[&str]) -> Option<String> {
    if words.iter().any(|word| matches!(*word, "and" | "or")) {
        return None;
    }
    let matches = PACK_ROLE_TERMS
        .iter()
        .filter(|(term, _, _)| contains_term(words, term))
        .collect::<Vec<_>>();
    let specificity = matches.iter().map(|(term, _, _)| term.len()).max()?;
    let priority = matches
        .iter()
        .filter(|(term, _, _)| term.len() == specificity)
        .map(|(_, _, priority)| *priority)
        .max()?;
    let concepts = matches
        .into_iter()
        .filter(|(term, _, candidate_priority)| {
            term.len() == specificity && *candidate_priority == priority
        })
        .map(|(_, concept, _)| concept.as_str())
        .collect::<BTreeSet<_>>();
    (concepts.len() == 1).then(|| concepts.into_iter().next().unwrap().to_owned())
}

fn contains_term(words: &[&str], term: &[String]) -> bool {
    words.windows(term.len()).any(|window| {
        window
            .iter()
            .zip(term)
            .all(|(word, expected)| singular_or_plural(word, expected))
    })
}

fn contains_singular_or_plural(words: &[&str], singular: &str) -> bool {
    words.iter().any(|word| singular_or_plural(word, singular))
}

fn singular_or_plural(word: &str, singular: &str) -> bool {
    word == singular || word.strip_suffix('s') == Some(singular)
}

#[cfg(test)]
mod tests {
    use super::{classify_role, classify_role_candidates, concepts_share_lineage};

    #[test]
    fn classifies_pack_quantities_through_the_shared_concept_vocabulary() {
        assert_eq!(
            classify_role("moving at speed").as_deref(),
            Some("quantities-units:velocity")
        );
        assert_eq!(
            classify_role("packet with signed charge").as_deref(),
            Some("quantities-units:electric-charge")
        );
        assert_eq!(
            classify_role("area-mean exit speed").as_deref(),
            Some("quantities-units:velocity")
        );
        assert_eq!(
            classify_role("electric potential").as_deref(),
            Some("quantities-units:voltage")
        );
        assert_eq!(
            classify_role("discharged mass per unit time").as_deref(),
            Some("quantities-units:mass-flow-rate")
        );
        assert_eq!(
            classify_role("per-example binary cross-entropy").as_deref(),
            Some("optimization-ml:loss-value")
        );
        assert_eq!(
            classify_role("preliminary volume rate").as_deref(),
            Some("quantities-units:volumetric-flow-rate")
        );
    }

    #[test]
    fn uses_compiled_pack_lineage_for_concept_compatibility() {
        assert!(concepts_share_lineage(
            "discrete-math:subset",
            "discrete-math:set"
        ));
        assert!(concepts_share_lineage(
            "linear-algebra:transpose",
            "linear-algebra:linear-operator"
        ));
        assert!(!concepts_share_lineage(
            "quantities-units:voltage",
            "quantities-units:resistance"
        ));
    }

    #[test]
    fn preserves_explicitly_coordinated_role_alternatives() {
        assert_eq!(classify_role("both kinetic energy and stiffness"), None);
        assert_eq!(
            classify_role_candidates("both kinetic energy and stiffness"),
            [
                "quantities-units:energy".to_owned(),
                "quantities-units:stiffness".to_owned(),
            ]
        );
    }
}
