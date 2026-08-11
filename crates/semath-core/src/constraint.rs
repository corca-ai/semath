use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::semantic_index::{
    Claim, ClaimCondition, ClaimId, ClaimObject, ClaimOperation, ClaimPredicate, ClaimRelation,
    ClaimShape, ClaimValue, DimensionExponent, EntityId, EvidenceModality, EvidenceOrigin,
    EvidencePolarity, EvidenceRecord,
};

const MAX_DERIVED_FACTS: usize = 50_000;
const MAX_WORK_ITEMS: u32 = 200_000;
const MAX_ROUNDS: usize = 8;

#[derive(Clone)]
pub(crate) struct ConstraintInputClaim {
    pub claim: Claim,
    pub evidence: EvidenceRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlannedDerivation {
    pub subject: EntityId,
    pub predicate: ClaimPredicate,
    pub value: ClaimValue,
    pub parent_claims: Vec<ClaimId>,
    pub provenance: Vec<crate::semantic_index::SourceOccurrenceId>,
    pub available_after: u64,
    pub rule_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConstraintPlan {
    pub derivations: Vec<PlannedDerivation>,
    pub conflicts: Vec<PlannedConflict>,
    pub work_items: u32,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PlannedConflict {
    pub subject: EntityId,
    pub code: String,
    pub summary: String,
    pub parent_claims: Vec<ClaimId>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FactKey {
    subject: EntityId,
    predicate: ClaimPredicate,
    value: ClaimValue,
}

#[derive(Clone, Debug)]
struct Proof {
    parents: BTreeSet<ClaimId>,
    provenance: BTreeSet<crate::semantic_index::SourceOccurrenceId>,
    available_after: u64,
    rule_id: String,
    derived: bool,
}

pub(crate) fn plan_constraint_derivations(input: &[ConstraintInputClaim]) -> ConstraintPlan {
    let mut known = BTreeMap::<FactKey, Proof>::new();
    let mut relations = Vec::<(ClaimId, ClaimRelation, EvidenceRecord)>::new();
    for item in input.iter().filter(|item| establishes(&item.evidence)) {
        let ClaimObject::Value(value) = &item.claim.object else {
            continue;
        };
        if item.claim.predicate == ClaimPredicate::Relates {
            if let ClaimValue::Relation(relation) = value {
                relations.push((
                    item.claim.id.clone(),
                    (**relation).clone(),
                    item.evidence.clone(),
                ));
            }
            continue;
        }
        if transferable(&item.claim.predicate, value) {
            known
                .entry(FactKey {
                    subject: item.claim.subject.clone(),
                    predicate: item.claim.predicate.clone(),
                    value: value.clone(),
                })
                .or_insert_with(|| Proof {
                    parents: BTreeSet::from([item.claim.id.clone()]),
                    provenance: item
                        .evidence
                        .provenance
                        .iter()
                        .cloned()
                        .chain(std::iter::once(item.evidence.source.clone()))
                        .collect(),
                    available_after: item.evidence.available_after,
                    rule_id: "semath/constraint/source".into(),
                    derived: false,
                });
        }
    }
    relations.sort_by(|left, right| left.0.cmp(&right.0));

    let mut work_items = 0u32;
    let mut truncated = false;
    for _ in 0..MAX_ROUNDS {
        let before = known.len();
        close_equalities(&mut known, &relations, &mut work_items, &mut truncated);
        for (relation_id, relation, evidence) in &relations {
            if work_items >= MAX_WORK_ITEMS || known.len() >= MAX_DERIVED_FACTS {
                truncated = true;
                break;
            }
            work_items += 1;
            apply_relation(&mut known, relation_id, relation, evidence, &mut truncated);
        }
        if known.len() == before || truncated {
            break;
        }
    }

    let conflicts = collect_conflicts(&known, &relations);
    let mut derivations = known
        .into_iter()
        .filter_map(|(key, proof)| {
            proof.derived.then(|| PlannedDerivation {
                subject: key.subject,
                predicate: key.predicate,
                value: key.value,
                parent_claims: proof.parents.into_iter().collect(),
                provenance: proof.provenance.into_iter().collect(),
                available_after: proof.available_after,
                rule_id: proof.rule_id,
            })
        })
        .collect::<Vec<_>>();
    derivations.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then(left.predicate.cmp(&right.predicate))
            .then(left.value.cmp(&right.value))
    });
    derivations.truncate(MAX_DERIVED_FACTS);
    ConstraintPlan {
        derivations,
        conflicts,
        work_items,
        truncated,
    }
}

fn collect_conflicts(
    known: &BTreeMap<FactKey, Proof>,
    relations: &[(ClaimId, ClaimRelation, EvidenceRecord)],
) -> Vec<PlannedConflict> {
    let mut conflicts = BTreeSet::new();
    let facts = known.iter().collect::<Vec<_>>();
    for (position, (left, left_proof)) in facts.iter().enumerate() {
        for (right, right_proof) in facts.iter().skip(position + 1) {
            if left.subject != right.subject
                || left.predicate != right.predicate
                || !values_conflict(&left.value, &right.value)
            {
                continue;
            }
            let mut parents = left_proof
                .parents
                .iter()
                .chain(&right_proof.parents)
                .cloned()
                .collect::<Vec<_>>();
            parents.sort();
            parents.dedup();
            conflicts.insert(PlannedConflict {
                subject: left.subject.clone(),
                code: match left.predicate {
                    ClaimPredicate::HasShape => "constraint-shape-conflict",
                    ClaimPredicate::HasDimension
                        if left_proof.rule_id == "semath/constraint/sum-compatibility"
                            || right_proof.rule_id == "semath/constraint/sum-compatibility" =>
                    {
                        "quantity-addition-dimension-mismatch"
                    }
                    ClaimPredicate::HasDimension
                        if left_proof.rule_id == "semath/constraint/equality"
                            || right_proof.rule_id == "semath/constraint/equality" =>
                    {
                        "quantity-assignment-dimension-mismatch"
                    }
                    ClaimPredicate::HasDimension => "constraint-dimension-conflict",
                    _ => continue,
                }
                .into(),
                summary: format!("{:?} conflicts with {:?}", left.value, right.value),
                parent_claims: parents,
            });
        }
    }
    for (relation_id, relation, _) in relations {
        let ClaimRelation::Product {
            result, factors, ..
        } = relation
        else {
            continue;
        };
        let Some(factor_shapes) = factors
            .iter()
            .map(|factor| first_fact(known, factor, &ClaimPredicate::HasShape))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let shapes = factor_shapes
            .iter()
            .filter_map(|(value, _)| match value {
                ClaimValue::Shape(shape) => Some(shape.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !product_is_demonstrably_incompatible(&shapes) {
            continue;
        }
        let mut parents = factor_shapes
            .iter()
            .flat_map(|(_, proof)| proof.parents.iter().cloned())
            .chain(std::iter::once(relation_id.clone()))
            .collect::<Vec<_>>();
        parents.sort();
        parents.dedup();
        conflicts.insert(PlannedConflict {
            subject: result.clone(),
            code: "constraint-product-shape-conflict".into(),
            summary: "Product operands have incompatible proven inner dimensions".into(),
            parent_claims: parents,
        });
    }
    conflicts.into_iter().collect()
}

fn values_conflict(left: &ClaimValue, right: &ClaimValue) -> bool {
    match (left, right) {
        (ClaimValue::Shape(left), ClaimValue::Shape(right)) => shapes_conflict(left, right),
        (ClaimValue::Dimension(left), ClaimValue::Dimension(right)) => left != right,
        _ => false,
    }
}

fn shapes_conflict(left: &ClaimShape, right: &ClaimShape) -> bool {
    match (left, right) {
        (ClaimShape::Unknown, _) | (_, ClaimShape::Unknown) => false,
        (ClaimShape::Scalar, ClaimShape::Scalar) => false,
        (ClaimShape::Vector(left), ClaimShape::Vector(right))
        | (ClaimShape::Matrix(left), ClaimShape::Matrix(right))
        | (ClaimShape::Tensor(left), ClaimShape::Tensor(right)) => {
            left.len() != right.len()
                || left
                    .iter()
                    .zip(right)
                    .any(|(left, right)| demonstrably_incompatible_extent(left, right))
        }
        (
            ClaimShape::Function {
                domain: left_domain,
                codomain: left_codomain,
            },
            ClaimShape::Function {
                domain: right_domain,
                codomain: right_codomain,
            },
        ) => {
            shapes_conflict(left_domain, right_domain)
                || shapes_conflict(left_codomain, right_codomain)
        }
        _ => true,
    }
}

fn product_is_demonstrably_incompatible(shapes: &[ClaimShape]) -> bool {
    shapes.windows(2).any(|pair| match (&pair[0], &pair[1]) {
        (ClaimShape::Matrix(left), ClaimShape::Vector(right))
            if left.len() == 2 && right.len() == 1 =>
        {
            demonstrably_incompatible_extent(&left[1], &right[0])
        }
        (ClaimShape::Matrix(left), ClaimShape::Matrix(right))
            if left.len() == 2 && right.len() == 2 =>
        {
            demonstrably_incompatible_extent(&left[1], &right[0])
        }
        _ => false,
    })
}

fn establishes(evidence: &EvidenceRecord) -> bool {
    evidence.origin == EvidenceOrigin::Explicit
        && evidence.polarity == EvidencePolarity::Positive
        && evidence.modality == EvidenceModality::Asserted
}

fn transferable(predicate: &ClaimPredicate, value: &ClaimValue) -> bool {
    matches!(
        (predicate, value),
        (
            ClaimPredicate::HasRole,
            ClaimValue::Concept(_) | ClaimValue::Role(_)
        ) | (ClaimPredicate::HasType, ClaimValue::Type(_))
            | (ClaimPredicate::HasShape, ClaimValue::Shape(_))
            | (ClaimPredicate::HasDimension, ClaimValue::Dimension(_))
            | (ClaimPredicate::HasQuantity, ClaimValue::QuantityKind(_))
            | (ClaimPredicate::HasUnit, ClaimValue::Unit(_))
            | (ClaimPredicate::Assumes, ClaimValue::Condition(_))
    )
}

fn close_equalities(
    known: &mut BTreeMap<FactKey, Proof>,
    relations: &[(ClaimId, ClaimRelation, EvidenceRecord)],
    work_items: &mut u32,
    truncated: &mut bool,
) {
    let mut adjacency = BTreeMap::<EntityId, Vec<(EntityId, ClaimId, EvidenceRecord)>>::new();
    for (claim_id, relation, evidence) in relations {
        if let ClaimRelation::Equality { left, right, .. } = relation {
            adjacency.entry(left.clone()).or_default().push((
                right.clone(),
                claim_id.clone(),
                evidence.clone(),
            ));
            adjacency.entry(right.clone()).or_default().push((
                left.clone(),
                claim_id.clone(),
                evidence.clone(),
            ));
        }
    }
    let mut queue = known.keys().cloned().collect::<VecDeque<_>>();
    while let Some(fact) = queue.pop_front() {
        if *work_items >= MAX_WORK_ITEMS || known.len() >= MAX_DERIVED_FACTS {
            *truncated = true;
            return;
        }
        *work_items += 1;
        let Some(proof) = known.get(&fact).cloned() else {
            continue;
        };
        for (target, relation_id, evidence) in adjacency.get(&fact.subject).into_iter().flatten() {
            let key = FactKey {
                subject: target.clone(),
                predicate: fact.predicate.clone(),
                value: retarget_value(&fact.value, target),
            };
            if known.contains_key(&key) {
                continue;
            }
            known.insert(
                key.clone(),
                extend_proof(
                    std::slice::from_ref(&proof),
                    relation_id,
                    evidence,
                    "semath/constraint/equality",
                ),
            );
            queue.push_back(key);
        }
    }
}

fn apply_relation(
    known: &mut BTreeMap<FactKey, Proof>,
    relation_id: &ClaimId,
    relation: &ClaimRelation,
    evidence: &EvidenceRecord,
    truncated: &mut bool,
) {
    match relation {
        ClaimRelation::Equality { .. } => {}
        ClaimRelation::Sum { result, terms, .. } => {
            let participants = std::iter::once(result)
                .chain(terms)
                .cloned()
                .collect::<Vec<_>>();
            for predicate in [
                ClaimPredicate::HasShape,
                ClaimPredicate::HasDimension,
                ClaimPredicate::HasUnit,
            ] {
                let Some((value, proof)) = participants
                    .iter()
                    .find_map(|entity| first_fact(known, entity, &predicate))
                else {
                    continue;
                };
                for entity in &participants {
                    insert_derived(
                        known,
                        FactKey {
                            subject: entity.clone(),
                            predicate: predicate.clone(),
                            value: value.clone(),
                        },
                        std::slice::from_ref(&proof),
                        relation_id,
                        evidence,
                        "semath/constraint/sum-compatibility",
                        truncated,
                    );
                }
            }
        }
        ClaimRelation::Product {
            result, factors, ..
        } => apply_product(known, result, factors, relation_id, evidence, truncated),
        ClaimRelation::Quotient {
            result,
            numerator,
            denominator,
            ..
        } => apply_quotient(
            known,
            result,
            numerator,
            denominator,
            relation_id,
            evidence,
            truncated,
        ),
        ClaimRelation::Operation {
            result,
            operator,
            operands,
            ..
        } => apply_operation(
            known,
            result,
            *operator,
            operands,
            relation_id,
            evidence,
            truncated,
        ),
        ClaimRelation::Application {
            result,
            function,
            arguments,
            ..
        } => {
            if let Some((ClaimValue::Shape(ClaimShape::Function { domain, codomain }), proof)) =
                first_fact(known, function, &ClaimPredicate::HasShape)
            {
                insert_derived(
                    known,
                    FactKey {
                        subject: result.clone(),
                        predicate: ClaimPredicate::HasShape,
                        value: ClaimValue::Shape(*codomain.clone()),
                    },
                    std::slice::from_ref(&proof),
                    relation_id,
                    evidence,
                    "semath/constraint/application-codomain",
                    truncated,
                );
                if arguments.len() == 1 {
                    insert_derived(
                        known,
                        FactKey {
                            subject: arguments[0].clone(),
                            predicate: ClaimPredicate::HasShape,
                            value: ClaimValue::Shape(*domain.clone()),
                        },
                        std::slice::from_ref(&proof),
                        relation_id,
                        evidence,
                        "semath/constraint/application-domain",
                        truncated,
                    );
                }
            }
        }
        ClaimRelation::Derivative {
            result,
            operand,
            variable,
            ..
        } => apply_calculus_relation(
            known,
            result,
            operand,
            variable.as_ref(),
            -1,
            relation_id,
            evidence,
            "semath/constraint/derivative",
            truncated,
        ),
        ClaimRelation::Integral {
            result,
            integrand,
            variable,
            ..
        } => apply_calculus_relation(
            known,
            result,
            integrand,
            variable.as_ref(),
            1,
            relation_id,
            evidence,
            "semath/constraint/integral",
            truncated,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_quotient(
    known: &mut BTreeMap<FactKey, Proof>,
    result: &EntityId,
    numerator: &EntityId,
    denominator: &EntityId,
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    truncated: &mut bool,
) {
    if let (Some((numerator_shape, numerator_proof)), Some((denominator_shape, denominator_proof))) = (
        first_fact(known, numerator, &ClaimPredicate::HasShape),
        first_fact(known, denominator, &ClaimPredicate::HasShape),
    ) && denominator_shape == ClaimValue::Shape(ClaimShape::Scalar)
    {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasShape,
                value: numerator_shape,
            },
            &[numerator_proof, denominator_proof],
            relation_id,
            evidence,
            "semath/constraint/quotient-shape",
            truncated,
        );
    }
    if let (
        Some((ClaimValue::Dimension(numerator_dimension), numerator_proof)),
        Some((ClaimValue::Dimension(denominator_dimension), denominator_proof)),
    ) = (
        first_fact(known, numerator, &ClaimPredicate::HasDimension),
        first_fact(known, denominator, &ClaimPredicate::HasDimension),
    ) && let Some(combined) = combine_dimensions(
        &[
            numerator_dimension.as_slice(),
            denominator_dimension.as_slice(),
        ],
        -1,
    ) {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasDimension,
                value: ClaimValue::Dimension(combined),
            },
            &[numerator_proof, denominator_proof],
            relation_id,
            evidence,
            "semath/constraint/quotient-dimension",
            truncated,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_operation(
    known: &mut BTreeMap<FactKey, Proof>,
    result: &EntityId,
    operator: ClaimOperation,
    operands: &[EntityId],
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    truncated: &mut bool,
) {
    let Some(operand) = operands.first() else {
        return;
    };
    let shape = first_fact(known, operand, &ClaimPredicate::HasShape);
    let derived_shape = match (operator, shape.as_ref().map(|(value, _)| value)) {
        (ClaimOperation::Negate, Some(value)) => Some(value.clone()),
        (ClaimOperation::Transpose, Some(ClaimValue::Shape(ClaimShape::Matrix(dimensions))))
            if dimensions.len() == 2 =>
        {
            Some(ClaimValue::Shape(ClaimShape::Matrix(vec![
                dimensions[1].clone(),
                dimensions[0].clone(),
            ])))
        }
        (ClaimOperation::Dot, Some(ClaimValue::Shape(ClaimShape::Vector(_))))
            if operands.len() == 2 =>
        {
            Some(ClaimValue::Shape(ClaimShape::Scalar))
        }
        (ClaimOperation::Cross, Some(ClaimValue::Shape(ClaimShape::Vector(dimensions))))
            if operands.len() == 2 =>
        {
            Some(ClaimValue::Shape(ClaimShape::Vector(dimensions.clone())))
        }
        _ => None,
    };
    if let (Some(value), Some((_, proof))) = (derived_shape, shape) {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasShape,
                value,
            },
            &[proof],
            relation_id,
            evidence,
            "semath/constraint/operation-shape",
            truncated,
        );
    }
    if matches!(operator, ClaimOperation::Negate | ClaimOperation::Transpose)
        && let Some((dimension, proof)) = first_fact(known, operand, &ClaimPredicate::HasDimension)
    {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasDimension,
                value: dimension,
            },
            &[proof],
            relation_id,
            evidence,
            "semath/constraint/operation-dimension",
            truncated,
        );
    }
    if matches!(operator, ClaimOperation::Dot | ClaimOperation::Cross) {
        let dimensions = operands
            .iter()
            .map(|operand| first_fact(known, operand, &ClaimPredicate::HasDimension))
            .collect::<Option<Vec<_>>>();
        if let Some(dimensions) = dimensions {
            let exponents = dimensions
                .iter()
                .filter_map(|(value, _)| match value {
                    ClaimValue::Dimension(exponents) => Some(exponents.as_slice()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if exponents.len() == dimensions.len()
                && let Some(combined) = combine_dimensions(&exponents, 1)
            {
                let proofs = dimensions
                    .iter()
                    .map(|(_, proof)| proof.clone())
                    .collect::<Vec<_>>();
                insert_derived(
                    known,
                    FactKey {
                        subject: result.clone(),
                        predicate: ClaimPredicate::HasDimension,
                        value: ClaimValue::Dimension(combined),
                    },
                    &proofs,
                    relation_id,
                    evidence,
                    "semath/constraint/vector-product-dimension",
                    truncated,
                );
            }
        }
    }
}

fn apply_product(
    known: &mut BTreeMap<FactKey, Proof>,
    result: &EntityId,
    factors: &[EntityId],
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    truncated: &mut bool,
) {
    let shapes = factors
        .iter()
        .map(|factor| first_fact(known, factor, &ClaimPredicate::HasShape))
        .collect::<Option<Vec<_>>>();
    if let Some(shapes) = shapes
        && let Some(shape) = product_shape(
            &shapes
                .iter()
                .filter_map(|(value, _)| match value {
                    ClaimValue::Shape(shape) => Some(shape.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )
    {
        let proofs = shapes
            .iter()
            .map(|(_, proof)| proof.clone())
            .collect::<Vec<_>>();
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasShape,
                value: ClaimValue::Shape(shape),
            },
            &proofs,
            relation_id,
            evidence,
            "semath/constraint/product-shape",
            truncated,
        );
    }
    let dimensions = factors
        .iter()
        .map(|factor| first_fact(known, factor, &ClaimPredicate::HasDimension))
        .collect::<Option<Vec<_>>>();
    if let Some(dimensions) = dimensions {
        let exponents = dimensions
            .iter()
            .filter_map(|(value, _)| match value {
                ClaimValue::Dimension(exponents) => Some(exponents.as_slice()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if exponents.len() == dimensions.len()
            && let Some(combined) = combine_dimensions(&exponents, 1)
        {
            let proofs = dimensions
                .iter()
                .map(|(_, proof)| proof.clone())
                .collect::<Vec<_>>();
            insert_derived(
                known,
                FactKey {
                    subject: result.clone(),
                    predicate: ClaimPredicate::HasDimension,
                    value: ClaimValue::Dimension(combined),
                },
                &proofs,
                relation_id,
                evidence,
                "semath/constraint/product-dimension",
                truncated,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_calculus_relation(
    known: &mut BTreeMap<FactKey, Proof>,
    result: &EntityId,
    operand: &EntityId,
    variable: Option<&EntityId>,
    variable_sign: i16,
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    rule_id: &str,
    truncated: &mut bool,
) {
    if let Some((shape, proof)) = first_fact(known, operand, &ClaimPredicate::HasShape) {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasShape,
                value: shape,
            },
            &[proof],
            relation_id,
            evidence,
            rule_id,
            truncated,
        );
    }
    let Some(variable) = variable else {
        return;
    };
    let Some((ClaimValue::Dimension(operand_dimension), operand_proof)) =
        first_fact(known, operand, &ClaimPredicate::HasDimension)
    else {
        return;
    };
    let Some((ClaimValue::Dimension(variable_dimension), variable_proof)) =
        first_fact(known, variable, &ClaimPredicate::HasDimension)
    else {
        return;
    };
    if let Some(combined) = combine_dimensions(
        &[operand_dimension.as_slice(), variable_dimension.as_slice()],
        variable_sign,
    ) {
        insert_derived(
            known,
            FactKey {
                subject: result.clone(),
                predicate: ClaimPredicate::HasDimension,
                value: ClaimValue::Dimension(combined),
            },
            &[operand_proof, variable_proof],
            relation_id,
            evidence,
            rule_id,
            truncated,
        );
    }
}

fn first_fact(
    known: &BTreeMap<FactKey, Proof>,
    entity: &EntityId,
    predicate: &ClaimPredicate,
) -> Option<(ClaimValue, Proof)> {
    known
        .iter()
        .find(|(key, _)| &key.subject == entity && &key.predicate == predicate)
        .map(|(key, proof)| (key.value.clone(), proof.clone()))
}

fn insert_derived(
    known: &mut BTreeMap<FactKey, Proof>,
    key: FactKey,
    proofs: &[Proof],
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    rule_id: &str,
    truncated: &mut bool,
) {
    if known.contains_key(&key) {
        return;
    }
    if known.len() >= MAX_DERIVED_FACTS {
        *truncated = true;
        return;
    }
    known.insert(key, extend_proof(proofs, relation_id, evidence, rule_id));
}

fn extend_proof(
    proofs: &[Proof],
    relation_id: &ClaimId,
    evidence: &EvidenceRecord,
    rule_id: &str,
) -> Proof {
    let mut parents = BTreeSet::from([relation_id.clone()]);
    let mut provenance = evidence
        .provenance
        .iter()
        .cloned()
        .chain(std::iter::once(evidence.source.clone()))
        .collect::<BTreeSet<_>>();
    let mut available_after = evidence.available_after;
    for proof in proofs {
        parents.extend(proof.parents.iter().cloned());
        provenance.extend(proof.provenance.iter().cloned());
        available_after = available_after.max(proof.available_after);
    }
    Proof {
        parents,
        provenance,
        available_after,
        rule_id: rule_id.into(),
        derived: true,
    }
}

fn retarget_value(value: &ClaimValue, target: &EntityId) -> ClaimValue {
    match value {
        ClaimValue::Condition(condition) => ClaimValue::Condition(match condition {
            ClaimCondition::Nonzero(_) => ClaimCondition::Nonzero(target.clone()),
            ClaimCondition::Positive(_) => ClaimCondition::Positive(target.clone()),
            ClaimCondition::Nonnegative(_) => ClaimCondition::Nonnegative(target.clone()),
            ClaimCondition::Invertible(_) => ClaimCondition::Invertible(target.clone()),
            ClaimCondition::Member { set, .. } => ClaimCondition::Member {
                entity: target.clone(),
                set: set.clone(),
            },
            ClaimCondition::Named(value) => ClaimCondition::Named(value.clone()),
        }),
        other => other.clone(),
    }
}

fn product_shape(shapes: &[ClaimShape]) -> Option<ClaimShape> {
    let mut result = ClaimShape::Scalar;
    for shape in shapes {
        result = match (&result, shape) {
            (ClaimShape::Scalar, other) | (other, ClaimShape::Scalar) => other.clone(),
            (ClaimShape::Matrix(left), ClaimShape::Vector(right))
                if left.len() == 2 && right.len() == 1 && compatible(&left[1], &right[0]) =>
            {
                ClaimShape::Vector(vec![left[0].clone()])
            }
            (ClaimShape::Matrix(left), ClaimShape::Matrix(right))
                if left.len() == 2 && right.len() == 2 && compatible(&left[1], &right[0]) =>
            {
                ClaimShape::Matrix(vec![left[0].clone(), right[1].clone()])
            }
            _ => return None,
        };
    }
    Some(result)
}

fn compatible(left: &str, right: &str) -> bool {
    left == right || left == "?" || right == "?"
}

fn demonstrably_incompatible_extent(left: &str, right: &str) -> bool {
    left != right && left.parse::<u64>().is_ok() && right.parse::<u64>().is_ok()
}

fn combine_dimensions(
    dimensions: &[&[DimensionExponent]],
    final_sign: i16,
) -> Option<Vec<DimensionExponent>> {
    let mut combined = BTreeMap::<String, (i32, u32)>::new();
    for (index, dimension) in dimensions.iter().enumerate() {
        let sign = if index + 1 == dimensions.len() {
            final_sign as i32
        } else {
            1
        };
        for exponent in *dimension {
            let entry = combined.entry(exponent.base.clone()).or_insert((0, 1));
            let denominator = lcm(entry.1, exponent.denominator as u32)?;
            entry.0 = entry.0.checked_mul((denominator / entry.1) as i32)?
                + sign
                    * i32::from(exponent.numerator)
                    * (denominator / exponent.denominator as u32) as i32;
            entry.1 = denominator;
            let divisor = gcd(entry.0.unsigned_abs(), entry.1);
            entry.0 /= divisor as i32;
            entry.1 /= divisor;
        }
    }
    combined
        .into_iter()
        .filter(|(_, (numerator, _))| *numerator != 0)
        .map(|(base, (numerator, denominator))| {
            Some(DimensionExponent {
                base,
                numerator: i16::try_from(numerator).ok()?,
                denominator: u16::try_from(denominator).ok()?,
            })
        })
        .collect()
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

fn lcm(left: u32, right: u32) -> Option<u32> {
    left.checked_div(gcd(left, right))?.checked_mul(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceRange;
    use crate::semantic_index::{
        EvidenceId, InferenceTier, OccurrenceKind, SourceOccurrence, SourceOccurrenceId,
    };

    fn occurrence(local_id: u32) -> SourceOccurrence {
        SourceOccurrence {
            id: SourceOccurrenceId {
                file_id: "main.tex".into(),
                document_version: 1,
                local_id,
            },
            component_id: "component".into(),
            kind: OccurrenceKind::Notation,
            range: SourceRange {
                start_offset: local_id * 2,
                end_offset: local_id * 2 + 1,
            },
            selection_range: SourceRange {
                start_offset: local_id * 2,
                end_offset: local_id * 2 + 1,
            },
            scope_path: Vec::new(),
            structural_path: Vec::new(),
            availability_order: u64::from(local_id),
            surface: format!("x{local_id}"),
            source_text: format!("x{local_id}"),
            notation: Vec::new(),
        }
    }

    fn entity(local_id: u32) -> EntityId {
        let occurrence = occurrence(local_id);
        EntityId {
            component_id: occurrence.component_id,
            scope_path: Vec::new(),
            kind: format!("entity-{local_id}"),
            anchor: occurrence.id,
        }
    }

    fn input_claim(
        id: &str,
        subject: EntityId,
        predicate: ClaimPredicate,
        value: ClaimValue,
    ) -> ConstraintInputClaim {
        let source = subject.anchor.clone();
        let evidence_id = EvidenceId(format!("evidence-{id}"));
        ConstraintInputClaim {
            claim: Claim {
                id: ClaimId(id.into()),
                subject: subject.clone(),
                predicate,
                object: ClaimObject::Value(value),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            },
            evidence: EvidenceRecord {
                id: evidence_id,
                source: source.clone(),
                scope_path: subject.scope_path,
                available_after: u64::from(source.local_id),
                polarity: EvidencePolarity::Positive,
                modality: EvidenceModality::Asserted,
                origin: EvidenceOrigin::Explicit,
                provenance: vec![source],
                parent_claims: Vec::new(),
                rule_id: "test-source".into(),
                rule_version: 1,
            },
        }
    }

    fn equality(id: &str, left: EntityId, right: EntityId) -> ConstraintInputClaim {
        input_claim(
            id,
            left.clone(),
            ClaimPredicate::Relates,
            ClaimValue::Relation(Box::new(ClaimRelation::Equality {
                left,
                right,
                canonical_digest: id.into(),
            })),
        )
    }

    #[test]
    fn equality_closure_is_order_independent_idempotent_and_cycle_safe() {
        let (x, y, z) = (entity(1), entity(2), entity(3));
        let input = vec![
            input_claim(
                "shape-x",
                x.clone(),
                ClaimPredicate::HasShape,
                ClaimValue::Shape(ClaimShape::Vector(vec!["n".into()])),
            ),
            equality("x-y", x.clone(), y.clone()),
            equality("y-z", y.clone(), z.clone()),
            equality("z-x", z.clone(), x.clone()),
        ];
        let plan = plan_constraint_derivations(&input);
        let mut reversed = input.clone();
        reversed.reverse();
        assert_eq!(plan, plan_constraint_derivations(&reversed));
        assert_eq!(plan, plan_constraint_derivations(&input));
        assert!(!plan.truncated);
        assert!(plan.derivations.iter().any(|derived| {
            derived.subject == z
                && derived.predicate == ClaimPredicate::HasShape
                && derived.parent_claims.contains(&ClaimId("shape-x".into()))
                && derived.parent_claims.len() <= 3
        }));
    }

    #[test]
    fn product_derives_shape_and_physical_dimension_from_typed_factors() {
        let (matrix, vector, result) = (entity(1), entity(2), entity(3));
        let relation = input_claim(
            "product",
            result.clone(),
            ClaimPredicate::Relates,
            ClaimValue::Relation(Box::new(ClaimRelation::Product {
                result: result.clone(),
                factors: vec![matrix.clone(), vector.clone()],
                canonical_digest: "product(A,x)".into(),
            })),
        );
        let input = vec![
            input_claim(
                "matrix-shape",
                matrix.clone(),
                ClaimPredicate::HasShape,
                ClaimValue::Shape(ClaimShape::Matrix(vec!["m".into(), "n".into()])),
            ),
            input_claim(
                "vector-shape",
                vector.clone(),
                ClaimPredicate::HasShape,
                ClaimValue::Shape(ClaimShape::Vector(vec!["n".into()])),
            ),
            input_claim(
                "matrix-dimension",
                matrix,
                ClaimPredicate::HasDimension,
                ClaimValue::Dimension(vec![DimensionExponent {
                    base: "mass".into(),
                    numerator: 1,
                    denominator: 1,
                }]),
            ),
            input_claim(
                "vector-dimension",
                vector,
                ClaimPredicate::HasDimension,
                ClaimValue::Dimension(vec![DimensionExponent {
                    base: "time".into(),
                    numerator: -1,
                    denominator: 1,
                }]),
            ),
            relation,
        ];
        let plan = plan_constraint_derivations(&input);
        assert!(plan.derivations.iter().any(|derived| {
            derived.subject == result
                && derived.value == ClaimValue::Shape(ClaimShape::Vector(vec!["m".into()]))
        }));
        assert!(plan.derivations.iter().any(|derived| {
            derived.subject == result
                && derived.value
                    == ClaimValue::Dimension(vec![
                        DimensionExponent {
                            base: "mass".into(),
                            numerator: 1,
                            denominator: 1,
                        },
                        DimensionExponent {
                            base: "time".into(),
                            numerator: -1,
                            denominator: 1,
                        },
                    ])
        }));
    }

    #[test]
    fn equality_retargets_guards_and_transfers_quantity_and_unit_facts() {
        let (x, y) = (entity(1), entity(2));
        let plan = plan_constraint_derivations(&[
            input_claim(
                "quantity-x",
                x.clone(),
                ClaimPredicate::HasQuantity,
                ClaimValue::QuantityKind("electric-potential".into()),
            ),
            input_claim(
                "unit-x",
                x.clone(),
                ClaimPredicate::HasUnit,
                ClaimValue::Unit("volt".into()),
            ),
            input_claim(
                "nonzero-x",
                x.clone(),
                ClaimPredicate::Assumes,
                ClaimValue::Condition(ClaimCondition::Nonzero(x.clone())),
            ),
            equality("x-y", x, y.clone()),
        ]);
        for (predicate, value) in [
            (
                ClaimPredicate::HasQuantity,
                ClaimValue::QuantityKind("electric-potential".into()),
            ),
            (ClaimPredicate::HasUnit, ClaimValue::Unit("volt".into())),
            (
                ClaimPredicate::Assumes,
                ClaimValue::Condition(ClaimCondition::Nonzero(y.clone())),
            ),
        ] {
            assert!(plan.derivations.iter().any(|derived| {
                derived.subject == y && derived.predicate == predicate && derived.value == value
            }));
        }
    }

    #[test]
    fn application_and_calculus_use_typed_domains_codomain_and_dimensions() {
        let (function, argument, applied, derivative, variable) =
            (entity(1), entity(2), entity(3), entity(4), entity(5));
        let plan = plan_constraint_derivations(&[
            input_claim(
                "function-shape",
                function.clone(),
                ClaimPredicate::HasShape,
                ClaimValue::Shape(ClaimShape::Function {
                    domain: Box::new(ClaimShape::Vector(vec!["n".into()])),
                    codomain: Box::new(ClaimShape::Scalar),
                }),
            ),
            input_claim(
                "applied-dimension",
                applied.clone(),
                ClaimPredicate::HasDimension,
                ClaimValue::Dimension(vec![DimensionExponent {
                    base: "length".into(),
                    numerator: 1,
                    denominator: 1,
                }]),
            ),
            input_claim(
                "variable-dimension",
                variable.clone(),
                ClaimPredicate::HasDimension,
                ClaimValue::Dimension(vec![DimensionExponent {
                    base: "time".into(),
                    numerator: 1,
                    denominator: 1,
                }]),
            ),
            input_claim(
                "application",
                applied.clone(),
                ClaimPredicate::Relates,
                ClaimValue::Relation(Box::new(ClaimRelation::Application {
                    result: applied.clone(),
                    function,
                    arguments: vec![argument.clone()],
                    canonical_digest: "application".into(),
                })),
            ),
            input_claim(
                "derivative",
                derivative.clone(),
                ClaimPredicate::Relates,
                ClaimValue::Relation(Box::new(ClaimRelation::Derivative {
                    result: derivative.clone(),
                    operand: applied,
                    variable: Some(variable),
                    order: 1,
                    canonical_digest: "derivative".into(),
                })),
            ),
        ]);
        assert!(plan.derivations.iter().any(|derived| {
            derived.subject == argument
                && derived.value == ClaimValue::Shape(ClaimShape::Vector(vec!["n".into()]))
        }));
        assert!(plan.derivations.iter().any(|derived| {
            derived.subject == derivative
                && derived.value
                    == ClaimValue::Dimension(vec![
                        DimensionExponent {
                            base: "length".into(),
                            numerator: 1,
                            denominator: 1,
                        },
                        DimensionExponent {
                            base: "time".into(),
                            numerator: -1,
                            denominator: 1,
                        },
                    ])
        }));
    }

    #[test]
    fn bounded_exhaustive_oracle_agrees_on_equality_reachability() {
        let entities = (0..4).map(entity).collect::<Vec<_>>();
        let edges = [(0usize, 1usize), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        for edge_mask in 0u32..(1 << edges.len()) {
            for seed in 0..entities.len() {
                let mut input = vec![input_claim(
                    "seed",
                    entities[seed].clone(),
                    ClaimPredicate::HasType,
                    ClaimValue::Type("real".into()),
                )];
                let mut reachable = BTreeSet::from([seed]);
                for (edge_index, (left, right)) in edges.iter().enumerate() {
                    if edge_mask & (1 << edge_index) != 0 {
                        input.push(equality(
                            &format!("edge-{edge_index}"),
                            entities[*left].clone(),
                            entities[*right].clone(),
                        ));
                    }
                }
                loop {
                    let before = reachable.len();
                    for (edge_index, (left, right)) in edges.iter().enumerate() {
                        if edge_mask & (1 << edge_index) == 0 {
                            continue;
                        }
                        if reachable.contains(left) {
                            reachable.insert(*right);
                        }
                        if reachable.contains(right) {
                            reachable.insert(*left);
                        }
                    }
                    if reachable.len() == before {
                        break;
                    }
                }
                let plan = plan_constraint_derivations(&input);
                let actual = std::iter::once(seed)
                    .chain(plan.derivations.iter().filter_map(|derived| {
                        (derived.predicate == ClaimPredicate::HasType)
                            .then(|| {
                                entities
                                    .iter()
                                    .position(|entity| *entity == derived.subject)
                            })
                            .flatten()
                    }))
                    .collect::<BTreeSet<_>>();
                assert_eq!(actual, reachable, "mask={edge_mask:06b}, seed={seed}");
            }
        }
    }
}
