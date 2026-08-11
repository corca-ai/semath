use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use thiserror::Error;

use crate::binder::{binder_at, binders, bound_occurrences, rename_rejection};
use crate::candidate::{
    StructuralCandidateOption, append_semantic_candidates, application_end_offset,
    structural_candidate_options,
};
use crate::canonical::{SemanticExpr, SemanticExprKind, lower_document_region, render_canonical};
use crate::cross_modal::{BindingPredicate, CrossModalBinding, extract_cross_modal_bindings};
use crate::cursor::{
    CursorOccurrence, interior_offset, item_at_cursor_with_trailing_edge, occurrence_at_cursor,
};
use crate::decision::{MeaningDecisionInput, decide_meaning};
use crate::hygiene::{HygieneAnalysis, analyze_hygiene};
use crate::law::ExternalTypeEnvironment;
use crate::parser::{ParsedMath, parse_snapshot, selection_path};
use crate::project_order::{ProjectOrder, ProjectOrderDocument};
use crate::prose::{LawActivationEvidence, definition_available_from};
use crate::scope::ScopeGraph;
use crate::semantic::DocumentSemanticObservations;
use crate::semantic_index::{
    CandidateFamily, Claim, ClaimComparison, ClaimCondition, ClaimExtent, ClaimId, ClaimObject,
    ClaimOperation, ClaimPredicate, ClaimRelation, ClaimShape, ClaimValue, DimensionExponent,
    DocumentSemanticFacts, EntityId, EvidenceId, EvidenceModality, EvidenceOrigin,
    EvidencePolarity, EvidenceRecord, InferenceTier, Mention, MentionModality, NotationComponent,
    OccurrenceKind, ProjectSemanticIndex, ResolutionStatus, SourceOccurrence, SourceOccurrenceId,
    occurrence_binding_key,
};
use crate::{
    AnalysisStats, ChangeEnvelope, DefinitionInfo, DimensionExponentInfo, Evidence, Location,
    PROTOCOL_VERSION, PhysicalDimensionInfo, ProjectChange, ProjectDocument, ProjectSnapshot,
    ProjectSnapshotMetadata, QuantityInfo, Query, QueryEnvelope, QueryResult, QueryValue,
    RenamePreparation, RoleInfo, SemanticCandidateInfo, SemanticCandidateStatus,
    SemanticClaimStatus, SemanticContextInfo, SemanticDiagnostic, SemanticEditFile,
    SemanticEditProposal, SemanticTextEdit, SemanticViewInfo, ShapeInfo, SourceRange, SymbolInfo,
    UpdateResult,
};

const MAX_SYMBOL_DEFINITIONS: usize = 8;
const MAX_SYMBOL_DIAGNOSTICS: usize = 8;
const MAX_SYMBOL_QUANTITIES: usize = 8;
const MAX_VIEW_DIAGNOSTICS: usize = 8;
const MAX_VIEW_DECLARATIONS: usize = 16;
const MAX_VIEW_CANDIDATES: usize = 16;
const MAX_VIEW_CLAIMS: usize = 32;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("unsupported protocol version {0}")]
    UnsupportedProtocol(u32),
    #[error("project epoch mismatch")]
    EpochMismatch,
    #[error("stale inventory version")]
    StaleInventory,
    #[error("document {0} does not exist")]
    MissingDocument(String),
    #[error("document version mismatch")]
    DocumentVersionMismatch,
    #[error("invalid wasmtex syntax snapshot: {0}")]
    InvalidSyntaxSnapshot(String),
    #[error("invalid semantic facts: {0}")]
    InvalidSemanticFacts(String),
    #[error("invalid JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

#[derive(Clone, Debug)]
struct AnalyzedDocument {
    document: ProjectDocument,
    parsed: Vec<ParsedMath>,
    hygiene: HygieneAnalysis,
    scopes: ScopeGraph,
    component_id: String,
    analysis_fingerprint: u64,
    canonical_expressions: Vec<SemanticExpr>,
    semantic_occurrences: Vec<SemanticOccurrenceSeed>,
    cross_modal_bindings: Vec<CrossModalBinding>,
    engine_limited_ranges: Vec<SourceRange>,
    observations: DocumentSemanticObservations,
}

#[derive(Clone, Debug)]
struct SemanticOccurrenceSeed {
    kind: OccurrenceKind,
    surface: String,
    selection_range: SourceRange,
    range: SourceRange,
    notation: Vec<NotationComponent>,
    candidate_options: Vec<StructuralCandidateOption>,
    application_end_offset: Option<u32>,
}

#[derive(Clone)]
enum ExportedTypeFact {
    Role(RoleInfo),
    Quantity(QuantityInfo),
    Shape(ShapeInfo),
}

impl ExportedTypeFact {
    fn symbol(&self) -> &str {
        match self {
            Self::Role(fact) => &fact.symbol,
            Self::Quantity(fact) => &fact.symbol,
            Self::Shape(fact) => &fact.symbol,
        }
    }
}

#[derive(Clone)]
struct IndexedTypeFact {
    component_id: String,
    fact: ExportedTypeFact,
    file_id: String,
    source_offset: u32,
}

#[derive(Clone)]
struct IndexedLawActivation {
    activation: LawActivationEvidence,
    file_id: String,
    source_offset: u32,
}

impl AnalyzedDocument {
    fn analyze(mut document: ProjectDocument) -> Result<Self, EngineError> {
        #[cfg(test)]
        let parsed = if document.nodes.is_empty() {
            crate::parser::parse_regions(&document.content, &document.math_regions)
        } else {
            parse_snapshot(&document).map_err(EngineError::InvalidSyntaxSnapshot)?
        };
        #[cfg(not(test))]
        let parsed = parse_snapshot(&document).map_err(EngineError::InvalidSyntaxSnapshot)?;
        let scopes = ScopeGraph::new(&document);
        let canonical_expressions = parsed
            .iter()
            .map(|math| {
                let mut expression = lower_document_region(&document, &math.region.content_range);
                expression.range = math.region.content_range.clone();
                expression
            })
            .collect::<Vec<_>>();
        let observations =
            DocumentSemanticObservations::build(&document, &parsed, &canonical_expressions);
        let hygiene = analyze_hygiene(&document, &parsed, &observations.definitions);
        let mut semantic_occurrences: Vec<SemanticOccurrenceSeed> = parsed
            .iter()
            .flat_map(|math| &math.symbols)
            .map(|(surface, selection_range)| {
                let range = notation_occurrence_range(&document, selection_range);
                let structural_path = notation_path(&document, selection_range);
                let candidate_options =
                    structural_candidate_options(&document, &structural_path, &range, surface);
                let notation = notation_components(&document, selection_range, surface);
                SemanticOccurrenceSeed {
                    kind: OccurrenceKind::Notation,
                    surface: compositional_surface(&document, &range, surface, &notation),
                    selection_range: selection_range.clone(),
                    application_end_offset: application_end_offset(
                        &document,
                        &structural_path,
                        &range,
                    ),
                    candidate_options,
                    notation,
                    range,
                }
            })
            .collect();
        semantic_occurrences.extend(structural_command_occurrences(
            &document,
            &semantic_occurrences,
        ));
        let cross_modal_bindings = extract_cross_modal_bindings(&document);
        for binding in &cross_modal_bindings {
            semantic_occurrences.push(SemanticOccurrenceSeed {
                kind: binding.occurrence_kind,
                surface: binding.short.clone(),
                selection_range: binding.short_range.clone(),
                range: binding.short_range.clone(),
                notation: vec![NotationComponent::NamedSurface {
                    value: binding.short.clone(),
                }],
                candidate_options: Vec::new(),
                application_end_offset: None,
            });
            if binding.long_range != binding.short_range {
                semantic_occurrences.push(SemanticOccurrenceSeed {
                    kind: binding.occurrence_kind,
                    surface: binding.long.clone(),
                    selection_range: binding.long_range.clone(),
                    range: binding.long_range.clone(),
                    notation: Vec::new(),
                    candidate_options: Vec::new(),
                    application_end_offset: None,
                });
            }
        }
        let analysis_fingerprint = analysis_fingerprint(&document);
        let engine_limited_ranges = document
            .macros
            .iter()
            .filter(|event| {
                event.kind == crate::ProjectMacroKind::Call
                    && (matches!(
                        event.expansion.status,
                        crate::ProjectMacroExpansionStatus::Cycle
                            | crate::ProjectMacroExpansionStatus::Truncated
                    ) || event.expansion.notation.as_ref().is_some_and(|notation| {
                        notation.nodes.iter().any(|node| {
                            matches!(
                                node.state,
                                crate::SyntaxState::Opaque
                                    | crate::SyntaxState::Cyclic
                                    | crate::SyntaxState::Truncated
                            )
                        })
                    }))
            })
            .flat_map(|event| {
                std::iter::once(event.source.range.clone())
                    .chain(event.expansion.input_range.iter().cloned())
            })
            .collect();
        compact_analyzed_document(&mut document);
        Ok(Self {
            component_id: document.file_id.clone(),
            document,
            parsed,
            hygiene,
            scopes,
            analysis_fingerprint,
            canonical_expressions,
            semantic_occurrences,
            cross_modal_bindings,
            engine_limited_ranges,
            observations,
        })
    }
}

fn structural_command_occurrences(
    document: &ProjectDocument,
    existing: &[SemanticOccurrenceSeed],
) -> Vec<SemanticOccurrenceSeed> {
    document
        .nodes
        .iter()
        .filter(|node| {
            node.kind == crate::NotationNodeKind::Command
                && node.state == crate::SyntaxState::Complete
        })
        .filter_map(|node| {
            let selection_range = node
                .ranges
                .command
                .as_ref()
                .or(node.ranges.name.as_ref())?
                .clone();
            if selection_range.start_offset == selection_range.end_offset
                || existing
                    .iter()
                    .any(|seed| seed.selection_range == selection_range)
            {
                return None;
            }
            let structural_path = notation_path(document, &selection_range);
            let surface = source_text(document, &selection_range);
            let candidate_options = structural_candidate_options(
                document,
                &structural_path,
                &node.ranges.full,
                &surface,
            );
            (!candidate_options.is_empty()).then(|| SemanticOccurrenceSeed {
                kind: OccurrenceKind::Notation,
                surface: surface.clone(),
                selection_range,
                range: node.ranges.full.clone(),
                application_end_offset: application_end_offset(
                    document,
                    &structural_path,
                    &node.ranges.full,
                ),
                notation: vec![NotationComponent::NamedSurface { value: surface }],
                candidate_options,
            })
        })
        .collect()
}

#[derive(Default)]
struct ProjectState {
    documents: HashMap<String, AnalyzedDocument>,
    order: ProjectOrder,
    external_types: HashMap<String, ExternalTypeEnvironment>,
    semantic: ProjectSemanticIndex,
    definitions_by_entity: BTreeMap<EntityId, DefinitionInfo>,
    occurrences_by_range: HashMap<(String, u32, u32), SourceOccurrenceId>,
}

impl ProjectState {
    fn replace(&mut self, document: ProjectDocument) -> Result<(), EngineError> {
        let file_id = document.file_id.clone();
        let previous_component = self
            .documents
            .get(&file_id)
            .map(|document| document.component_id.clone());
        let mut document = AnalyzedDocument::analyze(document)?;
        if let Some(component_id) = previous_component {
            document.component_id = component_id;
        }
        self.documents.insert(file_id, document);
        Ok(())
    }

    fn remove(&mut self, file_id: &str) {
        self.documents.remove(file_id);
        self.external_types.remove(file_id);
        self.semantic.remove_document(file_id);
        self.definitions_by_entity
            .retain(|entity, _| entity.anchor.file_id != file_id);
        self.occurrences_by_range
            .retain(|(candidate, _, _), _| candidate != file_id);
    }

    fn observations(&self, file_id: &str) -> &DocumentSemanticObservations {
        &self
            .documents
            .get(file_id)
            .expect("semantic observations require an analyzed document")
            .observations
    }

    fn observations_mut(&mut self, file_id: &str) -> &mut DocumentSemanticObservations {
        &mut self
            .documents
            .get_mut(file_id)
            .expect("semantic observations require an analyzed document")
            .observations
    }

