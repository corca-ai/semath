use std::collections::BTreeSet;

use crate::evidence_decision::{
    EqualAuthorityConflict, EvidenceAlternative, EvidenceAuthority, EvidenceDecision,
    EvidenceDecisionInput, EvidenceProof, decide_evidence,
};
use crate::{
    ConstraintStatus, DecisionReason, DecisionReasonKind, Evidence, LawBindingProof,
    LawRecognition, LawRecognitionStatus, MeaningAlternative, MeaningConclusion, MeaningConflict,
    MeaningDecision, MeaningFact, MeaningRequirement, SemanticCandidateInfo,
    SemanticCandidateStatus, SymbolInfo,
};

const MAX_DECISION_ITEMS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DecisionChoice {
    Formula(String),
    Symbol,
    Candidate(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DecisionComparison {
    Formula(u32, u32),
    Symbol,
    Candidate(u32, u32),
    Conflict(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DecisionRoot {
    Source(u32, u32),
    Claim(String),
    ConflictEvidence(usize, u32, u32),
}

pub(crate) struct MeaningDecisionInput<'a> {
    pub formulas: &'a [LawRecognition],
    pub symbol: Option<&'a SymbolInfo>,
    pub symbol_proof: &'a [Evidence],
    pub candidates: &'a [SemanticCandidateInfo],
    pub conflicts: &'a [MeaningConflict],
    pub engine_limited: bool,
    pub unsupported_relation_context: bool,
    pub truncated: bool,
}

pub(crate) fn decide_meaning(input: MeaningDecisionInput<'_>) -> MeaningDecision {
    let conflicts = collect_conflicts(&input);
    let (typed_conflicts, rejected_conflict) = typed_conflicts(&conflicts);
    let formulas = decision_formulas(&input);
    let mut alternatives = formulas
        .iter()
        .filter_map(|formula| formula_alternative(formula, input.truncated))
        .collect::<Vec<_>>();
    if alternatives.is_empty() {
        if input.symbol.is_some() && (!input.engine_limited || !input.symbol_proof.is_empty()) {
            alternatives.push(symbol_alternative(input.symbol_proof, input.truncated));
        } else {
            alternatives.extend(candidate_alternatives(&input));
        }
    }
    let decision = decide_evidence(EvidenceDecisionInput {
        alternatives,
        conflicts: typed_conflicts,
        refuted: input.unsupported_relation_context || rejected_conflict,
    });

    match decision {
        EvidenceDecision::Conflicting(conflict_ids) => {
            let mut conflicts = conflict_ids
                .into_iter()
                .filter_map(|id| conflicts.iter().find(|conflict| conflict.conflict_id == id))
                .cloned()
                .collect::<Vec<_>>();
            for conflict in &mut conflicts {
                conflict.evidence = deduplicate_evidence(std::mem::take(&mut conflict.evidence));
            }
            let reasons = conflicts
                .iter()
                .map(|conflict| DecisionReason {
                    kind: DecisionReasonKind::SourceConflict,
                    label: conflict.label.clone(),
                    evidence: deduplicate_evidence(conflict.evidence.clone()),
                })
                .collect();
            MeaningDecision::Conflicting { conflicts, reasons }
        }
        EvidenceDecision::Ambiguous(choices) => MeaningDecision::Ambiguous {
            alternatives: public_alternatives(&input, &formulas, &choices),
            reasons: vec![uncertainty_reason(
                "More than one independently supported interpretation remains.",
            )],
        },
        EvidenceDecision::Established(choice) => established_decision(&input, &formulas, &choice),
        EvidenceDecision::Partial(choice) => partial_decision(&input, &formulas, &choice),
        EvidenceDecision::Unsupported => unsupported_decision(&input),
    }
}

pub(crate) fn symbol_has_source_meaning(symbol: &SymbolInfo) -> bool {
    !symbol.definitions.is_empty()
        || !symbol.roles.is_empty()
        || !symbol.shapes.is_empty()
        || !symbol.quantities.is_empty()
}

fn collect_conflicts(input: &MeaningDecisionInput<'_>) -> Vec<MeaningConflict> {
    let mut conflicts = input.conflicts.to_vec();
    conflicts.sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
    let mut collected: Vec<MeaningConflict> = Vec::new();
    for mut conflict in conflicts {
        if let Some(existing) = collected
            .last_mut()
            .filter(|existing| existing.conflict_id == conflict.conflict_id)
        {
            existing.evidence.append(&mut conflict.evidence);
        } else {
            collected.push(conflict);
        }
    }
    collected.truncate(MAX_DECISION_ITEMS);
    collected
}

fn typed_conflicts(
    conflicts: &[MeaningConflict],
) -> (
    Vec<EqualAuthorityConflict<String, DecisionComparison, DecisionRoot>>,
    bool,
) {
    let mut rejected = false;
    let typed = conflicts
        .iter()
        .filter_map(|conflict| {
            let roots = conflict
                .evidence
                .iter()
                .enumerate()
                .flat_map(|(index, evidence)| {
                    evidence.source_ranges.iter().map(move |range| {
                        DecisionRoot::ConflictEvidence(index, range.start_offset, range.end_offset)
                    })
                })
                .collect::<BTreeSet<_>>();
            let Some(first) = roots.iter().next().cloned() else {
                rejected = true;
                return None;
            };
            let left = BTreeSet::from([first.clone()]);
            let right = roots
                .into_iter()
                .filter(|root| root != &first)
                .collect::<BTreeSet<_>>();
            let Some(conflict) = EqualAuthorityConflict::new(
                conflict.conflict_id.clone(),
                DecisionComparison::Conflict(conflict.conflict_id.clone()),
                EvidenceAuthority::ExplicitAuthor,
                left,
                EvidenceAuthority::ExplicitAuthor,
                right,
            ) else {
                rejected = true;
                return None;
            };
            Some(conflict)
        })
        .collect();
    (typed, rejected)
}

pub(crate) fn formula_has_establishment_proof(formula: &LawRecognition) -> bool {
    matches!(
        formula.status,
        LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
    ) && formula.relation.is_some()
        && formula.bindings.iter().all(|binding| {
            matches!(
                binding.proof,
                LawBindingProof::Typed | LawBindingProof::Derived
            ) && !binding.evidence.source_ranges.is_empty()
        })
        && formula
            .conditions
            .iter()
            .all(|condition| condition.status == ConstraintStatus::Verified)
}

fn evidence_roots(evidence: &[Evidence]) -> BTreeSet<DecisionRoot> {
    evidence
        .iter()
        .flat_map(|evidence| &evidence.source_ranges)
        .map(|range| DecisionRoot::Source(range.start_offset, range.end_offset))
        .collect()
}

fn formula_evidence_roots(
    evidence: &[Evidence],
    formula_range: &crate::SourceRange,
) -> BTreeSet<DecisionRoot> {
    evidence
        .iter()
        .flat_map(|evidence| &evidence.source_ranges)
        .map(|range| {
            if range.start_offset < formula_range.end_offset
                && formula_range.start_offset < range.end_offset
            {
                DecisionRoot::Source(formula_range.start_offset, formula_range.end_offset)
            } else {
                DecisionRoot::Source(range.start_offset, range.end_offset)
            }
        })
        .collect()
}

fn formula_alternative(
    formula: &LawRecognition,
    truncated: bool,
) -> Option<EvidenceAlternative<DecisionChoice, DecisionComparison, DecisionRoot>> {
    let relation = formula.relation.as_ref()?;
    let evidence = formula
        .evidence
        .iter()
        .chain(formula.bindings.iter().map(|binding| &binding.evidence))
        .chain(
            formula
                .conditions
                .iter()
                .flat_map(|condition| &condition.evidence),
        )
        .cloned()
        .collect::<Vec<_>>();
    Some(EvidenceAlternative {
        value: DecisionChoice::Formula(relation.relation_id.clone()),
        comparison: DecisionComparison::Formula(
            relation.range.start_offset,
            relation.range.end_offset,
        ),
        proof: EvidenceProof {
            // A derived role is part of the same reviewed law proof, not a
            // weaker competing interpretation. Its derivation parents remain
            // in the roots, where correlation and dominance are decided.
            authority: EvidenceAuthority::ExplicitAuthor,
            roots: formula_evidence_roots(&evidence, &relation.range),
            complete: formula_has_establishment_proof(formula) && !truncated,
        },
    })
}

fn symbol_alternative(
    proof: &[Evidence],
    truncated: bool,
) -> EvidenceAlternative<DecisionChoice, DecisionComparison, DecisionRoot> {
    EvidenceAlternative {
        value: DecisionChoice::Symbol,
        comparison: DecisionComparison::Symbol,
        proof: EvidenceProof {
            authority: EvidenceAuthority::ExplicitAuthor,
            roots: evidence_roots(proof),
            complete: !proof.is_empty() && !truncated,
        },
    }
}

fn candidate_alternatives(
    input: &MeaningDecisionInput<'_>,
) -> Vec<EvidenceAlternative<DecisionChoice, DecisionComparison, DecisionRoot>> {
    input
        .candidates
        .iter()
        .filter(|candidate| candidate.status == SemanticCandidateStatus::Supported)
        .map(|candidate| EvidenceAlternative {
            value: DecisionChoice::Candidate(candidate.candidate_id.clone()),
            comparison: DecisionComparison::Candidate(
                candidate.range.start_offset,
                candidate.range.end_offset,
            ),
            proof: EvidenceProof {
                authority: EvidenceAuthority::ExplicitAuthor,
                roots: candidate
                    .supporting_claim_ids
                    .iter()
                    .cloned()
                    .map(DecisionRoot::Claim)
                    .collect(),
                complete: false,
            },
        })
        .collect()
}

fn decision_formulas<'a>(input: &'a MeaningDecisionInput<'a>) -> Vec<&'a LawRecognition> {
    let candidates = input.formulas.iter().collect::<Vec<_>>();
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            !candidates
                .iter()
                .any(|other| formula_structurally_dominates(other, candidate))
        })
        .collect()
}

fn formula_structurally_dominates(outer: &LawRecognition, nested: &LawRecognition) -> bool {
    outer.law_id != nested.law_id
        && outer.pack_id == nested.pack_id
        && outer.range == nested.range
        && outer.bindings.len() > nested.bindings.len()
        && nested.bindings.iter().all(|binding| {
            binding.evidence.source_ranges.iter().all(|nested_range| {
                outer.bindings.iter().any(|outer_binding| {
                    outer_binding
                        .evidence
                        .source_ranges
                        .iter()
                        .any(|outer_range| {
                            outer_range.start_offset <= nested_range.start_offset
                                && nested_range.end_offset <= outer_range.end_offset
                        })
                })
            })
        })
}

fn formula_for_choice<'a>(
    formulas: &[&'a LawRecognition],
    choice: &DecisionChoice,
) -> Option<&'a LawRecognition> {
    let DecisionChoice::Formula(relation_id) = choice else {
        return None;
    };
    formulas.iter().copied().find(|formula| {
        formula
            .relation
            .as_ref()
            .is_some_and(|relation| relation.relation_id == *relation_id)
    })
}

