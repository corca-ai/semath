use crate::canonical::SemanticExpr;
use crate::consistency::{RoleObservations, observe_roles};
use crate::domain::{DomainObservations, observe_domains};
use crate::law::{ExternalTypeEnvironment, LawAnalysisContext, LawObservations, observe_laws};
use crate::parser::ParsedMath;
use crate::prose::{
    FormulaMeaningFact, ProseMatchStats, ScientificSemanticEvidence, observe_prose,
};
use crate::quantity::{QuantityObservations, observe_quantities};
use crate::scope::ScopeGraph;
use crate::semantic_index::EntityId;
use crate::shape::{ShapeObservations, observe_shapes};
use crate::{
    AssumptionInfo, DefinitionInfo, LawRecognition, ProjectDocument, QuantityInfo, RelationInfo,
    SemanticContextInfo,
};

const MAX_RELATIONS: usize = 16;

#[derive(Clone, Debug, Default)]
struct LocalContextProjection {
    // Relations, quantities, and assumptions are source-local display projections.
    // Identity, fact availability, and claim status are deliberately absent here;
    // the engine projects those only from ProjectSemanticIndex.
    relations: Vec<RelationInfo>,
    quantities: Vec<QuantityInfo>,
    assumptions: Vec<AssumptionInfo>,
}

impl LocalContextProjection {
    pub fn new(
        relations: Vec<RelationInfo>,
        quantities: Vec<QuantityInfo>,
        assumptions: Vec<AssumptionInfo>,
    ) -> Self {
        Self {
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
            concepts: Vec::new(),
            assumptions: self.assumptions.clone(),
            claims: Vec::new(),
            candidates: Vec::new(),
            relations,
            quantities: self.quantities.clone(),
            truncated: relations_truncated,
        }
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
    pub project_references: Vec<crate::ProjectInclude>,
    pub formula_meanings: Vec<FormulaMeaningFact>,
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
        let role_definitions = prose
            .definitions
            .iter()
            .chain(&prose.semantic_role_definitions)
            .cloned()
            .collect::<Vec<_>>();
        let roles = observe_roles(document, &role_definitions, &shapes);
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
            project_references: prose.project_references,
            formula_meanings: prose.formula_meanings,
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
        symbol: Option<String>,
        entity_id: Option<EntityId>,
        offset: u32,
        formulas: Vec<LawRecognition>,
    ) -> SemanticContextInfo {
        let quantities = symbol
            .as_deref()
            .map(|name| self.quantities.at(name, offset).0)
            .unwrap_or_default();
        let relations = formulas
            .iter()
            .filter_map(|formula| formula.relation.clone())
            .collect();
        LocalContextProjection::new(relations, quantities, self.assumptions_at(offset))
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
