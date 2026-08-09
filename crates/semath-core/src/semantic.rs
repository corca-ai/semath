use std::collections::BTreeMap;

use crate::consistency::roles_conflict;
use crate::consistency::{RoleObservations, observe_roles};
use crate::domain::{DomainObservations, observe_domains};
use crate::law::{ExternalTypeEnvironment, LawObservations, observe_laws};
use crate::parser::ParsedMath;
use crate::prose::observe_prose;
use crate::quantity::{QuantityObservations, observe_quantities};
use crate::shape::{ShapeObservations, observe_shapes};
use crate::{
    ConceptInfo, DefinitionInfo, Evidence, LawRecognition, ProjectDocument, QuantityInfo,
    RelationInfo, RoleInfo, SemanticClaimInfo, SemanticClaimStatus, SemanticContextInfo,
    SemanticSymbolId, ShapeInfo,
};

const MAX_CONCEPTS: usize = 16;
const MAX_CLAIMS: usize = 32;
const MAX_RELATIONS: usize = 16;

#[derive(Clone, Debug)]
enum SemanticObservation {
    Definition(DefinitionInfo),
    Role(RoleInfo),
    Shape(ShapeInfo),
    Quantity(QuantityInfo),
    Formula(Box<LawRecognition>),
}

#[derive(Clone, Debug, Default)]
struct SemanticClaims {
    observations: Vec<SemanticObservation>,
    relations: Vec<RelationInfo>,
    quantities: Vec<QuantityInfo>,
}

impl SemanticClaims {
    pub fn from_symbol_observations(
        definitions: Vec<DefinitionInfo>,
        roles: Vec<RoleInfo>,
        shapes: Vec<ShapeInfo>,
        formulas: Vec<LawRecognition>,
        relations: Vec<RelationInfo>,
        quantities: Vec<QuantityInfo>,
    ) -> Self {
        let mut observations = Vec::new();
        observations.extend(definitions.into_iter().map(SemanticObservation::Definition));
        observations.extend(roles.into_iter().map(SemanticObservation::Role));
        observations.extend(shapes.into_iter().map(SemanticObservation::Shape));
        observations.extend(
            formulas
                .into_iter()
                .map(Box::new)
                .map(SemanticObservation::Formula),
        );
        observations.extend(
            quantities
                .iter()
                .cloned()
                .map(SemanticObservation::Quantity),
        );
        Self {
            observations,
            relations,
            quantities,
        }
    }

    pub fn context(
        &self,
        symbol: Option<String>,
        semantic_id: Option<SemanticSymbolId>,
    ) -> SemanticContextInfo {
        let mut concepts = self.concepts();
        let concepts_truncated = concepts.len() > MAX_CONCEPTS;
        concepts.truncate(MAX_CONCEPTS);

        let mut claims = self.claims();
        let claims_truncated = claims.len() > MAX_CLAIMS;
        claims.truncate(MAX_CLAIMS);

        let mut relations = self.relations.clone();
        relations.sort_by_key(|relation| {
            (
                relation.range.start_offset,
                relation.range.end_offset,
                relation.relation_id.clone(),
            )
        });
        relations.dedup_by(|left, right| {
            left.relation_id == right.relation_id && left.range == right.range
        });
        let relations_truncated = relations.len() > MAX_RELATIONS;
        relations.truncate(MAX_RELATIONS);

        SemanticContextInfo {
            symbol,
            semantic_id,
            concepts,
            claims,
            relations,
            quantities: self.quantities.clone(),
            truncated: concepts_truncated || claims_truncated || relations_truncated,
        }
    }

    fn concepts(&self) -> Vec<ConceptInfo> {
        let mut concepts = BTreeMap::<String, ConceptInfo>::new();
        for observation in &self.observations {
            match observation {
                SemanticObservation::Role(role) => {
                    let concept_id = role.concept_id.clone();
                    concepts
                        .entry(concept_id.clone())
                        .or_insert_with(|| ConceptInfo {
                            concept_id,
                            label: role_label(&role.concept_id),
                            description: role.description.clone(),
                            evidence: role.evidence.clone(),
                        });
                }
                SemanticObservation::Quantity(quantity) => {
                    let (Some(concept_id), Some(label)) =
                        (&quantity.quantity_kind_id, &quantity.quantity_kind)
                    else {
                        continue;
                    };
                    concepts
                        .entry(concept_id.clone())
                        .or_insert_with(|| ConceptInfo {
                            concept_id: concept_id.clone(),
                            label: label.clone(),
                            description: quantity.display.clone(),
                            evidence: quantity.evidence.clone(),
                        });
                }
                _ => {}
            }
        }
        concepts.into_values().collect()
    }