fn public_alternatives(
    input: &MeaningDecisionInput<'_>,
    formulas: &[&LawRecognition],
    choices: &[DecisionChoice],
) -> Vec<MeaningAlternative> {
    let mut alternatives = choices
        .iter()
        .filter_map(|choice| match choice {
            DecisionChoice::Formula(_) => {
                let formula = formula_for_choice(formulas, choice)?;
                let relation = formula.relation.as_ref()?;
                Some((
                    formula.rank,
                    MeaningAlternative {
                        alternative_id: relation.relation_id.clone(),
                        label: relation.title.clone(),
                        range: relation.range.clone(),
                        evidence: relation.evidence.clone(),
                        relevance: formula.relevance.clone(),
                    },
                ))
            }
            DecisionChoice::Candidate(candidate_id) => {
                let candidate = input
                    .candidates
                    .iter()
                    .find(|candidate| candidate.candidate_id == *candidate_id)?;
                Some((
                    u32::MAX,
                    MeaningAlternative {
                        alternative_id: candidate.candidate_id.clone(),
                        label: candidate.interpretation.clone(),
                        range: candidate.range.clone(),
                        evidence: vec![candidate_evidence(candidate)],
                        relevance: None,
                    },
                ))
            }
            DecisionChoice::Symbol => None,
        })
        .collect::<Vec<_>>();
    alternatives.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.alternative_id.cmp(&right.1.alternative_id))
    });
    alternatives
        .into_iter()
        .map(|(_, alternative)| alternative)
        .take(MAX_DECISION_ITEMS)
        .collect()
}

