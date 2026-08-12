use std::collections::BTreeMap;

use crate::semantic_index::{
    Claim, ClaimId, ClaimObject, ClaimPredicate, ClaimValue, EntityId, EvidenceId,
    EvidenceModality, EvidenceOrigin, EvidencePolarity, EvidenceRecord,
};

/// Maximum number of typed atoms that may be projected into one analysis
/// boundary. Exceeding the bound rejects the complete projection rather than
/// leaking an order-dependent prefix into law recognition.
pub(crate) const MAX_PROJECTED_TYPE_FACTS: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProjectedTypeValue {
    Role(String),
    Type(String),
    Shape(crate::semantic_index::ClaimShape),
    Dimension(Vec<crate::semantic_index::DimensionExponent>),
    Quantity(String),
    Unit(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProjectedTypeFact {
    pub entity: EntityId,
    pub value: ProjectedTypeValue,
    pub evidence_id: EvidenceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeProjectionError {
    EngineLimited,
}

/// Projects the semantic index's typed evidence into downstream analysis
/// atoms. This is deliberately independent of presentation strings and law
/// identifiers: only asserted, unopposed typed claims are transferable.
pub(crate) fn project_type_facts<'a>(
    claims: impl IntoIterator<Item = (&'a Claim, &'a EvidenceRecord)>,
) -> Result<Vec<ProjectedTypeFact>, TypeProjectionError> {
    let mut grouped = BTreeMap::<(EntityId, ProjectedTypeValue), ProjectedEvidence>::new();
    let mut work_items = 0usize;
    for (claim, evidence) in claims {
        let Some(value) = projected_value(claim) else {
            continue;
        };
        work_items += 1;
        if work_items > MAX_PROJECTED_TYPE_FACTS {
            return Err(TypeProjectionError::EngineLimited);
        }
        let entry = grouped.entry((claim.subject.clone(), value)).or_default();
        match (evidence.polarity, evidence.modality) {
            (EvidencePolarity::Positive, EvidenceModality::Asserted) => {
                entry.positive.push((claim.id.clone(), evidence));
            }
            (EvidencePolarity::Negative, EvidenceModality::Asserted) => {
                entry.has_negative = true;
            }
            _ => {}
        }
    }

    let mut output = Vec::with_capacity(grouped.len());
    for ((entity, value), mut evidence) in grouped {
        if evidence.has_negative || evidence.positive.is_empty() {
            continue;
        }
        evidence
            .positive
            .sort_by(|(left_claim, left), (right_claim, right)| {
                evidence_rank(left)
                    .cmp(&evidence_rank(right))
                    .then(left_claim.cmp(right_claim))
            });
        let (_, selected) = evidence.positive[0];
        output.push(ProjectedTypeFact {
            entity,
            value,
            evidence_id: selected.id.clone(),
        });
    }
    Ok(output)
}

#[derive(Default)]
struct ProjectedEvidence<'a> {
    positive: Vec<(ClaimId, &'a EvidenceRecord)>,
    has_negative: bool,
}

fn evidence_rank(evidence: &EvidenceRecord) -> (u8, usize, &EvidenceId) {
    let origin = match evidence.origin {
        EvidenceOrigin::Explicit => 0,
        EvidenceOrigin::Derived => 1,
    };
    (origin, evidence.parent_claims.len(), &evidence.id)
}

fn projected_value(claim: &Claim) -> Option<ProjectedTypeValue> {
    let ClaimObject::Value(value) = &claim.object else {
        return None;
    };
    match (&claim.predicate, value) {
        (ClaimPredicate::HasRole, ClaimValue::Concept(value) | ClaimValue::Role(value)) => {
            Some(ProjectedTypeValue::Role(value.clone()))
        }
        (ClaimPredicate::HasType, ClaimValue::Type(value)) => {
            Some(ProjectedTypeValue::Type(value.clone()))
        }
        (ClaimPredicate::HasShape, ClaimValue::Shape(value)) => {
            Some(ProjectedTypeValue::Shape(value.clone()))
        }
        (ClaimPredicate::HasDimension, ClaimValue::Dimension(value)) => {
            Some(ProjectedTypeValue::Dimension(value.clone()))
        }
        (ClaimPredicate::HasQuantity, ClaimValue::QuantityKind(value)) => {
            Some(ProjectedTypeValue::Quantity(value.clone()))
        }
        (ClaimPredicate::HasUnit, ClaimValue::Unit(value)) => {
            Some(ProjectedTypeValue::Unit(value.clone()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_index::{ClaimId, InferenceTier, SourceOccurrenceId};

    fn entity(local_id: u32) -> EntityId {
        EntityId {
            component_id: "component".into(),
            scope_path: vec![0],
            kind: "definition".into(),
            anchor: source(local_id),
        }
    }

    fn source(local_id: u32) -> SourceOccurrenceId {
        SourceOccurrenceId {
            file_id: "main".into(),
            document_version: 1,
            local_id,
        }
    }

    fn pair(
        local_id: u32,
        entity: EntityId,
        polarity: EvidencePolarity,
        modality: EvidenceModality,
        origin: EvidenceOrigin,
    ) -> (Claim, EvidenceRecord) {
        let evidence_id = EvidenceId(format!("evidence-{local_id}"));
        (
            Claim {
                id: ClaimId(format!("claim-{local_id}")),
                subject: entity,
                predicate: ClaimPredicate::HasRole,
                object: ClaimObject::Value(ClaimValue::Concept("state-vector".into())),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::Constraint,
                derivation_depth: 1,
            },
            EvidenceRecord {
                id: evidence_id,
                source: source(local_id),
                scope_path: vec![0],
                available_after: u64::from(local_id),
                polarity,
                modality,
                origin,
                provenance: vec![source(local_id)],
                parent_claims: Vec::new(),
                rule_id: "test/typed-fact".into(),
                rule_version: 1,
            },
        )
    }

    #[test]
    fn projects_explicit_and_derived_asserted_type_evidence() {
        let explicit = pair(
            1,
            entity(1),
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
            EvidenceOrigin::Explicit,
        );
        let derived = pair(
            2,
            entity(2),
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
            EvidenceOrigin::Derived,
        );
        let facts =
            project_type_facts([(&explicit.0, &explicit.1), (&derived.0, &derived.1)]).unwrap();

        assert_eq!(facts.len(), 2);
        assert!(facts.iter().any(|fact| fact.entity == entity(1)));
        assert!(facts.iter().any(|fact| fact.entity == entity(2)));
    }

    #[test]
    fn withholds_opposed_and_non_asserted_evidence() {
        let positive = pair(
            1,
            entity(1),
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
            EvidenceOrigin::Explicit,
        );
        let negative = pair(
            2,
            entity(1),
            EvidencePolarity::Negative,
            EvidenceModality::Asserted,
            EvidenceOrigin::Explicit,
        );
        let hedged = pair(
            3,
            entity(2),
            EvidencePolarity::Positive,
            EvidenceModality::Hedged,
            EvidenceOrigin::Explicit,
        );

        assert!(
            project_type_facts([
                (&positive.0, &positive.1),
                (&negative.0, &negative.1),
                (&hedged.0, &hedged.1),
            ])
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn same_value_on_distinct_entities_does_not_conflict() {
        let first = pair(
            1,
            entity(1),
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
            EvidenceOrigin::Explicit,
        );
        let second = pair(
            2,
            entity(2),
            EvidencePolarity::Negative,
            EvidenceModality::Asserted,
            EvidenceOrigin::Explicit,
        );

        assert_eq!(
            project_type_facts([(&first.0, &first.1), (&second.0, &second.1)])
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn rejects_the_whole_projection_past_the_bound() {
        let pairs = (0..=MAX_PROJECTED_TYPE_FACTS as u32)
            .map(|index| {
                let mut pair = pair(
                    index,
                    entity(index),
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                    EvidenceOrigin::Explicit,
                );
                pair.0.object = ClaimObject::Value(ClaimValue::Concept(format!("role-{index}")));
                pair
            })
            .collect::<Vec<_>>();

        assert_eq!(
            project_type_facts(pairs.iter().map(|(claim, evidence)| (claim, evidence))),
            Err(TypeProjectionError::EngineLimited)
        );
    }
}