    fn order_document(&self, file_id: &str) -> Option<ProjectOrderDocument> {
        let document = self.documents.get(file_id)?;
        let observations = &document.observations;
        Some(ProjectOrderDocument {
            file_id: file_id.to_owned(),
            includes: document.document.includes.clone(),
            occurrence_offsets: document
                .semantic_occurrences
                .iter()
                .map(|occurrence| occurrence.selection_range.start_offset)
                .chain(
                    document
                        .cross_modal_bindings
                        .iter()
                        .map(|binding| binding.evidence_range.end_offset),
                )
                .chain(
                    observations
                        .definitions
                        .iter()
                        .map(definition_available_from),
                )
                .chain(
                    observations
                        .roles
                        .exported()
                        .into_iter()
                        .flat_map(|role| role.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .chain(
                    observations
                        .quantities
                        .exported()
                        .into_iter()
                        .flat_map(|quantity| quantity.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .chain(
                    observations
                        .shapes
                        .exported()
                        .into_iter()
                        .flat_map(|shape| shape.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .chain(
                    observations
                        .law_activations()
                        .iter()
                        .flat_map(|activation| activation.evidence.source_ranges.iter())
                        .map(|range| range.start_offset),
                )
                .collect(),
            path: document.document.path.clone(),
        })
    }

    fn rebuild_semantic_index(&mut self) -> Result<(), EngineError> {
        let mut semantic = ProjectSemanticIndex::default();
        self.definitions_by_entity.clear();
        self.occurrences_by_range.clear();
        let mut file_ids = self.documents.keys().cloned().collect::<Vec<_>>();
        file_ids.sort();
        let mut semantic_documents = Vec::with_capacity(file_ids.len());
        for file_id in file_ids {
            let document = self
                .documents
                .get(&file_id)
                .expect("semantic lowering requires an analyzed document");
            let lowered = lower_semantic_document(document, &document.observations, &self.order);
            self.definitions_by_entity.extend(lowered.definitions);
            self.occurrences_by_range.extend(lowered.occurrences);
            semantic_documents.push(lowered.facts);
        }
        semantic
            .replace_documents(semantic_documents)
            .map_err(EngineError::InvalidSemanticFacts)?;
        self.semantic = semantic;
        Ok(())
    }

    fn replace_semantic_documents(&mut self, file_ids: &[String]) -> Result<(), EngineError> {
        let lowered = file_ids
            .iter()
            .map(|file_id| {
                let document = self
                    .documents
                    .get(file_id)
                    .expect("semantic lowering requires an analyzed document");
                lower_semantic_document(document, &document.observations, &self.order)
            })
            .collect::<Vec<_>>();
        let mut semantic_documents = Vec::with_capacity(lowered.len());
        let mut definitions = BTreeMap::new();
        let mut occurrences = HashMap::new();
        for item in lowered {
            semantic_documents.push(item.facts);
            definitions.extend(item.definitions);
            occurrences.extend(item.occurrences);
        }
        self.semantic
            .replace_documents(semantic_documents)
            .map_err(EngineError::InvalidSemanticFacts)?;
        let replaced = file_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        self.definitions_by_entity
            .retain(|entity, _| !replaced.contains(entity.anchor.file_id.as_str()));
        self.occurrences_by_range
            .retain(|(candidate, _, _), _| !replaced.contains(candidate.as_str()));
        self.definitions_by_entity.extend(definitions);
        self.occurrences_by_range.extend(occurrences);
        Ok(())
    }
}

struct LoweredSemanticDocument {
    facts: DocumentSemanticFacts,
    definitions: BTreeMap<EntityId, DefinitionInfo>,
    occurrences: HashMap<(String, u32, u32), SourceOccurrenceId>,
}

fn append_relation_occurrences(
    document: &ProjectDocument,
    expressions: &[SemanticExpr],
    component_id: &str,
    scopes: &ScopeGraph,
    order: &ProjectOrder,
    output: &mut Vec<(SourceOccurrence, Vec<StructuralCandidateOption>)>,
) {
    let mut ranges = Vec::new();
    for expression in expressions {
        collect_relation_ranges(expression, &mut ranges);
    }
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    for range in ranges {
        if range.start_offset >= range.end_offset
            || output
                .iter()
                .any(|(occurrence, _)| occurrence.range == range)
        {
            continue;
        }
        let id = SourceOccurrenceId {
            file_id: document.file_id.clone(),
            document_version: document.document_version,
            local_id: output.len() as u32,
        };
        output.push((
            SourceOccurrence {
                id,
                component_id: component_id.into(),
                kind: OccurrenceKind::Notation,
                range: range.clone(),
                selection_range: range.clone(),
                scope_path: scopes.path_at(range.start_offset),
                structural_path: Vec::new(),
                availability_order: order
                    .position(&document.file_id, range.start_offset)
                    .unwrap_or(u64::MAX),
                surface: source_text(document, &range),
                source_text: source_text(document, &range),
                notation: Vec::new(),
            },
            Vec::new(),
        ));
    }
}

fn collect_relation_ranges(expression: &SemanticExpr, output: &mut Vec<SourceRange>) {
    match &expression.kind {
        SemanticExprKind::Relation { left, right, .. } => {
            output.push(expression.range.clone());
            collect_relation_ranges(left, output);
            collect_relation_ranges(right, output);
        }
        SemanticExprKind::Sum(items)
        | SemanticExprKind::Product(items)
        | SemanticExprKind::System(items) => {
            output.push(expression.range.clone());
            for item in items {
                collect_relation_ranges(item, output);
            }
        }
        SemanticExprKind::Dot(left, right)
        | SemanticExprKind::Cross(left, right)
        | SemanticExprKind::Fraction(left, right)
        | SemanticExprKind::Power(left, right) => {
            output.push(expression.range.clone());
            collect_relation_ranges(left, output);
            collect_relation_ranges(right, output);
        }
        SemanticExprKind::Negate(value) => {
            output.push(expression.range.clone());
            collect_relation_ranges(value, output);
        }
        SemanticExprKind::Derivative {
            expression: operand,
            ..
        } => {
            output.push(expression.range.clone());
            collect_relation_ranges(operand, output);
        }
        SemanticExprKind::Apply { arguments, .. } => {
            output.push(expression.range.clone());
            for argument in arguments {
                collect_relation_ranges(argument, output);
            }
        }
        SemanticExprKind::Binder {
            variables,
            lower,
            upper,
            body,
            ..
        } => {
            output.push(expression.range.clone());
            for variable in variables {
                collect_relation_ranges(variable, output);
            }
            if let Some(lower) = lower {
                collect_relation_ranges(lower, output);
            }
            if let Some(upper) = upper {
                collect_relation_ranges(upper, output);
            }
            collect_relation_ranges(body, output);
        }
        SemanticExprKind::Index { base, indices } => {
            collect_relation_ranges(base, output);
            for index in indices {
                collect_relation_ranges(index, output);
            }
        }
        SemanticExprKind::Condition { value, predicate } => {
            collect_relation_ranges(value, output);
            collect_relation_ranges(predicate, output);
        }
        SemanticExprKind::Piecewise(branches) => {
            output.push(expression.range.clone());
            for branch in branches {
                collect_relation_ranges(&branch.value, output);
                if let Some(condition) = &branch.condition {
                    collect_relation_ranges(condition, output);
                }
            }
        }
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Unknown(_) => {}
    }
}

fn lower_semantic_document(
    document: &AnalyzedDocument,
    observations: &DocumentSemanticObservations,
    order: &ProjectOrder,
) -> LoweredSemanticDocument {
    let source = &document.document;
    let mut occurrences = Vec::new();
    let mut occurrences_by_range = HashMap::new();
    for (local_id, seed) in document.semantic_occurrences.iter().enumerate() {
        let id = SourceOccurrenceId {
            file_id: source.file_id.clone(),
            document_version: source.document_version,
            local_id: local_id as u32,
        };
        occurrences_by_range.insert(
            (
                source.file_id.clone(),
                seed.selection_range.start_offset,
                seed.selection_range.end_offset,
            ),
            id.clone(),
        );
        occurrences_by_range.insert(
            (
                source.file_id.clone(),
                seed.range.start_offset,
                seed.range.end_offset,
            ),
            id.clone(),
        );
        occurrences.push((
            SourceOccurrence {
                id,
                component_id: document.component_id.clone(),
                kind: seed.kind,
                range: seed.range.clone(),
                selection_range: seed.selection_range.clone(),
                scope_path: document.scopes.path_at(seed.selection_range.start_offset),
                // Structural alternatives are already materialized from this
                // path in the analyzed document. Do not retain a second copy
                // in the project index.
                structural_path: Vec::new(),
                availability_order: order
                    .position(&source.file_id, seed.selection_range.start_offset)
                    .unwrap_or(u64::MAX),
                surface: seed.surface.clone(),
                source_text: source_text(source, &seed.range),
                notation: seed.notation.clone(),
            },
            seed.candidate_options.clone(),
        ));
    }
    append_relation_occurrences(
        source,
        &document.canonical_expressions,
        &document.component_id,
        &document.scopes,
        order,
        &mut occurrences,
    );
    occurrences.sort_by_key(|(item, _)| {
        (
            item.range.start_offset,
            item.range.end_offset,
            item.id.local_id,
        )
    });
    occurrences.dedup_by(|(left, _), (right, _)| {
        left.range == right.range && left.surface == right.surface
    });
    occurrences_by_range.clear();
    let mut candidates = Vec::new();
    for (local_id, (occurrence, options)) in occurrences.iter_mut().enumerate() {
        occurrence.id.local_id = local_id as u32;
        append_semantic_candidates(source, occurrence, options, &mut candidates);
        occurrences_by_range.insert(
            (
                source.file_id.clone(),
                occurrence.range.start_offset,
                occurrence.range.end_offset,
            ),
            occurrence.id.clone(),
        );
        occurrences_by_range.insert(
            (
                source.file_id.clone(),
                occurrence.selection_range.start_offset,
                occurrence.selection_range.end_offset,
            ),
            occurrence.id.clone(),
        );
    }
    let occurrences = occurrences
        .into_iter()
        .map(|(occurrence, _)| occurrence)
        .collect::<Vec<_>>();

    let mut entities = Vec::new();
    let mut evidence = Vec::new();
    let mut claims = Vec::new();
    let mut definitions = BTreeMap::new();
    for (definition_index, definition) in observations.definitions.iter().enumerate() {
        let Some(anchor) = definition_anchor(
            definition,
            &source.file_id,
            &occurrences,
            &occurrences_by_range,
        ) else {
            continue;
        };
        let anchor_occurrence = occurrences
            .iter()
            .find(|occurrence| occurrence.id == anchor)
            .expect("definition anchor belongs to the lowered occurrence set");
        let availability_offset = definition_available_from(definition);
        let mut definition = definition.clone();
        definition.location.range = anchor_occurrence.range.clone();
        let scope_path = document
            .scopes
            .path_at(definition.location.range.start_offset);
        let entity = EntityId {
            component_id: document.component_id.clone(),
            scope_path,
            kind: "definition".to_owned(),
            anchor: anchor.clone(),
        };
        let evidence_id = EvidenceId(format!(
            "{}:{}:definition-evidence:{}",
            source.file_id, source.document_version, definition_index
        ));
        let claim_id = ClaimId(format!(
            "{}:{}:definition-claim:{}",
            source.file_id, source.document_version, definition_index
        ));
        let availability = order
            .position(&source.file_id, availability_offset)
            .unwrap_or(u64::MAX)
            .max(anchor_occurrence.availability_order);
        evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: anchor.clone(),
            scope_path: entity.scope_path.clone(),
            available_after: availability,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![anchor.clone()],
            parent_claims: Vec::new(),
            rule_id: definition.evidence.rule_id.clone(),
            rule_version: 1,
        });
        claims.push(Claim {
            id: claim_id,
            subject: entity.clone(),
            predicate: ClaimPredicate::Defines,
            object: ClaimObject::Occurrence(anchor),
            evidence_id: evidence_id.clone(),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
        for (type_index, candidate_type) in explicit_candidate_types(&definition.description)
            .into_iter()
            .enumerate()
        {
            claims.push(Claim {
                id: ClaimId(format!(
                    "{}:{}:definition-type-claim:{}:{type_index}",
                    source.file_id, source.document_version, definition_index
                )),
                subject: entity.clone(),
                predicate: ClaimPredicate::HasType,
                object: ClaimObject::Value(ClaimValue::Type(candidate_type.to_owned())),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            });
        }
        definitions.insert(entity.clone(), definition);
        if !entities.contains(&entity) {
            entities.push(entity);
        }
    }
    let cross_modal = lower_cross_modal_facts(document, &occurrences, &occurrences_by_range, order);
    definitions.extend(
        cross_modal
            .definitions
            .iter()
            .map(|(entity, definition)| (entity.clone(), definition.clone())),
    );
    let relations = lower_canonical_relation_facts(
        source,
        &document.canonical_expressions,
        &occurrences,
        &definitions,
    );
    let typed = lower_typed_observation_facts(
        source,
        observations,
        &occurrences,
        &definitions,
        &relations.entities,
    );
    evidence.extend(typed.evidence);
    claims.extend(typed.claims);
    entities.extend(relations.entities);
    evidence.extend(relations.evidence);
    claims.extend(relations.claims);
    entities.extend(cross_modal.entities);
    evidence.extend(cross_modal.evidence);
    claims.extend(cross_modal.claims);
    entities.sort();
    entities.dedup();
    let mentions = occurrences
        .iter()
        .map(|occurrence| Mention {
            occurrence_id: occurrence.id.clone(),
            modality: match occurrence.kind {
                OccurrenceKind::Notation => MentionModality::Notation,
                OccurrenceKind::Prose => MentionModality::Prose,
                OccurrenceKind::MacroDeclaration => MentionModality::Declaration,
                OccurrenceKind::ResourceDeclaration => MentionModality::Resource,
            },
        })
        .collect();
    LoweredSemanticDocument {
        facts: DocumentSemanticFacts {
            file_id: source.file_id.clone(),
            document_version: source.document_version,
            source_utf16_length: source.content.encode_utf16().count() as u32,
            occurrences,
            entities,
            mentions,
            evidence,
            claims,
            candidates,
        },
        definitions,
        occurrences: occurrences_by_range,
    }
}

fn definition_anchor(
    definition: &DefinitionInfo,
    file_id: &str,
    occurrences: &[SourceOccurrence],
    occurrences_by_range: &HashMap<(String, u32, u32), SourceOccurrenceId>,
) -> Option<SourceOccurrenceId> {
    let range = &definition.location.range;
    if let Some(exact) =
        occurrences_by_range.get(&(file_id.to_owned(), range.start_offset, range.end_offset))
    {
        return Some(exact.clone());
    }
    let mut candidates = occurrences
        .iter()
        .filter(|occurrence| occurrence.kind == OccurrenceKind::Notation)
        .filter(|occurrence| {
            occurrence.range.start_offset <= range.start_offset
                && range.end_offset <= occurrence.range.end_offset
        })
        .filter(|occurrence| occurrence_declared_name(occurrence) == definition.symbol)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|occurrence| {
        (
            occurrence.range.end_offset - occurrence.range.start_offset,
            occurrence.selection_range.end_offset - occurrence.selection_range.start_offset,
            occurrence.id.local_id,
        )
    });
    let selected = *candidates.first()?;
    if candidates.get(1).is_some_and(|next| {
        next.range == selected.range && next.selection_range == selected.selection_range
    }) {
        return None;
    }
    Some(selected.id.clone())
}

fn occurrence_declared_name(occurrence: &SourceOccurrence) -> String {
    occurrence
        .notation
        .iter()
        .find_map(|component| match component {
            NotationComponent::Subscript { base, index } => Some(format!("{base}_{index}")),
            _ => None,
        })
        .or_else(|| {
            occurrence
                .notation
                .iter()
                .find_map(|component| match component {
                    NotationComponent::NamedSurface { value }
                    | NotationComponent::Identifier { value } => Some(value.clone()),
                    _ => None,
                })
        })
        .unwrap_or_else(|| occurrence.surface.clone())
}

#[derive(Default)]
struct LoweredTypedFacts {
    evidence: Vec<EvidenceRecord>,
    claims: Vec<Claim>,
}

fn lower_typed_observation_facts(
    source: &ProjectDocument,
    observations: &DocumentSemanticObservations,
    occurrences: &[SourceOccurrence],
    definitions: &BTreeMap<EntityId, DefinitionInfo>,
    relation_entities: &[EntityId],
) -> LoweredTypedFacts {
    let mut output = LoweredTypedFacts::default();
    {
        let mut append = |symbol: &str,
                          evidence: &Evidence,
                          predicate: ClaimPredicate,
                          value: ClaimValue,
                          category: &str| {
            let entity = closest_definition(definitions, symbol, evidence)
                .map(|(entity, _)| entity.clone())
                .or_else(|| {
                    closest_relation_entity(relation_entities, occurrences, symbol, evidence)
                });
            let Some(entity) = entity else { return };
            let Some(anchor) = occurrences
                .iter()
                .find(|occurrence| occurrence.id == entity.anchor)
            else {
                return;
            };
            let ordinal = output.claims.len();
            let evidence_id = EvidenceId(format!(
                "{}:{}:typed-{category}-evidence:{ordinal}",
                source.file_id, source.document_version
            ));
            output.evidence.push(EvidenceRecord {
                id: evidence_id.clone(),
                source: entity.anchor.clone(),
                scope_path: entity.scope_path.clone(),
                available_after: anchor.availability_order,
                polarity: EvidencePolarity::Positive,
                modality: EvidenceModality::Asserted,
                origin: EvidenceOrigin::Explicit,
                provenance: vec![entity.anchor.clone()],
                parent_claims: Vec::new(),
                rule_id: evidence.rule_id.clone(),
                rule_version: 1,
            });
            output.claims.push(Claim {
                id: ClaimId(format!(
                    "{}:{}:typed-{category}-claim:{ordinal}",
                    source.file_id, source.document_version
                )),
                subject: entity,
                predicate,
                object: ClaimObject::Value(value),
                evidence_id,
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            });
        };

        for role in observations.roles.all() {
            append(
                &role.symbol,
                &role.evidence,
                ClaimPredicate::HasRole,
                ClaimValue::Concept(role.concept_id),
                "role",
            );
        }
        for shape in observations.shapes.explicit_claims() {
            let value = ClaimValue::Shape(claim_shape(
                &shape.kind,
                shape.dimensions,
                definitions,
                relation_entities,
                occurrences,
                &shape.evidence,
            ));
            append(
                &shape.symbol,
                &shape.evidence,
                ClaimPredicate::HasShape,
                value,
                "shape",
            );
        }
        for quantity in observations.quantities.explicit() {
            if let Some(quantity_kind) = quantity.quantity_kind_id {
                append(
                    &quantity.symbol,
                    &quantity.evidence,
                    ClaimPredicate::HasQuantity,
                    ClaimValue::QuantityKind(quantity_kind),
                    "quantity",
                );
            }
            if let Some(unit) = quantity.unit_id {
                append(
                    &quantity.symbol,
                    &quantity.evidence,
                    ClaimPredicate::HasUnit,
                    ClaimValue::Unit(unit),
                    "unit",
                );
            }
            if !quantity.dimension.exponents.is_empty() {
                let exponents = quantity
                    .dimension
                    .exponents
                    .into_iter()
                    .filter_map(|exponent| {
                        Some(DimensionExponent {
                            base: exponent.base,
                            numerator: i16::try_from(exponent.numerator).ok()?,
                            denominator: u16::try_from(exponent.denominator).ok()?,
                        })
                    })
                    .collect();
                append(
                    &quantity.symbol,
                    &quantity.evidence,
                    ClaimPredicate::HasDimension,
                    ClaimValue::Dimension(exponents),
                    "dimension",
                );
            }
        }
    }
    for assumption in observations.assumptions() {
        for subject in &assumption.subjects {
            let Some((entity, _)) = closest_definition(definitions, subject, &assumption.evidence)
            else {
                continue;
            };
            let condition = match (assumption.kind.as_str(), assumption.value.as_str()) {
                ("sign", "nonzero") => ClaimCondition::Nonzero(entity.clone()),
                ("sign", "positive" | "strictly-positive") => {
                    ClaimCondition::Positive(entity.clone())
                }
                ("sign", "nonnegative") => ClaimCondition::Nonnegative(entity.clone()),
                ("algebraic-property", "invertible") => ClaimCondition::Invertible(entity.clone()),
                _ => ClaimCondition::Named(format!("{}:{}", assumption.kind, assumption.value)),
            };
            let ordinal = output.claims.len();
            let evidence_id = EvidenceId(format!(
                "{}:{}:typed-assumption-evidence:{ordinal}",
                source.file_id, source.document_version
            ));
            let anchor = occurrences
                .iter()
                .find(|occurrence| occurrence.id == entity.anchor)
                .expect("definition entity has a source anchor");
            output.evidence.push(EvidenceRecord {
                id: evidence_id.clone(),
                source: entity.anchor.clone(),
                scope_path: entity.scope_path.clone(),
                available_after: anchor.availability_order,
                polarity: EvidencePolarity::Positive,
                modality: EvidenceModality::Asserted,
                origin: EvidenceOrigin::Explicit,
                provenance: vec![entity.anchor.clone()],
                parent_claims: Vec::new(),
                rule_id: assumption.evidence.rule_id.clone(),
                rule_version: 1,
            });
            output.claims.push(Claim {
                id: ClaimId(format!(
                    "{}:{}:typed-assumption-claim:{ordinal}",
                    source.file_id, source.document_version
                )),
                subject: entity.clone(),
                predicate: ClaimPredicate::Assumes,
                object: ClaimObject::Value(ClaimValue::Condition(condition)),
                evidence_id,
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            });
        }
    }
    output
}

#[derive(Default)]
struct LoweredRelationFacts {
    entities: Vec<EntityId>,
    evidence: Vec<EvidenceRecord>,
    claims: Vec<Claim>,
}

struct RelationLowerer<'a> {
    source: &'a ProjectDocument,
    occurrences: &'a [SourceOccurrence],
    definitions: &'a BTreeMap<EntityId, DefinitionInfo>,
    output: LoweredRelationFacts,
    entities_by_expression: BTreeMap<(u32, u32, String), EntityId>,
    implicit_entities_by_identity: BTreeMap<(Vec<u32>, String), EntityId>,
}

fn lower_canonical_relation_facts(
    source: &ProjectDocument,
    expressions: &[SemanticExpr],
    occurrences: &[SourceOccurrence],
    definitions: &BTreeMap<EntityId, DefinitionInfo>,
) -> LoweredRelationFacts {
    let mut lowerer = RelationLowerer {
        source,
        occurrences,
        definitions,
        output: LoweredRelationFacts::default(),
        entities_by_expression: BTreeMap::new(),
        implicit_entities_by_identity: BTreeMap::new(),
    };
    for expression in expressions {
        lowerer.lower_expression(expression);
    }
    lowerer.output.entities.sort();
    lowerer.output.entities.dedup();
    lowerer.output
}

impl RelationLowerer<'_> {
    fn lower_expression(&mut self, expression: &SemanticExpr) -> Option<EntityId> {
        let result = self.entity_for(expression)?;
        let canonical = render_canonical(expression);
        let digest = stable_text_digest(&canonical);
        let relation = match &expression.kind {
            SemanticExprKind::Relation {
                operator,
                left,
                right,
            } if comparison_operator(operator.as_str()).is_some() => {
                let comparison = comparison_operator(operator.as_str())?;
                let left = self.lower_assignment_expression(left)?;
                let right = self.lower_assignment_expression(right)?;
                ClaimRelation::Comparison {
                    operator: comparison,
                    left,
                    right,
                    canonical_digest: digest,
                }
            }
            SemanticExprKind::Relation {
                operator,
                left,
                right,
            } if matches!(operator.as_str(), "member-of" | "not-member-of") => {
                let _ = self.lower_assignment_expression(left);
                self.establish_extent_entities(right);
                let _ = self.lower_expression(right);
                return Some(result);
            }
            SemanticExprKind::Sum(terms) => ClaimRelation::Sum {
                result: result.clone(),
                terms: self.lower_many(terms)?,
                canonical_digest: digest,
            },
            SemanticExprKind::Product(factors) => ClaimRelation::Product {
                result: result.clone(),
                factors: self.lower_many(factors)?,
                canonical_digest: digest,
            },
            SemanticExprKind::Fraction(numerator, denominator) => ClaimRelation::Quotient {
                result: result.clone(),
                numerator: self.lower_expression(numerator)?,
                denominator: self.lower_expression(denominator)?,
                canonical_digest: digest,
            },
            SemanticExprKind::Negate(operand) => ClaimRelation::Operation {
                result: result.clone(),
                operator: ClaimOperation::Negate,
                operands: vec![self.lower_expression(operand)?],
                canonical_digest: digest,
            },
            SemanticExprKind::Power(base, exponent) => ClaimRelation::Operation {
                result: result.clone(),
                operator: ClaimOperation::Power,
                operands: vec![
                    self.lower_expression(base)?,
                    self.lower_expression(exponent)?,
                ],
                canonical_digest: digest,
            },
            SemanticExprKind::Dot(left, right) => ClaimRelation::Operation {
                result: result.clone(),
                operator: ClaimOperation::Dot,
                operands: vec![self.lower_expression(left)?, self.lower_expression(right)?],
                canonical_digest: digest,
            },
            SemanticExprKind::Cross(left, right) => ClaimRelation::Operation {
                result: result.clone(),
                operator: ClaimOperation::Cross,
                operands: vec![self.lower_expression(left)?, self.lower_expression(right)?],
                canonical_digest: digest,
            },
            SemanticExprKind::Derivative {
                expression: operand,
                variable,
                order,
            } => ClaimRelation::Derivative {
                result: result.clone(),
                operand: self.lower_expression(operand)?,
                variable: self.entity_for_reference(variable.as_str(), &variable.range),
                order: *order,
                canonical_digest: digest,
            },
            SemanticExprKind::Apply {
                operator,
                arguments,
            } if operator.as_str() == "transpose" && arguments.len() == 1 => {
                ClaimRelation::Operation {
                    result: result.clone(),
                    operator: ClaimOperation::Transpose,
                    operands: self.lower_many(arguments)?,
                    canonical_digest: digest,
                }
            }
            SemanticExprKind::Apply {
                operator,
                arguments,
            } => ClaimRelation::Application {
                result: result.clone(),
                function: self.entity_for_reference(operator.as_str(), &operator.range)?,
                arguments: self.lower_many(arguments)?,
                canonical_digest: digest,
            },
            SemanticExprKind::Binder {
                operator,
                variables,
                body,
                ..
            } if operator.as_str() == "integral" => ClaimRelation::Integral {
                result: result.clone(),
                integrand: self.lower_expression(body)?,
                variable: variables
                    .first()
                    .and_then(|variable| self.lower_expression(variable)),
                canonical_digest: digest,
            },
            SemanticExprKind::Relation { left, right, .. } => {
                let _ = self.lower_expression(left);
                let _ = self.lower_expression(right);
                return Some(result);
            }
            SemanticExprKind::System(items) => {
                let _ = self.lower_many(items);
                return Some(result);
            }
            SemanticExprKind::Piecewise(branches) => {
                for branch in branches {
                    let _ = self.lower_expression(&branch.value);
                    if let Some(condition) = &branch.condition {
                        let _ = self.lower_expression(condition);
                    }
                }
                return Some(result);
            }
            SemanticExprKind::Index { base, indices } => {
                let _ = self.lower_expression(base);
                let _ = self.lower_many(indices);
                return Some(result);
            }
            SemanticExprKind::Condition { value, predicate } => {
                let _ = self.lower_expression(value);
                let _ = self.lower_expression(predicate);
                return Some(result);
            }
            SemanticExprKind::Binder {
                variables,
                lower,
                upper,
                body,
                ..
            } => {
                let _ = self.lower_many(variables);
                if let Some(lower) = lower {
                    let _ = self.lower_expression(lower);
                }
                if let Some(upper) = upper {
                    let _ = self.lower_expression(upper);
                }
                let _ = self.lower_expression(body);
                return Some(result);
            }
            SemanticExprKind::Number(_) => {
                self.emit_scalar_constant(&result);
                return Some(result);
            }
            _ => return Some(result),
        };
        self.emit_relation(result.clone(), relation);
        Some(result)
    }

    fn lower_many(&mut self, expressions: &[SemanticExpr]) -> Option<Vec<EntityId>> {
        expressions
            .iter()
            .map(|expression| self.lower_expression(expression))
            .collect()
    }

    fn lower_assignment_expression(&mut self, expression: &SemanticExpr) -> Option<EntityId> {
        if !matches!(
            expression.kind,
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. }
        ) {
            return self.lower_expression(expression);
        }

        let occurrence = self.occurrence_for_range(&expression.range)?.clone();
        if self.explicit_entity_for_occurrence(&occurrence).is_some()
            || crate::canonical::expression_name(expression)
                .and_then(|name| self.definition_entity(&name, &expression.range))
                .is_some()
        {
            return self.lower_expression(expression);
        }
        Some(self.establish_implicit_entity(expression, occurrence))
    }

    fn establish_extent_entities(&mut self, expression: &SemanticExpr) {
        let SemanticExprKind::Power(_, dimensions) = &expression.kind else {
            return;
        };
        self.establish_extent_terms(dimensions);
    }

    fn establish_extent_terms(&mut self, expression: &SemanticExpr) {
        match &expression.kind {
            SemanticExprKind::Cross(left, right) => {
                self.establish_extent_terms(left);
                self.establish_extent_terms(right);
            }
            SemanticExprKind::Product(items) => {
                for item in items {
                    self.establish_extent_terms(item);
                }
            }
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. } => {
                let _ = self.lower_assignment_expression(expression);
            }
            _ => {
                let _ = self.lower_expression(expression);
            }
        }
    }

    fn entity_for(&mut self, expression: &SemanticExpr) -> Option<EntityId> {
        if matches!(
            expression.kind,
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. }
        ) && let Some(occurrence) = self.occurrence_for_range(&expression.range)
            && let Some(entity) = self.explicit_entity_for_occurrence(occurrence)
        {
            return Some(entity);
        }
        if matches!(
            expression.kind,
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. }
        ) && let Some(name) = crate::canonical::expression_name(expression)
            && let Some(entity) = self.definition_entity(&name, &expression.range)
        {
            return Some(entity);
        }
        if matches!(
            expression.kind,
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. }
        ) && let Some(occurrence) = self.occurrence_for_range(&expression.range)
            && let Some(entity) = self.implicit_entity_for_expression(expression, occurrence)
        {
            return Some(entity);
        }
        let digest = render_canonical(expression);
        let key = (
            expression.range.start_offset,
            expression.range.end_offset,
            digest.clone(),
        );
        if let Some(entity) = self.entities_by_expression.get(&key) {
            return Some(entity.clone());
        }
        let anchor = self.occurrence_for_range(&expression.range)?;
        let entity = EntityId {
            component_id: anchor.component_id.clone(),
            scope_path: anchor.scope_path.clone(),
            kind: format!("expression:{}", stable_text_digest(&digest)),
            anchor: anchor.id.clone(),
        };
        self.output.entities.push(entity.clone());
        self.entities_by_expression.insert(key, entity.clone());
        Some(entity)
    }

    fn establish_implicit_entity(
        &mut self,
        expression: &SemanticExpr,
        occurrence: SourceOccurrence,
    ) -> EntityId {
        let identity = canonical_expression_binding_key(expression, &occurrence);
        let key = (occurrence.scope_path.clone(), identity);
        let entity = self
            .implicit_entities_by_identity
            .get(&key)
            .cloned()
            .unwrap_or_else(|| EntityId {
                component_id: occurrence.component_id.clone(),
                scope_path: occurrence.scope_path.clone(),
                kind: "implicit-symbol".to_owned(),
                anchor: occurrence.id.clone(),
            });
        self.implicit_entities_by_identity
            .insert(key, entity.clone());
        if !self.output.entities.contains(&entity) {
            self.output.entities.push(entity.clone());
        }
        self.entities_by_expression.insert(
            (
                expression.range.start_offset,
                expression.range.end_offset,
                render_canonical(expression),
            ),
            entity.clone(),
        );
        self.emit_implicit_symbol_binding(&entity, &occurrence);
        entity
    }

    fn occurrence_for_range(&self, range: &SourceRange) -> Option<&SourceOccurrence> {
        self.occurrences
            .iter()
            .find(|occurrence| occurrence.range == *range)
            .or_else(|| {
                self.occurrences.iter().find(|occurrence| {
                    range.start_offset <= occurrence.range.start_offset
                        && occurrence.range.end_offset <= range.end_offset
                })
            })
            .or_else(|| {
                self.occurrences
                    .iter()
                    .filter(|occurrence| {
                        occurrence.range.start_offset <= range.start_offset
                            && range.end_offset <= occurrence.range.end_offset
                    })
                    .min_by_key(|occurrence| {
                        occurrence.range.end_offset - occurrence.range.start_offset
                    })
            })
    }

    fn implicit_entity_for_expression(
        &self,
        expression: &SemanticExpr,
        occurrence: &SourceOccurrence,
    ) -> Option<EntityId> {
        let identity = canonical_expression_binding_key(expression, occurrence);
        self.implicit_entities_by_identity
            .iter()
            .filter(|((scope_path, binding), entity)| {
                binding == &identity
                    && entity.component_id == occurrence.component_id
                    && scope_path.len() <= occurrence.scope_path.len()
                    && scope_path
                        .iter()
                        .zip(&occurrence.scope_path)
                        .all(|(left, right)| left == right)
            })
            .max_by_key(|((scope_path, _), _)| scope_path.len())
            .map(|(_, entity)| entity.clone())
    }

    fn explicit_entity_for_occurrence(&self, occurrence: &SourceOccurrence) -> Option<EntityId> {
        let identity = occurrence_binding_key(occurrence);
        self.definitions
            .iter()
            .filter_map(|(entity, definition)| {
                let anchor = self
                    .occurrences
                    .iter()
                    .find(|candidate| candidate.id == entity.anchor)?;
                (occurrence_binding_key(anchor) == identity
                    && entity.component_id == occurrence.component_id
                    && entity.scope_path.len() <= occurrence.scope_path.len()
                    && entity
                        .scope_path
                        .iter()
                        .zip(&occurrence.scope_path)
                        .all(|(left, right)| left == right)
                    && definition_available_from(definition) <= occurrence.range.start_offset)
                    .then_some(entity.clone())
            })
            .max_by_key(|entity| entity.scope_path.len())
    }

    fn emit_implicit_symbol_binding(&mut self, entity: &EntityId, occurrence: &SourceOccurrence) {
        if self.output.claims.iter().any(|claim| {
            claim.subject == *entity
                && claim.predicate == ClaimPredicate::Names
                && claim.object == ClaimObject::Occurrence(occurrence.id.clone())
        }) {
            return;
        }
        let ordinal = self.output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:canonical-symbol-identity-evidence:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: occurrence.id.clone(),
            scope_path: occurrence.scope_path.clone(),
            available_after: occurrence.availability_order,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![occurrence.id.clone()],
            parent_claims: Vec::new(),
            rule_id: "semath/canonical-symbol-identity".into(),
            rule_version: 1,
        });
        self.output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:canonical-symbol-identity-claim:{ordinal}",
                self.source.file_id, self.source.document_version
            )),
            subject: entity.clone(),
            predicate: ClaimPredicate::Names,
            object: ClaimObject::Occurrence(occurrence.id.clone()),
            evidence_id,
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
    }

    fn entity_for_reference(&mut self, value: &str, range: &SourceRange) -> Option<EntityId> {
        if let Some(entity) = self.definition_entity(value, range) {
            return Some(entity);
        }
        let expression = SemanticExpr {
            kind: SemanticExprKind::Symbol(value.into()),
            range: range.clone(),
            provenance: vec![range.clone()],
        };
        self.entity_for(&expression)
    }

    fn definition_entity(&self, symbol: &str, range: &SourceRange) -> Option<EntityId> {
        let occurrence = self
            .occurrences
            .iter()
            .find(|occurrence| occurrence.range == *range)
            .or_else(|| {
                self.occurrences.iter().find(|occurrence| {
                    occurrence.range.start_offset <= range.start_offset
                        && range.end_offset <= occurrence.range.end_offset
                })
            });
        self.definitions
            .iter()
            .filter(|(entity, definition)| {
                definition.symbol.trim_start_matches('\\') == symbol.trim_start_matches('\\')
                    && definition_available_from(definition) <= range.start_offset
                    && occurrence.is_none_or(|occurrence| {
                        entity.component_id == occurrence.component_id
                            && entity.scope_path.len() <= occurrence.scope_path.len()
                            && entity
                                .scope_path
                                .iter()
                                .zip(&occurrence.scope_path)
                                .all(|(left, right)| left == right)
                    })
            })
            .min_by_key(|(entity, definition)| {
                let distance = definition
                    .location
                    .range
                    .start_offset
                    .abs_diff(range.start_offset);
                (std::cmp::Reverse(entity.scope_path.len()), distance)
            })
            .map(|(entity, _)| entity.clone())
    }

    fn emit_relation(&mut self, subject: EntityId, relation: ClaimRelation) {
        let ordinal = self.output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:canonical-relation-evidence:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        let claim_id = ClaimId(format!(
            "{}:{}:canonical-relation-claim:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        let mut provenance = relation
            .entities()
            .into_iter()
            .map(|entity| entity.anchor.clone())
            .chain(std::iter::once(subject.anchor.clone()))
            .collect::<Vec<_>>();
        provenance.sort();
        provenance.dedup();
        let available_after = relation
            .entities()
            .into_iter()
            .chain(std::iter::once(&subject))
            .filter_map(|entity| {
                self.occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == entity.anchor)
                    .map(|occurrence| occurrence.availability_order)
            })
            .max()
            .unwrap_or(0);
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: subject.anchor.clone(),
            scope_path: subject.scope_path.clone(),
            available_after,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Explicit,
            provenance,
            parent_claims: Vec::new(),
            rule_id: "semath/canonical-relation".into(),
            rule_version: 1,
        });
        self.output.claims.push(Claim {
            id: claim_id,
            subject,
            predicate: ClaimPredicate::Relates,
            object: ClaimObject::Value(ClaimValue::Relation(Box::new(relation))),
            evidence_id,
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
    }

    fn emit_scalar_constant(&mut self, subject: &EntityId) {
        if self
            .output
            .claims
            .iter()
            .any(|claim| claim.subject == *subject && claim.predicate == ClaimPredicate::HasShape)
        {
            return;
        }
        let ordinal = self.output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:canonical-constant-evidence:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: subject.anchor.clone(),
            scope_path: subject.scope_path.clone(),
            available_after: self
                .occurrences
                .iter()
                .find(|occurrence| occurrence.id == subject.anchor)
                .map_or(0, |occurrence| occurrence.availability_order),
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![subject.anchor.clone()],
            parent_claims: Vec::new(),
            rule_id: "semath/canonical-scalar-constant".into(),
            rule_version: 1,
        });
        self.output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:canonical-constant-claim:{ordinal}",
                self.source.file_id, self.source.document_version
            )),
            subject: subject.clone(),
            predicate: ClaimPredicate::HasShape,
            object: ClaimObject::Value(ClaimValue::Shape(ClaimShape::Scalar)),
            evidence_id,
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
    }
}