fn established_decision(
    input: &MeaningDecisionInput<'_>,
    formulas: &[&LawRecognition],
    choice: &DecisionChoice,
) -> MeaningDecision {
    if let Some(formula) = formula_for_choice(formulas, choice) {
        let relation = formula
            .relation
            .as_ref()
            .expect("a formula decision choice has a relation");
        return MeaningDecision::Established {
            meaning: MeaningConclusion {
                label: formula.title.clone(),
                relation_id: Some(relation.relation_id.clone()),
            },
            reasons: vec![DecisionReason {
                kind: DecisionReasonKind::Proof,
                label: "Supported by source-linked declarations and constraints.".into(),
                evidence: established_evidence(formula),
            }],
        };
    }
    if matches!(choice, DecisionChoice::Symbol)
        && let Some(symbol) = input.symbol
    {
        return MeaningDecision::Established {
            meaning: MeaningConclusion {
                label: symbol
                    .definitions
                    .first()
                    .map_or_else(|| symbol.symbol.clone(), |item| item.description.clone()),
                relation_id: None,
            },
            reasons: vec![DecisionReason {
                kind: DecisionReasonKind::Proof,
                label: "Established by an asserted source definition.".into(),
                evidence: deduplicate_evidence(input.symbol_proof.to_vec()),
            }],
        };
    }
    partial_decision(input, formulas, choice)
}

