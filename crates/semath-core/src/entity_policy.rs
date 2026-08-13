use crate::semantic_index::{
    EntityId, EvidenceModality, EvidenceOrigin, EvidencePolarity, EvidenceRecord, Resolution,
    ResolutionStatus, SourceOccurrence, SourceOccurrenceId,
};
use crate::{
    EntitySurfaceAuthorization, EntitySurfaceRefusal, EntitySurfaceRefusalKind, SourceRange,
};

pub(crate) const MAX_RENAME_OCCURRENCES: usize = 4_096;
pub(crate) const MAX_ENTITY_SURFACE_OCCURRENCES: usize = MAX_RENAME_OCCURRENCES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EntityEvidenceDecision {
    Established(EntityId),
    Ambiguous,
    Conflicting,
    Unsupported,
    EngineLimited,
}

pub(crate) fn decide_entity(resolution: &Resolution) -> EntityEvidenceDecision {
    if resolution.truncated {
        return EntityEvidenceDecision::EngineLimited;
    }
    match resolution.status {
        ResolutionStatus::Established if resolution.candidates.len() == 1 => {
            let candidate = &resolution.candidates[0];
            if candidate.supporting_claims.is_empty() || !candidate.rejecting_claims.is_empty() {
                EntityEvidenceDecision::Unsupported
            } else {
                EntityEvidenceDecision::Established(candidate.entity_id.clone())
            }
        }
        ResolutionStatus::Ambiguous => EntityEvidenceDecision::Ambiguous,
        ResolutionStatus::Conflicting => EntityEvidenceDecision::Conflicting,
        ResolutionStatus::Established | ResolutionStatus::Unsupported => {
            EntityEvidenceDecision::Unsupported
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthorizedEntitySurface {
    pub focus_occurrence_id: SourceOccurrenceId,
    pub entity_id: EntityId,
    pub occurrences: Vec<SourceOccurrence>,
}

impl AuthorizedEntitySurface {
    pub(crate) fn authorization(&self) -> EntitySurfaceAuthorization {
        EntitySurfaceAuthorization::Authorized {
            focus_occurrence_id: self.focus_occurrence_id.clone(),
            entity_id: self.entity_id.clone(),
        }
    }
}

pub(crate) fn authorize_entity_surface(
    focus_occurrence_id: &SourceOccurrenceId,
    decision: EntityEvidenceDecision,
    occurrences: Result<Vec<SourceOccurrence>, ()>,
) -> Result<AuthorizedEntitySurface, EntitySurfaceRefusal> {
    let EntityEvidenceDecision::Established(entity_id) = decision else {
        return Err(entity_decision_refusal(decision));
    };
    let occurrences = occurrences.map_err(|()| {
        refusal(
            EntitySurfaceRefusalKind::EngineLimit,
            format!(
                "The complete entity surface exceeds the {MAX_ENTITY_SURFACE_OCCURRENCES}-occurrence safety cap."
            ),
        )
    })?;
    if occurrences.is_empty()
        || !occurrences
            .iter()
            .any(|occurrence| occurrence.id == *focus_occurrence_id)
    {
        return Err(refusal(
            EntitySurfaceRefusalKind::IncompleteSource,
            "The established identity does not have one complete source-backed occurrence set.",
        ));
    }
    Ok(AuthorizedEntitySurface {
        focus_occurrence_id: focus_occurrence_id.clone(),
        entity_id,
        occurrences,
    })
}

pub(crate) fn refused_authorization(reason: EntitySurfaceRefusal) -> EntitySurfaceAuthorization {
    EntitySurfaceAuthorization::Refused { reason }
}

fn entity_decision_refusal(decision: EntityEvidenceDecision) -> EntitySurfaceRefusal {
    match decision {
        EntityEvidenceDecision::Ambiguous => refusal(
            EntitySurfaceRefusalKind::Ambiguous,
            "The symbol has more than one source-supported identity.",
        ),
        EntityEvidenceDecision::Conflicting => refusal(
            EntitySurfaceRefusalKind::Conflicting,
            "The symbol identity has conflicting source evidence.",
        ),
        EntityEvidenceDecision::EngineLimited => refusal(
            EntitySurfaceRefusalKind::EngineLimit,
            "The complete identity decision exceeds an engine evidence limit.",
        ),
        EntityEvidenceDecision::Unsupported | EntityEvidenceDecision::Established(_) => refusal(
            EntitySurfaceRefusalKind::Unsupported,
            "The symbol does not have one established source identity.",
        ),
    }
}

pub(crate) fn refusal(
    kind: EntitySurfaceRefusalKind,
    message: impl Into<String>,
) -> EntitySurfaceRefusal {
    EntitySurfaceRefusal {
        kind,
        message: message.into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntityFactDisposition {
    Certain,
    Supported,
    Speculative,
    Conflicting,
}

pub(crate) fn decide_fact(
    evidence: &EvidenceRecord,
    has_opposing_evidence: bool,
) -> EntityFactDisposition {
    if has_opposing_evidence {
        return EntityFactDisposition::Conflicting;
    }
    match (evidence.polarity, evidence.modality, evidence.origin) {
        (EvidencePolarity::Positive, EvidenceModality::Asserted, EvidenceOrigin::Explicit) => {
            EntityFactDisposition::Certain
        }
        (EvidencePolarity::Positive, EvidenceModality::Asserted, EvidenceOrigin::Derived) => {
            EntityFactDisposition::Supported
        }
        _ => EntityFactDisposition::Speculative,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameNotationFamily {
    PlainIdentifier,
    ControlSequence,
}

pub(crate) fn rename_focus_is_complete(occurrence: &SourceOccurrence) -> bool {
    let scripted = occurrence.notation.iter().any(|component| {
        matches!(
            component,
            crate::semantic_index::NotationComponent::Subscript { .. }
                | crate::semantic_index::NotationComponent::Superscript
        )
    });
    !scripted || occurrence.selection_range == occurrence.range
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenameSourceOccurrence {
    pub occurrence_id: SourceOccurrenceId,
    pub range: SourceRange,
    pub current_text: String,
    pub family: RenameNotationFamily,
    pub editable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlannedRename {
    pub entity_id: EntityId,
    pub old_name: String,
    pub new_name: String,
    pub occurrences: Vec<RenameSourceOccurrence>,
}

pub(crate) fn plan_entity_rename(
    decision: EntityEvidenceDecision,
    old_name: &str,
    new_name: &str,
    mut occurrences: Vec<RenameSourceOccurrence>,
) -> Result<PlannedRename, EntitySurfaceRefusal> {
    let EntityEvidenceDecision::Established(entity_id) = decision else {
        return Err(entity_decision_refusal(decision));
    };
    if old_name == new_name {
        return Err(refusal(
            EntitySurfaceRefusalKind::InvalidReplacement,
            "The new name is unchanged.",
        ));
    }
    if occurrences.is_empty() {
        return Err(refusal(
            EntitySurfaceRefusalKind::IncompleteSource,
            "The established entity has no editable source occurrences.",
        ));
    }
    if occurrences.len() > MAX_RENAME_OCCURRENCES {
        return Err(refusal(
            EntitySurfaceRefusalKind::EngineLimit,
            format!(
                "The complete rename set exceeds the {MAX_RENAME_OCCURRENCES}-occurrence safety cap."
            ),
        ));
    }
    occurrences.sort_by(|left, right| {
        left.occurrence_id
            .cmp(&right.occurrence_id)
            .then(left.range.start_offset.cmp(&right.range.start_offset))
    });
    occurrences.dedup_by(|left, right| {
        left.occurrence_id == right.occurrence_id && left.range == right.range
    });
    let family = occurrences[0].family;
    if occurrences.iter().any(|occurrence| {
        occurrence.range.start_offset >= occurrence.range.end_offset
            || !occurrence.editable
            || occurrence.family != family
            || occurrence.current_text != old_name
            || !valid_replacement(occurrence.family, &occurrence.current_text)
    }) {
        return Err(refusal(
            EntitySurfaceRefusalKind::NonEditable,
            "Every edit must be a real, exact source occurrence in the same notation family.",
        ));
    }
    if !valid_replacement(family, new_name) {
        return Err(refusal(
            EntitySurfaceRefusalKind::InvalidReplacement,
            "The new name is not valid for the established notation family.",
        ));
    }
    Ok(PlannedRename {
        entity_id,
        old_name: old_name.into(),
        new_name: new_name.into(),
        occurrences,
    })
}

fn valid_replacement(family: RenameNotationFamily, name: &str) -> bool {
    match family {
        RenameNotationFamily::PlainIdentifier => {
            let mut characters = name.chars();
            characters.next().is_some_and(char::is_alphabetic) && characters.next().is_none()
        }
        RenameNotationFamily::ControlSequence => name
            .strip_prefix('\\')
            .is_some_and(|tail| !tail.is_empty() && tail.chars().all(char::is_alphabetic)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_index::{ClaimId, ResolutionCandidate};

    fn source_occurrence(
        range: SourceRange,
        selection_range: SourceRange,
        notation: Vec<crate::semantic_index::NotationComponent>,
    ) -> SourceOccurrence {
        SourceOccurrence {
            id: occurrence(0).occurrence_id,
            component_id: "project".into(),
            kind: crate::semantic_index::OccurrenceKind::Notation,
            range,
            selection_range,
            scope_path: Vec::new(),
            structural_path: Vec::new(),
            availability_order: 0,
            surface: "x_s".into(),
            source_text: "x_s".into(),
            selection_text: "x".into(),
            notation,
        }
    }

    fn occurrence(local_id: u32) -> RenameSourceOccurrence {
        RenameSourceOccurrence {
            occurrence_id: SourceOccurrenceId {
                file_id: "main".into(),
                document_version: 1,
                local_id,
            },
            range: SourceRange {
                start_offset: local_id * 2,
                end_offset: local_id * 2 + 1,
            },
            current_text: "x".into(),
            family: RenameNotationFamily::PlainIdentifier,
            editable: true,
        }
    }

    fn entity() -> EntityId {
        EntityId {
            component_id: "project".into(),
            scope_path: vec![0],
            kind: "definition".into(),
            anchor: occurrence(0).occurrence_id,
        }
    }

    fn fact_evidence(
        polarity: EvidencePolarity,
        modality: EvidenceModality,
        origin: EvidenceOrigin,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: crate::semantic_index::EvidenceId("evidence".into()),
            source: occurrence(0).occurrence_id.clone(),
            scope_path: Vec::new(),
            available_after: 0,
            polarity,
            modality,
            origin,
            provenance: vec![occurrence(0).occurrence_id],
            parent_claims: Vec::new(),
            rule_id: "test/fact".into(),
            rule_version: 1,
        }
    }

    #[test]
    fn fact_decision_uses_typed_evidence_not_presentation_strings() {
        assert_eq!(
            decide_fact(
                &fact_evidence(
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                    EvidenceOrigin::Explicit,
                ),
                false,
            ),
            EntityFactDisposition::Certain
        );
        assert_eq!(
            decide_fact(
                &fact_evidence(
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                    EvidenceOrigin::Derived,
                ),
                false,
            ),
            EntityFactDisposition::Supported
        );
        assert_eq!(
            decide_fact(
                &fact_evidence(
                    EvidencePolarity::Positive,
                    EvidenceModality::Hedged,
                    EvidenceOrigin::Explicit,
                ),
                false,
            ),
            EntityFactDisposition::Speculative
        );
        assert_eq!(
            decide_fact(
                &fact_evidence(
                    EvidencePolarity::Negative,
                    EvidenceModality::Asserted,
                    EvidenceOrigin::Explicit,
                ),
                true,
            ),
            EntityFactDisposition::Conflicting
        );
    }

    #[test]
    fn rename_focus_rejects_only_partial_script_edits() {
        let whole = SourceRange {
            start_offset: 1,
            end_offset: 4,
        };
        let nucleus = SourceRange {
            start_offset: 1,
            end_offset: 2,
        };
        assert!(!rename_focus_is_complete(&source_occurrence(
            whole.clone(),
            nucleus.clone(),
            vec![crate::semantic_index::NotationComponent::Subscript {
                base: "x".into(),
                index: "s".into(),
            }],
        )));
        assert!(!rename_focus_is_complete(&source_occurrence(
            whole.clone(),
            nucleus.clone(),
            vec![crate::semantic_index::NotationComponent::Superscript],
        )));
        assert!(rename_focus_is_complete(&source_occurrence(
            whole.clone(),
            whole,
            vec![crate::semantic_index::NotationComponent::Superscript],
        )));
        assert!(rename_focus_is_complete(&source_occurrence(
            SourceRange {
                start_offset: 1,
                end_offset: 11,
            },
            nucleus,
            vec![crate::semantic_index::NotationComponent::Style {
                name: "mathbf".into(),
            }],
        )));
    }

    #[test]
    fn resolution_requires_one_complete_positive_candidate() {
        let candidate = ResolutionCandidate {
            entity_id: entity(),
            supporting_claims: vec![ClaimId("support".into())],
            rejecting_claims: Vec::new(),
        };
        let mut resolution = Resolution {
            occurrence_id: occurrence(0).occurrence_id,
            status: ResolutionStatus::Established,
            candidates: vec![candidate],
            truncated: false,
        };
        assert!(matches!(
            decide_entity(&resolution),
            EntityEvidenceDecision::Established(_)
        ));
        resolution.truncated = true;
        assert_eq!(
            decide_entity(&resolution),
            EntityEvidenceDecision::EngineLimited
        );
    }

    #[test]
    fn rename_is_all_or_reject_at_the_fanout_cap() {
        let accepted = (0..MAX_RENAME_OCCURRENCES as u32).map(occurrence).collect();
        assert!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "x",
                "y",
                accepted,
            )
            .is_ok()
        );
        let rejected = (0..=MAX_RENAME_OCCURRENCES as u32)
            .map(occurrence)
            .collect();
        assert_eq!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "x",
                "y",
                rejected,
            )
            .unwrap_err()
            .kind,
            EntitySurfaceRefusalKind::EngineLimit
        );
    }

    #[test]
    fn shared_surface_refuses_cap_plus_one_without_a_partial_identity() {
        let focus = occurrence(0).occurrence_id;
        let accepted = (0..MAX_ENTITY_SURFACE_OCCURRENCES as u32)
            .map(|local_id| {
                let item = occurrence(local_id);
                source_occurrence(item.range.clone(), item.range, Vec::new())
            })
            .enumerate()
            .map(|(local_id, mut item)| {
                item.id.local_id = local_id as u32;
                item
            })
            .collect::<Vec<_>>();
        assert!(
            authorize_entity_surface(
                &focus,
                EntityEvidenceDecision::Established(entity()),
                Ok(accepted),
            )
            .is_ok()
        );

        let refusal = authorize_entity_surface(
            &focus,
            EntityEvidenceDecision::Established(entity()),
            Err(()),
        )
        .unwrap_err();
        assert_eq!(refusal.kind, EntitySurfaceRefusalKind::EngineLimit);
    }

    #[test]
    fn rename_rejects_partial_or_cross_family_edits() {
        let mut inexact = occurrence(0);
        inexact.current_text = "x_i".into();
        assert_eq!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "x_i",
                "y",
                vec![inexact],
            )
            .unwrap_err()
            .kind,
            EntitySurfaceRefusalKind::NonEditable
        );
        let mut command = occurrence(0);
        command.current_text = "\\alpha".into();
        command.family = RenameNotationFamily::ControlSequence;
        assert_eq!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "\\alpha",
                "beta",
                vec![command],
            )
            .unwrap_err()
            .kind,
            EntitySurfaceRefusalKind::InvalidReplacement
        );
    }
}