fn canonical_expression_binding_key(
    expression: &SemanticExpr,
    occurrence: &SourceOccurrence,
) -> String {
    match &expression.kind {
        SemanticExprKind::Symbol(name)
            if !occurrence.notation.iter().any(|component| {
                matches!(
                    component,
                    NotationComponent::Modifier { .. }
                        | NotationComponent::Style { .. }
                        | NotationComponent::NamedSurface { .. }
                )
            }) =>
        {
            format!("symbol:{name}")
        }
        SemanticExprKind::Symbol(_) => occurrence_binding_key(occurrence),
        SemanticExprKind::Index { .. } => format!("index:{}", render_canonical(expression)),
        _ => occurrence_binding_key(occurrence),
    }
}

fn stable_text_digest(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn closest_definition<'a>(
    definitions: &'a BTreeMap<EntityId, DefinitionInfo>,
    symbol: &str,
    evidence: &Evidence,
) -> Option<(&'a EntityId, &'a DefinitionInfo)> {
    let evidence_start = evidence
        .source_ranges
        .iter()
        .map(|range| range.start_offset)
        .min()
        .unwrap_or(u32::MAX);
    definitions
        .iter()
        .filter(|(_, definition)| {
            definition.symbol.trim_start_matches('\\') == symbol.trim_start_matches('\\')
        })
        .min_by_key(|(_, definition)| {
            definition
                .location
                .range
                .start_offset
                .abs_diff(evidence_start)
        })
}