fn partial_decision(
    input: &MeaningDecisionInput<'_>,
    formulas: &[&LawRecognition],
    choice: &DecisionChoice,
) -> MeaningDecision {
    if let Some(formula) = formula_for_choice(formulas, choice) {
        return MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: formula.title.clone(),
                relation_id: formula
                    .relation
                    .as_ref()
                    .map(|relation| relation.relation_id.clone()),
            },
            facts: formula_facts(formula),
            requirements: missing_formula_requirements(formula),
            reasons: truncation_reason(input.truncated).into_iter().collect(),
        };
    }
    if matches!(choice, DecisionChoice::Symbol)
        && let Some(symbol) = input.symbol
    {
        return MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: symbol
                    .definitions
                    .first()
                    .map_or_else(|| symbol.symbol.clone(), |item| item.description.clone()),
                relation_id: None,
            },
            facts: symbol_facts(symbol),
            requirements: Vec::new(),
            reasons: truncation_reason(input.truncated).into_iter().collect(),
        };
    }
    if let DecisionChoice::Candidate(candidate_id) = choice
        && let Some(candidate) = input
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == *candidate_id)
    {
        return MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: candidate.interpretation.clone(),
                relation_id: None,
            },
            facts: Vec::new(),
            requirements: vec![MeaningRequirement {
                requirement_id: format!("resolve/{}", candidate.candidate_id),
                label: "Independent source evidence does not yet select this interpretation."
                    .into(),
                subjects: Vec::new(),
                evidence: vec![candidate_evidence(candidate)],
            }],
            reasons: vec![uncertainty_reason(
                "A structural interpretation is available without enough independent support.",
            )],
        };
    }
    unsupported_decision(input)
}

fn unsupported_decision(input: &MeaningDecisionInput<'_>) -> MeaningDecision {
    let has_source_meaning = !input.formulas.is_empty()
        || input
            .candidates
            .iter()
            .any(|candidate| candidate.status == SemanticCandidateStatus::Supported)
        || !input.symbol_proof.is_empty();
    let mut reasons = if input.engine_limited && !has_source_meaning {
        vec![DecisionReason {
            kind: DecisionReasonKind::EngineLimit,
            label: "The notation is opaque to the current syntax engine.".into(),
            evidence: Vec::new(),
        }]
    } else {
        vec![uncertainty_reason(
            "No source-supported interpretation is currently available.",
        )]
    };
    if let Some(reason) = truncation_reason(input.truncated) {
        reasons.push(reason);
    }
    MeaningDecision::Unsupported { reasons }
}

fn established_evidence(formula: &LawRecognition) -> Vec<Evidence> {
    let evidence = formula
        .evidence
        .iter()
        .chain(
            formula
                .conditions
                .iter()
                .filter(|condition| condition.status == ConstraintStatus::Verified)
                .flat_map(|condition| &condition.evidence),
        )
        .cloned()
        .collect::<Vec<_>>();
    deduplicate_evidence(evidence)
}

fn candidate_evidence(candidate: &SemanticCandidateInfo) -> Evidence {
    Evidence {
        rule_id: format!("semantic-candidate/{}", candidate.candidate_id),
        kind: "structural-candidate".into(),
        strength: "weak".into(),
        source_ranges: vec![candidate.range.clone()],
        source_anchors: Vec::new(),
    }
}

fn missing_formula_requirements(formula: &LawRecognition) -> Vec<MeaningRequirement> {
    let binding_requirements = formula
        .bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.proof,
                LawBindingProof::Asserted | LawBindingProof::Candidate
            )
        })
        .map(|binding| MeaningRequirement {
            requirement_id: format!("{}/binding/{}", formula.law_id, binding.parameter),
            label: format!(
                "The {} role is not independently established.",
                binding.parameter
            ),
            subjects: vec![binding.symbol.clone()],
            evidence: vec![binding.evidence.clone()],
        });
    let condition_requirements = formula
        .conditions
        .iter()
        .filter(|condition| {
            matches!(
                condition.status,
                ConstraintStatus::Conflicting
                    | ConstraintStatus::Required
                    | ConstraintStatus::Unsupported
            )
        })
        .map(|condition| MeaningRequirement {
            requirement_id: format!("{}/condition/{}", formula.law_id, condition.condition_id),
            label: condition.label.clone(),
            subjects: condition.subjects.clone(),
            evidence: condition.evidence.clone(),
        });
    let mut missing = binding_requirements
        .chain(condition_requirements)
        .collect::<Vec<_>>();
    missing.truncate(MAX_DECISION_ITEMS);
    missing
}

fn formula_facts(formula: &LawRecognition) -> Vec<MeaningFact> {
    formula
        .bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.proof,
                LawBindingProof::Typed | LawBindingProof::Derived
            )
        })
        .take(MAX_DECISION_ITEMS)
        .map(|binding| MeaningFact {
            fact_id: format!("{}/binding/{}", formula.law_id, binding.parameter),
            label: format!("{} is {}", binding.symbol, binding.parameter),
            evidence: vec![binding.evidence.clone()],
        })
        .collect()
}

fn symbol_facts(symbol: &SymbolInfo) -> Vec<MeaningFact> {
    symbol
        .definitions
        .iter()
        .take(MAX_DECISION_ITEMS)
        .map(|definition| MeaningFact {
            fact_id: format!(
                "definition/{}/{}",
                definition.location.file_id, definition.location.range.start_offset
            ),
            label: definition.description.clone(),
            evidence: vec![definition.evidence.clone()],
        })
        .collect()
}

