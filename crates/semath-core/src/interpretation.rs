use crate::domain::formula_has_independent_typed_evidence;
use crate::{
    ConstraintStatus, ConventionalCandidateInfo, DomainActivation, DomainSupportTier, Evidence,
    LawBindingProof, LawRecognition, LawRecognitionStatus, Location, MathAuthoringRequirementInfo,
    MathFormulaAnchorInfo, MathInterpretationAlternativeInfo, MathInterpretationAnalysisLimitInfo,
    MathInterpretationAnalysisLimitKind, MathInterpretationCandidateCapInfo,
    MathInterpretationConditionInfo, MathInterpretationDomainRelevanceInfo,
    MathInterpretationEvidenceInfo, MathInterpretationEvidenceProvenance,
    MathInterpretationEvidenceReferenceInfo, MathInterpretationEvidenceRole,
    MathInterpretationEvidenceSourceAnchorInfo, MathInterpretationExhaustiveness,
    MathInterpretationHypothesisInfo, MathInterpretationKind, MathInterpretationOrderingReason,
    MathInterpretationOrderingReasonKind, MathInterpretationRequirementInfo,
    MathInterpretationSetInfo, MathInterpretationSupportTier, MathSourceGeneration,
    MathSourceLifecycleInfo, MeaningAlternative, MeaningDecision, SemanticCandidateInfo,
    SemanticCandidateStatus, SemanticContextInfo, SourceRange,
};
use sha2::{Digest, Sha256};

use crate::MATH_INTERPRETATION_HYPOTHESIS_LIMIT;
const MAX_INTERPRETATION_EVIDENCE: usize = 32;
pub(crate) const MAX_INTERPRETATION_DISCRIMINATORS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InterpretationEvidenceAuthority {
    Explicit,
    Derived,
    Observational,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedInterpretationEvidence {
    pub anchors: Vec<MathInterpretationEvidenceSourceAnchorInfo>,
    pub authority: InterpretationEvidenceAuthority,
}

pub(crate) fn normalize_source_anchors(
    anchors: &mut Vec<MathInterpretationEvidenceSourceAnchorInfo>,
) {
    anchors.sort_by(|left, right| {
        left.location
            .file_id
            .cmp(&right.location.file_id)
            .then(left.document_version.cmp(&right.document_version))
            .then(
                left.location
                    .range
                    .start_offset
                    .cmp(&right.location.range.start_offset),
            )
            .then(
                left.location
                    .range
                    .end_offset
                    .cmp(&right.location.range.end_offset),
            )
    });
    anchors.dedup();
}

pub(crate) struct MathInterpretationInput<'a> {
    pub decision: &'a MeaningDecision,
    pub formulas: &'a [LawRecognition],
    pub conventional_candidates: &'a [ConventionalCandidateInfo],
    pub domains: &'a [DomainActivation],
    pub structural_candidates: &'a [SemanticCandidateInfo],
    pub context: &'a SemanticContextInfo,
    pub requirements: &'a [MathAuthoringRequirementInfo],
    pub formula: Option<&'a MathFormulaAnchorInfo>,
    pub focus_range: Option<&'a SourceRange>,
    pub file_id: &'a str,
    pub path: &'a str,
    pub scope_path: &'a [u32],
    pub lifecycle: &'a MathSourceLifecycleInfo,
    pub discriminator_set_capped: bool,
    pub resolve_evidence: &'a dyn Fn(&Evidence) -> ResolvedInterpretationEvidence,
    pub refutation_evidence: &'a [Evidence],
}

pub(crate) fn project_math_interpretations(
    input: MathInterpretationInput<'_>,
) -> MathInterpretationSetInfo {
    let mut hypotheses = Vec::new();
    for formula in input.formulas {
        if let Some(relation) = &formula.relation {
            let support = formula_support(input.decision, formula);
            let bindings = projected_formula_bindings(formula, support);
            let (supporting, contradicting) =
                formula_evidence(input.decision, formula, &bindings, input.context);
            let ordering_reasons = ordering_reasons(
                &input,
                support,
                &supporting,
                formula
                    .relevance
                    .as_ref()
                    .map(|relevance| &relevance.evidence),
                false,
            );
            hypotheses.push(MathInterpretationHypothesisInfo {
                hypothesis_id: relation.relation_id.clone(),
                kind: MathInterpretationKind::TypedLaw,
                label: relation.title.clone(),
                support,
                rank: projected_formula_rank(formula, support),
                range: relation.range.clone(),
                location: location(&input, &relation.range),
                document_version: input.lifecycle.document_version,
                scope_path: input.scope_path.to_vec(),
                formula: input.formula.cloned(),
                relation: Some(relation.clone()),
                bindings,
                conditions: formula.conditions.clone(),
                evidence: graded_evidence(&input, supporting, contradicting, false),
                missing_discriminator_ids: requirements_for_prefix(
                    input.requirements,
                    &formula.law_id,
                ),
                ordering_reasons,
            });
        }
    }

    for domain in input.domains {
        hypotheses.push(domain_hypothesis(&input, domain));
    }

    if let MeaningDecision::Conflicting { conflicts, .. } = input.decision {
        hypotheses.extend(
            conflicts
                .iter()
                .filter_map(|conflict| conflict_hypothesis(&input, conflict)),
        );
    }

    for candidate in input.structural_candidates {
        hypotheses.push(structural_hypothesis(&input, candidate));
    }

    for candidate in input.conventional_candidates {
        let mut supporting = candidate.evidence.clone();
        supporting.extend(
            candidate
                .bindings
                .iter()
                .map(|binding| binding.evidence.clone()),
        );
        supporting.extend(
            candidate
                .conditions()
                .flat_map(|condition| condition.evidence.clone()),
        );
        let ordering_reasons = ordering_reasons(
            &input,
            MathInterpretationSupportTier::Tentative,
            &supporting,
            Some(&candidate.relevance.evidence),
            true,
        );
        hypotheses.push(MathInterpretationHypothesisInfo {
            hypothesis_id: candidate.candidate_id.clone(),
            kind: MathInterpretationKind::ReviewedConvention,
            label: candidate.title.clone(),
            support: MathInterpretationSupportTier::Tentative,
            rank: u32::MAX,
            range: candidate.relation.range.clone(),
            location: location(&input, &candidate.relation.range),
            document_version: input.lifecycle.document_version,
            scope_path: input.scope_path.to_vec(),
            formula: input.formula.cloned(),
            relation: Some(candidate.relation.clone()),
            bindings: candidate.bindings.clone(),
            conditions: candidate.conditions().cloned().collect(),
            evidence: graded_evidence(&input, supporting, Vec::new(), true),
            missing_discriminator_ids: candidate
                .requirements
                .iter()
                .map(conventional_requirement_id)
                .filter(|candidate_id| {
                    input
                        .requirements
                        .iter()
                        .any(|requirement| requirement_id(requirement) == candidate_id.as_str())
                })
                .collect(),
            ordering_reasons,
        });
    }

    if let MeaningDecision::Ambiguous { alternatives, .. } = input.decision {
        for alternative in alternatives {
            if !hypotheses
                .iter()
                .any(|hypothesis| hypothesis.hypothesis_id == alternative.alternative_id)
            {
                hypotheses.push(alternative_hypothesis(&input, alternative));
            }
        }
    }

    if (!input.refutation_evidence.is_empty() || hypotheses.is_empty())
        && let Some(hypothesis) = source_meaning_hypothesis(&input)
        && !hypotheses
            .iter()
            .any(|existing| existing.hypothesis_id == hypothesis.hypothesis_id)
    {
        hypotheses.push(hypothesis);
    }

    let mut keyed_hypotheses = hypotheses
        .into_iter()
        .map(|hypothesis| {
            let semantic_key = hypothesis_semantic_key(&input, &hypothesis);
            (hypothesis, semantic_key)
        })
        .collect::<Vec<_>>();
    keyed_hypotheses.sort_by(|(left, left_key), (right, right_key)| {
        support_rank(left.support)
            .cmp(&support_rank(right.support))
            .then(left.rank.cmp(&right.rank))
            .then(left.range.start_offset.cmp(&right.range.start_offset))
            .then(left.range.end_offset.cmp(&right.range.end_offset))
            .then(left_key.cmp(right_key))
    });
    keyed_hypotheses.dedup_by(|(_, left_key), (_, right_key)| left_key == right_key);
    let candidate_count_before_cap = keyed_hypotheses.len();
    let candidate_set_capped = candidate_count_before_cap > MATH_INTERPRETATION_HYPOTHESIS_LIMIT;
    let candidate_cap = candidate_set_capped.then(|| MathInterpretationCandidateCapInfo {
        candidate_count_before_cap: candidate_count_before_cap
            .try_into()
            .expect("bounded interpretation candidate count fits u32"),
        pre_cap_semantic_key_digest: pre_cap_semantic_key_digest(
            keyed_hypotheses
                .iter()
                .map(|(_, semantic_key)| semantic_key.as_str()),
        ),
    });
    keyed_hypotheses.truncate(MATH_INTERPRETATION_HYPOTHESIS_LIMIT);
    let mut hypotheses = keyed_hypotheses
        .into_iter()
        .map(|(hypothesis, _)| hypothesis)
        .collect::<Vec<_>>();
    let mut evidence_truncated = false;
    for hypothesis in &mut hypotheses {
        evidence_truncated |= hypothesis.evidence.len() > MAX_INTERPRETATION_EVIDENCE;
        hypothesis.evidence.truncate(MAX_INTERPRETATION_EVIDENCE);
    }
    for (rank, hypothesis) in hypotheses.iter_mut().enumerate() {
        hypothesis.rank = rank as u32;
    }

    let mut analysis_limits = analysis_limits(&input, candidate_set_capped, evidence_truncated);
    analysis_limits.sort_by_key(|limit| limit_rank(limit.kind));
    MathInterpretationSetInfo {
        hypotheses,
        missing_discriminators: input
            .requirements
            .iter()
            .map(|requirement| project_requirement(&input, requirement))
            .collect(),
        analysis_limits,
        candidate_cap,
        exhaustiveness: MathInterpretationExhaustiveness::BoundedOpenWorld,
        truncated: candidate_set_capped || evidence_truncated || input.discriminator_set_capped,
    }
}

