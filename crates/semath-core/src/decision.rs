use crate::{
    ConstraintStatus, Evidence, LawRecognition, MeaningAlternative, MeaningConflict,
    MeaningDecision, MeaningFact, MeaningRequirement, SemanticCandidateInfo,
    SemanticCandidateStatus, SemanticDiagnostic, SymbolInfo,
};

const MAX_DECISION_ITEMS: usize = 8;

pub(crate) struct MeaningDecisionInput<'a> {
    pub formulas: &'a [LawRecognition],
    pub symbol: Option<&'a SymbolInfo>,
    pub candidates: &'a [SemanticCandidateInfo],
    pub diagnostics: &'a [SemanticDiagnostic],
    pub truncated: bool,
}

pub(crate) fn decide_meaning(input: MeaningDecisionInput<'_>) -> MeaningDecision {
    let conflicts = collect_conflicts(&input);
    if !conflicts.is_empty() {
        return MeaningDecision::Conflicting {
            summary: "Conflicting semantic evidence".into(),
            conflicts,
        };
    }

    let alternatives = collect_alternatives(&input);
    if alternatives.len() > 1 {
        return MeaningDecision::Ambiguous {
            summary: "Multiple semantic interpretations remain".into(),
            alternatives,
        };
    }

    if let Some(formula) = input.formulas.first() {
        let missing = missing_formula_requirements(formula, input.truncated);
        if missing.is_empty() {
            let relation = formula
                .relation
                .as_ref()
                .expect("recognized formulas have a relation projection");
            return MeaningDecision::Established {
                summary: formula.title.clone(),
                relation_id: relation.relation_id.clone(),
                evidence: established_evidence(formula),
            };
        }
        return MeaningDecision::Partial {
            summary: formula.title.clone(),
            facts: formula_facts(formula),
            missing,
        };
    }

    if let Some(symbol) = input.symbol {
        return MeaningDecision::Partial {
            summary: symbol
                .definitions
                .first()
                .map_or_else(|| symbol.symbol.clone(), |item| item.description.clone()),
            facts: symbol_facts(symbol),
            missing: truncation_requirement(input.truncated)
                .into_iter()
                .collect(),
        };
    }

    if let Some(alternative) = alternatives.into_iter().next() {
        return MeaningDecision::Partial {
            summary: alternative.label.clone(),
            facts: Vec::new(),
            missing: vec![MeaningRequirement {
                requirement_id: format!("resolve/{}", alternative.alternative_id),
                label: "Add source-linked type or role evidence for this interpretation.".into(),
                subjects: Vec::new(),
                evidence: alternative.evidence,
            }],
        };
    }

    let mut missing = vec![MeaningRequirement {
        requirement_id: "meaning/typed-evidence".into(),
        label: "Add a declaration or supported relation that establishes typed meaning.".into(),
        subjects: Vec::new(),
        evidence: Vec::new(),
    }];
    missing.extend(truncation_requirement(input.truncated));
    MeaningDecision::Unsupported {
        summary: "No supported semantic interpretation".into(),
        missing,
    }
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
    let mut alternatives = input
        .formulas
        .iter()
        .filter_map(|formula| {
            let relation = formula.relation.as_ref()?;
            Some(MeaningAlternative {
                alternative_id: relation.relation_id.clone(),
                label: relation.title.clone(),
                range: relation.range.clone(),
                evidence: relation.evidence.clone(),
            })
        })
        .chain(
            input
                .candidates
                .iter()
                .filter(|candidate| {
                    matches!(
                        candidate.status,
                        SemanticCandidateStatus::Supported | SemanticCandidateStatus::Unresolved
                    )
                })
                .map(|candidate| MeaningAlternative {
                    alternative_id: candidate.candidate_id.clone(),
                    label: candidate.interpretation.clone(),
                    range: candidate.range.clone(),
                    evidence: vec![candidate_evidence(candidate)],
                }),
        )
        .collect::<Vec<_>>();
    alternatives.sort_by(|left, right| left.alternative_id.cmp(&right.alternative_id));
    alternatives.dedup_by(|left, right| left.alternative_id == right.alternative_id);
    alternatives.truncate(MAX_DECISION_ITEMS);
    alternatives
}

fn established_evidence(formula: &LawRecognition) -> Vec<Evidence> {
    let mut evidence = formula
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
    evidence.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    evidence.dedup();
    evidence
}

fn candidate_evidence(candidate: &SemanticCandidateInfo) -> Evidence {
    Evidence {
        rule_id: format!("semantic-candidate/{}", candidate.candidate_id),
        kind: "structural-candidate".into(),
        strength: "weak".into(),
        source_ranges: vec![candidate.range.clone()],
    }
}

fn missing_formula_requirements(
    formula: &LawRecognition,
    truncated: bool,
) -> Vec<MeaningRequirement> {
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
    missing.extend(truncation_requirement(truncated));
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

fn truncation_requirement(truncated: bool) -> Option<MeaningRequirement> {
    truncated.then(|| MeaningRequirement {
        requirement_id: "meaning/untruncated-evidence".into(),
        label: "Inspect the omitted evidence before establishing one interpretation.".into(),
        subjects: Vec::new(),
        evidence: Vec::new(),
    })
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

    fn input(formulas: &[LawRecognition]) -> MeaningDecisionInput<'_> {
        MeaningDecisionInput {
            formulas,
            symbol: None,
            candidates: &[],
            diagnostics: &[],
            truncated: false,
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