fn closest_relation_entity(
    entities: &[EntityId],
    occurrences: &[SourceOccurrence],
    symbol: &str,
    evidence: &Evidence,
) -> Option<EntityId> {
    let normalized = symbol.trim_start_matches('\\');
    entities
        .iter()
        .filter(|entity| entity.kind == "implicit-symbol")
        .filter_map(|entity| {
            let occurrence = occurrences
                .iter()
                .find(|occurrence| occurrence.id == entity.anchor)?;
            (occurrence_declared_name(occurrence).trim_start_matches('\\') == normalized
                && evidence.source_ranges.iter().any(|range| {
                    range.start_offset <= occurrence.range.start_offset
                        && occurrence.range.end_offset <= range.end_offset
                }))
            .then_some((entity, occurrence))
        })
        .min_by_key(|(_, occurrence)| {
            evidence
                .source_ranges
                .iter()
                .map(|range| range.start_offset.abs_diff(occurrence.range.start_offset))
                .min()
                .unwrap_or(u32::MAX)
        })
        .map(|(entity, _)| entity.clone())
}

fn claim_shape(
    kind: &str,
    dimensions: Vec<String>,
    definitions: &BTreeMap<EntityId, DefinitionInfo>,
    relation_entities: &[EntityId],
    occurrences: &[SourceOccurrence],
    evidence: &Evidence,
) -> ClaimShape {
    let dimensions = dimensions
        .into_iter()
        .map(|display| {
            if let Ok(value) = display.parse::<u64>() {
                return ClaimExtent::Known { value };
            }
            let entity =
                closest_relation_entity(relation_entities, occurrences, &display, evidence)
                    .or_else(|| {
                        closest_definition(definitions, &display, evidence)
                            .map(|(entity, _)| entity.clone())
                    });
            match entity {
                Some(entity) => ClaimExtent::Symbolic { entity, display },
                None => ClaimExtent::Unknown { display },
            }
        })
        .collect();
    match kind {
        "scalar" => ClaimShape::Scalar,
        "vector" => ClaimShape::Vector(dimensions),
        "matrix" => ClaimShape::Matrix(dimensions),
        "tensor" => ClaimShape::Tensor(dimensions),
        _ => ClaimShape::Unknown,
    }
}

fn comparison_operator(operator: &str) -> Option<ClaimComparison> {
    Some(match operator {
        "equals" => ClaimComparison::Equal,
        "not-equals" => ClaimComparison::NotEqual,
        "less-than" => ClaimComparison::LessThan,
        "less-or-equal" => ClaimComparison::LessOrEqual,
        "greater-than" => ClaimComparison::GreaterThan,
        "greater-or-equal" => ClaimComparison::GreaterOrEqual,
        _ => return None,
    })
}

#[derive(Default)]
struct LoweredCrossModalFacts {
    entities: Vec<EntityId>,
    evidence: Vec<EvidenceRecord>,
    claims: Vec<Claim>,
    definitions: BTreeMap<EntityId, DefinitionInfo>,
}

fn lower_cross_modal_facts(
    document: &AnalyzedDocument,
    occurrences: &[SourceOccurrence],
    occurrences_by_range: &HashMap<(String, u32, u32), SourceOccurrenceId>,
    order: &ProjectOrder,
) -> LoweredCrossModalFacts {
    let source = &document.document;
    let mut output = LoweredCrossModalFacts::default();
    for (binding_index, binding) in document.cross_modal_bindings.iter().enumerate() {
        let lookup = |range: &SourceRange| {
            occurrences_by_range
                .get(&(source.file_id.clone(), range.start_offset, range.end_offset))
                .cloned()
        };
        let (Some(short), Some(anchor)) =
            (lookup(&binding.short_range), lookup(&binding.long_range))
        else {
            continue;
        };
        let Some(anchor_occurrence) = occurrences
            .iter()
            .find(|occurrence| occurrence.id == anchor)
        else {
            continue;
        };
        let Some(short_occurrence) = occurrences.iter().find(|occurrence| occurrence.id == short)
        else {
            continue;
        };
        let entity = EntityId {
            component_id: document.component_id.clone(),
            scope_path: anchor_occurrence.scope_path.clone(),
            kind: match binding.predicate {
                BindingPredicate::Abbreviates => "acronym",
                BindingPredicate::Aliases => "alias",
                BindingPredicate::Names => "named-operator",
            }
            .to_owned(),
            anchor: anchor.clone(),
        };
        let evidence_id = EvidenceId(format!(
            "{}:{}:cross-modal-evidence:{binding_index}",
            source.file_id, source.document_version
        ));
        let available_after = order
            .position(&source.file_id, binding.evidence_range.end_offset)
            .unwrap_or(u64::MAX)
            .max(short_occurrence.availability_order);
        let mut provenance = vec![short.clone(), anchor.clone()];
        provenance.sort();
        provenance.dedup();
        output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: short.clone(),
            scope_path: entity.scope_path.clone(),
            available_after,
            polarity: binding.polarity,
            modality: binding.modality,
            origin: EvidenceOrigin::Explicit,
            provenance,
            parent_claims: Vec::new(),
            rule_id: binding.rule_id.clone(),
            rule_version: 1,
        });
        output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:cross-modal-binding:{binding_index}",
                source.file_id, source.document_version
            )),
            subject: entity.clone(),
            predicate: match binding.predicate {
                BindingPredicate::Abbreviates => ClaimPredicate::Abbreviates,
                BindingPredicate::Aliases => ClaimPredicate::Aliases,
                BindingPredicate::Names => ClaimPredicate::Names,
            },
            object: ClaimObject::Occurrence(short.clone()),
            evidence_id: evidence_id.clone(),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
        output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:cross-modal-definition:{binding_index}",
                source.file_id, source.document_version
            )),
            subject: entity.clone(),
            predicate: ClaimPredicate::Defines,
            object: ClaimObject::Occurrence(anchor),
            evidence_id: evidence_id.clone(),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
        for (type_index, candidate_type) in explicit_candidate_types(&binding.long)
            .into_iter()
            .enumerate()
        {
            output.claims.push(Claim {
                id: ClaimId(format!(
                    "{}:{}:cross-modal-type:{binding_index}:{type_index}",
                    source.file_id, source.document_version
                )),
                subject: entity.clone(),
                predicate: ClaimPredicate::HasType,
                object: ClaimObject::Value(ClaimValue::Type(candidate_type.to_owned())),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            });
        }
        output.definitions.insert(
            entity.clone(),
            DefinitionInfo {
                symbol: binding.short.clone(),
                description: binding.long.clone(),
                location: Location {
                    file_id: source.file_id.clone(),
                    path: source.path.clone(),
                    range: binding.short_range.clone(),
                },
                evidence: Evidence {
                    rule_id: binding.rule_id.clone(),
                    kind: match binding.occurrence_kind {
                        OccurrenceKind::Prose => "explicit-prose",
                        _ => "structural-declaration",
                    }
                    .to_owned(),
                    strength: if binding.modality == EvidenceModality::Asserted {
                        "strong"
                    } else {
                        "contextual"
                    }
                    .to_owned(),
                    source_ranges: vec![binding.evidence_range.clone()],
                },
                entity_id: Some(entity.clone()),
            },
        );
        output.entities.push(entity);
    }
    output
}

fn explicit_candidate_types(description: &str) -> Vec<&'static str> {
    let lower = description.to_ascii_lowercase();
    let normalized = lower
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<HashSet<_>>();
    [
        ("function", "function"),
        ("operator", "operator"),
        ("mapping", "map"),
        ("map", "map"),
        ("metric", "metric"),
        ("estimate", "estimate"),
        ("transform", "transform"),
        ("mean", "mean"),
        ("vector", "vector"),
        ("tensor", "tensor"),
        ("set", "set"),
        ("derivative", "derivative"),
        ("differential", "differential"),
    ]
    .into_iter()
    .filter_map(|(word, value)| normalized.contains(word).then_some(value))
    .take(4)
    .collect()
}

fn notation_occurrence_range(document: &ProjectDocument, selection: &SourceRange) -> SourceRange {
    document
        .nodes
        .iter()
        .filter(|node| notation_identity_contains(document, node, selection, 0))
        .max_by_key(|node| node.ranges.full.end_offset - node.ranges.full.start_offset)
        .map_or_else(|| selection.clone(), |node| node.ranges.full.clone())
}