fn truncation_reason(truncated: bool) -> Option<DecisionReason> {
    truncated.then(|| DecisionReason {
        kind: DecisionReasonKind::EngineLimit,
        label: "Additional engine evidence was omitted by a bounded result limit.".into(),
        evidence: Vec::new(),
    })
}

fn uncertainty_reason(label: &str) -> DecisionReason {
    DecisionReason {
        kind: DecisionReasonKind::Uncertainty,
        label: label.into(),
        evidence: Vec::new(),
    }
}

fn deduplicate_evidence(mut evidence: Vec<Evidence>) -> Vec<Evidence> {
    evidence.sort_by(|left, right| {
        let left_range = left.source_ranges.first();
        let right_range = right.source_ranges.first();
        (
            left.kind.as_str(),
            left_range.map_or(0, |range| range.start_offset),
            left_range.map_or(0, |range| range.end_offset),
            left.rule_id.as_str(),
        )
            .cmp(&(
                right.kind.as_str(),
                right_range.map_or(0, |range| range.start_offset),
                right_range.map_or(0, |range| range.end_offset),
                right.rule_id.as_str(),
            ))
    });
    evidence.dedup_by(|left, right| {
        left.kind == right.kind && left.source_ranges == right.source_ranges
    });
    evidence.truncate(MAX_DECISION_ITEMS);
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConstraintStatus, Evidence, LawBinding, LawBindingProof, LawConditionInfo,
        LawRecognitionStatus, RelationInfo, SemanticConstraint, SemanticConstraintKind,
        SourceRange,
    };

    #[test]
    fn incomplete_correlated_formula_does_not_block_complete_proof() {
        let verified = formula("verified", ConstraintStatus::Verified);
        let required = formula("required", ConstraintStatus::Required);
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&verified))),
            MeaningDecision::Established { .. }
        ));
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&required))),
            MeaningDecision::Partial { .. }
        ));
        assert!(matches!(
            decide_meaning(input(&[verified.clone(), required.clone()])),
            MeaningDecision::Established { .. }
        ));

        let mut conflicting = required;
        conflicting.conditions[0].status = ConstraintStatus::Conflicting;
        conflicting.status = LawRecognitionStatus::Conflicting;
        assert!(matches!(
            decide_meaning(input(&[verified, conflicting])),
            MeaningDecision::Established { .. }
        ));
    }

    #[test]
    fn a_rejected_law_condition_is_a_missing_proof_not_a_document_conflict() {
        let mut formula = formula("conditional", ConstraintStatus::Conflicting);
        formula.status = LawRecognitionStatus::Conflicting;
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { requirements, .. }
                if requirements.iter().any(|requirement| {
                    requirement.requirement_id == "conditional/condition/condition"
                })
        ));
    }

    #[test]
    fn a_formula_needs_typed_recognition_proof_not_presentation_markers() {
        let mut formula = formula("hypothesis", ConstraintStatus::Verified);
        formula.status = LawRecognitionStatus::Conflicting;
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { .. }
        ));

        formula.status = LawRecognitionStatus::Verified;
        formula.evidence[0].kind = "structural-candidate".into();
        formula.evidence[0].strength = "weak".into();
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Established { .. }
        ));
    }

    #[test]
    fn an_unresolved_role_binding_cannot_establish_a_recognized_formula() {
        let mut formula = formula("candidate", ConstraintStatus::Verified);
        formula.bindings[0].proof = LawBindingProof::Candidate;

        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { .. }
        ));
    }

    #[test]
    fn an_asserted_formula_does_not_invent_role_identity() {
        let mut formula = formula("asserted", ConstraintStatus::Verified);
        formula.bindings[0].proof = LawBindingProof::Asserted;
        formula.bindings[0].evidence.kind = "asserted-binding".into();
        formula.bindings[0].evidence.strength = "strong".into();

        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { facts, requirements, .. }
                if facts.is_empty()
                    && requirements.iter().any(|requirement| {
                        requirement.requirement_id == "asserted/binding/value"
                            && requirement.subjects == ["x"]
                    })
        ));
    }

    #[test]
    fn unresolved_role_binding_is_a_requirement_not_a_fact() {
        let mut formula = formula("candidate", ConstraintStatus::Verified);
        formula.bindings[0].proof = LawBindingProof::Candidate;
        formula.bindings[0].evidence.kind = "candidate-binding".into();
        formula.bindings[0].evidence.strength = "weak".into();

        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { facts, requirements, .. }
                if facts.is_empty()
                    && requirements.iter().any(|requirement| {
                        requirement.requirement_id == "candidate/binding/value"
                            && requirement.subjects == ["x"]
                    })
        ));
    }

    #[test]
    fn only_typed_or_derived_role_proof_can_establish_a_formula() {
        for (proof, established) in [
            (LawBindingProof::Typed, true),
            (LawBindingProof::Derived, true),
            (LawBindingProof::Asserted, false),
            (LawBindingProof::Candidate, false),
        ] {
            let mut formula = formula("proof", ConstraintStatus::Verified);
            formula.bindings[0].proof = proof;
            let decision = decide_meaning(input(std::slice::from_ref(&formula)));
            assert_eq!(
                matches!(decision, MeaningDecision::Established { .. }),
                established,
                "{proof:?}",
            );
        }
    }

    #[test]
    fn domain_rank_orders_but_does_not_resolve_correlated_alternatives() {
        let mut preferred = formula("preferred", ConstraintStatus::Verified);
        preferred.rank = 10;
        let mut fallback = formula("fallback", ConstraintStatus::Verified);
        fallback.rank = 30;
        assert!(matches!(
            decide_meaning(input(&[fallback, preferred])),
            MeaningDecision::Partial { .. }
        ));
    }

    #[test]
    fn enclosing_typed_relation_suppresses_nested_law_ambiguity() {
        let nested = formula("nested", ConstraintStatus::Verified);
        let mut enclosing = formula("enclosing", ConstraintStatus::Verified);
        enclosing.bindings.push(enclosing.bindings[0].clone());
        assert!(matches!(
            decide_meaning(input(&[nested, enclosing])),
            MeaningDecision::Established { meaning, .. }
                if meaning.relation_id.as_deref() == Some("enclosing")
        ));
    }

    #[test]
    fn independent_same_range_laws_remain_ambiguous() {
        let mut first = formula("first", ConstraintStatus::Verified);
        first.evidence.push(source_evidence(2, 3));
        let mut second = formula("second", ConstraintStatus::Verified);
        second.evidence.push(source_evidence(4, 5));
        assert!(matches!(
            decide_meaning(input(&[first, second])),
            MeaningDecision::Ambiguous { .. }
        ));
    }

    #[test]
    fn enclosing_law_support_over_a_shared_formula_root_selects_the_enclosing_law() {
        let mut enclosing = formula("enclosing", ConstraintStatus::Verified);
        enclosing.evidence.push(source_evidence(2, 3));
        let nested = formula("nested", ConstraintStatus::Verified);
        assert!(matches!(
            decide_meaning(input(&[nested, enclosing])),
            MeaningDecision::Established { meaning, .. }
                if meaning.relation_id.as_deref() == Some("enclosing")
        ));
    }

    #[test]
    fn truncation_prevents_an_established_decision() {
        let formula = formula("verified", ConstraintStatus::Verified);
        let mut input = input(std::slice::from_ref(&formula));
        input.truncated = true;
        assert!(matches!(
            decide_meaning(input),
            MeaningDecision::Partial { .. }
        ));
    }

    #[test]
    fn asserted_symbol_definition_is_established_without_a_formula() {
        let symbol = defined_symbol();
        let proof = symbol.definitions[0].evidence.clone();
        let mut input = input(&[]);
        input.symbol = Some(&symbol);
        input.symbol_proof = std::slice::from_ref(&proof);
        assert!(matches!(
            decide_meaning(input),
            MeaningDecision::Established { meaning, reasons }
                if meaning.label == "specific enthalpy"
                    && reasons.iter().all(|reason| reason.kind == DecisionReasonKind::Proof)
        ));
    }

    #[test]
    fn symbol_without_typed_definition_proof_remains_partial() {
        let symbol = defined_symbol();
        let mut input = input(&[]);
        input.symbol = Some(&symbol);
        assert!(matches!(
            decide_meaning(input),
            MeaningDecision::Partial { .. }
        ));
    }

    #[test]
    fn engine_limited_symbol_without_typed_proof_is_unsupported() {
        let symbol = defined_symbol();
        let mut input = input(&[]);
        input.symbol = Some(&symbol);
        input.engine_limited = true;
        assert!(matches!(
            decide_meaning(input),
            MeaningDecision::Unsupported { reasons }
                if reasons.iter().any(|reason| reason.kind == DecisionReasonKind::EngineLimit)
        ));
    }

    #[test]
    fn only_independently_supported_candidates_become_public_alternatives() {
        let supported = candidate("application", SemanticCandidateStatus::Supported);
        let unresolved = candidate("multiplication", SemanticCandidateStatus::Unresolved);
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[supported.clone(), unresolved.clone()],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(
            decision,
            MeaningDecision::Partial { meaning, .. } if meaning.label == "application"
        ));

        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[
                candidate("application", SemanticCandidateStatus::Unresolved),
                unresolved,
            ],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Unsupported { .. }));

        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[
                candidate("application", SemanticCandidateStatus::Supported),
                candidate("multiplication", SemanticCandidateStatus::Supported),
            ],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Ambiguous { .. }));

        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[candidate(
                "application",
                SemanticCandidateStatus::Conflicting,
            )],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Unsupported { .. }));
    }

    #[test]
    fn a_source_typed_formula_resolves_weaker_structural_candidates() {
        let verified = formula("verified", ConstraintStatus::Verified);
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: std::slice::from_ref(&verified),
            symbol: None,
            symbol_proof: &[],
            candidates: &[
                candidate("divergence", SemanticCandidateStatus::Unresolved),
                candidate("gradient", SemanticCandidateStatus::Unresolved),
            ],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(
            decision,
            MeaningDecision::Established { meaning, .. } if meaning.relation_id.as_deref() == Some("verified")
        ));
    }

    #[test]
    fn reports_an_opaque_notation_as_an_engine_limit_without_guessing() {
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[],
            conflicts: &[],
            engine_limited: true,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(
            decision,
            MeaningDecision::Unsupported { reasons }
                if reasons.len() == 1 && reasons[0].kind == DecisionReasonKind::EngineLimit
        ));
    }

    #[test]
    fn an_explicitly_unsupported_relation_outranks_local_symbol_or_candidate_meaning() {
        let supported = candidate("local-symbol-meaning", SemanticCandidateStatus::Supported);
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[supported],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: true,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Unsupported { .. }));
    }

    #[test]
    fn an_opaque_macro_child_does_not_hide_a_source_supported_formula() {
        let verified = formula("verified", ConstraintStatus::Verified);
        let mut input = input(std::slice::from_ref(&verified));
        input.engine_limited = true;
        assert!(matches!(
            decide_meaning(input),
            MeaningDecision::Established { meaning, .. }
                if meaning.relation_id.as_deref() == Some("verified")
        ));
    }

    #[test]
    fn uncertainty_and_source_conflict_have_separate_reason_kinds() {
        let unsupported = decide_meaning(input(&[]));
        assert!(matches!(
            unsupported,
            MeaningDecision::Unsupported { reasons }
                if reasons.iter().all(|reason| reason.kind != DecisionReasonKind::SourceConflict)
        ));

        let first = Evidence {
            rule_id: "definition/role-a".into(),
            kind: "explicit-prose".into(),
            strength: "strong".into(),
            source_ranges: vec![SourceRange {
                start_offset: 0,
                end_offset: 1,
            }],
            source_anchors: Vec::new(),
        };
        let second = Evidence {
            rule_id: "definition/role-b".into(),
            kind: "explicit-prose".into(),
            strength: "strong".into(),
            source_ranges: vec![SourceRange {
                start_offset: 3,
                end_offset: 4,
            }],
            source_anchors: Vec::new(),
        };
        let conflict = MeaningConflict {
            conflict_id: "typed-role-conflict".into(),
            label: "Incompatible role declarations".into(),
            evidence: vec![first, second],
        };
        let conflicting = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[],
            conflicts: &[conflict],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(
            conflicting,
            MeaningDecision::Conflicting { reasons, .. }
                if reasons.iter().all(|reason| reason.kind == DecisionReasonKind::SourceConflict)
        ));
    }

    #[test]
    fn typed_conflict_proof_survives_identical_public_source_ranges() {
        let evidence = Evidence {
            rule_id: "definition/role".into(),
            kind: "explicit-prose".into(),
            strength: "strong".into(),
            source_ranges: vec![SourceRange {
                start_offset: 0,
                end_offset: 10,
            }],
            source_anchors: Vec::new(),
        };
        let conflict = MeaningConflict {
            conflict_id: "typed-role-conflict".into(),
            label: "Incompatible role declarations".into(),
            evidence: vec![evidence.clone(), evidence],
        };
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            symbol_proof: &[],
            candidates: &[],
            conflicts: &[conflict],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(
            decision,
            MeaningDecision::Conflicting { conflicts, .. }
                if conflicts[0].evidence.len() == 1
        ));
    }

    #[test]
    fn weak_candidate_permutations_cannot_create_establishment_or_conflict() {
        let statuses = [
            SemanticCandidateStatus::Rejected,
            SemanticCandidateStatus::Unresolved,
            SemanticCandidateStatus::Conflicting,
        ];
        for first in statuses {
            for second in statuses {
                let candidates = [candidate("first", first), candidate("second", second)];
                for candidates in [
                    candidates.clone(),
                    [candidates[1].clone(), candidates[0].clone()],
                ] {
                    let decision = decide_meaning(MeaningDecisionInput {
                        formulas: &[],
                        symbol: None,
                        symbol_proof: &[],
                        candidates: &candidates,
                        conflicts: &[],
                        engine_limited: false,
                        unsupported_relation_context: false,
                        truncated: false,
                    });
                    assert!(matches!(decision, MeaningDecision::Unsupported { .. }));
                }
            }
        }
    }

    #[test]
    fn mutating_presentation_evidence_does_not_change_typed_proof() {
        let mut formula = formula("law", ConstraintStatus::Verified);
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Established { .. }
        ));

        formula.evidence[0].kind = "anything".into();
        formula.evidence[0].strength = "anything".into();
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Established { .. }
        ));
    }

    #[test]
    fn removing_typed_formula_proof_never_increases_public_certainty() {
        let mut formula = formula("law", ConstraintStatus::Verified);
        formula.bindings[0].evidence.source_ranges.clear();
        assert!(matches!(
            decide_meaning(input(std::slice::from_ref(&formula))),
            MeaningDecision::Partial { .. }
        ));
    }

    fn input(formulas: &[LawRecognition]) -> MeaningDecisionInput<'_> {
        MeaningDecisionInput {
            formulas,
            symbol: None,
            symbol_proof: &[],
            candidates: &[],
            conflicts: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        }
    }

    fn candidate(interpretation: &str, status: SemanticCandidateStatus) -> SemanticCandidateInfo {
        SemanticCandidateInfo {
            candidate_id: interpretation.into(),
            family: "application".into(),
            interpretation: interpretation.into(),
            status,
            range: SourceRange {
                start_offset: 0,
                end_offset: 1,
            },
            supporting_claim_ids: if status == SemanticCandidateStatus::Supported {
                vec![format!("support/{interpretation}")]
            } else {
                Vec::new()
            },
            rejecting_claim_ids: if status == SemanticCandidateStatus::Conflicting {
                vec!["reject".into()]
            } else {
                Vec::new()
            },
        }
    }

    fn defined_symbol() -> SymbolInfo {
        let range = SourceRange {
            start_offset: 2,
            end_offset: 3,
        };
        let evidence = Evidence {
            rule_id: "english-scientific-definition".into(),
            kind: "source-definition".into(),
            strength: "hard".into(),
            source_ranges: vec![range.clone()],
            source_anchors: Vec::new(),
        };
        SymbolInfo {
            symbol: "H".into(),
            occurrence_id: crate::semantic_index::SourceOccurrenceId {
                file_id: "main.tex".into(),
                document_version: 1,
                local_id: 1,
            },
            notation: vec![crate::semantic_index::NotationComponent::Identifier {
                value: "H".into(),
            }],
            source_notation: "H".into(),
            entity_id: None,
            location: crate::Location {
                file_id: "main.tex".into(),
                path: "main.tex".into(),
                range: range.clone(),
            },
            definitions: vec![crate::DefinitionInfo {
                symbol: "H".into(),
                description: "specific enthalpy".into(),
                location: crate::Location {
                    file_id: "main.tex".into(),
                    path: "main.tex".into(),
                    range,
                },
                evidence,
                entity_id: None,
            }],
            shapes: Vec::new(),
            quantities: Vec::new(),
            roles: Vec::new(),
            diagnostics: Vec::new(),
            truncated: false,
        }
    }

    fn source_evidence(start_offset: u32, end_offset: u32) -> Evidence {
        Evidence {
            rule_id: "presentation-does-not-authorize".into(),
            kind: "display-only".into(),
            strength: "display-only".into(),
            source_ranges: vec![SourceRange {
                start_offset,
                end_offset,
            }],
            source_anchors: Vec::new(),
        }
    }

    fn formula(id: &str, condition_status: ConstraintStatus) -> LawRecognition {
        let range = SourceRange {
            start_offset: 0,
            end_offset: 1,
        };
        let evidence = Evidence {
            rule_id: "test".into(),
            kind: "canonical-math".into(),
            strength: "hard".into(),
            source_ranges: vec![range.clone()],
            source_anchors: Vec::new(),
        };
        LawRecognition {
            law_id: id.into(),
            title: id.into(),
            description: id.into(),
            description_key: id.into(),
            maturity: "recognition".into(),
            status: LawRecognitionStatus::Verified,
            pack_id: "test".into(),
            pack_version: "1.0.0".into(),
            range: range.clone(),
            bindings: vec![LawBinding {
                parameter: "value".into(),
                symbol: "x".into(),
                constraint: SemanticConstraint {
                    kind: SemanticConstraintKind::Expression,
                    concepts: Vec::new(),
                    dimensions: Vec::new(),
                    refinements: Vec::new(),
                },
                proof: LawBindingProof::Typed,
                evidence: evidence.clone(),
            }],
            result: SemanticConstraint {
                kind: SemanticConstraintKind::Proposition,
                concepts: Vec::new(),
                dimensions: Vec::new(),
                refinements: Vec::new(),
            },
            conditions: vec![LawConditionInfo {
                condition_id: "condition".into(),
                kind: crate::ScientificConstraintKind::Assumption,
                subjects: vec!["x".into()],
                label: "condition".into(),
                operator_property: None,
                status: condition_status,
                evidence: vec![evidence.clone()],
            }],
            evidence: vec![evidence.clone()],
            relevance: None,
            relation: Some(RelationInfo {
                relation_id: id.into(),
                title: id.into(),
                description: id.into(),
                roles: Vec::new(),
                conditions: Vec::new(),
                evidence: vec![evidence],
                range,
            }),
            rank: 100,
            conventional_candidate: false,
            non_authoritative: false,
        }
    }
}
