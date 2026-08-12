use crate::{
    ConstraintStatus, DecisionReason, DecisionReasonKind, Evidence, LawRecognition,
    MeaningAlternative, MeaningConclusion, MeaningConflict, MeaningDecision, MeaningFact,
    MeaningRequirement, SemanticCandidateInfo, SemanticCandidateStatus, SemanticDiagnostic,
    SymbolInfo,
};

const MAX_DECISION_ITEMS: usize = 8;

pub(crate) struct MeaningDecisionInput<'a> {
    pub formulas: &'a [LawRecognition],
    pub symbol: Option<&'a SymbolInfo>,
    pub candidates: &'a [SemanticCandidateInfo],
    pub diagnostics: &'a [SemanticDiagnostic],
    pub engine_limited: bool,
    pub unsupported_relation_context: bool,
    pub truncated: bool,
}

pub(crate) fn decide_meaning(input: MeaningDecisionInput<'_>) -> MeaningDecision {
    let conflicts = collect_conflicts(&input);
    if !conflicts.is_empty() {
        let reasons = conflicts
            .iter()
            .map(|conflict| DecisionReason {
                kind: DecisionReasonKind::SourceConflict,
                label: conflict.label.clone(),
                evidence: deduplicate_evidence(conflict.evidence.clone()),
            })
            .collect();
        return MeaningDecision::Conflicting { conflicts, reasons };
    }

    let has_source_meaning = !input.formulas.is_empty()
        || input
            .candidates
            .iter()
            .any(|candidate| candidate.status == SemanticCandidateStatus::Supported)
        || input.symbol.is_some_and(symbol_has_source_meaning);
    if input.engine_limited && !has_source_meaning {
        return MeaningDecision::Unsupported {
            reasons: vec![DecisionReason {
                kind: DecisionReasonKind::EngineLimit,
                label: "The notation is opaque to the current syntax engine.".into(),
                evidence: Vec::new(),
            }],
        };
    }
    if input.unsupported_relation_context {
        return MeaningDecision::Unsupported {
            reasons: vec![uncertainty_reason(
                "No source-supported interpretation matches this relation in the active field.",
            )],
        };
    }

    let alternatives = collect_alternatives(&input);
    if alternatives.len() > 1 {
        return MeaningDecision::Ambiguous {
            alternatives,
            reasons: vec![uncertainty_reason(
                "More than one source-compatible interpretation remains.",
            )],
        };
    }

    if let Some(formula) = preferred_formulas(&input).into_iter().next() {
        let missing = missing_formula_requirements(formula);
        if missing.is_empty() && !input.truncated {
            let relation = formula
                .relation
                .as_ref()
                .expect("recognized formulas have a relation projection");
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
        return MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: formula.title.clone(),
                relation_id: formula
                    .relation
                    .as_ref()
                    .map(|relation| relation.relation_id.clone()),
            },
            facts: formula_facts(formula),
            requirements: missing,
            reasons: truncation_reason(input.truncated).into_iter().collect(),
        };
    }

    if let Some(symbol) = input.symbol {
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

    if let Some(alternative) = alternatives.into_iter().next() {
        return MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: alternative.label.clone(),
                relation_id: None,
            },
            facts: Vec::new(),
            requirements: vec![MeaningRequirement {
                requirement_id: format!("resolve/{}", alternative.alternative_id),
                label: "Independent source evidence does not yet select this interpretation."
                    .into(),
                subjects: Vec::new(),
                evidence: alternative.evidence,
            }],
            reasons: vec![uncertainty_reason(
                "A structural interpretation is available without enough independent support.",
            )],
        };
    }

    MeaningDecision::Unsupported {
        reasons: [
            Some(DecisionReason {
                kind: DecisionReasonKind::Uncertainty,
                label: "No source-supported interpretation is currently available.".into(),
                evidence: Vec::new(),
            }),
            truncation_reason(input.truncated),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

pub(crate) fn symbol_has_source_meaning(symbol: &SymbolInfo) -> bool {
    !symbol.definitions.is_empty()
        || !symbol.roles.is_empty()
        || !symbol.shapes.is_empty()
        || !symbol.quantities.is_empty()
}

fn collect_conflicts(input: &MeaningDecisionInput<'_>) -> Vec<MeaningConflict> {
    let mut conflicts = input
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity.as_str(), "error" | "warning"))
        .map(|diagnostic| MeaningConflict {
            conflict_id: diagnostic.code.clone(),
            label: diagnostic.message.clone(),
            evidence: diagnostic.evidence.clone(),
        })
        .chain(input.formulas.iter().flat_map(|formula| {
            formula
                .conditions
                .iter()
                .filter(|condition| condition.status == ConstraintStatus::Conflicting)
                .map(|condition| MeaningConflict {
                    conflict_id: format!("{}/condition/{}", formula.law_id, condition.condition_id),
                    label: condition.label.clone(),
                    evidence: condition.evidence.clone(),
                })
        }))
        .chain(
            input
                .candidates
                .iter()
                .filter(|candidate| candidate.status == SemanticCandidateStatus::Conflicting)
                .map(|candidate| MeaningConflict {
                    conflict_id: candidate.candidate_id.clone(),
                    label: candidate.interpretation.clone(),
                    evidence: vec![candidate_evidence(candidate)],
                }),
        )
        .collect::<Vec<_>>();
    conflicts.sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
    conflicts.dedup_by(|left, right| left.conflict_id == right.conflict_id);
    conflicts.truncate(MAX_DECISION_ITEMS);
    conflicts
}