fn notation_identity_contains(
    document: &ProjectDocument,
    node: &crate::NotationNode,
    selection: &SourceRange,
    depth: u8,
) -> bool {
    if depth == 16 {
        return false;
    }
    let identity = match node.kind {
        crate::NotationNodeKind::NamedOperator => node.ranges.name.as_ref(),
        crate::NotationNodeKind::Modifier | crate::NotationNodeKind::Script => {
            node.ranges.nucleus.as_ref().or_else(|| {
                node.arguments
                    .iter()
                    .find(|argument| argument.role == "nucleus")
                    .map(|argument| &argument.range)
            })
        }
        crate::NotationNodeKind::Style => node
            .arguments
            .iter()
            .find(|argument| argument.role == "body")
            .map(|argument| &argument.range),
        _ => None,
    };
    let Some(identity) = identity else {
        return false;
    };
    if identity.start_offset > selection.start_offset || selection.end_offset > identity.end_offset
    {
        return false;
    }
    node.children
        .iter()
        .filter_map(|child| document.nodes.get(*child as usize))
        .find(|child| {
            identity.start_offset <= child.ranges.full.start_offset
                && child.ranges.full.end_offset <= identity.end_offset
                && child.ranges.full.start_offset <= selection.start_offset
                && selection.end_offset <= child.ranges.full.end_offset
        })
        .is_none_or(|child| identity_descendant_contains(document, child, selection, depth + 1))
}

fn identity_descendant_contains(
    document: &ProjectDocument,
    node: &crate::NotationNode,
    selection: &SourceRange,
    depth: u8,
) -> bool {
    if matches!(
        node.kind,
        crate::NotationNodeKind::NamedOperator
            | crate::NotationNodeKind::Modifier
            | crate::NotationNodeKind::Script
            | crate::NotationNodeKind::Style
    ) {
        return notation_identity_contains(document, node, selection, depth);
    }
    if depth == 16 {
        return false;
    }
    node.children
        .iter()
        .filter_map(|child| document.nodes.get(*child as usize))
        .find(|child| {
            child.ranges.full.start_offset <= selection.start_offset
                && selection.end_offset <= child.ranges.full.end_offset
        })
        .is_none_or(|child| identity_descendant_contains(document, child, selection, depth + 1))
}

