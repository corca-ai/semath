use crate::{
    ConstraintStatus, ConventionalCandidateInfo, DomainActivation, DomainSupportTier, Evidence,
    LawBindingProof, LawRecognition, LawRecognitionStatus, Location, MathAuthoringRequirementInfo,
    MathFormulaAnchorInfo, MathInterpretationAnalysisLimitInfo,
    MathInterpretationAnalysisLimitKind, MathInterpretationEvidenceInfo,
    MathInterpretationEvidenceProvenance, MathInterpretationEvidenceRole,
    MathInterpretationExhaustiveness, MathInterpretationHypothesisInfo, MathInterpretationKind,
    MathInterpretationOrderingReason, MathInterpretationOrderingReasonKind,
    MathInterpretationSetInfo, MathInterpretationSupportTier, MathSourceGeneration,
    MathSourceLifecycleInfo, MeaningAlternative, MeaningDecision, SemanticCandidateInfo,
    SemanticCandidateStatus, SemanticContextInfo, SourceRange,
};

const MAX_INTERPRETATION_HYPOTHESES: usize = 16;

pub(crate) struct MathInterpretationInput<'a> {
    pub decision: &'a MeaningDecision,
    pub formulas: &'a [LawRecognition],
    pub conventional_candidates: &'a [ConventionalCandidateInfo],
    pub domains: &'a [DomainActivation],
    pub context: &'a SemanticContextInfo,
    pub requirements: &'a [MathAuthoringRequirementInfo],
    pub formula: Option<&'a MathFormulaAnchorInfo>,
    pub focus_range: Option<&'a SourceRange>,
    pub file_id: &'a str,
    pub path: &'a str,
    pub scope_path: &'a [u32],
    pub lifecycle: &'a MathSourceLifecycleInfo,
    pub view_truncated: bool,
}

