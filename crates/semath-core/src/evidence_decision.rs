use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EvidenceAuthority {
    ExplicitAuthor,
    Derived(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceProof<R> {
    pub authority: EvidenceAuthority,
    pub roots: BTreeSet<R>,
    pub complete: bool,
}

impl<R: Ord> EvidenceProof<R> {
    fn independently_discriminates(&self, other: &Self) -> bool {
        self.authority == other.authority
            && self.roots.iter().any(|root| !other.roots.contains(root))
            && other.roots.iter().any(|root| !self.roots.contains(root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceAlternative<T, K, R> {
    pub value: T,
    pub comparison: K,
    pub proof: EvidenceProof<R>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EqualAuthorityConflict<C, K, R> {
    pub value: C,
    pub slot: K,
    pub authority: EvidenceAuthority,
    pub left_roots: BTreeSet<R>,
    pub right_roots: BTreeSet<R>,
}

impl<C, K, R: Ord> EqualAuthorityConflict<C, K, R> {
    pub(crate) fn new(
        value: C,
        slot: K,
        left_authority: EvidenceAuthority,
        left_roots: BTreeSet<R>,
        right_authority: EvidenceAuthority,
        right_roots: BTreeSet<R>,
    ) -> Option<Self> {
        (left_authority == right_authority && !left_roots.is_empty() && !right_roots.is_empty())
            .then_some(Self {
                value,
                slot,
                authority: left_authority,
                left_roots,
                right_roots,
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvidenceDecision<T, C> {
    Established(T),
    Partial(T),
    Ambiguous(Vec<T>),
    Conflicting(Vec<C>),
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceDecisionInput<T, K, R, C> {
    pub alternatives: Vec<EvidenceAlternative<T, K, R>>,
    pub conflicts: Vec<EqualAuthorityConflict<C, K, R>>,
    pub refuted: bool,
}

pub(crate) fn decide_evidence<T, K, R, C>(
    mut input: EvidenceDecisionInput<T, K, R, C>,
) -> EvidenceDecision<T, C>
where
    T: Clone + Ord,
    K: Eq + Ord,
    R: Ord,
    C: Ord,
{
    input.conflicts.sort();
    input.conflicts.dedup();
    if !input.conflicts.is_empty() {
        return EvidenceDecision::Conflicting(
            input
                .conflicts
                .into_iter()
                .map(|conflict| conflict.value)
                .collect(),
        );
    }
    if input.refuted {
        return EvidenceDecision::Unsupported;
    }

    for alternative in &mut input.alternatives {
        alternative.proof.complete &= !alternative.proof.roots.is_empty();
    }
    input.alternatives.sort_by(|left, right| {
        left.value
            .cmp(&right.value)
            .then(left.comparison.cmp(&right.comparison))
    });
    let mut alternatives: Vec<EvidenceAlternative<T, K, R>> = Vec::new();
    for mut alternative in input.alternatives {
        if let Some(existing) = alternatives
            .last_mut()
            .filter(|existing| existing.value == alternative.value)
        {
            let alternative_is_better = (!existing.proof.complete && alternative.proof.complete)
                || (existing.proof.complete == alternative.proof.complete
                    && alternative.proof.authority < existing.proof.authority);
            let same_proof_class = existing.proof.complete == alternative.proof.complete
                && existing.proof.authority == alternative.proof.authority;
            if alternative_is_better {
                existing.proof = alternative.proof;
            } else if same_proof_class {
                existing.proof.roots.append(&mut alternative.proof.roots);
            }
        } else {
            alternatives.push(alternative);
        }
    }
    input.alternatives = alternatives;
    let Some(first) = input.alternatives.first() else {
        return EvidenceDecision::Unsupported;
    };
    if input.alternatives.len() == 1 {
        return if first.proof.complete {
            EvidenceDecision::Established(first.value.clone())
        } else {
            EvidenceDecision::Partial(first.value.clone())
        };
    }

    let independently_comparable = input.alternatives.iter().enumerate().all(|(index, left)| {
        input.alternatives[index + 1..].iter().all(|right| {
            left.comparison == right.comparison
                && left.proof.independently_discriminates(&right.proof)
        })
    });
    if independently_comparable {
        return EvidenceDecision::Ambiguous(
            input
                .alternatives
                .into_iter()
                .map(|alternative| alternative.value)
                .collect(),
        );
    }

    let strongest_authority = input
        .alternatives
        .iter()
        .map(|alternative| alternative.proof.authority)
        .min();
    if let Some(authority) = strongest_authority {
        let complete = input
            .alternatives
            .iter()
            .filter(|alternative| {
                alternative.proof.complete && alternative.proof.authority == authority
            })
            .collect::<Vec<_>>();
        if let [unique_complete] = complete.as_slice() {
            return EvidenceDecision::Established(unique_complete.value.clone());
        }
        let dominant = complete
            .iter()
            .copied()
            .filter(|candidate| {
                complete.iter().copied().all(|other| {
                    candidate.value == other.value
                        || (candidate.comparison == other.comparison
                            && other.proof.roots.is_subset(&candidate.proof.roots)
                            && other.proof.roots != candidate.proof.roots)
                })
            })
            .collect::<Vec<_>>();
        if let [dominant] = dominant.as_slice() {
            return EvidenceDecision::Established(dominant.value.clone());
        }
    }
    let preferred = input
        .alternatives
        .iter()
        .min_by_key(|alternative| {
            (
                alternative.proof.authority,
                !alternative.proof.complete,
                &alternative.value,
            )
        })
        .expect("nonempty alternatives have a preferred partial result");
    EvidenceDecision::Partial(preferred.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof(roots: &[u8], complete: bool) -> EvidenceProof<u8> {
        EvidenceProof {
            authority: EvidenceAuthority::ExplicitAuthor,
            roots: roots.iter().copied().collect(),
            complete,
        }
    }

    fn alternative(
        value: &str,
        roots: &[u8],
        complete: bool,
    ) -> EvidenceAlternative<String, u8, u8> {
        EvidenceAlternative {
            value: value.into(),
            comparison: 1,
            proof: proof(roots, complete),
        }
    }

    fn input(
        alternatives: Vec<EvidenceAlternative<String, u8, u8>>,
    ) -> EvidenceDecisionInput<String, u8, u8, String> {
        EvidenceDecisionInput {
            alternatives,
            conflicts: Vec::new(),
            refuted: false,
        }
    }

    #[test]
    fn establishment_requires_one_complete_typed_proof() {
        assert_eq!(
            decide_evidence(input(vec![alternative("law", &[1], true)])),
            EvidenceDecision::Established("law".into())
        );
        assert_eq!(
            decide_evidence(input(vec![alternative("law", &[1], false)])),
            EvidenceDecision::Partial("law".into())
        );
    }

    #[test]
    fn removing_the_last_proof_root_revokes_establishment() {
        assert_eq!(
            decide_evidence(input(vec![alternative("law", &[], true)])),
            EvidenceDecision::Partial("law".into())
        );
    }

    #[test]
    fn ambiguity_requires_comparable_independent_discriminators() {
        assert!(matches!(
            decide_evidence(input(vec![
                alternative("first", &[0, 1], true),
                alternative("second", &[0, 2], true),
            ])),
            EvidenceDecision::Ambiguous(_)
        ));
        assert!(matches!(
            decide_evidence(input(vec![
                alternative("first", &[0, 1], true),
                alternative("second", &[0, 1], true),
            ])),
            EvidenceDecision::Partial(_)
        ));
    }

    #[test]
    fn one_sided_support_over_shared_roots_selects_the_supported_alternative() {
        assert_eq!(
            decide_evidence(input(vec![
                alternative("enclosing", &[0, 1], true),
                alternative("nested", &[0], true),
            ])),
            EvidenceDecision::Established("enclosing".into())
        );
        assert!(matches!(
            decide_evidence(input(vec![
                alternative("first", &[0], true),
                alternative("second", &[0], true),
            ])),
            EvidenceDecision::Partial(_)
        ));
    }

    #[test]
    fn only_equal_authority_source_opposition_can_conflict() {
        let explicit = EvidenceAuthority::ExplicitAuthor;
        let valid = EqualAuthorityConflict::new(
            "opposition".to_owned(),
            1,
            explicit,
            BTreeSet::from([1]),
            explicit,
            BTreeSet::from([2]),
        )
        .unwrap();
        assert!(matches!(
            decide_evidence(EvidenceDecisionInput {
                alternatives: vec![alternative("law", &[1], true)],
                conflicts: vec![valid],
                refuted: false,
            }),
            EvidenceDecision::Conflicting(_)
        ));
        assert!(
            EqualAuthorityConflict::new(
                "opposition".to_owned(),
                1,
                explicit,
                BTreeSet::from([1]),
                EvidenceAuthority::Derived(1),
                BTreeSet::from([2]),
            )
            .is_none()
        );
    }

    #[test]
    fn order_duplicates_and_incomplete_noise_do_not_increase_certainty() {
        let first = alternative("law", &[1], true);
        let duplicate_gap = alternative("law", &[1], false);
        let noise = alternative("hypothesis", &[1], false);
        for alternatives in [
            vec![first.clone(), noise.clone(), duplicate_gap.clone()],
            vec![noise, duplicate_gap, first],
        ] {
            assert_eq!(
                decide_evidence(input(alternatives)),
                EvidenceDecision::Established("law".into())
            );
        }
    }

    #[test]
    fn weaker_complete_alternative_does_not_reduce_explicit_establishment() {
        let mut derived = alternative("derived", &[2], true);
        derived.proof.authority = EvidenceAuthority::Derived(1);
        assert_eq!(
            decide_evidence(input(vec![derived, alternative("explicit", &[1], true)])),
            EvidenceDecision::Established("explicit".into())
        );
    }

    #[test]
    fn weaker_complete_proof_cannot_override_stronger_incomplete_evidence() {
        let explicit_incomplete = alternative("explicit", &[1], false);
        let mut derived_complete = alternative("derived", &[2], true);
        derived_complete.proof.authority = EvidenceAuthority::Derived(1);
        assert_eq!(
            decide_evidence(input(vec![derived_complete, explicit_incomplete])),
            EvidenceDecision::Partial("explicit".into())
        );
    }

    #[test]
    fn partial_projection_prefers_authority_over_lexical_order() {
        let mut lexical_first = alternative("a-derived", &[1], false);
        lexical_first.proof.authority = EvidenceAuthority::Derived(1);
        assert_eq!(
            decide_evidence(input(vec![
                lexical_first,
                alternative("z-explicit", &[1], false),
            ])),
            EvidenceDecision::Partial("z-explicit".into())
        );
    }

    #[test]
    fn incomplete_duplicate_noise_cannot_invent_independent_ambiguity() {
        assert!(matches!(
            decide_evidence(input(vec![
                alternative("first", &[1], true),
                alternative("second", &[1], true),
                alternative("first", &[2], false),
                alternative("second", &[3], false),
            ])),
            EvidenceDecision::Partial(_)
        ));
    }

    #[test]
    fn weaker_duplicate_roots_cannot_invent_explicit_ambiguity() {
        let mut weaker = alternative("first", &[2], true);
        weaker.proof.authority = EvidenceAuthority::Derived(1);
        assert!(matches!(
            decide_evidence(input(vec![
                alternative("first", &[1], true),
                weaker,
                alternative("second", &[1], true),
            ])),
            EvidenceDecision::Partial(_)
        ));
    }

    #[test]
    fn refutation_is_calm_without_a_typed_conflict() {
        assert_eq!(
            decide_evidence(EvidenceDecisionInput::<String, u8, u8, String> {
                alternatives: Vec::new(),
                conflicts: Vec::new(),
                refuted: true,
            }),
            EvidenceDecision::Unsupported
        );
    }
}