fn notation_path(document: &ProjectDocument, range: &SourceRange) -> Vec<u32> {
    let mut candidates = document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.ranges.full.start_offset <= range.start_offset
                && range.end_offset <= node.ranges.full.end_offset
        })
        .map(|(id, node)| {
            (
                id as u32,
                node.ranges.full.end_offset - node.ranges.full.start_offset,
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(_, width)| std::cmp::Reverse(*width));
    candidates.into_iter().map(|(id, _)| id).collect()
}

fn notation_components(
    document: &ProjectDocument,
    range: &SourceRange,
    surface: &str,
) -> Vec<NotationComponent> {
    let mut components = Vec::new();
    for node_id in notation_path(document, range) {
        let node = &document.nodes[node_id as usize];
        match node.kind {
            crate::NotationNodeKind::Modifier => {
                if let Some(name) = &node.name {
                    components.push(NotationComponent::Modifier { name: name.clone() });
                }
            }
            crate::NotationNodeKind::Style => {
                if let Some(name) = &node.name {
                    components.push(NotationComponent::Style { name: name.clone() });
                }
            }
            crate::NotationNodeKind::Script => match node.name.as_deref() {
                Some("superscript") => components.push(NotationComponent::Superscript),
                Some("subscript") => {
                    let base = node
                        .children
                        .first()
                        .map(|child| bounded_notation_text(document, *child, 0))
                        .unwrap_or_default();
                    let index = node
                        .children
                        .get(1)
                        .map(|child| bounded_notation_text(document, *child, 0))
                        .unwrap_or_default();
                    if !base.is_empty() && !index.is_empty() {
                        components.push(NotationComponent::Subscript { base, index });
                    }
                }
                _ => {}
            },
            crate::NotationNodeKind::NamedOperator => {
                components.push(NotationComponent::NamedSurface {
                    value: node.name.clone().unwrap_or_else(|| surface.to_owned()),
                });
            }
            _ => {}
        }
    }
    if !components
        .iter()
        .any(|component| matches!(component, NotationComponent::NamedSurface { .. }))
    {
        components.push(NotationComponent::Identifier {
            value: surface.to_owned(),
        });
    }
    components
}

fn bounded_notation_text(document: &ProjectDocument, node_id: u32, depth: u8) -> String {
    if depth == 8 {
        return String::new();
    }
    let Some(node) = document.nodes.get(node_id as usize) else {
        return String::new();
    };
    if let Some(text) = &node.text {
        return text.clone();
    }
    if node.children.is_empty() {
        return node.name.clone().unwrap_or_default();
    }
    node.children
        .iter()
        .map(|child| bounded_notation_text(document, *child, depth + 1))
        .collect()
}

fn source_text(document: &ProjectDocument, range: &SourceRange) -> String {
    let index = crate::SourceIndex::new(&document.content);
    let start = index.byte_for_utf16(range.start_offset);
    let end = index.byte_for_utf16(range.end_offset);
    document.content.get(start..end).unwrap_or("").to_owned()
}

fn analysis_fingerprint(document: &ProjectDocument) -> u64 {
    let scopes = document
        .scopes
        .iter()
        .map(|scope| {
            (
                &scope.kind,
                scope.parent,
                scope.range.start_offset,
                &scope.name,
                &scope.level,
                &scope.source,
            )
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&(
        document.schema_version,
        document.language,
        &document.nodes,
        &document.math_roots,
        &document.prose_annotations,
        scopes,
        &document.declarations,
        &document.macros,
        &document.includes,
    ))
    .expect("validated syntax snapshots must remain serializable");
    let mut hasher = DefaultHasher::new();
    encoded.hash(&mut hasher);
    hasher.finish()
}

fn compact_analyzed_document(document: &mut ProjectDocument) {
    document.nodes.clear();
    document.math_roots.clear();
    document.visible_prose.clear();
    document.declarations.clear();
    document.macros.clear();
    #[cfg(test)]
    document.math_regions.clear();
}

#[derive(Default)]
pub struct SemathEngine {
    epoch: String,
    inventory_version: u64,
    project_id: String,
    main_file_id: Option<String>,
    analysis_generation: u64,
    index: ProjectState,
}

impl SemathEngine {
    pub fn reset(&mut self, snapshot: ProjectSnapshot) -> Result<UpdateResult, EngineError> {
        let ProjectSnapshot {
            protocol_version,
            epoch,
            inventory_version,
            project_id,
            main_file_id,
            documents,
        } = snapshot;
        self.begin_reset(ProjectSnapshotMetadata {
            protocol_version,
            epoch,
            inventory_version,
            project_id,
            main_file_id,
        })?;
        for document in documents {
            self.ingest_reset_document(document)?;
        }
        self.finish_reset()
    }

    pub fn begin_reset(&mut self, metadata: ProjectSnapshotMetadata) -> Result<(), EngineError> {
        check_protocol(metadata.protocol_version)?;
        self.epoch = metadata.epoch;
        self.inventory_version = metadata.inventory_version;
        self.project_id = metadata.project_id;
        self.main_file_id = metadata.main_file_id;
        self.analysis_generation = 0;
        self.index = ProjectState::default();
        Ok(())
    }

    pub fn ingest_reset_document(&mut self, document: ProjectDocument) -> Result<(), EngineError> {
        self.index.replace(document)
    }

    pub fn finish_reset(&mut self) -> Result<UpdateResult, EngineError> {
        let mut changed_file_ids = self.index.documents.keys().cloned().collect::<Vec<_>>();
        changed_file_ids.sort();
        self.refresh_project_topology();
        self.refresh_project_laws(&changed_file_ids.iter().cloned().collect());
        self.index.rebuild_semantic_index()?;
        Ok(self.update_result(changed_file_ids.clone(), changed_file_ids))
    }

    pub fn apply(&mut self, envelope: ChangeEnvelope) -> Result<UpdateResult, EngineError> {
        check_protocol(envelope.protocol_version)?;
        if envelope.epoch != self.epoch {
            return Err(EngineError::EpochMismatch);
        }
        if envelope.inventory_version <= self.inventory_version {
            return Err(EngineError::StaleInventory);
        }
        self.validate_changes(&envelope.changes)?;
        let requested = envelope
            .changes
            .iter()
            .map(|change| match change {
                ProjectChange::Upsert { document } => document.file_id.clone(),
                ProjectChange::PathChange { file_id, .. } | ProjectChange::Remove { file_id } => {
                    file_id.clone()
                }
            })
            .collect::<HashSet<_>>();
        let mut affected = self.index.order.affected_by(&requested);
        let mut changed = Vec::new();
        let mut revision_only = Vec::new();
        let mut order_changed = false;
        let mut topology_changed = false;
        let mut semantic_changed = false;
        for change in envelope.changes {
            match change {
                ProjectChange::Upsert { document } => {
                    let file_id = document.file_id.clone();
                    let accept = self.accepts_upsert(&document);
                    if accept {
                        let previous_order = self.index.order_document(&file_id);
                        if self.can_reuse_analysis(&document) {
                            let current = self.index.documents.get_mut(&file_id).unwrap();
                            current.scopes = ScopeGraph::new(&document);
                            let mut document = *document;
                            compact_analyzed_document(&mut document);
                            current.document = document;
                            revision_only.push(file_id.clone());
                        } else {
                            self.index.replace(*document)?;
                            semantic_changed = true;
                        }
                        let next_order = self.index.order_document(&file_id);
                        topology_changed |=
                            !same_order_topology(previous_order.as_ref(), next_order.as_ref());
                        order_changed |= previous_order != next_order;
                        changed.push(file_id);
                    }
                }
                ProjectChange::PathChange { file_id, path } => {
                    let document = self.index.documents.get_mut(&file_id).unwrap();
                    order_changed |= document.document.path != path;
                    topology_changed |= document.document.path != path;
                    semantic_changed = true;
                    document.document.path = path;
                    changed.push(file_id);
                }
                ProjectChange::Remove { file_id } => {
                    order_changed = true;
                    topology_changed = true;
                    semantic_changed = true;
                    self.index.remove(&file_id);
                    changed.push(file_id);
                }
            }
        }
        self.inventory_version = envelope.inventory_version;
        self.analysis_generation = envelope.analysis_generation;
        if order_changed {
            self.refresh_project_topology();
            affected.extend(self.index.order.affected_by(&requested));
        }
        let mut analyzed = if semantic_changed || order_changed {
            affected
                .into_iter()
                .filter(|file_id| self.index.documents.contains_key(file_id))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        analyzed.sort();
        if !analyzed.is_empty() {
            self.refresh_project_laws(&analyzed.iter().cloned().collect());
            if topology_changed {
                self.index.rebuild_semantic_index()?;
            }
        }
        if !topology_changed {
            let mut semantic_updates = analyzed.clone();
            semantic_updates.extend(revision_only);
            semantic_updates.sort();
            semantic_updates.dedup();
            if !semantic_updates.is_empty() {
                self.index.replace_semantic_documents(&semantic_updates)?;
            }
        }
        changed.sort();
        Ok(self.update_result(changed, analyzed))
    }

    pub fn query(&self, envelope: QueryEnvelope) -> Result<QueryResult, EngineError> {
        check_protocol(envelope.protocol_version)?;
        if envelope.epoch != self.epoch {
            return Err(EngineError::EpochMismatch);
        }
        if envelope.inventory_version != self.inventory_version {
            return Err(EngineError::StaleInventory);
        }
        let (file_id, query_offset) = match &envelope.query {
            Query::Selection { file_id, offset }
            | Query::SemanticView { file_id, offset }
            | Query::Definition { file_id, offset }
            | Query::References { file_id, offset }
            | Query::PrepareRename { file_id, offset }
            | Query::Rename {
                file_id, offset, ..
            }
            | Query::ExplainDiagnostic {
                file_id, offset, ..
            } => (file_id, Some(*offset)),
            Query::Diagnostics { file_id } => (file_id, None),
        };
        let document = self
            .index
            .documents
            .get(file_id)
            .ok_or_else(|| EngineError::MissingDocument(file_id.clone()))?;
        let observations = &document.observations;
        if document.document.document_version != envelope.document_version {
            return Err(EngineError::DocumentVersionMismatch);
        }
        let offset = query_offset.unwrap_or(0);
        let parsed =
            query_offset.and_then(|offset| parsed_math_at_cursor(&document.parsed, offset));
        let symbol = parsed.and_then(|math| semantic_symbol_at_cursor(document, math, offset));
        let cursor_offset = symbol.as_ref().map_or_else(
            || {
                parsed.map_or(offset, |math| {
                    interior_offset(&math.region.content_range, offset)
                })
            },
            |(_, range)| interior_offset(range, offset),
        );

        let hygiene_enabled = self.index.documents.len() == 1;
        let value = match envelope.query {
            Query::Selection { .. } => {
                let mut ranges = Vec::new();
                if let Some(math) = parsed {
                    selection_path(&math.root, cursor_offset, &mut ranges);
                    if ranges.last() != Some(&math.region.full_range) {
                        ranges.push(math.region.full_range.clone());
                    }
                }
                QueryValue::Selection { ranges }
            }
            Query::SemanticView { .. } => QueryValue::SemanticView {
                view: Box::new(self.semantic_view(
                    document,
                    observations,
                    parsed,
                    symbol.as_ref(),
                    cursor_offset,
                    hygiene_enabled,
                )),
            },
            Query::Definition { .. } => QueryValue::Locations {
                locations: symbol
                    .as_ref()
                    .and_then(|(name, occurrence)| {
                        self.resolve_definition(file_id, occurrence, name)
                    })
                    .map(|definition| vec![definition.location])
                    .unwrap_or_default(),
            },
            Query::References { .. } => QueryValue::Locations {
                locations: symbol
                    .as_ref()
                    .and_then(|(name, occurrence)| {
                        self.resolve_definition(file_id, occurrence, name)
                    })
                    .map(|definition| self.references_for(&definition))
                    .unwrap_or_default(),
            },
            Query::PrepareRename { .. } => prepare_rename(parsed, cursor_offset),
            Query::Rename { new_name, .. } => {
                rename_proposal(document, parsed, cursor_offset, &new_name)
            }
            Query::Diagnostics { .. } => QueryValue::Diagnostics {
                diagnostics: document_diagnostics(
                    document,
                    observations,
                    &self.index.semantic,
                    hygiene_enabled,
                ),
            },
            Query::ExplainDiagnostic { ref code, .. } => QueryValue::DiagnosticExplanation {
                diagnostic: observations
                    .shapes
                    .diagnostic(code, cursor_offset)
                    .or_else(|| observations.roles.diagnostic(code, cursor_offset))
                    .or_else(|| observations.quantities.diagnostic(code, cursor_offset))
                    .or_else(|| {
                        constraint_diagnostics(&self.index.semantic, file_id)
                            .into_iter()
                            .find(|diagnostic| {
                                diagnostic.code == code.as_str()
                                    && diagnostic.range.contains(cursor_offset)
                            })
                    })
                    .or_else(|| {
                        hygiene_enabled
                            .then(|| document.hygiene.diagnostic(code, cursor_offset))
                            .flatten()
                    }),
            },
        };

        Ok(QueryResult {
            protocol_version: PROTOCOL_VERSION,
            epoch: self.epoch.clone(),
            inventory_version: self.inventory_version,
            document_version: document.document.document_version,
            analysis_generation: envelope.analysis_generation,
            value,
        })
    }

    pub fn reset_json(&mut self, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        let result = self.reset(serde_json::from_slice(payload)?)?;
        Ok(serde_json::to_vec(&result)?)
    }

    pub fn begin_reset_json(&mut self, payload: &[u8]) -> Result<(), EngineError> {
        self.begin_reset(serde_json::from_slice(payload)?)
    }

    pub fn ingest_reset_document_json(&mut self, payload: &[u8]) -> Result<(), EngineError> {
        self.ingest_reset_document(serde_json::from_slice(payload)?)
    }

    pub fn finish_reset_json(&mut self) -> Result<Vec<u8>, EngineError> {
        Ok(serde_json::to_vec(&self.finish_reset()?)?)
    }

    pub fn apply_json(&mut self, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        let result = self.apply(serde_json::from_slice(payload)?)?;
        Ok(serde_json::to_vec(&result)?)
    }

    pub fn query_json(&self, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        let result = self.query(serde_json::from_slice(payload)?)?;
        Ok(serde_json::to_vec(&result)?)
    }

    fn update_result(
        &self,
        changed_file_ids: Vec<String>,
        analyzed_file_ids: Vec<String>,
    ) -> UpdateResult {
        let semantic_stats = self.index.semantic.stats();
        UpdateResult {
            protocol_version: PROTOCOL_VERSION,
            epoch: self.epoch.clone(),
            inventory_version: self.inventory_version,
            analysis_generation: self.analysis_generation,
            changed_file_ids,
            stats: AnalysisStats {
                analyzed_documents: analyzed_file_ids.len() as u32,
                total_documents: self.index.documents.len() as u32,
                recognized_laws: self
                    .index
                    .documents
                    .values()
                    .map(|document| document.observations.laws.all().len() as u32)
                    .sum(),
                semantic_nodes: analyzed_file_ids
                    .iter()
                    .filter_map(|file_id| self.index.documents.get(file_id))
                    .flat_map(|document| &document.parsed)
                    .map(|math| equation_node_count(&math.root))
                    .sum(),
                constraints: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).constraint_count())
                    .sum(),
                law_rules_visited: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).laws.visited_rules())
                    .sum(),
                pack_frontier_candidates: analyzed_file_ids
                    .iter()
                    .map(|file_id| {
                        self.index
                            .observations(file_id)
                            .laws
                            .pack_frontier_candidates()
                    })
                    .sum(),
                pack_latent_candidates: analyzed_file_ids
                    .iter()
                    .map(|file_id| {
                        self.index
                            .observations(file_id)
                            .laws
                            .pack_latent_candidates()
                    })
                    .sum(),
                pack_latent_fallbacks: analyzed_file_ids
                    .iter()
                    .map(|file_id| {
                        self.index
                            .observations(file_id)
                            .laws
                            .pack_latent_fallbacks()
                    })
                    .sum(),
                domain_hypotheses: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).domains.hypothesis_count())
                    .sum(),
                domain_evidence: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).domains.evidence_count())
                    .sum(),
                equivalence_states: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).laws.equivalence_states())
                    .sum(),
                equivalence_guard_checks: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).laws.guard_checks())
                    .sum(),
                semantic_occurrences: semantic_stats.occurrences,
                semantic_entities: semantic_stats.entities,
                semantic_claims: semantic_stats.claims,
                semantic_evidence: semantic_stats.evidence,
                semantic_dependency_edges: semantic_stats.dependency_edges,
                invalidated_semantic_claims: semantic_stats.invalidated_claims,
                semantic_candidates: semantic_stats.candidates,
                semantic_constraint_work: semantic_stats.constraint_work,
                semantic_derived_claims: semantic_stats.derived_claims,
                semantic_constraint_truncated: semantic_stats.constraint_truncated,
                prose_clauses: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.observations(file_id).prose_match_stats().clauses)
                    .sum(),
                prose_construction_candidates: analyzed_file_ids
                    .iter()
                    .map(|file_id| {
                        self.index
                            .observations(file_id)
                            .prose_match_stats()
                            .construction_candidates
                    })
                    .sum(),
                prose_matcher_work: analyzed_file_ids
                    .iter()
                    .map(|file_id| {
                        self.index
                            .observations(file_id)
                            .prose_match_stats()
                            .matcher_work
                    })
                    .sum(),
            },
            analyzed_file_ids,
        }
    }

    fn validate_changes(&self, changes: &[ProjectChange]) -> Result<(), EngineError> {
        let mut file_ids = self.index.documents.keys().cloned().collect::<HashSet<_>>();
        for change in changes {
            match change {
                ProjectChange::Upsert { document } => {
                    file_ids.insert(document.file_id.clone());
                }
                ProjectChange::PathChange { file_id, .. } => {
                    if !file_ids.contains(file_id) {
                        return Err(EngineError::MissingDocument(file_id.clone()));
                    }
                }
                ProjectChange::Remove { file_id } => {
                    file_ids.remove(file_id);
                }
            }
        }
        Ok(())
    }

    fn can_reuse_analysis(&self, next: &ProjectDocument) -> bool {
        let Some(current) = self.index.documents.get(&next.file_id) else {
            return false;
        };
        current.document.path == next.path
            && current.document.language == next.language
            && current.document.schema_version == next.schema_version
            && current.analysis_fingerprint == analysis_fingerprint(next)
            && appended_comments_only(&current.document.content, &next.content)
    }

    fn accepts_upsert(&self, next: &ProjectDocument) -> bool {
        let Some(current) = self.index.documents.get(&next.file_id) else {
            return true;
        };
        if next.document_version > current.document.document_version {
            return true;
        }
        next.document_version == current.document.document_version
            && next.content == current.document.content
            && next.path == current.document.path
            && next.language == current.document.language
            && next.schema_version == current.document.schema_version
            && analysis_fingerprint(next) != current.analysis_fingerprint
    }

    fn visible_definitions(
        &self,
        file_id: &str,
        occurrence: &SourceRange,
        _symbol: &str,
    ) -> Vec<DefinitionInfo> {
        self.resolved_entity(file_id, occurrence)
            .and_then(|entity| {
                self.index
                    .definitions_by_entity
                    .get(&entity)
                    .cloned()
                    .map(|mut definition| {
                        definition.entity_id = Some(entity);
                        definition
                    })
            })
            .into_iter()
            .collect()
    }

    fn resolve_definition(
        &self,
        file_id: &str,
        occurrence: &SourceRange,
        symbol: &str,
    ) -> Option<DefinitionInfo> {
        self.visible_definitions(file_id, occurrence, symbol)
            .into_iter()
            .next()
    }

    fn references_for(&self, definition: &DefinitionInfo) -> Vec<Location> {
        let Some(entity) = &definition.entity_id else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        for occurrence in self.index.semantic.occurrences() {
            let resolution = self.index.semantic.resolve(&occurrence.id);
            if resolution.status == ResolutionStatus::Established
                && resolution.candidates.len() == 1
                && resolution.candidates[0].entity_id == *entity
            {
                let document = &self.index.documents[&occurrence.id.file_id];
                locations.push(Location {
                    file_id: occurrence.id.file_id.clone(),
                    path: document.document.path.clone(),
                    range: occurrence.range.clone(),
                });
            }
        }
        locations.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.range.start_offset.cmp(&right.range.start_offset))
        });
        locations
    }

    fn resolved_entity(&self, file_id: &str, range: &SourceRange) -> Option<EntityId> {
        let occurrence_id = self.index.occurrences_by_range.get(&(
            file_id.to_owned(),
            range.start_offset,
            range.end_offset,
        ))?;
        let resolution = self.index.semantic.resolve(occurrence_id);
        (resolution.status == ResolutionStatus::Established && resolution.candidates.len() == 1)
            .then(|| resolution.candidates[0].entity_id.clone())
    }

    fn semantic_entity(&self, file_id: &str, range: &SourceRange) -> Option<EntityId> {
        if let Some(entity) = self.resolved_entity(file_id, range) {
            return Some(entity);
        }
        let occurrence_id = self.index.occurrences_by_range.get(&(
            file_id.to_owned(),
            range.start_offset,
            range.end_offset,
        ))?;
        let occurrence = self.index.semantic.occurrence(occurrence_id)?;
        self.index
            .semantic
            .entities()
            .filter(|entity| entity.anchor.file_id == occurrence_id.file_id)
            .filter(|entity| {
                let anchor_is_within_surface = self
                    .index
                    .semantic
                    .occurrence(&entity.anchor)
                    .is_some_and(|anchor| {
                        range.start_offset <= anchor.range.start_offset
                            && anchor.range.end_offset <= range.end_offset
                    });
                let occurrence_is_provenance = self
                    .index
                    .semantic
                    .claims_for_entity_at(entity, occurrence)
                    .iter()
                    .filter(|claim| claim.tier == InferenceTier::Constraint)
                    .filter_map(|claim| self.index.semantic.evidence(&claim.evidence_id))
                    .any(|evidence| evidence.provenance.contains(occurrence_id));
                anchor_is_within_surface || occurrence_is_provenance
            })
            .filter(|entity| {
                self.index
                    .semantic
                    .claims_for_entity_at(entity, occurrence)
                    .iter()
                    .any(|claim| claim.tier == InferenceTier::Constraint)
            })
            .max_by_key(|entity| {
                let exact = (entity.anchor == *occurrence_id) as usize;
                let provenance = self
                    .index
                    .semantic
                    .claims_for_entity_at(entity, occurrence)
                    .iter()
                    .filter(|claim| claim.tier == InferenceTier::Constraint)
                    .filter_map(|claim| self.index.semantic.evidence(&claim.evidence_id))
                    .filter(|evidence| evidence.provenance.contains(occurrence_id))
                    .count();
                let claims = self
                    .index
                    .semantic
                    .claims_for_entity(entity)
                    .iter()
                    .filter(|claim| claim.tier == InferenceTier::Constraint)
                    .count();
                (exact, provenance, claims)
            })
            .cloned()
    }

    fn semantic_context(
        &self,
        observations: &DocumentSemanticObservations,
        file_id: &str,
        symbol: Option<&(String, SourceRange)>,
        offset: u32,
    ) -> SemanticContextInfo {
        let (definitions, entity_id, symbol_name) = symbol
            .map(|(name, occurrence)| {
                (
                    self.visible_definitions(file_id, occurrence, name),
                    self.semantic_entity(file_id, occurrence),
                    Some(name.clone()),
                )
            })
            .unwrap_or_else(|| (Vec::new(), None, None));
        let mut context = observations.context(
            definitions,
            symbol_name,
            entity_id.clone(),
            offset,
            self.index.external_types.get(file_id),
        );
        let semantic_occurrence = symbol.and_then(|(_, occurrence)| {
            self.index
                .occurrences_by_range
                .get(&(
                    file_id.to_owned(),
                    occurrence.start_offset,
                    occurrence.end_offset,
                ))
                .and_then(|id| self.index.semantic.occurrence(id))
        });
        if let (Some(entity), Some(semantic_occurrence)) = (&entity_id, semantic_occurrence) {
            let context_symbol = context.symbol.clone();
            context.quantities.extend(derived_quantity_infos(
                &self.index.semantic,
                entity,
                context_symbol.as_deref(),
                semantic_occurrence,
            ));
            normalize_quantities(&mut context.quantities);
            append_index_claims(
                &self.index.semantic,
                entity,
                semantic_occurrence,
                &mut context,
            );
        }
        if let Some((_, occurrence)) = symbol
            && let Some(occurrence_id) = self.index.occurrences_by_range.get(&(
                file_id.to_owned(),
                occurrence.start_offset,
                occurrence.end_offset,
            ))
        {
            let mut candidates = self
                .index
                .semantic
                .candidates_for(occurrence_id)
                .into_iter()
                .map(|candidate| SemanticCandidateInfo {
                    candidate_id: candidate.id.0.clone(),
                    family: candidate_family_name(candidate.family).to_owned(),
                    interpretation: candidate.interpretation.clone(),
                    status: candidate_status(
                        &candidate.supporting_claims,
                        &candidate.rejecting_claims,
                    ),
                    range: candidate.range.clone(),
                    supporting_claim_ids: candidate
                        .supporting_claims
                        .iter()
                        .map(|claim| claim.0.clone())
                        .collect(),
                    rejecting_claim_ids: candidate
                        .rejecting_claims
                        .iter()
                        .map(|claim| claim.0.clone())
                        .collect(),
                })
                .collect::<Vec<_>>();
            context.truncated |= candidates.len() > MAX_VIEW_CANDIDATES;
            candidates.truncate(MAX_VIEW_CANDIDATES);
            context.candidates = candidates;
        }
        context
    }

    fn semantic_view(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        parsed: Option<&ParsedMath>,
        symbol: Option<&(String, SourceRange)>,
        offset: u32,
        hygiene_enabled: bool,
    ) -> SemanticViewInfo {
        let symbol_info = symbol.as_ref().and_then(|(name, occurrence)| {
            self.symbol_info(
                document,
                observations,
                name,
                occurrence,
                offset,
                hygiene_enabled,
            )
        });
        let context =
            self.semantic_context(observations, &document.document.file_id, symbol, offset);
        let mut declarations = symbol_info
            .as_ref()
            .into_iter()
            .flat_map(|info| {
                info.definitions
                    .iter()
                    .map(|definition| definition.location.clone())
            })
            .collect::<Vec<_>>();
        let declarations_truncated = declarations.len() > MAX_VIEW_DECLARATIONS;
        declarations.truncate(MAX_VIEW_DECLARATIONS);
        let mut diagnostics = document_diagnostics(
            document,
            observations,
            &self.index.semantic,
            hygiene_enabled,
        )
        .into_iter()
        .filter(|diagnostic| {
            parsed.is_some_and(|math| ranges_overlap(&diagnostic.range, &math.region.content_range))
                || diagnostic.range.contains(offset)
                || diagnostic.evidence.iter().any(|evidence| {
                    evidence.source_ranges.iter().any(|range| {
                        range.contains(offset)
                            || parsed.is_some_and(|math| {
                                ranges_overlap(range, &math.region.content_range)
                            })
                    })
                })
        })
        .collect::<Vec<_>>();
        diagnostics.extend(
            symbol_info
                .as_ref()
                .into_iter()
                .flat_map(|symbol| symbol.diagnostics.iter().cloned()),
        );
        diagnostics.sort_by(|left, right| left.code.cmp(&right.code));
        diagnostics.dedup();
        let diagnostics_truncated = diagnostics.len() > MAX_VIEW_DIAGNOSTICS;
        diagnostics.truncate(MAX_VIEW_DIAGNOSTICS);
        let (domains, domains_truncated) = observations.domains.at(offset);
        let truncated = declarations_truncated
            || diagnostics_truncated
            || domains_truncated
            || context.truncated
            || symbol_info.as_ref().is_some_and(|info| info.truncated);
        let formulas = observations.laws.at(offset);
        let engine_limited = document
            .engine_limited_ranges
            .iter()
            .any(|range| range.contains(offset));
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &formulas,
            symbol: symbol_info.as_ref(),
            candidates: &context.candidates,
            diagnostics: &diagnostics,
            engine_limited,
            truncated,
        });
        SemanticViewInfo {
            decision,
            symbol: symbol_info,
            context,
            declarations,
            diagnostics,
            domains,
            truncated,
        }
    }

    fn symbol_info(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        name: &str,
        occurrence: &SourceRange,
        offset: u32,
        hygiene_enabled: bool,
    ) -> Option<SymbolInfo> {
        let semantic_name = name.trim_start_matches('\\');
        let mut definitions =
            self.visible_definitions(&document.document.file_id, occurrence, name);
        let definitions_truncated = definitions.len() > MAX_SYMBOL_DEFINITIONS;
        definitions.truncate(MAX_SYMBOL_DEFINITIONS);
        let external = self.index.external_types.get(&document.document.file_id);
        let (mut shapes, shapes_truncated) = observations.shapes.claims_at(semantic_name, offset);
        let (mut roles, roles_truncated) = observations.roles.roles_at(semantic_name, offset);
        let (mut quantities, quantities_truncated) =
            observations.quantities.at(semantic_name, offset);
        if let Some(external) = external {
            shapes.extend(external.shapes_at(offset, semantic_name));
            roles.extend(external.roles_at(offset, semantic_name));
            quantities.extend(external.quantities_at(offset, semantic_name));
            shapes.sort_by(|left, right| left.kind.cmp(&right.kind));
            shapes.dedup();
            roles.sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
            roles.dedup();
            quantities.sort_by(|left, right| left.display.cmp(&right.display));
            quantities.dedup();
        }
        let (diagnostics, diagnostics_truncated) = symbol_diagnostics(
            document,
            observations,
            semantic_name,
            offset,
            &shapes,
            &quantities,
            hygiene_enabled,
        );
        let occurrence_id = self.index.occurrences_by_range.get(&(
            document.document.file_id.clone(),
            occurrence.start_offset,
            occurrence.end_offset,
        ))?;
        let semantic_occurrence = self.index.semantic.occurrence(occurrence_id)?;
        let entity_id = self.semantic_entity(&document.document.file_id, occurrence);
        if let Some(entity) = &entity_id {
            shapes.extend(derived_shape_infos(
                &self.index.semantic,
                entity,
                &semantic_occurrence.surface,
                semantic_occurrence,
            ));
            quantities.extend(derived_quantity_infos(
                &self.index.semantic,
                entity,
                Some(&semantic_occurrence.surface),
                semantic_occurrence,
            ));
        }
        shapes.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then(left.dimensions.cmp(&right.dimensions))
        });
        shapes
            .dedup_by(|left, right| left.kind == right.kind && left.dimensions == right.dimensions);
        normalize_quantities(&mut quantities);
        let semantic_quantities_truncated = quantities.len() > MAX_SYMBOL_QUANTITIES;
        quantities.truncate(MAX_SYMBOL_QUANTITIES);
        Some(SymbolInfo {
            symbol: semantic_occurrence.surface.clone(),
            occurrence_id: occurrence_id.clone(),
            notation: semantic_occurrence.notation.clone(),
            source_notation: semantic_occurrence.source_text.clone(),
            entity_id,
            location: Location {
                file_id: document.document.file_id.clone(),
                path: document.document.path.clone(),
                range: semantic_occurrence.range.clone(),
            },
            definitions,
            shapes,
            quantities,
            roles,
            diagnostics,
            truncated: definitions_truncated
                || shapes_truncated
                || quantities_truncated
                || semantic_quantities_truncated
                || roles_truncated
                || diagnostics_truncated,
        })
    }

    fn refresh_project_laws(&mut self, targets: &HashSet<String>) {
        let target_components = targets
            .iter()
            .filter_map(|file_id| self.index.documents.get(file_id))
            .map(|document| document.component_id.clone())
            .collect::<HashSet<_>>();
        let target_symbols = targets
            .iter()
            .filter_map(|file_id| self.index.documents.get(file_id))
            .flat_map(|document| document.parsed.iter())
            .flat_map(|math| math.symbols.iter().map(|(symbol, _)| symbol.clone()))
            .collect::<HashSet<_>>();
        let mut activations = HashMap::<String, Vec<IndexedLawActivation>>::new();
        for (file_id, document) in self
            .index
            .documents
            .iter()
            .filter(|(_, document)| target_components.contains(&document.component_id))
        {
            for activation in document.observations.law_activations() {
                activations
                    .entry(document.component_id.clone())
                    .or_default()
                    .push(IndexedLawActivation {
                        source_offset: evidence_anchor(&activation.evidence),
                        activation: activation.clone(),
                        file_id: file_id.clone(),
                    });
            }
        }
        let facts = self
            .index
            .documents
            .iter()
            .filter(|(_, document)| target_components.contains(&document.component_id))
            .flat_map(|(file_id, document)| {
                let component_id = document.component_id.clone();
                let roles = document
                    .observations
                    .roles
                    .exported()
                    .into_iter()
                    .filter(|role| target_symbols.contains(role.symbol.as_str()))
                    .map({
                        let component_id = component_id.clone();
                        let file_id = file_id.clone();
                        move |role| IndexedTypeFact {
                            component_id: component_id.clone(),
                            source_offset: evidence_anchor(&role.evidence),
                            file_id: file_id.clone(),
                            fact: ExportedTypeFact::Role(role),
                        }
                    });
                let quantities = document
                    .observations
                    .quantities
                    .exported()
                    .into_iter()
                    .filter(|quantity| target_symbols.contains(quantity.symbol.as_str()))
                    .map({
                        let component_id = component_id.clone();
                        let file_id = file_id.clone();
                        move |quantity| IndexedTypeFact {
                            component_id: component_id.clone(),
                            source_offset: evidence_anchor(&quantity.evidence),
                            file_id: file_id.clone(),
                            fact: ExportedTypeFact::Quantity(quantity),
                        }
                    });
                let shapes = document
                    .observations
                    .shapes
                    .exported()
                    .into_iter()
                    .filter(|shape| target_symbols.contains(shape.symbol.as_str()))
                    .map({
                        let component_id = component_id.clone();
                        let file_id = file_id.clone();
                        move |shape| IndexedTypeFact {
                            component_id: component_id.clone(),
                            source_offset: evidence_anchor(&shape.evidence),
                            file_id: file_id.clone(),
                            fact: ExportedTypeFact::Shape(shape),
                        }
                    });
                roles.chain(quantities).chain(shapes)
            })
            .collect::<Vec<_>>();
        let mut facts_by_symbol = HashMap::<String, Vec<IndexedTypeFact>>::new();
        for fact in facts {
            facts_by_symbol
                .entry(fact.fact.symbol().to_owned())
                .or_default()
                .push(fact);
        }

        let environments = targets
            .iter()
            .filter_map(|file_id| {
                let target = self.index.documents.get(file_id)?;
                let mut environment = ExternalTypeEnvironment::default();
                for math in target.parsed.iter().filter(|math| math.region.closed) {
                    let semantic_offset = math.region.content_range.start_offset;
                    let order_offset = math
                        .symbols
                        .first()
                        .map_or(semantic_offset, |(_, range)| range.start_offset);
                    let symbols = math
                        .symbols
                        .iter()
                        .map(|(symbol, _)| symbol.as_str())
                        .collect::<HashSet<_>>();
                    for activation in activations.get(&target.component_id).into_iter().flatten() {
                        if activation.file_id != *file_id
                            && self.index.order.precedes(
                                &activation.file_id,
                                activation.source_offset,
                                file_id,
                                order_offset,
                            )
                        {
                            environment
                                .add_law_activation(semantic_offset, activation.activation.clone());
                        }
                    }
                    for symbol in symbols {
                        for fact in facts_by_symbol.get(symbol).into_iter().flatten() {
                            if fact.file_id == *file_id
                                || fact.component_id != target.component_id
                                || !self.index.order.precedes(
                                    &fact.file_id,
                                    fact.source_offset,
                                    file_id,
                                    order_offset,
                                )
                            {
                                continue;
                            }
                            match &fact.fact {
                                ExportedTypeFact::Role(role) => {
                                    environment.add_role(semantic_offset, role.clone());
                                }
                                ExportedTypeFact::Quantity(quantity) => {
                                    environment.add_quantity(semantic_offset, quantity.clone());
                                }
                                ExportedTypeFact::Shape(shape) => {
                                    environment.add_shape(semantic_offset, shape.clone());
                                }
                            }
                        }
                    }
                }
                Some((file_id.clone(), environment))
            })
            .collect::<Vec<_>>();
        for (file_id, environment) in environments {
            let document = self.index.documents.get(&file_id).unwrap();
            let source = document.document.clone();
            let canonical_expressions = document.canonical_expressions.clone();
            self.index.observations_mut(&file_id).refresh_laws(
                &source,
                &canonical_expressions,
                &environment,
            );
            self.index.external_types.insert(file_id, environment);
        }
    }

    fn refresh_project_topology(&mut self) {
        let project_order = ProjectOrder::new(
            self.index
                .documents
                .keys()
                .filter_map(|file_id| self.index.order_document(file_id))
                .collect(),
            self.main_file_id.as_deref(),
        );
        for (file_id, document) in &mut self.index.documents {
            document.component_id = project_order
                .component_for(file_id)
                .map(str::to_owned)
                .unwrap_or_else(|| file_id.clone());
        }
        self.index.order = project_order;
    }
}