    fn claims(&self) -> Vec<SemanticClaimInfo> {
        let mut claims = self
            .observations
            .iter()
            .map(claim_from_observation)
            .collect::<Vec<_>>();
        mark_concept_conflicts(&mut claims);
        claims.sort_by(|left, right| {
            left.predicate
                .cmp(&right.predicate)
                .then(left.value.cmp(&right.value))
                .then(left.claim_id.cmp(&right.claim_id))
        });
        claims.dedup_by(|left, right| left.claim_id == right.claim_id);
        claims
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticFactStore {
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: ShapeObservations,
    pub quantities: QuantityObservations,
    pub roles: RoleObservations,
    pub laws: LawObservations,
    pub domains: DomainObservations,
}

impl SemanticFactStore {
    pub fn build(document: &ProjectDocument, parsed: &[ParsedMath]) -> Self {
        let prose = observe_prose(document, parsed);
        let shapes = observe_shapes(document, parsed, &prose.shapes);
        let quantities = observe_quantities(document, parsed, &prose.definitions);
        let roles = observe_roles(document, &prose.definitions, &shapes);
        let laws = observe_laws(
            document,
            parsed,
            &shapes,
            &quantities,
            &roles,
            &ExternalTypeEnvironment::default(),
        );
        let domains = observe_domains(document, laws.all());
        Self {
            definitions: prose.definitions,
            shapes,
            quantities,
            roles,
            laws,
            domains,
        }
    }

    pub fn refresh_laws(
        &mut self,
        document: &ProjectDocument,
        parsed: &[ParsedMath],
        external: &ExternalTypeEnvironment,
    ) {
        self.laws = observe_laws(
            document,
            parsed,
            &self.shapes,
            &self.quantities,
            &self.roles,
            external,
        );
        self.domains = observe_domains(document, self.laws.all());
    }

    pub fn context(
        &self,
        definitions: Vec<DefinitionInfo>,
        symbol: Option<String>,
        semantic_id: Option<SemanticSymbolId>,
        offset: u32,
        external: Option<&ExternalTypeEnvironment>,
    ) -> SemanticContextInfo {
        let mut roles = symbol
            .as_deref()
            .map(|name| self.roles.roles_at(name, offset).0)
            .unwrap_or_default();
        let mut shapes = symbol
            .as_deref()
            .map(|name| self.shapes.claims_at(name, offset).0)
            .unwrap_or_default();
        let mut quantities = symbol
            .as_deref()
            .map(|name| self.quantities.at(name, offset).0)
            .unwrap_or_default();
        if let (Some(name), Some(external)) = (symbol.as_deref(), external) {
            roles.extend(external.roles_at(offset, name));
            shapes.extend(external.shapes_at(offset, name));
            quantities.extend(external.quantities_at(offset, name));
            roles.sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
            roles.dedup();
            shapes.sort_by(|left, right| left.kind.cmp(&right.kind));
            shapes.dedup();
            quantities.sort_by(|left, right| left.display.cmp(&right.display));
            quantities.dedup();
        }
        let formulas = self.laws.at(offset);
        let relations = formulas
            .iter()
            .filter_map(|formula| formula.relation.clone())
            .collect();
        SemanticClaims::from_symbol_observations(
            definitions,
            roles,
            shapes,
            formulas,
            relations,
            quantities,
        )
        .context(symbol, semantic_id)
    }

    pub fn constraint_count(&self) -> u32 {
        (self.definitions.len()
            + self.shapes.exported().len()
            + self.quantities.exported().len()
            + self.roles.exported().len()
            + self.laws.all().len()) as u32
    }
}

fn claim_from_observation(observation: &SemanticObservation) -> SemanticClaimInfo {
    let (predicate, value, evidence) = match observation {
        SemanticObservation::Definition(definition) => (
            "definition",
            definition.description.clone(),
            vec![definition.evidence.clone()],
        ),
        SemanticObservation::Role(role) => (
            "concept",
            role.concept_id.clone(),
            vec![role.evidence.clone()],
        ),
        SemanticObservation::Shape(shape) => {
            ("shape", shape.display.clone(), vec![shape.evidence.clone()])
        }
        SemanticObservation::Quantity(quantity) => (
            "quantity",
            quantity.display.clone(),
            vec![quantity.evidence.clone()],
        ),
        SemanticObservation::Formula(formula) => (
            "formula",
            format!("{}:{}", formula.pack_id, formula.law_id),
            formula.evidence.clone(),
        ),
    };
    let claim_id = claim_id(predicate, &value, &evidence);
    SemanticClaimInfo {
        claim_id,
        predicate: predicate.into(),
        value,
        status: status_for(&evidence),
        evidence,
        conflicts: Vec::new(),
    }
}

fn claim_id(predicate: &str, value: &str, evidence: &[Evidence]) -> String {
    let anchor = evidence
        .iter()
        .flat_map(|item| &item.source_ranges)
        .map(|range| range.start_offset)
        .min()
        .unwrap_or_default();
    format!("{predicate}:{value}:{anchor}")
}

fn status_for(evidence: &[Evidence]) -> SemanticClaimStatus {
    if evidence.iter().any(|item| {
        item.strength == "hard" || matches!(item.kind.as_str(), "explicit-math" | "explicit-prose")
    }) {
        SemanticClaimStatus::Certain
    } else if evidence.iter().any(|item| item.strength == "strong") {
        SemanticClaimStatus::Supported
    } else {
        SemanticClaimStatus::Speculative
    }
}

fn mark_concept_conflicts(claims: &mut [SemanticClaimInfo]) {
    let concept_ids = claims
        .iter()
        .enumerate()
        .filter(|(_, claim)| claim.predicate == "concept")
        .map(|(index, claim)| (index, claim.value.clone(), claim.claim_id.clone()))
        .collect::<Vec<_>>();
    if concept_ids.len() < 2 {
        return;
    }
    for (index, value, _) in &concept_ids {
        let conflicts = concept_ids
            .iter()
            .filter(|(_, other_value, _)| {
                concept_role(value)
                    .zip(concept_role(other_value))
                    .is_some_and(|(left, right)| roles_conflict(left, right))
            })
            .map(|(_, _, claim_id)| claim_id.clone())
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            claims[*index].status = SemanticClaimStatus::Conflicting;
            claims[*index].conflicts = conflicts;
        }
    }
}

fn concept_role(concept_id: &str) -> Option<&str> {
    concept_id.split(':').next_back()
}

fn role_label(role: &str) -> String {
    role.split(':')
        .next_back()
        .unwrap_or(role)
        .split('-')
        .map(|part| {
            let mut characters = part.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + characters.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::SemanticClaims;
    use crate::{Evidence, RoleInfo, SemanticClaimStatus, SourceRange};

    fn role(role: &str, start: u32) -> RoleInfo {
        RoleInfo {
            symbol: "x".into(),
            concept_id: format!("test:{role}"),
            description: format!("an explicit {role}"),
            evidence: Evidence {
                rule_id: format!("test/{role}"),
                kind: "explicit-prose".into(),
                strength: "strong".into(),
                source_ranges: vec![SourceRange {
                    start_offset: start,
                    end_offset: start + 1,
                }],
            },
        }
    }

    #[test]
    fn namespaced_concepts_do_not_require_a_closed_role_enum() {
        let context = SemanticClaims::from_symbol_observations(
            Vec::new(),
            vec![role("state-vector", 4)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .context(Some("x".into()), None);

        assert_eq!(context.concepts[0].concept_id, "test:state-vector");
        assert_eq!(context.concepts[0].label, "State Vector");
        assert_eq!(context.claims[0].status, SemanticClaimStatus::Certain);
    }

    #[test]
    fn incompatible_concept_claims_remain_visible_as_conflicts() {
        let context = SemanticClaims::from_symbol_observations(
            Vec::new(),
            vec![role("event", 4), role("function", 12)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .context(Some("x".into()), None);

        assert_eq!(context.claims.len(), 2);
        assert!(
            context
                .claims
                .iter()
                .all(|claim| claim.status == SemanticClaimStatus::Conflicting)
        );
        assert!(
            context
                .claims
                .iter()
                .all(|claim| claim.conflicts.len() == 1)
        );
    }

    #[test]
    fn compatible_concept_claims_are_not_reported_as_conflicts() {
        let context = SemanticClaims::from_symbol_observations(
            Vec::new(),
            vec![role("function", 4), role("operator", 12)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .context(Some("x".into()), None);

        assert!(
            context
                .claims
                .iter()
                .all(|claim| claim.status == SemanticClaimStatus::Certain)
        );
    }
}