pub(crate) fn project_math_interpretations(
    input: MathInterpretationInput<'_>,
) -> MathInterpretationSetInfo {
    let mut hypotheses = Vec::new();
    for formula in input.formulas {
        if let Some(relation) = &formula.relation {
            let support = formula_support(input.decision, formula);
            let (supporting, contradicting) =
                formula_evidence(input.decision, formula, input.context);
            let ordering_reasons = ordering_reasons(
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
                rank: formula.rank,
                range: relation.range.clone(),
                location: location(&input, &relation.range),
                document_version: input.lifecycle.document_version,
                scope_path: input.scope_path.to_vec(),
                formula: input.formula.cloned(),
                relation: Some(relation.clone()),
                bindings: formula.bindings.clone(),
                conditions: formula.conditions.clone(),
                evidence: graded_evidence(supporting, contradicting, false),
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

    for candidate in &input.context.candidates {
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
            evidence: graded_evidence(supporting, Vec::new(), true),
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

    if hypotheses.is_empty()
        && let Some(hypothesis) = source_meaning_hypothesis(&input)
    {
        hypotheses.push(hypothesis);
    }

    hypotheses.sort_by(|left, right| {
        support_rank(left.support)
            .cmp(&support_rank(right.support))
            .then(left.rank.cmp(&right.rank))
            .then(left.range.start_offset.cmp(&right.range.start_offset))
            .then(left.hypothesis_id.cmp(&right.hypothesis_id))
    });
    hypotheses.dedup_by(|left, right| left.hypothesis_id == right.hypothesis_id);
    let candidate_set_capped = hypotheses.len() > MAX_INTERPRETATION_HYPOTHESES;
    hypotheses.truncate(MAX_INTERPRETATION_HYPOTHESES);
    for (rank, hypothesis) in hypotheses.iter_mut().enumerate() {
        hypothesis.rank = rank as u32;
    }

    let mut analysis_limits = analysis_limits(&input, candidate_set_capped);
    analysis_limits.sort_by_key(|limit| limit_rank(limit.kind));
    MathInterpretationSetInfo {
        hypotheses,
        missing_discriminators: input.requirements.to_vec(),
        analysis_limits,
        exhaustiveness: MathInterpretationExhaustiveness::BoundedOpenWorld,
        truncated: input.view_truncated || candidate_set_capped,
    }
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
            MathInterpretationSupportTier::Supported
        }
        MeaningDecision::Ambiguous { alternatives, .. }
            if alternatives
                .iter()
                .any(|alternative| Some(alternative.alternative_id.as_str()) == relation_id) =>
        {
            match formula
                .relevance
                .as_ref()
                .map(|relevance| relevance.support)
            {
                Some(DomainSupportTier::Tentative) => MathInterpretationSupportTier::Tentative,
                _ => MathInterpretationSupportTier::Supported,
            }
        }
        _ if matches!(
            formula.status,
            LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
        ) =>
        {
            MathInterpretationSupportTier::Supported
        }
        _ => MathInterpretationSupportTier::Tentative,
    }
}

fn formula_evidence(
    decision: &MeaningDecision,
    formula: &LawRecognition,
    context: &SemanticContextInfo,
) -> (Vec<Evidence>, Vec<Evidence>) {
    let mut supporting = formula.evidence.clone();
    supporting.extend(
        formula
            .bindings
            .iter()
            .map(|binding| binding.evidence.clone()),
    );
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
        .flat_map(|condition| condition.evidence.clone())
        .collect::<Vec<_>>();
    if let MeaningDecision::Conflicting { conflicts, .. } = decision {
        contradicting.extend(
            conflicts
                .iter()
                .flat_map(|conflict| conflict.evidence.clone()),
        );
    }
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
    let ordering_reasons = ordering_reasons(support, &supporting, None, false);
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
        evidence: graded_evidence(supporting, contradicting, false),
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
                evidence,
            })
            .collect(),
        missing_discriminator_ids: disambiguation_ids(input.requirements),
        ordering_reasons: vec![
            MathInterpretationOrderingReason {
                kind: MathInterpretationOrderingReasonKind::DomainRelevance,
                evidence: domain.evidence.iter().take(1).cloned().collect(),
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
        evidence: graded_evidence(Vec::new(), conflict.evidence.clone(), false),
        missing_discriminator_ids: input.requirements.iter().map(requirement_id).collect(),
        ordering_reasons: ordering_reasons(
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
        evidence: graded_evidence(alternative.evidence.clone(), Vec::new(), false),
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
    let (label, support, evidence) = match input.decision {
        MeaningDecision::Established {
            meaning, reasons, ..
        } => (
            meaning.label.clone(),
            MathInterpretationSupportTier::Explicit,
            reasons
                .iter()
                .flat_map(|reason| reason.evidence.clone())
                .collect(),
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
                MathInterpretationSupportTier::Supported,
                evidence,
            )
        }
        _ => return None,
    };
    let range = input
        .formula
        .map(|formula| formula.location.range.clone())
        .or_else(|| input.focus_range.cloned())?;
    let ordering_reasons = ordering_reasons(support, &evidence, None, false);
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
        evidence: graded_evidence(evidence, Vec::new(), false),
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
        .map(|(role, evidence)| MathInterpretationEvidenceInfo {
            role,
            provenance: evidence_provenance(&evidence, convention),
            evidence,
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
    } else if kind.contains("derived") || evidence.rule_id.starts_with("canonical-propagation/") {
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
            evidence: primary_evidence.iter().take(1).cloned().collect(),
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
            evidence: primary_evidence.iter().take(1).cloned().collect(),
        });
    }
    if let Some(evidence) = relevance.filter(|evidence| !evidence.is_empty()) {
        reasons.push(MathInterpretationOrderingReason {
            kind: MathInterpretationOrderingReasonKind::DomainRelevance,
            evidence: evidence.iter().take(1).cloned().collect(),
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
) -> Vec<MathInterpretationAnalysisLimitInfo> {
    let anchor_evidence = input.formula.map_or_else(Vec::new, |formula| {
        vec![Evidence {
            rule_id: "semath/authoring/analysis-limit".into(),
            kind: "source-occurrence".into(),
            strength: "contextual".into(),
            source_ranges: vec![formula.location.range.clone()],
        }]
    });
    let mut limits = Vec::new();
    let mut push = |kind| {
        limits.push(MathInterpretationAnalysisLimitInfo {
            kind,
            evidence: anchor_evidence.clone(),
        });
    };
    if candidate_set_capped || input.lifecycle.capped {
        push(MathInterpretationAnalysisLimitKind::CandidateSetCapped);
    }
    if input.view_truncated {
        push(MathInterpretationAnalysisLimitKind::EvidenceTruncated);
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
        MathInterpretationAnalysisLimitKind::EngineLimit => 2,
        MathInterpretationAnalysisLimitKind::GeneratedSource => 3,
        MathInterpretationAnalysisLimitKind::RetractedSource => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConventionalCandidateDisposition, ConventionalRequirementInfo, DomainRelevance, LawBinding,
        LawConditionInfo, MathSourceFreshness, MeaningConclusion, RelationInfo,
        ScientificConstraintKind, SemanticClaimInfo, SemanticClaimStatus, SemanticConstraint,
        SemanticConstraintKind,
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
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[0, 1],
            lifecycle: &lifecycle,
            view_truncated: false,
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
            context: &context,
            requirements: &[authored_requirement],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            view_truncated: false,
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
            context: &context,
            requirements: &[],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "notes",
            path: "notes/kinematics.md",
            scope_path: &[1],
            lifecycle: &lifecycle,
            view_truncated: false,
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
            context: &context,
            requirements: &[requirement],
            formula: None,
            focus_range: Some(&range(10, 20)),
            file_id: "main",
            path: "main.tex",
            scope_path: &[],
            lifecycle: &lifecycle,
            view_truncated: true,
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
                MathInterpretationAnalysisLimitKind::CandidateSetCapped,
                MathInterpretationAnalysisLimitKind::EvidenceTruncated,
                MathInterpretationAnalysisLimitKind::EngineLimit,
                MathInterpretationAnalysisLimitKind::GeneratedSource,
                MathInterpretationAnalysisLimitKind::RetractedSource,
            ]
        );
    }
}