fn compositional_surface(
    document: &ProjectDocument,
    range: &SourceRange,
    surface: &str,
    notation: &[NotationComponent],
) -> String {
    if notation.iter().any(|component| {
        matches!(
            component,
            NotationComponent::Modifier { .. }
                | NotationComponent::Style { .. }
                | NotationComponent::Subscript { .. }
                | NotationComponent::Superscript
        )
    }) {
        return source_text(document, range);
    }
    notation
        .iter()
        .find_map(|component| match component {
            NotationComponent::NamedSurface { value } => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_else(|| surface.to_owned())
}

fn same_order_topology(
    previous: Option<&ProjectOrderDocument>,
    next: Option<&ProjectOrderDocument>,
) -> bool {
    matches!(
        (previous, next),
        (Some(previous), Some(next))
            if previous.path == next.path && previous.includes == next.includes
    )
}

fn evidence_anchor(evidence: &Evidence) -> u32 {
    evidence
        .source_ranges
        .iter()
        .map(|range| range.start_offset)
        .max()
        .unwrap_or_default()
}

fn appended_comments_only(current: &str, next: &str) -> bool {
    let Some(appended) = next.strip_prefix(current) else {
        return false;
    };
    !appended.is_empty()
        && appended.starts_with(char::is_whitespace)
        && appended
            .lines()
            .all(|line| line.trim().is_empty() || line.trim_start().starts_with('%'))
}

fn parsed_math_at_cursor(parsed: &[ParsedMath], offset: u32) -> Option<&ParsedMath> {
    parsed
        .iter()
        .find(|math| math.region.full_range.contains(offset))
        .or_else(|| {
            let mut trailing = parsed.iter().filter(|math| {
                math.region.full_range.start_offset < math.region.full_range.end_offset
                    && math.region.full_range.end_offset == offset
            });
            let selected = trailing.next()?;
            if trailing.next().is_some() {
                return None;
            }
            Some(selected)
        })
        .or_else(|| {
            parsed.iter().find(|math| {
                math.symbols.iter().any(|(_, range)| {
                    range.start_offset < range.end_offset && range.end_offset == offset
                })
            })
        })
}

fn semantic_symbol_at_cursor(
    document: &AnalyzedDocument,
    math: &ParsedMath,
    offset: u32,
) -> Option<(String, SourceRange)> {
    if let Some((symbol, range)) = symbol_range_at_cursor(&math.symbols, offset) {
        return Some((symbol.clone(), range.clone()));
    }
    let candidates = document
        .semantic_occurrences
        .iter()
        .filter(|seed| {
            math.region.full_range.start_offset <= seed.range.start_offset
                && seed.range.end_offset <= math.region.full_range.end_offset
        })
        .collect::<Vec<_>>();
    let ownership = candidates
        .iter()
        .map(|seed| CursorOccurrence {
            occurrence: &seed.range,
            selection: &seed.selection_range,
            application_end: seed.application_end_offset.filter(|candidate| {
                math.region.full_range.start_offset <= seed.selection_range.start_offset
                    && seed.selection_range.end_offset <= math.region.full_range.end_offset
                    && *candidate <= math.region.full_range.end_offset
            }),
        })
        .collect::<Vec<_>>();
    let selected = candidates[occurrence_at_cursor(&ownership, offset)?];
    Some((selected.surface.clone(), selected.selection_range.clone()))
}

fn symbol_range_at_cursor(
    symbols: &[(String, SourceRange)],
    offset: u32,
) -> Option<(&String, &SourceRange)> {
    item_at_cursor_with_trailing_edge(symbols, offset)
}

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

fn equation_node_count(node: &crate::EquationNode) -> u32 {
    1 + node.children.iter().map(equation_node_count).sum::<u32>()
}

fn candidate_family_name(family: CandidateFamily) -> &'static str {
    match family {
        CandidateFamily::Application => "application",
        CandidateFamily::Binder => "binder",
        CandidateFamily::Bracketed => "bracketed",
        CandidateFamily::Decoration => "decoration",
        CandidateFamily::Differential => "differential",
        CandidateFamily::Juxtaposition => "juxtaposition",
        CandidateFamily::Operator => "operator",
        CandidateFamily::Script => "script",
        CandidateFamily::Style => "style",
    }
}

fn candidate_status(supporting: &[ClaimId], rejecting: &[ClaimId]) -> SemanticCandidateStatus {
    match (supporting.is_empty(), rejecting.is_empty()) {
        (false, false) => SemanticCandidateStatus::Conflicting,
        (false, true) => SemanticCandidateStatus::Supported,
        (true, false) => SemanticCandidateStatus::Rejected,
        (true, true) => SemanticCandidateStatus::Unresolved,
    }
}

fn derived_quantity_infos(
    index: &ProjectSemanticIndex,
    entity: &EntityId,
    symbol: Option<&str>,
    occurrence: &SourceOccurrence,
) -> Vec<QuantityInfo> {
    let claims = index.claims_for_entity_at(entity, occurrence);
    let quantity_kind_id = claims.iter().find_map(|claim| match &claim.object {
        ClaimObject::Value(ClaimValue::QuantityKind(value))
            if claim.predicate == ClaimPredicate::HasQuantity =>
        {
            Some(value.clone())
        }
        _ => None,
    });
    let unit_id = claims.iter().find_map(|claim| match &claim.object {
        ClaimObject::Value(ClaimValue::Unit(value))
            if claim.predicate == ClaimPredicate::HasUnit =>
        {
            Some(value.clone())
        }
        _ => None,
    });
    claims
        .into_iter()
        .filter(|claim| claim.tier == InferenceTier::Constraint)
        .filter_map(|claim| {
            let ClaimObject::Value(ClaimValue::Dimension(exponents)) = &claim.object else {
                return None;
            };
            let evidence = index.evidence(&claim.evidence_id)?;
            let dimension = physical_dimension_info(exponents);
            let mut derived_from = evidence
                .provenance
                .iter()
                .filter_map(|source| index.occurrence(source))
                .map(|occurrence| occurrence.surface.clone())
                .collect::<Vec<_>>();
            derived_from.sort();
            derived_from.dedup();
            Some(QuantityInfo {
                symbol: symbol.unwrap_or_default().to_owned(),
                quantity_kind_id: quantity_kind_id.clone(),
                quantity_kind: None,
                unit_id: unit_id.clone(),
                unit: None,
                display: dimension.display.clone(),
                dimension,
                evidence: semantic_evidence(index, evidence, "derived-constraint", "strong"),
                derived_from,
            })
        })
        .collect()
}

fn derived_shape_infos(
    index: &ProjectSemanticIndex,
    entity: &EntityId,
    symbol: &str,
    occurrence: &SourceOccurrence,
) -> Vec<ShapeInfo> {
    index
        .claims_for_entity_at(entity, occurrence)
        .into_iter()
        .filter(|claim| claim.tier == InferenceTier::Constraint)
        .filter_map(|claim| {
            let ClaimObject::Value(ClaimValue::Shape(shape)) = &claim.object else {
                return None;
            };
            let (kind, dimensions, display) = match shape {
                ClaimShape::Scalar => ("scalar", Vec::new(), "Scalar".to_owned()),
                ClaimShape::Vector(dimensions) => (
                    "vector",
                    dimensions.iter().map(ClaimExtent::display).collect(),
                    format!(
                        "Vector[{}]",
                        dimensions
                            .iter()
                            .map(ClaimExtent::display)
                            .collect::<Vec<_>>()
                            .join(" × ")
                    ),
                ),
                ClaimShape::Matrix(dimensions) => (
                    "matrix",
                    dimensions.iter().map(ClaimExtent::display).collect(),
                    format!(
                        "Matrix[{}]",
                        dimensions
                            .iter()
                            .map(ClaimExtent::display)
                            .collect::<Vec<_>>()
                            .join(" × ")
                    ),
                ),
                ClaimShape::Tensor(dimensions) => (
                    "tensor",
                    dimensions.iter().map(ClaimExtent::display).collect(),
                    format!(
                        "Tensor[{}]",
                        dimensions
                            .iter()
                            .map(ClaimExtent::display)
                            .collect::<Vec<_>>()
                            .join(" × ")
                    ),
                ),
                ClaimShape::Function { .. } | ClaimShape::Unknown => return None,
            };
            let evidence = index.evidence(&claim.evidence_id)?;
            Some(ShapeInfo {
                symbol: symbol.to_owned(),
                kind: kind.to_owned(),
                dimensions,
                refinements: Vec::new(),
                display,
                evidence: semantic_evidence(index, evidence, "derived-constraint", "strong"),
            })
        })
        .collect()
}

fn physical_dimension_info(exponents: &[DimensionExponent]) -> PhysicalDimensionInfo {
    let exponents = exponents
        .iter()
        .map(|exponent| DimensionExponentInfo {
            base: exponent.base.clone(),
            numerator: i32::from(exponent.numerator),
            denominator: u32::from(exponent.denominator),
        })
        .collect::<Vec<_>>();
    let display = if exponents.is_empty() {
        "dimensionless".to_owned()
    } else {
        exponents
            .iter()
            .map(
                |exponent| match (exponent.numerator, exponent.denominator) {
                    (1, 1) => exponent.base.clone(),
                    (numerator, 1) => format!("{}^{numerator}", exponent.base),
                    (numerator, denominator) => {
                        format!("{}^({numerator}/{denominator})", exponent.base)
                    }
                },
            )
            .collect::<Vec<_>>()
            .join(" · ")
    };
    PhysicalDimensionInfo { exponents, display }
}

fn normalize_quantities(quantities: &mut Vec<QuantityInfo>) {
    quantities.sort_by(|left, right| {
        left.display
            .cmp(&right.display)
            .then(left.quantity_kind_id.cmp(&right.quantity_kind_id))
            .then(left.unit_id.cmp(&right.unit_id))
    });
    quantities.dedup_by(|left, right| {
        left.dimension == right.dimension
            && left.quantity_kind_id == right.quantity_kind_id
            && left.unit_id == right.unit_id
    });
}

fn semantic_evidence(
    index: &ProjectSemanticIndex,
    evidence: &EvidenceRecord,
    kind: &str,
    strength: &str,
) -> Evidence {
    let mut source_ranges = evidence
        .provenance
        .iter()
        .filter_map(|source| index.occurrence(source))
        .map(|occurrence| occurrence.range.clone())
        .collect::<Vec<_>>();
    source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    source_ranges.dedup();
    Evidence {
        rule_id: evidence.rule_id.clone(),
        kind: kind.to_owned(),
        strength: strength.to_owned(),
        source_ranges,
    }
}

fn append_index_claims(
    index: &ProjectSemanticIndex,
    entity: &EntityId,
    occurrence: &SourceOccurrence,
    context: &mut SemanticContextInfo,
) {
    let mut claims = index
        .claims_for_entity_at(entity, occurrence)
        .into_iter()
        .filter(|claim| claim.tier == InferenceTier::Constraint)
        .filter_map(|claim| {
            let ClaimObject::Value(value) = &claim.object else {
                return None;
            };
            let evidence = index.evidence(&claim.evidence_id)?;
            Some(crate::SemanticClaimInfo {
                claim_id: claim.id.0.clone(),
                predicate: claim_predicate_name(&claim.predicate).into(),
                value: claim_value_display(value)?,
                status: SemanticClaimStatus::Supported,
                evidence: vec![semantic_evidence(
                    index,
                    evidence,
                    "derived-constraint",
                    "strong",
                )],
                conflicts: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    claims.sort_by(|left, right| {
        left.predicate
            .cmp(&right.predicate)
            .then(left.value.cmp(&right.value))
            .then(left.claim_id.cmp(&right.claim_id))
    });
    context.claims.extend(claims);
    context.claims.sort_by(|left, right| {
        left.predicate
            .cmp(&right.predicate)
            .then(left.value.cmp(&right.value))
            .then(left.claim_id.cmp(&right.claim_id))
    });
    context
        .claims
        .dedup_by(|left, right| left.claim_id == right.claim_id);
    context.truncated |= context.claims.len() > MAX_VIEW_CLAIMS;
    context.claims.truncate(MAX_VIEW_CLAIMS);
}

fn claim_predicate_name(predicate: &ClaimPredicate) -> &'static str {
    match predicate {
        ClaimPredicate::Defines => "definition",
        ClaimPredicate::Names => "name",
        ClaimPredicate::Abbreviates => "abbreviation",
        ClaimPredicate::Aliases => "alias",
        ClaimPredicate::HasRole => "concept",
        ClaimPredicate::HasType => "type",
        ClaimPredicate::HasShape => "shape",
        ClaimPredicate::HasDimension => "dimension",
        ClaimPredicate::HasQuantity => "quantity",
        ClaimPredicate::HasUnit => "unit",
        ClaimPredicate::Assumes => "assumption",
        ClaimPredicate::Relates => "relation",
    }
}

fn claim_value_display(value: &ClaimValue) -> Option<String> {
    match value {
        ClaimValue::Concept(value)
        | ClaimValue::Role(value)
        | ClaimValue::Type(value)
        | ClaimValue::Unit(value)
        | ClaimValue::QuantityKind(value)
        | ClaimValue::Scalar(value)
        | ClaimValue::Text(value) => Some(value.clone()),
        ClaimValue::Shape(shape) => Some(match shape {
            ClaimShape::Scalar => "Scalar".into(),
            ClaimShape::Vector(dimensions) => format!(
                "Vector[{}]",
                dimensions
                    .iter()
                    .map(ClaimExtent::display)
                    .collect::<Vec<_>>()
                    .join(" × ")
            ),
            ClaimShape::Matrix(dimensions) => format!(
                "Matrix[{}]",
                dimensions
                    .iter()
                    .map(ClaimExtent::display)
                    .collect::<Vec<_>>()
                    .join(" × ")
            ),
            ClaimShape::Tensor(dimensions) => format!(
                "Tensor[{}]",
                dimensions
                    .iter()
                    .map(ClaimExtent::display)
                    .collect::<Vec<_>>()
                    .join(" × ")
            ),
            ClaimShape::Function { domain, codomain } => format!(
                "Function[{} → {}]",
                claim_value_display(&ClaimValue::Shape(*domain.clone()))?,
                claim_value_display(&ClaimValue::Shape(*codomain.clone()))?
            ),
            ClaimShape::Unknown => "Unknown shape".into(),
        }),
        ClaimValue::Dimension(exponents) => Some(
            exponents
                .iter()
                .map(|exponent| {
                    if exponent.denominator == 1 {
                        format!("{}^{}", exponent.base, exponent.numerator)
                    } else {
                        format!(
                            "{}^{}/{}",
                            exponent.base, exponent.numerator, exponent.denominator
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(" · "),
        ),
        ClaimValue::Condition(condition) => Some(format!("{condition:?}")),
        ClaimValue::Relation(_) => None,
    }
}

fn document_diagnostics(
    document: &AnalyzedDocument,
    observations: &DocumentSemanticObservations,
    semantic: &ProjectSemanticIndex,
    hygiene_enabled: bool,
) -> Vec<SemanticDiagnostic> {
    let mut diagnostics = observations.shapes.diagnostics.clone();
    diagnostics.extend(observations.quantities.diagnostics.iter().cloned());
    diagnostics.extend(observations.roles.diagnostics.iter().cloned());
    diagnostics.extend(constraint_diagnostics(semantic, &document.document.file_id));
    if hygiene_enabled {
        diagnostics.extend(document.hygiene.diagnostics.iter().cloned());
    }
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start_offset);
    diagnostics
}

fn constraint_diagnostics(
    semantic: &ProjectSemanticIndex,
    file_id: &str,
) -> Vec<SemanticDiagnostic> {
    semantic
        .constraint_conflicts_for(file_id)
        .into_iter()
        .filter_map(|conflict| {
            let anchor = semantic.occurrence(&conflict.subject.anchor)?;
            let parent_claims = conflict
                .parent_claims
                .iter()
                .filter_map(|claim_id| semantic.claim(claim_id))
                .collect::<Vec<_>>();
            let shape_labels = parent_claims
                .iter()
                .filter(|claim| claim.predicate == ClaimPredicate::HasShape)
                .filter_map(|claim| match &claim.object {
                    ClaimObject::Value(value) => claim_value_display(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut evidence = conflict
                .parent_claims
                .iter()
                .filter_map(|claim_id| semantic.claim(claim_id))
                .filter_map(|claim| semantic.evidence(&claim.evidence_id))
                .map(|record| {
                    let mut source_ranges = record
                        .provenance
                        .iter()
                        .filter_map(|source| semantic.occurrence(source))
                        .map(|occurrence| occurrence.range.clone())
                        .collect::<Vec<_>>();
                    source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
                    source_ranges.dedup();
                    Evidence {
                        rule_id: record.rule_id.clone(),
                        kind: if record.origin == EvidenceOrigin::Derived {
                            "derived-constraint"
                        } else {
                            "explicit-constraint"
                        }
                        .into(),
                        strength: "hard".into(),
                        source_ranges,
                    }
                })
                .collect::<Vec<_>>();
            evidence.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
            evidence.dedup();
            let (message, explanation) = if conflict.code == "constraint-product-shape-conflict"
                && shape_labels.len() >= 2
            {
                (
                    format!("Cannot multiply {} by {}.", shape_labels[0], shape_labels[1]),
                    "Matrix multiplication requires the left inner dimension to equal the right dimension, but the source establishes them as unequal.".into(),
                )
            } else {
                (
                    "Established semantic constraints are incompatible.".into(),
                    conflict.summary.clone(),
                )
            };
            Some(SemanticDiagnostic {
                code: conflict.code.clone(),
                severity: "warning".into(),
                message,
                explanation,
                range: anchor.range.clone(),
                evidence,
            })
        })
        .collect()
}

fn symbol_diagnostics(
    document: &AnalyzedDocument,
    observations: &DocumentSemanticObservations,
    symbol: &str,
    offset: u32,
    shapes: &[ShapeInfo],
    quantities: &[QuantityInfo],
    hygiene_enabled: bool,
) -> (Vec<SemanticDiagnostic>, bool) {
    let (mut diagnostics, shape_truncated) = observations.shapes.diagnostics_for(offset, shapes);
    let (role_diagnostics, role_truncated) = observations.roles.diagnostics_for(symbol, offset);
    diagnostics.extend(role_diagnostics);
    diagnostics.extend(
        observations
            .quantities
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.range.contains(offset)
                    || diagnostic.evidence.iter().any(|diagnostic_evidence| {
                        quantities.iter().any(|quantity| {
                            diagnostic_evidence
                                .source_ranges
                                .iter()
                                .any(|diagnostic_range| {
                                    quantity
                                        .evidence
                                        .source_ranges
                                        .iter()
                                        .any(|quantity_range| {
                                            ranges_overlap(diagnostic_range, quantity_range)
                                        })
                                })
                        })
                    })
            })
            .cloned(),
    );
    let (hygiene_diagnostics, hygiene_truncated) = if hygiene_enabled {
        document.hygiene.diagnostics_for(symbol, offset)
    } else {
        (Vec::new(), false)
    };
    diagnostics.extend(hygiene_diagnostics);
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start_offset);
    diagnostics.dedup();
    let truncated = shape_truncated
        || role_truncated
        || hygiene_truncated
        || diagnostics.len() > MAX_SYMBOL_DIAGNOSTICS;
    diagnostics.truncate(MAX_SYMBOL_DIAGNOSTICS);
    (diagnostics, truncated)
}

fn prepare_rename(parsed: Option<&ParsedMath>, offset: u32) -> QueryValue {
    let preparation = prepare_rename_info(parsed, offset);
    QueryValue::RenamePreparation {
        range: preparation.range,
        placeholder: preparation.placeholder,
        rejection: preparation.rejection,
    }
}

fn prepare_rename_info(parsed: Option<&ParsedMath>, offset: u32) -> RenamePreparation {
    let Some(parsed) = parsed else {
        return rename_preparation_rejection("The cursor is not inside a math expression.");
    };
    if !parsed.region.closed {
        return rename_preparation_rejection(
            "Finish the math expression before renaming a bound variable.",
        );
    }
    let found = binders(parsed);
    let Some(target) = binder_at(parsed, &found, offset) else {
        return rename_preparation_rejection(
            "Only resolved sum, limit, and quantifier bound variables can be renamed here.",
        );
    };
    RenamePreparation {
        range: parsed
            .symbols
            .iter()
            .find(|(_, range)| range.contains(offset))
            .map(|(_, range)| range.clone()),
        placeholder: Some(target.symbol.clone()),
        rejection: None,
    }
}

fn rename_preparation_rejection(message: &str) -> RenamePreparation {
    RenamePreparation {
        range: None,
        placeholder: None,
        rejection: Some(message.into()),
    }
}

fn rename_proposal(
    document: &AnalyzedDocument,
    parsed: Option<&ParsedMath>,
    offset: u32,
    new_name: &str,
) -> QueryValue {
    let Some(parsed) = parsed else {
        return edit_proposal_rejection("The cursor is not inside a math expression.");
    };
    if !parsed.region.closed {
        return edit_proposal_rejection(
            "Finish the math expression before renaming a bound variable.",
        );
    }
    let found = binders(parsed);
    let Some(target) = binder_at(parsed, &found, offset) else {
        return edit_proposal_rejection(
            "Only resolved sum, limit, and quantifier bound variables can be renamed here.",
        );
    };
    if let Some(rejection) = rename_rejection(parsed, &found, target, new_name) {
        return edit_proposal_rejection(&rejection);
    }
    let occurrences = bound_occurrences(parsed, &found, target);
    let evidence = Evidence {
        rule_id: "capture-avoiding-bound-variable-rename".into(),
        kind: "syntax".into(),
        strength: "hard".into(),
        source_ranges: occurrences.clone(),
    };
    QueryValue::EditProposal {
        proposal: Some(SemanticEditProposal {
            title: format!("Rename bound `{}` to `{new_name}`", target.symbol),
            safety: "deterministic".into(),
            evidence: vec![evidence],
            files: vec![SemanticEditFile {
                file_id: document.document.file_id.clone(),
                path: document.document.path.clone(),
                document_version: document.document.document_version,
                edits: occurrences
                    .into_iter()
                    .map(|range| SemanticTextEdit {
                        range,
                        expected_text: target.symbol.clone(),
                        replacement_text: new_name.into(),
                    })
                    .collect(),
            }],
        }),
        rejection: None,
    }
}

fn edit_proposal_rejection(message: &str) -> QueryValue {
    QueryValue::EditProposal {
        proposal: None,
        rejection: Some(message.into()),
    }
}

fn check_protocol(version: u32) -> Result<(), EngineError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(EngineError::UnsupportedProtocol(version))
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