fn hypothesis_semantic_key(
    input: &MathInterpretationInput<'_>,
    hypothesis: &MathInterpretationHypothesisInfo,
) -> String {
    let mut bindings = hypothesis
        .bindings
        .iter()
        .map(|binding| {
            serde_json::json!({
                "parameter": binding.parameter,
                "symbol": binding.symbol,
            })
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(serde_json::Value::to_string);

    let mut conditions = hypothesis
        .conditions
        .iter()
        .map(|condition| {
            serde_json::json!({
                "conditionId": condition.condition_id,
                "status": condition.status,
            })
        })
        .collect::<Vec<_>>();
    conditions.sort_by_key(serde_json::Value::to_string);

    let mut evidence = hypothesis
        .evidence
        .iter()
        .map(|item| {
            let mut source_anchors = item
                .source_anchors
                .iter()
                .map(|anchor| {
                    serde_json::json!({
                        "location": anchor.location,
                        "documentVersion": anchor.document_version,
                        "generation": anchor.generation,
                        "lifecycle": anchor.lifecycle,
                    })
                })
                .collect::<Vec<_>>();
            source_anchors.sort_by_key(serde_json::Value::to_string);
            serde_json::json!({
                "provenance": item.provenance,
                "role": item.role,
                "sourceAnchors": source_anchors,
            })
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(serde_json::Value::to_string);
    let source = hypothesis.formula.as_ref().map_or_else(
        || {
            serde_json::json!({
                "location": hypothesis.location,
                "documentVersion": hypothesis.document_version,
                "generation": input.lifecycle.generation,
                "lifecycle": if input.lifecycle.retracted { "retracted" } else { "current" },
            })
        },
        |formula| {
            serde_json::json!({
                "location": formula.location,
                "documentVersion": formula.document_version,
                "generation": input.lifecycle.generation,
                "lifecycle": if input.lifecycle.retracted { "retracted" } else { "current" },
            })
        },
    );
    serde_json::json!({
        "kind": hypothesis.kind,
        "label": hypothesis.label,
        "relationId": hypothesis.relation.as_ref().map(|relation| relation.relation_id.as_str()),
        "formulaSource": source,
        "support": hypothesis.support,
        "bindings": bindings,
        "conditions": conditions,
        "evidence": evidence,
    })
    .to_string()
}

fn pre_cap_semantic_key_digest<'a>(keys: impl Iterator<Item = &'a str>) -> String {
    let mut keys = keys.collect::<Vec<_>>();
    keys.sort_unstable();
    let canonical = serde_json::to_vec(&keys).expect("interpretation semantic keys serialize");
    format!("{:x}", Sha256::digest(canonical))
}

trait ConventionalCandidateConditions {
    fn conditions(&self) -> impl Iterator<Item = &crate::LawConditionInfo>;
}

impl ConventionalCandidateConditions for ConventionalCandidateInfo {
    fn conditions(&self) -> impl Iterator<Item = &crate::LawConditionInfo> {
        self.requirements
            .iter()
            .filter_map(|requirement| match requirement {
                crate::ConventionalRequirementInfo::Condition { condition, .. } => Some(condition),
                crate::ConventionalRequirementInfo::RoleDeclaration { .. } => None,
            })
    }
}

fn formula_support(
    decision: &MeaningDecision,
    formula: &LawRecognition,
) -> MathInterpretationSupportTier {
    let relation_id = formula
        .relation
        .as_ref()
        .map(|relation| relation.relation_id.as_str());
    if formula.status == LawRecognitionStatus::Conflicting
        || matches!(decision, MeaningDecision::Conflicting { .. })
    {
        return MathInterpretationSupportTier::Contradicted;
    }
    if formula.conditions.iter().any(|condition| {
        condition.kind == crate::ScientificConstraintKind::SignConvention
            && condition.status != ConstraintStatus::Verified
            && condition
                .evidence
                .iter()
                .any(|evidence| evidence.rule_id == "scientific-prose/sign-convention-unselected")
    }) {
        return MathInterpretationSupportTier::Tentative;
    }
    match decision {
        MeaningDecision::Established { meaning, .. }
            if meaning.relation_id.as_deref() == relation_id =>
        {
            if formula
                .bindings
                .iter()
                .any(|binding| binding.proof == LawBindingProof::Derived)
            {
                MathInterpretationSupportTier::Derived
            } else {
                MathInterpretationSupportTier::Explicit
            }
        }
        MeaningDecision::Partial { meaning, .. }
            if meaning.relation_id.as_deref() == relation_id =>
        {
            if formula_has_independent_support(formula) {
                MathInterpretationSupportTier::Supported
            } else {
                MathInterpretationSupportTier::Tentative
            }
        }
        MeaningDecision::Ambiguous { alternatives, .. }
            if alternatives
                .iter()
                .any(|alternative| Some(alternative.alternative_id.as_str()) == relation_id) =>
        {
            if formula_has_independent_support(formula) {
                MathInterpretationSupportTier::Supported
            } else {
                MathInterpretationSupportTier::Tentative
            }
        }
        _ if matches!(
            formula.status,
            LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
        ) && formula_has_independent_support(formula) =>
        {
            MathInterpretationSupportTier::Supported
        }
        _ => MathInterpretationSupportTier::Tentative,
    }
}

fn formula_has_independent_support(formula: &LawRecognition) -> bool {
    formula_has_independent_typed_evidence(formula)
        || formula.relevance.as_ref().is_some_and(|relevance| {
            matches!(
                relevance.support,
                DomainSupportTier::Explicit | DomainSupportTier::Supported
            )
        })
}

fn projected_formula_bindings(
    formula: &LawRecognition,
    support: MathInterpretationSupportTier,
) -> Vec<crate::LawBinding> {
    let downgrade_ungrounded =
        support == MathInterpretationSupportTier::Tentative && formula.relevance.is_none();
    formula
        .bindings
        .iter()
        .cloned()
        .map(|mut binding| {
            if downgrade_ungrounded
                && binding.proof == LawBindingProof::Derived
                && binding.evidence.kind == "derived-binding"
            {
                binding.proof = LawBindingProof::Candidate;
                binding.evidence = Evidence {
                    rule_id: format!("unresolved-law-role/{}", binding.parameter),
                    kind: "candidate-binding".into(),
                    strength: "weak".into(),
                    source_ranges: vec![formula.range.clone()],
                    source_anchors: Vec::new(),
                };
            }
            binding
        })
        .collect()
}

fn projected_formula_rank(formula: &LawRecognition, support: MathInterpretationSupportTier) -> u32 {
    let ungrounded_generic_derivation = support == MathInterpretationSupportTier::Tentative
        && formula.relevance.is_none()
        && formula.bindings.iter().any(|binding| {
            binding.proof == LawBindingProof::Derived && binding.evidence.kind == "derived-binding"
        });
    formula.rank + u32::from(ungrounded_generic_derivation)
}

fn formula_evidence(
    decision: &MeaningDecision,
    formula: &LawRecognition,
    bindings: &[crate::LawBinding],
    context: &SemanticContextInfo,
) -> (Vec<Evidence>, Vec<Evidence>) {
    let mut supporting = formula.evidence.clone();
    supporting.extend(bindings.iter().map(|binding| binding.evidence.clone()));
    supporting.extend(
        formula
            .conditions
            .iter()
            .filter(|condition| condition.status == ConstraintStatus::Verified)
            .flat_map(|condition| condition.evidence.clone()),
    );
    let supporting_ranges = supporting
        .iter()
        .flat_map(|evidence| evidence.source_ranges.iter())
        .cloned()
        .collect::<Vec<_>>();
    supporting.extend(
        context
            .claims
            .iter()
            .flat_map(|claim| claim.evidence.iter())
            .filter(|evidence| {
                evidence.source_ranges.iter().any(|candidate| {
                    supporting_ranges
                        .iter()
                        .any(|range| ranges_overlap(range, candidate))
                })
            })
            .cloned(),
    );
    let mut contradicting = formula
        .conditions
        .iter()
        .filter(|condition| condition.status == ConstraintStatus::Conflicting)
        .flat_map(|condition| {
            condition.evidence.iter().filter(|evidence| {
                condition.kind != crate::ScientificConstraintKind::SignConvention
                    || (evidence.rule_id == "english-scientific-assumption"
                        && matches!(evidence.kind.as_str(), "explicit-prose" | "attached-prose"))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if let MeaningDecision::Conflicting { conflicts, .. } = decision {
        contradicting.extend(
            conflicts
                .iter()
                .flat_map(|conflict| conflict.evidence.clone()),
        );
    }
    contradicting.retain(|evidence| !supporting.contains(evidence));
    (supporting, contradicting)
}

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

fn structural_hypothesis(
    input: &MathInterpretationInput<'_>,
    candidate: &SemanticCandidateInfo,
) -> MathInterpretationHypothesisInfo {
    let mut supporting = claim_evidence(input.context, &candidate.supporting_claim_ids);
    if supporting.is_empty() {
        supporting.push(Evidence {
            rule_id: "semath/authoring/structural-alternative".into(),
            kind: "source-structure".into(),
            strength: "contextual".into(),
            source_ranges: vec![candidate.range.clone()],
            source_anchors: Vec::new(),
        });
    }
    let contradicting = claim_evidence(input.context, &candidate.rejecting_claim_ids);
    let support = match candidate.status {
        SemanticCandidateStatus::Supported => MathInterpretationSupportTier::Supported,
        SemanticCandidateStatus::Unresolved => MathInterpretationSupportTier::Tentative,
        SemanticCandidateStatus::Conflicting | SemanticCandidateStatus::Rejected => {
            MathInterpretationSupportTier::Contradicted
        }
    };
    let ordering_reasons = ordering_reasons(input, support, &supporting, None, false);
    MathInterpretationHypothesisInfo {
        hypothesis_id: candidate.candidate_id.clone(),
        kind: MathInterpretationKind::StructuralAlternative,
        label: candidate.interpretation.clone(),
        support,
        rank: u32::MAX,
        range: candidate.range.clone(),
        location: location(input, &candidate.range),
        document_version: input.lifecycle.document_version,
        scope_path: input.scope_path.to_vec(),
        formula: None,
        relation: None,
        bindings: Vec::new(),
        conditions: Vec::new(),
        evidence: graded_evidence(input, supporting, contradicting, false),
        missing_discriminator_ids: disambiguation_ids_for_alternative(
            input.requirements,
            &candidate.candidate_id,
        ),
        ordering_reasons,
    }
}

fn domain_hypothesis(
    input: &MathInterpretationInput<'_>,
    domain: &DomainActivation,
) -> MathInterpretationHypothesisInfo {
    let support = match domain.support {
        DomainSupportTier::Explicit | DomainSupportTier::Supported => {
            MathInterpretationSupportTier::Supported
        }
        DomainSupportTier::Tentative => MathInterpretationSupportTier::Tentative,
    };
    MathInterpretationHypothesisInfo {
        hypothesis_id: format!("domain/{}", domain.pack_id),
        kind: MathInterpretationKind::ScopedDomain,
        label: domain.title.clone(),
        support,
        rank: u32::MAX,
        range: domain.scope_range.clone(),
        location: location(input, &domain.scope_range),
        document_version: input.lifecycle.document_version,
        scope_path: input.scope_path.to_vec(),
        formula: None,
        relation: None,
        bindings: Vec::new(),
        conditions: Vec::new(),
        evidence: domain
            .evidence
            .iter()
            .cloned()
            .map(|evidence| MathInterpretationEvidenceInfo {
                role: MathInterpretationEvidenceRole::Supporting,
                provenance: MathInterpretationEvidenceProvenance::DomainContext,
                source_anchors: (input.resolve_evidence)(&evidence).anchors,
                evidence,
            })
            .collect(),
        missing_discriminator_ids: disambiguation_ids(input.requirements),
        ordering_reasons: vec![
            MathInterpretationOrderingReason {
                kind: MathInterpretationOrderingReasonKind::DomainRelevance,
                evidence: evidence_references(input, domain.evidence.iter().take(1)),
            },
            MathInterpretationOrderingReason {
                kind: MathInterpretationOrderingReasonKind::StableSourceOrder,
                evidence: Vec::new(),
            },
        ],
    }
}

fn conflict_hypothesis(
    input: &MathInterpretationInput<'_>,
    conflict: &crate::MeaningConflict,
) -> Option<MathInterpretationHypothesisInfo> {
    let range = conflict
        .evidence
        .iter()
        .flat_map(|evidence| evidence.source_ranges.iter())
        .next()
        .cloned()
        .or_else(|| input.focus_range.cloned())?;
    Some(MathInterpretationHypothesisInfo {
        hypothesis_id: conflict.conflict_id.clone(),
        kind: MathInterpretationKind::SourceMeaning,
        label: conflict.label.clone(),
        support: MathInterpretationSupportTier::Contradicted,
        rank: u32::MAX,
        location: location(input, &range),
        document_version: input.lifecycle.document_version,
        scope_path: input.scope_path.to_vec(),
        range,
        formula: None,
        relation: None,
        bindings: Vec::new(),
        conditions: Vec::new(),
        evidence: graded_evidence(input, Vec::new(), conflict.evidence.clone(), false),
        missing_discriminator_ids: input.requirements.iter().map(requirement_id).collect(),
        ordering_reasons: ordering_reasons(
            input,
            MathInterpretationSupportTier::Contradicted,
            &conflict.evidence,
            None,
            false,
        ),
    })
}

fn alternative_hypothesis(
    input: &MathInterpretationInput<'_>,
    alternative: &MeaningAlternative,
) -> MathInterpretationHypothesisInfo {
    let support = match alternative
        .relevance
        .as_ref()
        .map(|relevance| relevance.support)
    {
        Some(DomainSupportTier::Tentative) => MathInterpretationSupportTier::Tentative,
        _ => MathInterpretationSupportTier::Supported,
    };
    let ordering_reasons = ordering_reasons(
        input,
        support,
        &alternative.evidence,
        alternative
            .relevance
            .as_ref()
            .map(|relevance| &relevance.evidence),
        false,
    );
    MathInterpretationHypothesisInfo {
        hypothesis_id: alternative.alternative_id.clone(),
        kind: MathInterpretationKind::StructuralAlternative,
        label: alternative.label.clone(),
        support,
        rank: u32::MAX,
        range: alternative.range.clone(),
        location: location(input, &alternative.range),
        document_version: input.lifecycle.document_version,
        scope_path: input.scope_path.to_vec(),
        formula: None,
        relation: None,
        bindings: Vec::new(),
        conditions: Vec::new(),
        evidence: graded_evidence(input, alternative.evidence.clone(), Vec::new(), false),
        missing_discriminator_ids: disambiguation_ids_for_alternative(
            input.requirements,
            &alternative.alternative_id,
        ),
        ordering_reasons,
    }
}

fn source_meaning_hypothesis(
    input: &MathInterpretationInput<'_>,
) -> Option<MathInterpretationHypothesisInfo> {
    let (label, support, supporting, contradicting) = match input.decision {
        MeaningDecision::Established {
            meaning, reasons, ..
        } => (
            meaning.label.clone(),
            MathInterpretationSupportTier::Explicit,
            reasons
                .iter()
                .flat_map(|reason| reason.evidence.clone())
                .collect(),
            Vec::new(),
        ),
        MeaningDecision::Partial {
            meaning,
            facts,
            reasons,
            ..
        } => {
            let mut evidence = facts
                .iter()
                .flat_map(|fact| fact.evidence.clone())
                .collect::<Vec<_>>();
            evidence.extend(reasons.iter().flat_map(|reason| reason.evidence.clone()));
            (
                meaning.label.clone(),
                MathInterpretationSupportTier::Tentative,
                evidence,
                Vec::new(),
            )
        }
        MeaningDecision::Unsupported { .. } if !input.refutation_evidence.is_empty() => (
            "Rejected source meaning".into(),
            MathInterpretationSupportTier::Contradicted,
            Vec::new(),
            input.refutation_evidence.to_vec(),
        ),
        _ => return None,
    };
    let range = input
        .formula
        .map(|formula| formula.location.range.clone())
        .or_else(|| input.focus_range.cloned())?;
    let projected_evidence =
        graded_evidence(input, supporting.clone(), contradicting.clone(), false);
    let support = if support == MathInterpretationSupportTier::Contradicted {
        support
    } else {
        source_meaning_support(
            input,
            support == MathInterpretationSupportTier::Explicit,
            &projected_evidence,
        )
    };
    let mut ordering_evidence = supporting;
    ordering_evidence.extend(contradicting);
    let ordering_reasons = ordering_reasons(input, support, &ordering_evidence, None, false);
    Some(MathInterpretationHypothesisInfo {
        hypothesis_id: "source-meaning".into(),
        kind: MathInterpretationKind::SourceMeaning,
        label,
        support,
        rank: 0,
        location: location(input, &range),
        document_version: input.lifecycle.document_version,
        scope_path: input.scope_path.to_vec(),
        range,
        formula: input.formula.cloned(),
        relation: None,
        bindings: Vec::new(),
        conditions: Vec::new(),
        evidence: projected_evidence,
        missing_discriminator_ids: input.requirements.iter().map(requirement_id).collect(),
        ordering_reasons,
    })
}

fn claim_evidence(context: &SemanticContextInfo, ids: &[String]) -> Vec<Evidence> {
    ids.iter()
        .filter_map(|id| context.claims.iter().find(|claim| &claim.claim_id == id))
        .flat_map(|claim| claim.evidence.clone())
        .collect()
}

fn location(input: &MathInterpretationInput<'_>, range: &SourceRange) -> Location {
    Location {
        file_id: input.file_id.into(),
        path: input.path.into(),
        range: range.clone(),
    }
}

fn graded_evidence(
    input: &MathInterpretationInput<'_>,
    supporting: Vec<Evidence>,
    contradicting: Vec<Evidence>,
    convention: bool,
) -> Vec<MathInterpretationEvidenceInfo> {
    let mut projected = supporting
        .into_iter()
        .map(|evidence| (MathInterpretationEvidenceRole::Supporting, evidence))
        .chain(
            contradicting
                .into_iter()
                .map(|evidence| (MathInterpretationEvidenceRole::Contradicting, evidence)),
        )
        .map(|(role, evidence)| {
            let provenance = evidence_provenance(&evidence, convention);
            let source_anchors = (input.resolve_evidence)(&evidence).anchors;
            let mut evidence = evidence;
            evidence.source_ranges = source_anchors
                .iter()
                .map(|anchor| anchor.location.range.clone())
                .collect();
            evidence.source_anchors = source_anchors.clone();
            MathInterpretationEvidenceInfo {
                role,
                provenance,
                evidence,
                source_anchors,
            }
        })
        .collect::<Vec<_>>();
    let mut seen = Vec::new();
    projected.retain(|item| {
        let key = (item.role, item.evidence.clone());
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    projected
}

fn evidence_reference(
    input: &MathInterpretationInput<'_>,
    evidence: &Evidence,
) -> MathInterpretationEvidenceReferenceInfo {
    let source_anchors = (input.resolve_evidence)(evidence).anchors;
    let mut evidence = evidence.clone();
    evidence.source_ranges = source_anchors
        .iter()
        .map(|anchor| anchor.location.range.clone())
        .collect();
    evidence.source_anchors = source_anchors.clone();
    MathInterpretationEvidenceReferenceInfo {
        evidence,
        source_anchors,
    }
}

fn evidence_references<'a>(
    input: &MathInterpretationInput<'_>,
    evidence: impl IntoIterator<Item = &'a Evidence>,
) -> Vec<MathInterpretationEvidenceReferenceInfo> {
    evidence
        .into_iter()
        .map(|evidence| evidence_reference(input, evidence))
        .collect()
}

fn project_condition(
    input: &MathInterpretationInput<'_>,
    condition: &crate::LawConditionInfo,
) -> MathInterpretationConditionInfo {
    MathInterpretationConditionInfo {
        condition_id: condition.condition_id.clone(),
        kind: condition.kind,
        subjects: condition.subjects.clone(),
        label: condition.label.clone(),
        operator_property: condition.operator_property,
        status: condition.status,
        evidence: evidence_references(input, &condition.evidence),
    }
}

fn project_alternative(
    input: &MathInterpretationInput<'_>,
    alternative: &MeaningAlternative,
) -> MathInterpretationAlternativeInfo {
    MathInterpretationAlternativeInfo {
        alternative_id: alternative.alternative_id.clone(),
        label: alternative.label.clone(),
        range: alternative.range.clone(),
        evidence: evidence_references(input, &alternative.evidence),
        relevance: alternative.relevance.as_ref().map(|relevance| {
            MathInterpretationDomainRelevanceInfo {
                support: relevance.support,
                evidence: evidence_references(input, &relevance.evidence),
            }
        }),
    }
}

fn project_requirement(
    input: &MathInterpretationInput<'_>,
    requirement: &MathAuthoringRequirementInfo,
) -> MathInterpretationRequirementInfo {
    match requirement {
        MathAuthoringRequirementInfo::Declaration {
            requirement_id,
            symbol,
            occurrence_id,
            evidence,
        } => MathInterpretationRequirementInfo::Declaration {
            requirement_id: requirement_id.clone(),
            symbol: symbol.clone(),
            occurrence_id: occurrence_id.clone(),
            evidence: evidence_references(input, evidence),
        },
        MathAuthoringRequirementInfo::RoleDeclaration {
            requirement_id,
            parameter,
            symbol,
            constraint,
            evidence,
        } => MathInterpretationRequirementInfo::RoleDeclaration {
            requirement_id: requirement_id.clone(),
            parameter: parameter.clone(),
            symbol: symbol.clone(),
            constraint: constraint.clone(),
            evidence: evidence_references(input, evidence),
        },
        MathAuthoringRequirementInfo::Condition {
            requirement_id,
            condition,
        } => MathInterpretationRequirementInfo::Condition {
            requirement_id: requirement_id.clone(),
            condition: project_condition(input, condition),
        },
        MathAuthoringRequirementInfo::Disambiguation {
            requirement_id,
            alternatives,
            evidence,
        } => MathInterpretationRequirementInfo::Disambiguation {
            requirement_id: requirement_id.clone(),
            alternatives: alternatives
                .iter()
                .map(|alternative| project_alternative(input, alternative))
                .collect(),
            evidence: evidence_references(input, evidence),
        },
    }
}

fn source_meaning_support(
    input: &MathInterpretationInput<'_>,
    established: bool,
    evidence: &[MathInterpretationEvidenceInfo],
) -> MathInterpretationSupportTier {
    if !established {
        return if evidence.is_empty() {
            MathInterpretationSupportTier::Tentative
        } else {
            MathInterpretationSupportTier::Supported
        };
    }
    let mut derived = false;
    for item in evidence
        .iter()
        .filter(|item| item.role == MathInterpretationEvidenceRole::Supporting)
    {
        if item.provenance == MathInterpretationEvidenceProvenance::NaturalLanguageExtraction {
            continue;
        }
        match (input.resolve_evidence)(&item.evidence).authority {
            InterpretationEvidenceAuthority::Explicit => {
                return MathInterpretationSupportTier::Explicit;
            }
            InterpretationEvidenceAuthority::Derived => derived = true,
            InterpretationEvidenceAuthority::Observational => {}
        }
    }
    if derived {
        MathInterpretationSupportTier::Derived
    } else {
        MathInterpretationSupportTier::Supported
    }
}

fn evidence_provenance(
    evidence: &Evidence,
    convention: bool,
) -> MathInterpretationEvidenceProvenance {
    let kind = evidence.kind.as_str();
    if kind.contains("domain") || matches!(kind, "document" | "document-field" | "section") {
        MathInterpretationEvidenceProvenance::DomainContext
    } else if convention {
        MathInterpretationEvidenceProvenance::ReviewedConvention
    } else if kind.contains("prose")
        || kind == "attached-prose"
        || evidence.rule_id.starts_with("english-")
        || evidence.rule_id.starts_with("scientific-prose/")
    {
        MathInterpretationEvidenceProvenance::NaturalLanguageExtraction
    } else if kind.contains("derived")
        || kind.contains("law-chain")
        || evidence.rule_id.starts_with("canonical-propagation/")
    {
        MathInterpretationEvidenceProvenance::DerivedEvidence
    } else if matches!(
        kind,
        "definition" | "explicit-math" | "source-claim" | "source-definition" | "source-relation"
    ) {
        MathInterpretationEvidenceProvenance::ExplicitDeclaration
    } else {
        MathInterpretationEvidenceProvenance::TypedStructure
    }
}

fn ordering_reasons(
    input: &MathInterpretationInput<'_>,
    support: MathInterpretationSupportTier,
    primary_evidence: &[Evidence],
    relevance: Option<&Vec<Evidence>>,
    convention: bool,
) -> Vec<MathInterpretationOrderingReason> {
    let mut reasons = Vec::new();
    if primary_evidence.is_empty() {
        // The stable source range still makes the candidate inspectable, but it
        // must not be described as evidence-backed.
    } else if convention {
        reasons.push(MathInterpretationOrderingReason {
            kind: MathInterpretationOrderingReasonKind::ReviewedConvention,
            evidence: evidence_references(input, primary_evidence.iter().take(1)),
        });
    } else {
        reasons.push(MathInterpretationOrderingReason {
            kind: match support {
                MathInterpretationSupportTier::Explicit => {
                    MathInterpretationOrderingReasonKind::ExplicitEvidence
                }
                MathInterpretationSupportTier::Derived => {
                    MathInterpretationOrderingReasonKind::DerivedEvidence
                }
                _ => MathInterpretationOrderingReasonKind::TypedEvidence,
            },
            evidence: evidence_references(input, primary_evidence.iter().take(1)),
        });
    }
    if let Some(evidence) = relevance.filter(|evidence| !evidence.is_empty()) {
        reasons.push(MathInterpretationOrderingReason {
            kind: MathInterpretationOrderingReasonKind::DomainRelevance,
            evidence: evidence_references(input, evidence.iter().take(1)),
        });
    }
    reasons.push(MathInterpretationOrderingReason {
        kind: MathInterpretationOrderingReasonKind::StableSourceOrder,
        evidence: Vec::new(),
    });
    reasons
}

fn requirements_for_prefix(
    requirements: &[MathAuthoringRequirementInfo],
    prefix: &str,
) -> Vec<String> {
    requirements
        .iter()
        .map(requirement_id)
        .filter(|id| id.starts_with(prefix))
        .collect()
}

fn disambiguation_ids(requirements: &[MathAuthoringRequirementInfo]) -> Vec<String> {
    requirements
        .iter()
        .filter_map(|requirement| match requirement {
            MathAuthoringRequirementInfo::Disambiguation { requirement_id, .. } => {
                Some(requirement_id.clone())
            }
            _ => None,
        })
        .collect()
}

fn disambiguation_ids_for_alternative(
    requirements: &[MathAuthoringRequirementInfo],
    alternative_id: &str,
) -> Vec<String> {
    requirements
        .iter()
        .filter_map(|requirement| match requirement {
            MathAuthoringRequirementInfo::Disambiguation {
                requirement_id,
                alternatives,
                ..
            } if alternatives
                .iter()
                .any(|alternative| alternative.alternative_id == alternative_id) =>
            {
                Some(requirement_id.clone())
            }
            _ => None,
        })
        .collect()
}

fn requirement_id(requirement: &MathAuthoringRequirementInfo) -> String {
    match requirement {
        MathAuthoringRequirementInfo::Declaration { requirement_id, .. }
        | MathAuthoringRequirementInfo::RoleDeclaration { requirement_id, .. }
        | MathAuthoringRequirementInfo::Condition { requirement_id, .. }
        | MathAuthoringRequirementInfo::Disambiguation { requirement_id, .. } => {
            requirement_id.clone()
        }
    }
}

fn conventional_requirement_id(requirement: &crate::ConventionalRequirementInfo) -> String {
    match requirement {
        crate::ConventionalRequirementInfo::RoleDeclaration { requirement_id, .. }
        | crate::ConventionalRequirementInfo::Condition { requirement_id, .. } => {
            requirement_id.clone()
        }
    }
}

fn analysis_limits(
    input: &MathInterpretationInput<'_>,
    candidate_set_capped: bool,
    evidence_truncated: bool,
) -> Vec<MathInterpretationAnalysisLimitInfo> {
    let anchor_evidence = input.formula.map_or_else(Vec::new, |formula| {
        vec![Evidence {
            rule_id: "semath/authoring/analysis-limit".into(),
            kind: "source-occurrence".into(),
            strength: "contextual".into(),
            source_ranges: vec![formula.location.range.clone()],
            source_anchors: Vec::new(),
        }]
    });
    let mut limits = Vec::new();
    let mut push = |kind| {
        limits.push(MathInterpretationAnalysisLimitInfo {
            kind,
            evidence: evidence_references(input, &anchor_evidence),
        });
    };
    if candidate_set_capped {
        push(MathInterpretationAnalysisLimitKind::CandidateSetCapped);
    }
    if evidence_truncated {
        push(MathInterpretationAnalysisLimitKind::EvidenceTruncated);
    }
    if input.discriminator_set_capped {
        push(MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped);
    }
    if input.lifecycle.engine_limited {
        push(MathInterpretationAnalysisLimitKind::EngineLimit);
    }
    if input.lifecycle.generation == MathSourceGeneration::Generated {
        push(MathInterpretationAnalysisLimitKind::GeneratedSource);
    }
    if input.lifecycle.retracted {
        push(MathInterpretationAnalysisLimitKind::RetractedSource);
    }
    limits
}

const fn support_rank(support: MathInterpretationSupportTier) -> u8 {
    match support {
        MathInterpretationSupportTier::Explicit => 0,
        MathInterpretationSupportTier::Derived => 1,
        MathInterpretationSupportTier::Supported => 2,
        MathInterpretationSupportTier::Tentative => 3,
        MathInterpretationSupportTier::Contradicted => 4,
    }
}

const fn limit_rank(kind: MathInterpretationAnalysisLimitKind) -> u8 {
    match kind {
        MathInterpretationAnalysisLimitKind::CandidateSetCapped => 0,
        MathInterpretationAnalysisLimitKind::EvidenceTruncated => 1,
        MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped => 2,
        MathInterpretationAnalysisLimitKind::EngineLimit => 3,
        MathInterpretationAnalysisLimitKind::GeneratedSource => 4,
        MathInterpretationAnalysisLimitKind::RetractedSource => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConventionalCandidateDisposition, ConventionalRequirementInfo, DecisionReason,
        DecisionReasonKind, DomainRelevance, LawBinding, LawConditionInfo, MathSourceFreshness,
        MeaningConclusion, RelationInfo, ScientificConstraintKind, SemanticClaimInfo,
        SemanticClaimStatus, SemanticConstraint, SemanticConstraintKind,
    };

    fn range(start_offset: u32, end_offset: u32) -> SourceRange {
        SourceRange {
            start_offset,
            end_offset,
        }
    }

    fn evidence(kind: &str, rule_id: &str, start_offset: u32) -> Evidence {
        Evidence {
            kind: kind.into(),
            rule_id: rule_id.into(),
            source_ranges: vec![range(start_offset, start_offset + 1)],
            strength: "strong".into(),
            source_anchors: Vec::new(),
        }
    }

    fn constraint() -> SemanticConstraint {
        SemanticConstraint {
            kind: SemanticConstraintKind::Scalar,
            concepts: Vec::new(),
            dimensions: Vec::new(),
            refinements: Vec::new(),
        }
    }

    fn relation(id: &str, title: &str) -> RelationInfo {
        RelationInfo {
            relation_id: id.into(),
            title: title.into(),
            description: title.into(),
            roles: Vec::new(),
            conditions: Vec::new(),
            evidence: vec![evidence("source-relation", "test/relation", 10)],
            range: range(10, 20),
        }
    }

    fn formula() -> LawRecognition {
        LawRecognition {
            law_id: "law".into(),
            title: "Typed law".into(),
            description: "A reviewed typed law.".into(),
            description_key: "typed-law".into(),
            maturity: "recognition".into(),
            status: LawRecognitionStatus::Verified,
            pack_id: "test-pack".into(),
            pack_version: "1.0.0".into(),
            range: range(10, 20),
            bindings: vec![LawBinding {
                parameter: "value".into(),
                symbol: "x".into(),
                constraint: constraint(),
                proof: LawBindingProof::Typed,
                evidence: evidence("structural-declaration", "test/typed", 1),
            }],
            result: constraint(),
            conditions: vec![LawConditionInfo {
                condition_id: "same-context".into(),
                kind: ScientificConstraintKind::SameContext,
                subjects: vec!["x".into()],
                label: "The values share a context.".into(),
                operator_property: None,
                status: ConstraintStatus::Verified,
                evidence: vec![evidence("explicit-prose", "english-test", 3)],
            }],
            evidence: vec![evidence("source-relation", "test/formula", 10)],
            relevance: Some(DomainRelevance {
                support: DomainSupportTier::Supported,
                evidence: vec![evidence("section", "test/domain", 0)],
            }),
            relation: Some(relation("test-pack:law", "Typed law")),
            rank: 2,
            conventional_candidate: false,
            non_authoritative: false,
        }
    }

    fn context(
        candidates: Vec<SemanticCandidateInfo>,
        claims: Vec<SemanticClaimInfo>,
    ) -> SemanticContextInfo {
        SemanticContextInfo {
            symbol: None,
            entity_id: None,
            concepts: Vec::new(),
            assumptions: Vec::new(),
            claims,
            candidates,
            relations: Vec::new(),
            quantities: Vec::new(),
            truncated: false,
        }
    }

    fn lifecycle() -> MathSourceLifecycleInfo {
        MathSourceLifecycleInfo {
            document_version: 1,
            generation: MathSourceGeneration::Authored,
            freshness: MathSourceFreshness::Current,
            editable: true,
            retracted: false,
            capped: false,
            engine_limited: false,
        }
    }

    fn resolve_test_evidence(evidence: &Evidence) -> ResolvedInterpretationEvidence {
        ResolvedInterpretationEvidence {
            anchors: if evidence.source_anchors.is_empty() {
                evidence
                    .source_ranges
                    .iter()
                    .cloned()
                    .map(|range| MathInterpretationEvidenceSourceAnchorInfo {
                        location: Location {
                            file_id: "main".into(),
                            path: "main.tex".into(),
                            range,
                        },
                        document_version: 1,
                        scope_path: vec![0, 1],
                        lifecycle: crate::MathInterpretationSourceLifecycle::Current,
                        generation: MathSourceGeneration::Authored,
                    })
                    .collect()
            } else {
                evidence.source_anchors.clone()
            },
            authority: if evidence.kind.contains("derived") {
                InterpretationEvidenceAuthority::Derived
            } else if evidence_provenance(evidence, false)
                == MathInterpretationEvidenceProvenance::ExplicitDeclaration
            {
                InterpretationEvidenceAuthority::Explicit
            } else {
                InterpretationEvidenceAuthority::Observational
            },
        }
    }

    #[test]
    fn projects_typed_and_natural_language_evidence_without_collapsing_provenance() {
        let formula = formula();
        let decision = MeaningDecision::Established {
            meaning: MeaningConclusion {
                label: "Typed law".into(),
                relation_id: Some("test-pack:law".into()),
            },
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[formula],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[0, 1],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        assert_eq!(
            projected.exhaustiveness,
            MathInterpretationExhaustiveness::BoundedOpenWorld
        );
        assert!(!projected.truncated);
        assert_eq!(projected.hypotheses.len(), 1);
        let hypothesis = &projected.hypotheses[0];
        assert_eq!(hypothesis.support, MathInterpretationSupportTier::Explicit);
        assert_eq!(hypothesis.location.file_id, "main");
        assert_eq!(hypothesis.document_version, 1);
        assert_eq!(hypothesis.scope_path, [0, 1]);
        assert!(hypothesis.evidence.iter().any(|item| {
            item.provenance == MathInterpretationEvidenceProvenance::TypedStructure
        }));
        assert!(hypothesis.evidence.iter().any(|item| {
            item.provenance == MathInterpretationEvidenceProvenance::NaturalLanguageExtraction
        }));
        assert!(hypothesis.ordering_reasons.iter().any(|reason| {
            reason.kind == MathInterpretationOrderingReasonKind::DomainRelevance
                && !reason.evidence.is_empty()
        }));
    }

    #[test]
    fn secondary_interpretation_evidence_surfaces_serialize_exact_cross_document_anchors() {
        let external_anchor = MathInterpretationEvidenceSourceAnchorInfo {
            location: Location {
                file_id: "roles".into(),
                path: "roles.tex".into(),
                range: range(50, 60),
            },
            document_version: 3,
            scope_path: vec![0, 2],
            lifecycle: crate::MathInterpretationSourceLifecycle::Current,
            generation: MathSourceGeneration::Authored,
        };
        let external_evidence = Evidence {
            kind: "explicit-prose".into(),
            rule_id: "english-respectively-definition".into(),
            source_ranges: vec![range(50, 60)],
            strength: "strong".into(),
            source_anchors: vec![external_anchor],
        };
        let mut formula = formula();
        formula.evidence = vec![external_evidence.clone()];
        formula.relevance = Some(DomainRelevance {
            support: DomainSupportTier::Supported,
            evidence: vec![external_evidence.clone()],
        });
        let condition = LawConditionInfo {
            condition_id: "external-condition".into(),
            kind: ScientificConstraintKind::SameContext,
            subjects: vec!["x".into()],
            label: "External condition".into(),
            operator_property: None,
            status: ConstraintStatus::Required,
            evidence: vec![external_evidence.clone()],
        };
        let requirements = [
            MathAuthoringRequirementInfo::Condition {
                requirement_id: "law/condition/external".into(),
                condition,
            },
            MathAuthoringRequirementInfo::Disambiguation {
                requirement_id: "meaning/disambiguation".into(),
                alternatives: vec![MeaningAlternative {
                    alternative_id: "alternative/external".into(),
                    label: "External alternative".into(),
                    range: range(10, 20),
                    evidence: vec![external_evidence.clone()],
                    relevance: Some(DomainRelevance {
                        support: DomainSupportTier::Tentative,
                        evidence: vec![external_evidence.clone()],
                    }),
                }],
                evidence: vec![external_evidence],
            },
        ];
        let decision = MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: "Typed law".into(),
                relation_id: Some("test-pack:law".into()),
            },
            facts: Vec::new(),
            requirements: Vec::new(),
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = MathSourceLifecycleInfo {
            engine_limited: true,
            ..lifecycle()
        };
        let formula_anchor = MathFormulaAnchorInfo {
            location: Location {
                file_id: "main".into(),
                path: "main.tex".into(),
                range: range(10, 20),
            },
            document_version: 1,
            scope_path: Vec::new(),
            source_notation: "x = y".into(),
            provenance: Vec::new(),
        };
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[formula],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &requirements,
            formula: Some(&formula_anchor),
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });
        let wire = serde_json::to_value(projected).unwrap();
        let requirements = wire["missingDiscriminators"].as_array().unwrap();
        let condition = requirements
            .iter()
            .find(|item| item["kind"] == "condition")
            .unwrap();
        assert_eq!(
            condition["condition"]["evidence"][0]["sourceAnchors"][0]["location"]["fileId"],
            "roles"
        );
        let disambiguation = requirements
            .iter()
            .find(|item| item["kind"] == "disambiguation")
            .unwrap();
        assert_eq!(
            disambiguation["evidence"][0]["sourceAnchors"][0]["location"]["fileId"],
            "roles"
        );
        assert_eq!(
            disambiguation["alternatives"][0]["evidence"][0]["sourceAnchors"][0]["location"]["fileId"],
            "roles"
        );
        assert_eq!(
            disambiguation["alternatives"][0]["relevance"]["evidence"][0]["sourceAnchors"][0]["location"]
                ["fileId"],
            "roles"
        );
        assert_eq!(
            wire["hypotheses"][0]["orderingReasons"][0]["evidence"][0]["sourceAnchors"][0]["location"]
                ["fileId"],
            "roles"
        );
        assert_eq!(
            wire["analysisLimits"][0]["evidence"][0]["sourceAnchors"][0]["location"]["fileId"],
            "main"
        );
    }

    #[test]
    fn unscoped_derived_partial_collision_stays_tentative_with_candidate_roles() {
        let mut formula = formula();
        formula.status = LawRecognitionStatus::ConditionMissing;
        formula.relevance = None;
        formula.bindings[0].proof = LawBindingProof::Derived;
        formula.bindings[0].evidence = evidence("derived-binding", "derived-law-role/value", 12);
        formula.conditions[0].status = ConstraintStatus::Required;
        let decision = MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: "Typed law".into(),
                relation_id: Some("test-pack:law".into()),
            },
            facts: Vec::new(),
            requirements: Vec::new(),
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[formula],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        let hypothesis = &projected.hypotheses[0];
        assert_eq!(hypothesis.support, MathInterpretationSupportTier::Tentative);
        assert_eq!(hypothesis.bindings[0].proof, LawBindingProof::Candidate);
        assert_eq!(hypothesis.bindings[0].evidence.kind, "candidate-binding");
        assert!(
            hypothesis
                .evidence
                .iter()
                .all(|item| item.evidence.kind != "derived-binding")
        );
    }

    #[test]
    fn exact_forward_law_evidence_remains_derived_while_ambiguous() {
        let mut formula = formula();
        formula.status = LawRecognitionStatus::ConditionMissing;
        formula.relevance = None;
        formula.bindings[0].proof = LawBindingProof::Derived;
        formula.bindings[0].evidence = evidence("law-chain-binding", "law-chain-role/value", 2);
        formula.conditions[0].status = ConstraintStatus::Required;
        let decision = MeaningDecision::Partial {
            meaning: MeaningConclusion {
                label: "Typed law".into(),
                relation_id: Some("test-pack:law".into()),
            },
            facts: Vec::new(),
            requirements: Vec::new(),
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[formula],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        let hypothesis = &projected.hypotheses[0];
        assert_eq!(hypothesis.support, MathInterpretationSupportTier::Tentative);
        assert_eq!(hypothesis.bindings[0].proof, LawBindingProof::Derived);
        assert_eq!(hypothesis.bindings[0].evidence.kind, "law-chain-binding");
        assert!(hypothesis.evidence.iter().any(|item| {
            item.evidence.kind == "law-chain-binding"
                && item.provenance == MathInterpretationEvidenceProvenance::DerivedEvidence
        }));
    }

    #[test]
    fn keeps_conventional_hypotheses_tentative_with_exact_missing_discriminators() {
        let requirement = ConventionalRequirementInfo::RoleDeclaration {
            requirement_id: "law/binding/value".into(),
            parameter: "value".into(),
            symbol: "x".into(),
            constraint: constraint(),
            evidence: vec![evidence("structural-candidate", "test/candidate", 12)],
        };
        let candidate = ConventionalCandidateInfo {
            candidate_id: "conventional/test-pack/law/10:20".into(),
            disposition: ConventionalCandidateDisposition::ConventionalCandidate,
            pack_id: "test-pack".into(),
            pack_version: "1.0.0".into(),
            law_id: "law".into(),
            title: "Typed law".into(),
            relation: relation("test-pack:law", "Typed law"),
            bindings: Vec::new(),
            requirements: vec![requirement],
            relevance: DomainRelevance {
                support: DomainSupportTier::Tentative,
                evidence: vec![evidence("prose-domain-prior", "test/domain", 0)],
            },
            evidence: vec![
                evidence("prose-domain-prior", "test/domain", 0),
                evidence("structural-candidate", "test/convention", 12),
            ],
        };
        let decision = MeaningDecision::Unsupported {
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        let authored_requirement = MathAuthoringRequirementInfo::RoleDeclaration {
            requirement_id: "law/binding/value".into(),
            parameter: "value".into(),
            symbol: "x".into(),
            constraint: constraint(),
            evidence: vec![evidence("structural-candidate", "test/candidate", 12)],
        };
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[],
            conventional_candidates: &[candidate],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[authored_requirement],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        let hypothesis = &projected.hypotheses[0];
        assert_eq!(hypothesis.kind, MathInterpretationKind::ReviewedConvention);
        assert_eq!(hypothesis.support, MathInterpretationSupportTier::Tentative);
        assert_eq!(hypothesis.missing_discriminator_ids, ["law/binding/value"]);
        assert!(hypothesis.evidence.iter().any(|item| {
            item.provenance == MathInterpretationEvidenceProvenance::DomainContext
        }));
        assert!(hypothesis.evidence.iter().any(|item| {
            item.provenance == MathInterpretationEvidenceProvenance::ReviewedConvention
        }));
    }

    #[test]
    fn preserves_multiple_scoped_domains_as_fallible_ranked_hypotheses() {
        let domains = [
            DomainActivation {
                pack_id: "calculus-analysis".into(),
                pack_version: "1.1.0".into(),
                title: "Calculus and analysis".into(),
                support: DomainSupportTier::Tentative,
                scope_kind: "section".into(),
                scope_range: range(0, 80),
                evidence: vec![evidence("structural-domain-prior", "test/calculus", 12)],
            },
            DomainActivation {
                pack_id: "linear-algebra".into(),
                pack_version: "1.4.0".into(),
                title: "Linear algebra".into(),
                support: DomainSupportTier::Tentative,
                scope_kind: "section".into(),
                scope_range: range(0, 80),
                evidence: vec![evidence("prose-domain-prior", "test/linear", 20)],
            },
        ];
        let decision = MeaningDecision::Unsupported {
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[],
            conventional_candidates: &[],
            domains: &domains,
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "notes",
            path: "notes/kinematics.md",
            scope_path: &[1],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        assert_eq!(projected.hypotheses.len(), 2);
        assert!(projected.hypotheses.iter().all(|hypothesis| {
            hypothesis.kind == MathInterpretationKind::ScopedDomain
                && hypothesis.support == MathInterpretationSupportTier::Tentative
                && hypothesis.location.file_id == "notes"
                && hypothesis.scope_path == [1]
                && hypothesis.evidence.iter().all(|item| {
                    item.provenance == MathInterpretationEvidenceProvenance::DomainContext
                })
        }));
    }

    #[test]
    fn separates_support_from_contradiction_and_reports_lifecycle_limits() {
        let supporting = SemanticClaimInfo {
            claim_id: "support".into(),
            predicate: "role".into(),
            value: "vector".into(),
            status: SemanticClaimStatus::Supported,
            evidence: vec![evidence("explicit-prose", "english-support", 1)],
            conflicts: Vec::new(),
        };
        let rejecting = SemanticClaimInfo {
            claim_id: "reject".into(),
            predicate: "role".into(),
            value: "scalar".into(),
            status: SemanticClaimStatus::Conflicting,
            evidence: vec![evidence("source-definition", "test-reject", 2)],
            conflicts: vec!["support".into()],
        };
        let candidate = SemanticCandidateInfo {
            candidate_id: "application".into(),
            family: "application".into(),
            interpretation: "function application".into(),
            status: SemanticCandidateStatus::Conflicting,
            range: range(10, 20),
            supporting_claim_ids: vec!["support".into()],
            rejecting_claim_ids: vec!["reject".into()],
        };
        let alternative = MeaningAlternative {
            alternative_id: "application".into(),
            label: "function application".into(),
            range: range(10, 20),
            evidence: Vec::new(),
            relevance: None,
        };
        let decision = MeaningDecision::Ambiguous {
            alternatives: vec![alternative],
            reasons: Vec::new(),
        };
        let requirement = MathAuthoringRequirementInfo::Disambiguation {
            requirement_id: "meaning/disambiguation".into(),
            alternatives: vec![MeaningAlternative {
                alternative_id: "application".into(),
                label: "function application".into(),
                range: range(10, 20),
                evidence: Vec::new(),
                relevance: None,
            }],
            evidence: Vec::new(),
        };
        let context = context(vec![candidate], vec![supporting, rejecting]);
        let lifecycle = MathSourceLifecycleInfo {
            generation: MathSourceGeneration::Generated,
            retracted: true,
            capped: true,
            engine_limited: true,
            editable: false,
            ..lifecycle()
        };
        let projected = project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[requirement],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: true,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        });

        let hypothesis = &projected.hypotheses[0];
        assert_eq!(
            hypothesis.support,
            MathInterpretationSupportTier::Contradicted
        );
        assert_eq!(
            hypothesis.missing_discriminator_ids,
            ["meaning/disambiguation"]
        );
        assert!(hypothesis.evidence.iter().any(|item| {
            item.role == MathInterpretationEvidenceRole::Supporting
                && item.provenance
                    == MathInterpretationEvidenceProvenance::NaturalLanguageExtraction
        }));
        assert!(hypothesis.evidence.iter().any(|item| {
            item.role == MathInterpretationEvidenceRole::Contradicting
                && item.provenance == MathInterpretationEvidenceProvenance::ExplicitDeclaration
        }));
        assert_eq!(
            projected
                .analysis_limits
                .iter()
                .map(|limit| limit.kind)
                .collect::<Vec<_>>(),
            [
                MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped,
                MathInterpretationAnalysisLimitKind::EngineLimit,
                MathInterpretationAnalysisLimitKind::GeneratedSource,
                MathInterpretationAnalysisLimitKind::RetractedSource,
            ]
        );
    }

    fn source_meaning_with(evidence: Evidence) -> MathInterpretationSetInfo {
        let decision = MeaningDecision::Established {
            meaning: MeaningConclusion {
                label: "source meaning".into(),
                relation_id: None,
            },
            reasons: vec![DecisionReason {
                kind: DecisionReasonKind::Proof,
                label: "source-backed".into(),
                evidence: vec![evidence],
            }],
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        })
    }

    #[test]
    fn source_meaning_support_follows_evidence_authority() {
        let natural_language = source_meaning_with(evidence(
            "explicit-prose",
            "scientific-prose/description",
            1,
        ));
        assert_eq!(
            natural_language.hypotheses[0].support,
            MathInterpretationSupportTier::Supported
        );

        let derived =
            source_meaning_with(evidence("derived-claim", "canonical-propagation/result", 1));
        assert_eq!(
            derived.hypotheses[0].support,
            MathInterpretationSupportTier::Derived
        );

        let explicit =
            source_meaning_with(evidence("source-definition", "test/explicit-definition", 1));
        assert_eq!(
            explicit.hypotheses[0].support,
            MathInterpretationSupportTier::Explicit
        );
    }

    fn structural_candidates(count: usize) -> Vec<SemanticCandidateInfo> {
        (0..count)
            .map(|index| SemanticCandidateInfo {
                candidate_id: format!("candidate/{index}"),
                family: "application".into(),
                interpretation: format!("candidate {index}"),
                status: SemanticCandidateStatus::Unresolved,
                range: range(index as u32, index as u32 + 1),
                supporting_claim_ids: Vec::new(),
                rejecting_claim_ids: Vec::new(),
            })
            .collect()
    }

    fn project_candidates(count: usize) -> MathInterpretationSetInfo {
        project_structural_candidates(structural_candidates(count), false)
    }

    fn project_structural_candidates(
        candidates: Vec<SemanticCandidateInfo>,
        lifecycle_capped: bool,
    ) -> MathInterpretationSetInfo {
        let decision = MeaningDecision::Unsupported {
            reasons: Vec::new(),
        };
        let context = context(candidates, Vec::new());
        let lifecycle = MathSourceLifecycleInfo {
            capped: lifecycle_capped,
            ..lifecycle()
        };
        let formula = MathFormulaAnchorInfo {
            location: Location {
                file_id: "main".into(),
                path: "main.tex".into(),
                range: range(10, 20),
            },
            document_version: 1,
            scope_path: vec![0, 1],
            source_notation: "x + y".into(),
            provenance: Vec::new(),
        };
        project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: Some(&formula),
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        })
    }

    #[test]
    fn candidate_cap_reports_only_at_boundary_plus_one() {
        let boundary = project_candidates(MATH_INTERPRETATION_HYPOTHESIS_LIMIT);
        assert!(!boundary.truncated);
        assert!(boundary.analysis_limits.is_empty());
        assert!(boundary.candidate_cap.is_none());

        let capped = project_candidates(MATH_INTERPRETATION_HYPOTHESIS_LIMIT + 1);
        assert!(capped.truncated);
        assert_eq!(
            capped.hypotheses.len(),
            MATH_INTERPRETATION_HYPOTHESIS_LIMIT
        );
        let candidate_cap = capped.candidate_cap.as_ref().unwrap();
        assert_eq!(candidate_cap.candidate_count_before_cap, 17);
        assert_eq!(
            candidate_cap.pre_cap_semantic_key_digest,
            "da08f15f67c82e557e56b90af5aa7dd38db391b6f94c13ce982f43fb794646c4"
        );
        assert_eq!(candidate_cap.pre_cap_semantic_key_digest.len(), 64);
        assert!(
            candidate_cap
                .pre_cap_semantic_key_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert_eq!(
            capped.analysis_limits[0].kind,
            MathInterpretationAnalysisLimitKind::CandidateSetCapped
        );
        assert_eq!(capped.analysis_limits[0].evidence.len(), 1);
        assert_eq!(
            capped.analysis_limits[0].evidence[0].source_anchors[0]
                .location
                .range,
            range(10, 20)
        );

        let unrelated_view_cap = project_structural_candidates(
            structural_candidates(MATH_INTERPRETATION_HYPOTHESIS_LIMIT),
            true,
        );
        assert!(!unrelated_view_cap.truncated);
        assert!(unrelated_view_cap.candidate_cap.is_none());
        assert!(unrelated_view_cap.analysis_limits.is_empty());
    }

    #[test]
    fn candidate_cap_digest_is_stable_under_input_order_and_opaque_ids() {
        let candidates = structural_candidates(MATH_INTERPRETATION_HYPOTHESIS_LIMIT + 1);
        let expected = project_structural_candidates(candidates.clone(), false);
        let mut reordered = candidates.clone();
        reordered.reverse();
        let reordered = project_structural_candidates(reordered, false);
        let mut renamed = candidates;
        for (index, candidate) in renamed.iter_mut().enumerate() {
            candidate.candidate_id = format!("opaque-renamed/{index}");
        }
        let renamed = project_structural_candidates(renamed, false);
        let mut duplicated = structural_candidates(MATH_INTERPRETATION_HYPOTHESIS_LIMIT + 1);
        let mut duplicate = duplicated[0].clone();
        duplicate.candidate_id = "opaque-duplicate".into();
        duplicated.push(duplicate);
        let duplicated = project_structural_candidates(duplicated, false);

        assert_eq!(expected.candidate_cap, reordered.candidate_cap);
        assert_eq!(expected.candidate_cap, renamed.candidate_cap);
        assert_eq!(expected.candidate_cap, duplicated.candidate_cap);
        assert_eq!(expected.hypotheses.len(), duplicated.hypotheses.len());
        assert_eq!(
            expected
                .hypotheses
                .iter()
                .map(|hypothesis| (&hypothesis.label, &hypothesis.range))
                .collect::<Vec<_>>(),
            reordered
                .hypotheses
                .iter()
                .map(|hypothesis| (&hypothesis.label, &hypothesis.range))
                .collect::<Vec<_>>()
        );
    }

    fn project_formula_evidence(count: usize) -> MathInterpretationSetInfo {
        let mut formula = formula();
        formula.bindings.clear();
        formula.conditions.clear();
        formula.relevance = None;
        formula.evidence = (0..count)
            .map(|index| {
                evidence(
                    "source-relation",
                    &format!("test/evidence/{index}"),
                    index as u32,
                )
            })
            .collect();
        let decision = MeaningDecision::Established {
            meaning: MeaningConclusion {
                label: "Typed law".into(),
                relation_id: Some("test-pack:law".into()),
            },
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = lifecycle();
        project_math_interpretations(MathInterpretationInput {
            decision: &decision,
            formulas: &[formula],
            conventional_candidates: &[],
            domains: &[],
            structural_candidates: &context.candidates,
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            discriminator_set_capped: false,
            resolve_evidence: &resolve_test_evidence,
            refutation_evidence: &[],
        })
    }

    #[test]
    fn evidence_cap_reports_only_at_boundary_plus_one() {
        let boundary = project_formula_evidence(MAX_INTERPRETATION_EVIDENCE);
        assert!(!boundary.truncated);
        assert!(boundary.analysis_limits.is_empty());

        let capped = project_formula_evidence(MAX_INTERPRETATION_EVIDENCE + 1);
        assert!(capped.truncated);
        assert_eq!(
            capped.hypotheses[0].evidence.len(),
            MAX_INTERPRETATION_EVIDENCE
        );
        assert_eq!(
            capped.analysis_limits[0].kind,
            MathInterpretationAnalysisLimitKind::EvidenceTruncated
        );
    }

    #[test]
    fn discriminator_cap_is_independent_from_other_authoring_view_limits() {
        let decision = MeaningDecision::Unsupported {
            reasons: Vec::new(),
        };
        let context = context(Vec::new(), Vec::new());
        let lifecycle = MathSourceLifecycleInfo {
            capped: true,
            ..lifecycle()
        };
        let project = |discriminator_set_capped| {
            project_math_interpretations(MathInterpretationInput {
                decision: &decision,
                formulas: &[],
                conventional_candidates: &[],
                domains: &[],
                structural_candidates: &context.candidates,
                context: &context,
                requirements: &[],
                formula: None,
                focus_range: Some(&range(10, 20)),
                file_id: "main",
                path: "main.tex",
                scope_path: &[],
                lifecycle: &lifecycle,
                discriminator_set_capped,
                resolve_evidence: &resolve_test_evidence,
                refutation_evidence: &[],
            })
        };
        let unrelated_view_cap = project(false);
        assert!(!unrelated_view_cap.truncated);
        assert!(unrelated_view_cap.analysis_limits.is_empty());

        let discriminator_cap = project(true);
        assert!(discriminator_cap.truncated);
        assert_eq!(
            discriminator_cap.analysis_limits[0].kind,
            MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped
        );

        let project_count = |count| {
            let mut requirements = (0..count)
                .map(|index| MathAuthoringRequirementInfo::Disambiguation {
                    requirement_id: format!("requirement/{index}"),
                    alternatives: Vec::new(),
                    evidence: Vec::new(),
                })
                .collect::<Vec<_>>();
            let capped = requirements.len() > MAX_INTERPRETATION_DISCRIMINATORS;
            requirements.truncate(MAX_INTERPRETATION_DISCRIMINATORS);
            project_math_interpretations(MathInterpretationInput {
                decision: &decision,
                formulas: &[],
                conventional_candidates: &[],
                domains: &[],
                structural_candidates: &context.candidates,
                context: &context,
                requirements: &requirements,
                formula: None,
                focus_range: Some(&range(10, 20)),
                file_id: "main",
                path: "main.tex",
                scope_path: &[],
                lifecycle: &lifecycle,
                discriminator_set_capped: capped,
                resolve_evidence: &resolve_test_evidence,
                refutation_evidence: &[],
            })
        };
        let boundary = project_count(MAX_INTERPRETATION_DISCRIMINATORS);
        assert!(!boundary.truncated);
        assert!(boundary.analysis_limits.is_empty());
        let plus_one = project_count(MAX_INTERPRETATION_DISCRIMINATORS + 1);
        assert!(plus_one.truncated);
        assert_eq!(
            plus_one.missing_discriminators.len(),
            MAX_INTERPRETATION_DISCRIMINATORS
        );
        assert_eq!(
            plus_one.analysis_limits[0].kind,
            MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped
        );
    }
}
