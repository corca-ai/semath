use crate::SourceRange;
use crate::semantic_index::{EntityId, Resolution, ResolutionStatus, SourceOccurrenceId};

pub(crate) const MAX_RENAME_OCCURRENCES: usize = 4_096;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameNotationFamily {
    PlainIdentifier,
    ControlSequence,
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
) -> Result<PlannedRename, String> {
    let EntityEvidenceDecision::Established(entity_id) = decision else {
        return Err(match decision {
            EntityEvidenceDecision::Ambiguous => {
                "The symbol has more than one source-supported identity.".into()
            }
            EntityEvidenceDecision::Conflicting => {
                "The symbol identity has conflicting source evidence.".into()
            }
            EntityEvidenceDecision::EngineLimited => {
                "The complete rename set exceeds an engine evidence limit.".into()
            }
            EntityEvidenceDecision::Unsupported | EntityEvidenceDecision::Established(_) => {
                "The symbol does not have one established source identity.".into()
            }
        });
    };
    if old_name == new_name {
        return Err("The new name is unchanged.".into());
    }
    if occurrences.is_empty() {
        return Err("The established entity has no editable source occurrences.".into());
    }
    if occurrences.len() > MAX_RENAME_OCCURRENCES {
        return Err(format!(
            "The complete rename set exceeds the {MAX_RENAME_OCCURRENCES}-occurrence safety cap."
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
            || !valid_replacement(occurrence.family, new_name)
    }) {
        return Err(
            "Every edit must be a real, exact source occurrence in the same notation family."
                .into(),
        );
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
        assert!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "x",
                "y",
                rejected,
            )
            .unwrap_err()
            .contains("safety cap")
        );
    }

    #[test]
    fn rename_rejects_partial_or_cross_family_edits() {
        let mut inexact = occurrence(0);
        inexact.current_text = "x_i".into();
        assert!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "x",
                "y",
                vec![inexact],
            )
            .is_err()
        );
        let mut command = occurrence(0);
        command.current_text = "\\alpha".into();
        command.family = RenameNotationFamily::ControlSequence;
        assert!(
            plan_entity_rename(
                EntityEvidenceDecision::Established(entity()),
                "\\alpha",
                "beta",
                vec![command],
            )
            .is_err()
        );
    }
}