fn collect_alternatives(input: &MeaningDecisionInput<'_>) -> Vec<MeaningAlternative> {
    let formulas = preferred_formulas(input);
    let supported_candidate_exists = input
        .candidates
        .iter()
        .any(|candidate| candidate.status == SemanticCandidateStatus::Supported);
    let mut alternatives = formulas
        .into_iter()
        .filter_map(|formula| {
            let relation = formula.relation.as_ref()?;
            Some(MeaningAlternative {
                alternative_id: relation.relation_id.clone(),
                label: relation.title.clone(),
                range: relation.range.clone(),
                evidence: relation.evidence.clone(),
                relevance: formula.relevance.clone(),
            })
        })
        .chain(
            input
                .candidates
                .iter()
                .filter(|_| {
                    input.formulas.is_empty()
                        && input
                            .symbol
                            .is_none_or(|symbol| symbol.definitions.is_empty())
                })
                .filter(|candidate| {
                    candidate.status == SemanticCandidateStatus::Supported
                        || (!supported_candidate_exists
                            && candidate.status == SemanticCandidateStatus::Unresolved)
                })
                .map(|candidate| MeaningAlternative {
                    alternative_id: candidate.candidate_id.clone(),
                    label: candidate.interpretation.clone(),
                    range: candidate.range.clone(),
                    evidence: vec![candidate_evidence(candidate)],
                    relevance: None,
                }),
        )
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    alternatives.retain(|alternative| seen.insert(alternative.alternative_id.clone()));
    alternatives.truncate(MAX_DECISION_ITEMS);
    alternatives
}

fn preferred_formulas<'a>(input: &'a MeaningDecisionInput<'a>) -> Vec<&'a LawRecognition> {
    let explicitly_named = input.formulas.iter().any(has_law_activation);
    input
        .formulas
        .iter()
        .filter(|formula| !explicitly_named || has_law_activation(formula))
        .collect()
}

fn has_law_activation(formula: &LawRecognition) -> bool {
    formula.evidence.iter().any(|evidence| {
        evidence.kind == "explicit-prose" && evidence.rule_id.ends_with("/activation-phrase")
    })
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
    }
}

fn missing_formula_requirements(formula: &LawRecognition) -> Vec<MeaningRequirement> {
    let mut missing = formula
        .conditions
        .iter()
        .filter(|condition| {
            matches!(
                condition.status,
                ConstraintStatus::Required | ConstraintStatus::Unsupported
            )
        })
        .map(|condition| MeaningRequirement {
            requirement_id: format!("{}/condition/{}", formula.law_id, condition.condition_id),
            label: condition.label.clone(),
            subjects: condition.subjects.clone(),
            evidence: condition.evidence.clone(),
        })
        .collect::<Vec<_>>();
    missing.truncate(MAX_DECISION_ITEMS);
    missing
}

fn formula_facts(formula: &LawRecognition) -> Vec<MeaningFact> {
    formula
        .bindings
        .iter()
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
        ConstraintStatus, Evidence, LawBinding, LawConditionInfo, LawRecognitionStatus,
        RelationInfo, SemanticConstraint, SemanticConstraintKind, SourceRange,
    };

    #[test]
    fn decision_precedence_is_conflict_then_ambiguity_then_completeness() {
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
            MeaningDecision::Ambiguous { .. }
        ));

        let mut conflicting = required;
        conflicting.conditions[0].status = ConstraintStatus::Conflicting;
        assert!(matches!(
            decide_meaning(input(&[verified, conflicting])),
            MeaningDecision::Conflicting { .. }
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
    fn independent_candidate_support_refines_ambiguity_monotonically() {
        let supported = candidate("application", SemanticCandidateStatus::Supported);
        let unresolved = candidate("multiplication", SemanticCandidateStatus::Unresolved);
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            candidates: &[supported.clone(), unresolved.clone()],
            diagnostics: &[],
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
            candidates: &[
                candidate("application", SemanticCandidateStatus::Unresolved),
                unresolved,
            ],
            diagnostics: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Ambiguous { .. }));

        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            candidates: &[candidate(
                "application",
                SemanticCandidateStatus::Conflicting,
            )],
            diagnostics: &[],
            engine_limited: false,
            unsupported_relation_context: false,
            truncated: false,
        });
        assert!(matches!(decision, MeaningDecision::Conflicting { .. }));
    }

    #[test]
    fn a_source_typed_formula_resolves_weaker_structural_candidates() {
        let verified = formula("verified", ConstraintStatus::Verified);
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: std::slice::from_ref(&verified),
            symbol: None,
            candidates: &[
                candidate("divergence", SemanticCandidateStatus::Unresolved),
                candidate("gradient", SemanticCandidateStatus::Unresolved),
            ],
            diagnostics: &[],
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
            candidates: &[],
            diagnostics: &[],
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
            candidates: &[supported],
            diagnostics: &[],
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

        let diagnostic = SemanticDiagnostic {
            code: "duplicate-role".into(),
            message: "Incompatible role declarations".into(),
            severity: "warning".into(),
            range: SourceRange {
                start_offset: 1,
                end_offset: 2,
            },
            explanation: "The same occurrence has incompatible explicit roles.".into(),
            evidence: Vec::new(),
        };
        let conflicting = decide_meaning(MeaningDecisionInput {
            formulas: &[],
            symbol: None,
            candidates: &[],
            diagnostics: &[diagnostic],
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

    fn input(formulas: &[LawRecognition]) -> MeaningDecisionInput<'_> {
        MeaningDecisionInput {
            formulas,
            symbol: None,
            candidates: &[],
            diagnostics: &[],
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
                vec!["support".into()]
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
        }
    }
}
