use std::collections::BTreeMap;

use crate::canonical::SemanticExpr;
use crate::consistency::roles_conflict;
use crate::consistency::{RoleObservations, observe_roles};
use crate::domain::{DomainObservations, observe_domains};
use crate::law::{ExternalTypeEnvironment, LawAnalysisContext, LawObservations, observe_laws};
use crate::parser::ParsedMath;
use crate::prose::{ProseMatchStats, ScientificSemanticEvidence, observe_prose};
use crate::quantity::{QuantityObservations, observe_quantities};
use crate::scope::ScopeGraph;
use crate::semantic_index::EntityId;
use crate::shape::{ShapeObservations, observe_shapes};
use crate::{
    AssumptionInfo, ConceptInfo, DefinitionInfo, Evidence, LawRecognition, ProjectDocument,
    QuantityInfo, RelationInfo, RoleInfo, SemanticClaimInfo, SemanticClaimStatus,
    SemanticContextInfo, ShapeInfo,
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
    assumptions: Vec<AssumptionInfo>,
}

impl SemanticClaims {
    pub fn from_symbol_observations(
        definitions: Vec<DefinitionInfo>,
        roles: Vec<RoleInfo>,
        shapes: Vec<ShapeInfo>,
        formulas: Vec<LawRecognition>,
        relations: Vec<RelationInfo>,
        quantities: Vec<QuantityInfo>,
        assumptions: Vec<AssumptionInfo>,
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
            assumptions,
        }
    }

    pub fn context(
        &self,
        symbol: Option<String>,
        entity_id: Option<EntityId>,
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
            entity_id,
            concepts,
            assumptions: self.assumptions.clone(),
            claims,
            candidates: Vec::new(),
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
/// Immutable, document-local observations produced by pure extractors.
///
/// These values are an analysis cache, not a second project identity graph.
/// Project-wide identity, claims, evidence, and resolution live exclusively in
/// `ProjectSemanticIndex`.
pub(crate) struct DocumentSemanticObservations {
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: ShapeObservations,
    pub quantities: QuantityObservations,
    pub roles: RoleObservations,
    pub laws: LawObservations,
    pub domains: DomainObservations,
    semantic_evidence: ScientificSemanticEvidence,
    prose_match_stats: ProseMatchStats,
    assumptions: Vec<AssumptionInfo>,
    assumption_scopes: ScopeGraph,
}

impl DocumentSemanticObservations {
    pub fn law_activations(&self) -> &[crate::prose::LawActivationEvidence] {
        &self.semantic_evidence.law_activations
    }

    pub(crate) fn semantic_evidence(&self) -> &ScientificSemanticEvidence {
        &self.semantic_evidence
    }

    pub fn assumptions(&self) -> &[AssumptionInfo] {
        &self.assumptions
    }

    pub fn build(
        document: &ProjectDocument,
        parsed: &[ParsedMath],
        canonical_expressions: &[SemanticExpr],
    ) -> Self {
        let prose = observe_prose(document, parsed, canonical_expressions);
        let prose_match_stats = prose.match_stats;
        let shapes = observe_shapes(document, parsed, canonical_expressions, &prose.shapes);
        let quantities = observe_quantities(document, parsed, &prose.definitions);
        let roles = observe_roles(document, &prose.definitions, &shapes);
        // Project analysis always supplies the source-ordered external type
        // environment immediately after base observations are built. Running
        // every compiled law here would duplicate the dominant analysis pass.
        let laws = LawObservations::default();
        let domains = observe_domains(
            document,
            ScopeGraph::new(document),
            &prose.semantic_evidence,
            laws.all(),
        );
        let assumption_scopes = ScopeGraph::new(document);
        Self {
            definitions: prose.definitions,
            shapes,
            quantities,
            roles,
            laws,
            domains,
            semantic_evidence: prose.semantic_evidence,
            prose_match_stats,
            assumptions: prose.assumptions,
            assumption_scopes,
        }
    }

    pub fn refresh_laws(
        &mut self,
        document: &ProjectDocument,
        canonical_expressions: &[SemanticExpr],
        formula_ranges: &[crate::SourceRange],
        external: &ExternalTypeEnvironment,
    ) {
        self.laws = observe_laws(
            canonical_expressions,
            &self.semantic_evidence,
            &LawAnalysisContext {
                source: &document.content,
                formula_ranges,
                shapes: &self.shapes,
                quantities: &self.quantities,
                consistency: &self.roles,
                assumptions: &self.assumptions,
                external,
                domains: &self.domains,
            },
        );
        self.domains = observe_domains(
            document,
            ScopeGraph::new(document),
            &self.semantic_evidence,
            self.laws.all(),
        );
    }

    pub fn context(
        &self,
        definitions: Vec<DefinitionInfo>,
        symbol: Option<String>,
        entity_id: Option<EntityId>,
        offset: u32,
        external: Option<&ExternalTypeEnvironment>,
        formulas: Vec<LawRecognition>,
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
            self.assumptions_at(offset),
        )
        .context(symbol, entity_id)
    }

    fn assumptions_at(&self, offset: u32) -> Vec<AssumptionInfo> {
        self.assumptions
            .iter()
            .filter(|assumption| {
                let Some(phrase) = assumption.evidence.source_ranges.last() else {
                    return false;
                };
                let scope_id = self.assumption_scopes.id_at(phrase.start_offset);
                self.assumption_scopes.visible(scope_id, offset)
                    && (phrase.end_offset <= offset
                        || assumption
                            .evidence
                            .source_ranges
                            .iter()
                            .any(|range| range.contains(offset)))
            })
            .cloned()
            .collect()
    }

    pub fn constraint_count(&self) -> u32 {
        (self.definitions.len()
            + self.shapes.exported().len()
            + self.quantities.exported().len()
            + self.roles.exported().len()
            + self.laws.all().len()) as u32
    }

    pub fn prose_match_stats(&self) -> ProseMatchStats {
        self.prose_match_stats
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
