use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use thiserror::Error;

use crate::binder::{MathBinder, binders, bound_occurrences};
use crate::candidate::{
    StructuralCandidateOption, append_semantic_candidates, application_end_offset,
    structural_candidate_options,
};
use crate::canonical::{
    SemanticExpr, SemanticExprKind, expression_children, lower_document_region, relation_head,
    render_canonical,
};
use crate::constraint::PlannedConflict;
use crate::cross_modal::{BindingPredicate, CrossModalBinding, extract_cross_modal_bindings};
use crate::cursor::{CursorOccurrence, interior_offset, occurrence_at_cursor};
use crate::decision::{MeaningDecisionInput, decide_meaning, symbol_has_source_meaning};
use crate::entity_policy::{
    AuthorizedEntitySurface, EntityEvidenceDecision, EntityFactDisposition, RenameNotationFamily,
    RenameSourceOccurrence, authorize_entity_surface, decide_fact, plan_entity_rename, refusal,
    refused_authorization,
};
use crate::hygiene::{HygieneAnalysis, analyze_hygiene};
use crate::interpretation::{
    InterpretationEvidenceAuthority, MAX_INTERPRETATION_DISCRIMINATORS, MathInterpretationInput,
    ResolvedInterpretationEvidence, normalize_source_anchors, project_math_interpretations,
};
use crate::law::{ExternalTypeEnvironment, rejected_formula_sign_conflicts};
use crate::parser::{ParsedMath, parse_snapshot, selection_path};
use crate::project_order::{ProjectOrder, ProjectOrderDocument};
use crate::prose::{LawActivationEvidence, ScientificSemanticEvidence, definition_available_from};
use crate::scope::ScopeGraph;
use crate::semantic::DocumentSemanticObservations;
use crate::semantic_index::{
    CandidateFamily, Claim, ClaimComparison, ClaimCondition, ClaimExtent, ClaimId, ClaimObject,
    ClaimOperation, ClaimPredicate, ClaimRelation, ClaimShape, ClaimValue, DimensionExponent,
    DocumentSemanticFacts, EntityId, EvidenceId, EvidenceModality, EvidenceOrigin,
    EvidencePolarity, EvidenceRecord, InferenceTier, Mention, MentionModality, NotationComponent,
    OccurrenceKind, ProjectSemanticIndex, SourceOccurrence, SourceOccurrenceId,
    occurrence_binding_key,
};
use crate::{
    AnalysisStats, AssumptionInfo, ChangeEnvelope, ConceptInfo, ConventionalCandidateDisposition,
    ConventionalCandidateInfo, ConventionalRequirementInfo, DefinitionInfo, DimensionExponentInfo,
    DomainActivation, EntitySurfaceRefusal, EntitySurfaceRefusalKind, Evidence, LawBindingProof,
    LawRecognition, LawRecognitionStatus, Location, MathApproximationInfo, MathAuthoringContext,
    MathAuthoringDisposition, MathAuthoringRequirementInfo, MathClaimEvidenceLinkInfo,
    MathClaimModality, MathClaimPolarity, MathClaimStrengthCeiling, MathEquationLinkInfo,
    MathEquationLinkKind, MathExactness, MathFormulaAnchorInfo,
    MathInterpretationEvidenceSourceAnchorInfo, MathInterpretationSourceLifecycle,
    MathNotationOccurrenceInfo, MathSourceFreshness, MathSourceGeneration, MathSourceLifecycleInfo,
    MeaningAlternative, MeaningConflict, MeaningDecision, PROTOCOL_VERSION, PhysicalDimensionInfo,
    ProjectChange, ProjectDocument, ProjectSnapshot, ProjectSnapshotMetadata, QuantityInfo, Query,
    QueryEnvelope, QueryResult, QueryValue, RoleInfo, SemanticCandidateInfo,
    SemanticCandidateStatus, SemanticClaimStatus, SemanticContextInfo, SemanticDiagnostic,
    SemanticEditFile, SemanticEditProposal, SemanticTextEdit, SemanticViewInfo, ShapeInfo,
    SourceRange, SymbolInfo, UpdateResult,
};

const MAX_SYMBOL_DEFINITIONS: usize = 8;
const MAX_SYMBOL_DIAGNOSTICS: usize = 8;
const MAX_SYMBOL_QUANTITIES: usize = 8;
const MAX_VIEW_DIAGNOSTICS: usize = 8;
const MAX_VIEW_DECLARATIONS: usize = 16;
const MAX_VIEW_CANDIDATES: usize = 16;
const MAX_CONVENTIONAL_CANDIDATES: usize = 8;
const MAX_CONVENTIONAL_REQUIREMENTS: usize = 16;
const MAX_AUTHORING_ITEMS: usize = 16;
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
    formula_ranges: Vec<SourceRange>,
    semantic_occurrences: Vec<SemanticOccurrenceSeed>,
    cross_modal_bindings: Vec<CrossModalBinding>,
    engine_limited_ranges: Vec<SourceRange>,
    observations: DocumentSemanticObservations,
}

#[derive(Clone, Debug)]
struct SourceLinkedFormula {
    recognition: LawRecognition,
    shared_entities: Vec<EntityId>,
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

#[derive(Clone)]
struct IndexedAssumption {
    assumption: AssumptionInfo,
    component_id: String,
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
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let observations =
            DocumentSemanticObservations::build(&document, &parsed, &canonical_expressions);
        let hygiene = analyze_hygiene(&document, &parsed, &observations.definitions);
        let mut semantic_occurrences: Vec<SemanticOccurrenceSeed> = parsed
            .iter()
            .flat_map(|math| &math.symbols)
            .filter(|(surface, selection_range)| {
                semantic_occurrence_is_meaningful(&document, surface, selection_range)
            })
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
        semantic_occurrences.sort_by(|left, right| {
            (
                left.selection_range.start_offset,
                left.selection_range.end_offset,
                left.range.start_offset,
                left.range.end_offset,
                left.surface.as_str(),
            )
                .cmp(&(
                    right.selection_range.start_offset,
                    right.selection_range.end_offset,
                    right.range.start_offset,
                    right.range.end_offset,
                    right.surface.as_str(),
                ))
        });
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
        let formula_ranges = if document.math_roots.is_empty() {
            parsed
                .iter()
                .map(|math| math.region.content_range.clone())
                .collect()
        } else {
            document
                .math_roots
                .iter()
                .map(|root| root.content_range.clone())
                .collect()
        };
        compact_analyzed_document(&mut document);
        Ok(Self {
            component_id: document.file_id.clone(),
            document,
            parsed,
            hygiene,
            scopes,
            analysis_fingerprint,
            canonical_expressions,
            formula_ranges,
            semantic_occurrences,
            cross_modal_bindings,
            engine_limited_ranges,
            observations,
        })
    }
}

fn semantic_occurrence_is_meaningful(
    document: &ProjectDocument,
    surface: &str,
    selection: &SourceRange,
) -> bool {
    if let Some(name) = surface.strip_prefix('\\')
        && (crate::canonical::is_ignorable_command(Some(name))
            || crate::canonical::is_math_class_wrapper(Some(name)))
    {
        return false;
    }
    !document.nodes.iter().any(|node| {
        node.kind == crate::NotationNodeKind::Command
            && node.ranges.command.as_ref().or(node.ranges.name.as_ref()) == Some(selection)
            && (crate::canonical::is_ignorable_command(node.name.as_deref())
                || crate::canonical::is_math_class_wrapper(node.name.as_deref()))
    })
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
    occurrences_by_range: OccurrenceRangeIndex,
}

type OccurrenceRangeKey = (String, u32, u32);
type OccurrenceRangeIndex = HashMap<OccurrenceRangeKey, Vec<SourceOccurrenceId>>;

#[derive(Clone, Debug)]
struct CursorFocus {
    name: String,
    range: SourceRange,
    occurrence_id: SourceOccurrenceId,
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

    fn occurrence_id_for_range(
        &self,
        file_id: &str,
        range: &SourceRange,
    ) -> Option<SourceOccurrenceId> {
        let ids = self.occurrences_by_range.get(&(
            file_id.to_owned(),
            range.start_offset,
            range.end_offset,
        ))?;
        let mut exact = ids
            .iter()
            .filter_map(|id| self.semantic.occurrence(id))
            .filter(|occurrence| occurrence.range == *range)
            .collect::<Vec<_>>();
        if exact.is_empty() {
            exact = ids
                .iter()
                .filter_map(|id| self.semantic.occurrence(id))
                .filter(|occurrence| occurrence.selection_range == *range)
                .collect();
        }
        (exact.len() == 1).then(|| exact[0].id.clone())
    }

    fn cursor_focus(&self, file_id: &str, seed: &SemanticOccurrenceSeed) -> Option<CursorFocus> {
        let mut ids = self
            .occurrences_by_range
            .get(&(
                file_id.to_owned(),
                seed.range.start_offset,
                seed.range.end_offset,
            ))?
            .iter()
            .filter_map(|id| self.semantic.occurrence(id))
            .filter(|occurrence| {
                occurrence.kind == seed.kind
                    && occurrence.range == seed.range
                    && occurrence.selection_range == seed.selection_range
                    && occurrence.surface == seed.surface
            })
            .collect::<Vec<_>>();
        ids.sort_by_key(|occurrence| occurrence.id.local_id);
        ids.dedup_by_key(|occurrence| occurrence.id.clone());
        let occurrence = (ids.len() == 1).then(|| ids[0])?;
        Some(CursorFocus {
            name: occurrence.surface.clone(),
            range: occurrence.range.clone(),
            occurrence_id: occurrence.id.clone(),
        })
    }

    fn cursor_focus_at(&self, file_id: &str, offset: u32) -> Option<CursorFocus> {
        let document = self.documents.get(file_id)?;
        let source_length = document.document.content.encode_utf16().count() as u32;
        let candidates = self
            .semantic
            .occurrences_for_file(file_id)
            .filter(|occurrence| {
                !occurrence.notation.is_empty()
                    || self
                        .semantic
                        .occurrence_has_explicit_identity(&occurrence.id)
            })
            .collect::<Vec<_>>();
        let ownership = candidates
            .iter()
            .map(|occurrence| CursorOccurrence {
                occurrence: &occurrence.range,
                selection: &occurrence.selection_range,
                application_end: document
                    .semantic_occurrences
                    .iter()
                    .find(|seed| {
                        seed.kind == occurrence.kind
                            && seed.range == occurrence.range
                            && seed.selection_range == occurrence.selection_range
                            && seed.surface == occurrence.surface
                    })
                    .and_then(|seed| seed.application_end_offset)
                    .filter(|end| *end <= source_length),
            })
            .collect::<Vec<_>>();
        let occurrence = candidates.get(occurrence_at_cursor(&ownership, offset)?)?;
        Some(CursorFocus {
            name: occurrence.surface.clone(),
            range: occurrence.range.clone(),
            occurrence_id: occurrence.id.clone(),
        })
    }

    fn order_document(&self, file_id: &str) -> Option<ProjectOrderDocument> {
        let document = self.documents.get(file_id)?;
        let observations = &document.observations;
        Some(ProjectOrderDocument {
            file_id: file_id.to_owned(),
            includes: document
                .document
                .includes
                .iter()
                .chain(&observations.project_references)
                .cloned()
                .collect(),
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
                .chain(
                    observations
                        .assumptions()
                        .iter()
                        .flat_map(|assumption| assumption.evidence.source_ranges.iter())
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
    occurrences: OccurrenceRangeIndex,
}

fn index_occurrence_range(
    index: &mut OccurrenceRangeIndex,
    key: OccurrenceRangeKey,
    occurrence_id: SourceOccurrenceId,
) {
    let occurrences = index.entry(key).or_default();
    if !occurrences.contains(&occurrence_id) {
        occurrences.push(occurrence_id);
    }
}

fn occurrence_id_at_range(
    index: &OccurrenceRangeIndex,
    occurrences: &[SourceOccurrence],
    file_id: &str,
    range: &SourceRange,
) -> Option<SourceOccurrenceId> {
    let ids = index.get(&(file_id.to_owned(), range.start_offset, range.end_offset))?;
    let mut exact = ids
        .iter()
        .filter_map(|id| occurrences.iter().find(|occurrence| occurrence.id == *id))
        .filter(|occurrence| occurrence.range == *range)
        .collect::<Vec<_>>();
    if exact.is_empty() {
        exact = ids
            .iter()
            .filter_map(|id| occurrences.iter().find(|occurrence| occurrence.id == *id))
            .filter(|occurrence| occurrence.selection_range == *range)
            .collect();
    }
    (exact.len() == 1).then(|| exact[0].id.clone())
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
                availability_order: expression_availability_order(document, &range, order, output),
                surface: source_text(document, &range),
                source_text: source_text(document, &range),
                selection_text: source_text(document, &range),
                notation: Vec::new(),
            },
            Vec::new(),
        ));
    }
}

fn expression_availability_order(
    document: &ProjectDocument,
    range: &SourceRange,
    order: &ProjectOrder,
    occurrences: &[(SourceOccurrence, Vec<StructuralCandidateOption>)],
) -> u64 {
    order
        .position(&document.file_id, range.start_offset)
        .or_else(|| {
            occurrences
                .iter()
                .filter(|(occurrence, _)| {
                    range.start_offset <= occurrence.range.start_offset
                        && occurrence.range.end_offset <= range.end_offset
                })
                .map(|(occurrence, _)| occurrence.availability_order)
                .filter(|position| *position != u64::MAX)
                .max()
        })
        .unwrap_or(u64::MAX)
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
    let mut occurrences_by_range = OccurrenceRangeIndex::new();
    for (local_id, seed) in document.semantic_occurrences.iter().enumerate() {
        let id = SourceOccurrenceId {
            file_id: source.file_id.clone(),
            document_version: source.document_version,
            local_id: local_id as u32,
        };
        index_occurrence_range(
            &mut occurrences_by_range,
            (
                source.file_id.clone(),
                seed.selection_range.start_offset,
                seed.selection_range.end_offset,
            ),
            id.clone(),
        );
        index_occurrence_range(
            &mut occurrences_by_range,
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
                selection_text: source_text(source, &seed.selection_range),
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
        index_occurrence_range(
            &mut occurrences_by_range,
            (
                source.file_id.clone(),
                occurrence.range.start_offset,
                occurrence.range.end_offset,
            ),
            occurrence.id.clone(),
        );
        index_occurrence_range(
            &mut occurrences_by_range,
            (
                source.file_id.clone(),
                occurrence.selection_range.start_offset,
                occurrence.selection_range.end_offset,
            ),
            occurrence.id.clone(),
        );
    }
    let mut occurrences = occurrences
        .into_iter()
        .map(|(occurrence, _)| occurrence)
        .collect::<Vec<_>>();
    let document_binders = document
        .parsed
        .iter()
        .filter(|math| math.region.closed)
        .flat_map(binders)
        .collect::<Vec<_>>();
    append_binder_scope_paths(&mut occurrences, &document_binders);

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
    let binder_facts = lower_binder_facts(document, observations, &occurrences, &document_binders);
    definitions.extend(binder_facts.definitions);
    entities.extend(binder_facts.entities);
    evidence.extend(binder_facts.evidence);
    claims.extend(binder_facts.claims);
    let cross_modal = lower_cross_modal_facts(document, &occurrences, &occurrences_by_range, order);
    definitions.extend(
        cross_modal
            .definitions
            .iter()
            .map(|(entity, definition)| (entity.clone(), definition.clone())),
    );
    let mut relations = lower_canonical_relation_facts(
        source,
        &document.canonical_expressions,
        &occurrences,
        &definitions,
        observations.semantic_evidence(),
    );
    let typed = lower_typed_observation_facts(
        source,
        observations,
        &occurrences,
        &definitions,
        &relations.entities,
    );
    let law_roles =
        lower_law_derived_role_facts(source, observations, &occurrences, &definitions, &relations);
    relations.prune_unreferenced_entities(&typed.claims);
    evidence.extend(typed.evidence);
    claims.extend(typed.claims);
    entities.extend(relations.entities);
    evidence.extend(relations.evidence);
    claims.extend(relations.claims);
    evidence.extend(law_roles.evidence);
    claims.extend(law_roles.claims);
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

const BINDER_SCOPE_MARKER: u32 = u32::MAX;

fn append_binder_scope_paths(occurrences: &mut [SourceOccurrence], binders: &[MathBinder]) {
    for occurrence in occurrences {
        let mut enclosing = binders
            .iter()
            .filter(|binder| {
                binder.declaration == occurrence.selection_range
                    || binder
                        .scope
                        .contains(occurrence.selection_range.start_offset)
            })
            .collect::<Vec<_>>();
        enclosing.sort_by_key(|binder| {
            (
                binder.declaration.start_offset,
                u32::MAX - (binder.scope.end_offset - binder.scope.start_offset),
            )
        });
        if enclosing.is_empty() {
            continue;
        }
        occurrence.scope_path.push(BINDER_SCOPE_MARKER);
        occurrence.scope_path.extend(
            enclosing
                .into_iter()
                .map(|binder| binder.declaration.start_offset),
        );
    }
}

#[derive(Default)]
struct LoweredBinderFacts {
    entities: Vec<EntityId>,
    evidence: Vec<EvidenceRecord>,
    claims: Vec<Claim>,
    definitions: BTreeMap<EntityId, DefinitionInfo>,
}

fn lower_binder_facts(
    document: &AnalyzedDocument,
    observations: &DocumentSemanticObservations,
    occurrences: &[SourceOccurrence],
    binders: &[MathBinder],
) -> LoweredBinderFacts {
    let source = &document.document;
    let mut output = LoweredBinderFacts::default();
    for (binder_index, binder) in binders.iter().enumerate() {
        let occurrence_at = |range: &SourceRange| {
            occurrences
                .iter()
                .filter(|occurrence| {
                    occurrence.kind == OccurrenceKind::Notation
                        && occurrence.selection_range == *range
                        && source_text(source, &occurrence.selection_range) == binder.symbol
                })
                .min_by_key(|occurrence| {
                    (
                        occurrence.range.end_offset - occurrence.range.start_offset,
                        occurrence.id.local_id,
                    )
                })
        };
        let Some(declaration) = occurrence_at(&binder.declaration) else {
            continue;
        };
        let mut bound = document
            .parsed
            .iter()
            .filter(|math| {
                math.region.closed
                    && math
                        .region
                        .full_range
                        .contains(binder.declaration.start_offset)
            })
            .flat_map(|math| bound_occurrences(math, binders, binder))
            .filter_map(|range| occurrence_at(&range))
            .collect::<Vec<_>>();
        bound.sort_by_key(|occurrence| occurrence.id.local_id);
        bound.dedup_by_key(|occurrence| occurrence.id.clone());
        if bound.is_empty() {
            continue;
        }
        let entity = EntityId {
            component_id: document.component_id.clone(),
            scope_path: declaration.scope_path.clone(),
            kind: format!("binder:{}", binder.kind),
            anchor: declaration.id.clone(),
        };
        let evidence_id = EvidenceId(format!(
            "{}:{}:binder-evidence:{binder_index}",
            source.file_id, source.document_version
        ));
        let (polarity, modality) = observations
            .semantic_evidence()
            .formula_disposition(&binder.scope);
        output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: declaration.id.clone(),
            scope_path: declaration.scope_path.clone(),
            available_after: declaration.availability_order,
            polarity,
            modality,
            origin: EvidenceOrigin::Explicit,
            provenance: bound
                .iter()
                .map(|occurrence| occurrence.id.clone())
                .collect(),
            parent_claims: Vec::new(),
            rule_id: "semath/structural-binder-identity".into(),
            rule_version: 1,
        });
        output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:binder-definition:{binder_index}",
                source.file_id, source.document_version
            )),
            subject: entity.clone(),
            predicate: ClaimPredicate::Defines,
            object: ClaimObject::Occurrence(declaration.id.clone()),
            evidence_id: evidence_id.clone(),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
        for (occurrence_index, occurrence) in bound.iter().enumerate() {
            output.claims.push(Claim {
                id: ClaimId(format!(
                    "{}:{}:binder-name:{binder_index}:{occurrence_index}",
                    source.file_id, source.document_version
                )),
                subject: entity.clone(),
                predicate: ClaimPredicate::Names,
                object: ClaimObject::Occurrence(occurrence.id.clone()),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::ExplicitClaim,
                derivation_depth: 0,
            });
        }
        output.definitions.insert(
            entity.clone(),
            DefinitionInfo {
                symbol: binder.symbol.clone(),
                description: format!("{} bound variable", binder.kind),
                location: Location {
                    file_id: source.file_id.clone(),
                    path: source.path.clone(),
                    range: binder.declaration.clone(),
                },
                evidence: Evidence {
                    rule_id: "semath/structural-binder-identity".into(),
                    kind: "structural-declaration".into(),
                    strength: "strong".into(),
                    source_ranges: vec![binder.declaration.clone()],
                    source_anchors: Vec::new(),
                },
                entity_id: Some(entity.clone()),
            },
        );
        output.entities.push(entity);
    }
    output
}

fn definition_anchor(
    definition: &DefinitionInfo,
    file_id: &str,
    occurrences: &[SourceOccurrence],
    occurrences_by_range: &OccurrenceRangeIndex,
) -> Option<SourceOccurrenceId> {
    let range = &definition.location.range;
    if let Some(exact) = occurrence_id_at_range(occurrences_by_range, occurrences, file_id, range) {
        return Some(exact);
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

fn lower_law_derived_role_facts(
    source: &ProjectDocument,
    observations: &DocumentSemanticObservations,
    occurrences: &[SourceOccurrence],
    definitions: &BTreeMap<EntityId, DefinitionInfo>,
    relations: &LoweredRelationFacts,
) -> LoweredTypedFacts {
    let mut output = LoweredTypedFacts::default();
    for (role, formula_range) in observations.laws.retained_roles() {
        let entity = closest_definition(definitions, &role.symbol, &role.evidence)
            .map(|(entity, _)| entity.clone())
            .or_else(|| {
                closest_relation_entity(
                    &relations.entities,
                    occurrences,
                    &role.symbol,
                    &role.evidence,
                )
            });
        let Some(entity) = entity else { continue };
        let mut parent_claims = relations
            .claims
            .iter()
            .filter(|claim| {
                relations.ranges.get(&claim.id).is_some_and(|range| {
                    ranges_overlap(range, &formula_range)
                        || role
                            .evidence
                            .source_ranges
                            .iter()
                            .any(|evidence| ranges_overlap(range, evidence))
                })
            })
            .map(|claim| claim.id.clone())
            .take(16)
            .collect::<Vec<_>>();
        parent_claims.sort();
        parent_claims.dedup();
        if parent_claims.is_empty() {
            continue;
        }
        let parent_evidence = parent_claims
            .iter()
            .filter_map(|parent_id| {
                let claim = relations
                    .claims
                    .iter()
                    .find(|claim| claim.id == *parent_id)?;
                relations
                    .evidence
                    .iter()
                    .find(|evidence| evidence.id == claim.evidence_id)
            })
            .collect::<Vec<_>>();
        let Some(source_occurrence) = parent_evidence
            .iter()
            .filter_map(|evidence| {
                occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == evidence.source)
            })
            .max_by_key(|occurrence| occurrence.availability_order)
        else {
            continue;
        };
        let available_after = parent_evidence
            .iter()
            .map(|evidence| evidence.available_after)
            .max()
            .unwrap_or(source_occurrence.availability_order);
        let mut provenance = parent_evidence
            .iter()
            .map(|evidence| evidence.source.clone())
            .collect::<Vec<_>>();
        provenance.sort();
        provenance.dedup();
        let ordinal = output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:law-role-evidence:{ordinal}",
            source.file_id, source.document_version
        ));
        output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: source_occurrence.id.clone(),
            scope_path: source_occurrence.scope_path.clone(),
            available_after,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Derived,
            provenance,
            parent_claims,
            rule_id: role.evidence.rule_id,
            rule_version: 1,
        });
        output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:law-role-claim:{ordinal}",
                source.file_id, source.document_version
            )),
            subject: entity,
            predicate: ClaimPredicate::HasRole,
            object: ClaimObject::Value(ClaimValue::Concept(role.concept_id)),
            evidence_id,
            tier: InferenceTier::DerivedLaw,
            derivation_depth: 1,
        });
    }
    output
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
    ranges: BTreeMap<ClaimId, SourceRange>,
}

impl LoweredRelationFacts {
    fn prune_unreferenced_entities(&mut self, typed_claims: &[Claim]) {
        let mut referenced = BTreeSet::new();
        for claim in self.claims.iter().chain(typed_claims) {
            referenced.insert(claim.subject.clone());
            if let ClaimObject::Entity(entity) = &claim.object {
                referenced.insert(entity.clone());
            }
            if let ClaimObject::Value(ClaimValue::Relation(relation)) = &claim.object {
                referenced.extend(relation.entities().into_iter().cloned());
            }
        }
        self.entities.retain(|entity| referenced.contains(entity));
    }
}

struct RelationLowerer<'a> {
    source: &'a ProjectDocument,
    occurrences: &'a [SourceOccurrence],
    definitions: &'a BTreeMap<EntityId, DefinitionInfo>,
    semantic_evidence: &'a ScientificSemanticEvidence,
    output: LoweredRelationFacts,
    entities_by_expression: BTreeMap<(u32, u32, String), EntityId>,
    implicit_entities_by_identity: BTreeMap<(Vec<u32>, String), EntityId>,
}

fn lower_canonical_relation_facts(
    source: &ProjectDocument,
    expressions: &[SemanticExpr],
    occurrences: &[SourceOccurrence],
    definitions: &BTreeMap<EntityId, DefinitionInfo>,
    semantic_evidence: &ScientificSemanticEvidence,
) -> LoweredRelationFacts {
    let mut lowerer = RelationLowerer {
        source,
        occurrences,
        definitions,
        semantic_evidence,
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
    fn disposition_for(&self, anchor: &SourceOccurrenceId) -> (EvidencePolarity, EvidenceModality) {
        self.occurrences
            .iter()
            .find(|occurrence| occurrence.id == *anchor)
            .map_or(
                (EvidencePolarity::Positive, EvidenceModality::Asserted),
                |occurrence| {
                    self.semantic_evidence
                        .formula_disposition(&occurrence.range)
                },
            )
    }

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
        if !relation.exceeds_entity_bound() {
            self.emit_relation(result.clone(), relation, &expression.range);
        }
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
        let anchor = self.occurrence_for_range(&expression.range)?.clone();
        let entity = canonical_expression_entity(expression, &anchor);
        self.output.entities.push(entity.clone());
        self.entities_by_expression.insert(key, entity.clone());
        if matches!(expression.kind, SemanticExprKind::Derivative { .. }) {
            self.emit_composite_identity(&entity, &anchor, "derivative");
        }
        Some(entity)
    }

    fn emit_composite_identity(
        &mut self,
        entity: &EntityId,
        occurrence: &SourceOccurrence,
        kind: &str,
    ) {
        let ordinal = self.output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:canonical-{kind}-identity-evidence:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        let (polarity, modality) = self
            .semantic_evidence
            .formula_disposition(&occurrence.range);
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: occurrence.id.clone(),
            scope_path: occurrence.scope_path.clone(),
            available_after: occurrence.availability_order,
            polarity,
            modality,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![occurrence.id.clone()],
            parent_claims: Vec::new(),
            rule_id: format!("semath/canonical-{kind}-identity"),
            rule_version: 1,
        });
        self.output.claims.push(Claim {
            id: ClaimId(format!(
                "{}:{}:canonical-{kind}-identity-claim:{ordinal}",
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
        let (polarity, modality) = self
            .semantic_evidence
            .formula_disposition(&occurrence.range);
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: occurrence.id.clone(),
            scope_path: occurrence.scope_path.clone(),
            available_after: occurrence.availability_order,
            polarity,
            modality,
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

    fn emit_relation(&mut self, subject: EntityId, relation: ClaimRelation, range: &SourceRange) {
        let ordinal = self.output.claims.len();
        let evidence_id = EvidenceId(format!(
            "{}:{}:canonical-relation-evidence:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        let claim_id = ClaimId(format!(
            "{}:{}:canonical-relation-claim:{ordinal}",
            self.source.file_id, self.source.document_version
        ));
        let source_occurrence = self
            .occurrences
            .iter()
            .find(|occurrence| occurrence.range == *range)
            .or_else(|| {
                self.occurrences.iter().find(|occurrence| {
                    occurrence.range.start_offset <= range.start_offset
                        && range.end_offset <= occurrence.range.end_offset
                })
            })
            .or_else(|| {
                self.occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == subject.anchor)
            })
            .expect("relation subjects have a source occurrence");
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
            .chain(
                self.occurrences
                    .iter()
                    .filter(|occurrence| {
                        range.start_offset <= occurrence.range.start_offset
                            && occurrence.range.end_offset <= range.end_offset
                    })
                    .map(|occurrence| occurrence.availability_order),
            )
            .max()
            .unwrap_or(0);
        let (polarity, modality) = self.disposition_for(&source_occurrence.id);
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: source_occurrence.id.clone(),
            scope_path: source_occurrence.scope_path.clone(),
            available_after,
            polarity,
            modality,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![source_occurrence.id.clone()],
            parent_claims: Vec::new(),
            rule_id: "semath/canonical-relation".into(),
            rule_version: 1,
        });
        self.output.ranges.insert(claim_id.clone(), range.clone());
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
        let (polarity, modality) = self.disposition_for(&subject.anchor);
        self.output.evidence.push(EvidenceRecord {
            id: evidence_id.clone(),
            source: subject.anchor.clone(),
            scope_path: subject.scope_path.clone(),
            available_after: self
                .occurrences
                .iter()
                .find(|occurrence| occurrence.id == subject.anchor)
                .map_or(0, |occurrence| occurrence.availability_order),
            polarity,
            modality,
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

fn canonical_expression_entity(expression: &SemanticExpr, anchor: &SourceOccurrence) -> EntityId {
    EntityId {
        component_id: anchor.component_id.clone(),
        scope_path: anchor.scope_path.clone(),
        kind: format!(
            "expression:{}",
            stable_text_digest(&render_canonical(expression))
        ),
        anchor: anchor.id.clone(),
    }
}

fn expression_carries_formula_fact(expression: &SemanticExpr) -> bool {
    matches!(
        expression.kind,
        SemanticExprKind::Power(_, ref exponent)
            if matches!(exponent.kind, SemanticExprKind::Number(_))
    )
}

fn canonical_expression_owner<'a>(
    relation: &'a SemanticExpr,
    focus: &SourceRange,
    structurally_composite: bool,
    application_end: Option<u32>,
) -> Option<&'a SemanticExpr> {
    let mut pending = vec![relation];
    let mut owner = None;
    while let Some(expression) = pending.pop() {
        pending.extend(expression_children(expression));
        if matches!(
            expression.kind,
            SemanticExprKind::Symbol(_)
                | SemanticExprKind::Number(_)
                | SemanticExprKind::Unknown(_)
        ) || expression.range.start_offset > focus.start_offset
            || focus.end_offset > expression.range.end_offset
        {
            continue;
        }
        let owns_application = application_end.is_some_and(|end| {
            expression.range.start_offset == focus.start_offset
                && expression.range.end_offset == end
        });
        if !structurally_composite && !owns_application {
            continue;
        }
        if owner.is_none_or(|current: &SemanticExpr| {
            expression.range.end_offset - expression.range.start_offset
                < current.range.end_offset - current.range.start_offset
        }) {
            owner = Some(expression);
        }
    }
    owner
}

fn relation_expression_at_cursor<'a>(
    expressions: &'a [SemanticExpr],
    document: &ProjectDocument,
    math_range: &SourceRange,
    focus_range: Option<&SourceRange>,
    offset: u32,
) -> Option<&'a SemanticExpr> {
    let mut candidates = Vec::new();
    for expression in expressions {
        collect_relation_expressions(expression, math_range, &mut candidates);
    }
    let exact = candidates
        .iter()
        .copied()
        .filter(|expression| {
            focus_range.map_or_else(
                || expression.range.contains(offset) || expression.range.end_offset == offset,
                |focus| ranges_overlap(&expression.range, focus),
            )
        })
        .min_by_key(|expression| expression.range.end_offset - expression.range.start_offset);
    if exact.is_some() {
        return exact;
    }
    if focus_range.is_some() {
        return None;
    }
    let preceding = candidates
        .into_iter()
        .filter(|expression| expression.range.end_offset <= offset)
        .max_by_key(|expression| expression.range.end_offset)?;
    relation_trailing_gap_is_owned(document, preceding, offset).then_some(preceding)
}

fn relation_trailing_gap_is_owned(
    document: &ProjectDocument,
    relation: &SemanticExpr,
    offset: u32,
) -> bool {
    let gap = offset.saturating_sub(relation.range.end_offset);
    if gap == 0 || gap > 3 {
        return false;
    }
    source_text(
        document,
        &SourceRange {
            start_offset: relation.range.end_offset,
            end_offset: offset,
        },
    )
    .chars()
    .all(|character| character.is_whitespace() || matches!(character, '.' | ',' | ';' | ':'))
}

fn collect_relation_expressions<'a>(
    expression: &'a SemanticExpr,
    math_range: &SourceRange,
    output: &mut Vec<&'a SemanticExpr>,
) {
    if expression.range.start_offset < math_range.start_offset
        || expression.range.end_offset > math_range.end_offset
    {
        return;
    }
    match &expression.kind {
        SemanticExprKind::Relation { .. } => output.push(expression),
        SemanticExprKind::System(expressions) => {
            for expression in expressions {
                collect_relation_expressions(expression, math_range, output);
            }
        }
        _ => {}
    }
}

fn canonical_expression_at_range<'a>(
    expressions: &'a [SemanticExpr],
    range: &SourceRange,
) -> Option<&'a SemanticExpr> {
    let mut candidates = Vec::new();
    for expression in expressions {
        collect_canonical_expressions(expression, &mut candidates);
    }
    candidates
        .into_iter()
        .filter(|expression| {
            expression.range.start_offset == range.start_offset
                && expression.range.end_offset <= range.end_offset
        })
        .max_by_key(|expression| expression.range.end_offset - expression.range.start_offset)
}

fn expression_contains_relation(expression: &SemanticExpr) -> bool {
    matches!(expression.kind, SemanticExprKind::Relation { .. })
        || expression_children(expression)
            .into_iter()
            .any(expression_contains_relation)
}

fn collect_canonical_expressions<'a>(
    expression: &'a SemanticExpr,
    output: &mut Vec<&'a SemanticExpr>,
) {
    output.push(expression);
    if let SemanticExprKind::System(expressions) = &expression.kind {
        for expression in expressions {
            collect_canonical_expressions(expression, output);
        }
    }
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
        "approximately-equals" => ClaimComparison::Approximate,
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
    occurrences_by_range: &OccurrenceRangeIndex,
    order: &ProjectOrder,
) -> LoweredCrossModalFacts {
    let source = &document.document;
    let mut output = LoweredCrossModalFacts::default();
    for (binding_index, binding) in document.cross_modal_bindings.iter().enumerate() {
        let lookup = |range: &SourceRange| {
            occurrence_id_at_range(occurrences_by_range, occurrences, &source.file_id, range)
        };
        let (Some(short), Some(anchor)) =
            (lookup(&binding.short_range), lookup(&binding.long_range))
        else {
            continue;
        };
        if !occurrences.iter().any(|occurrence| occurrence.id == anchor) {
            continue;
        }
        let Some(short_occurrence) = occurrences.iter().find(|occurrence| occurrence.id == short)
        else {
            continue;
        };
        let entity = EntityId {
            component_id: document.component_id.clone(),
            scope_path: short_occurrence.scope_path.clone(),
            kind: match binding.predicate {
                BindingPredicate::Abbreviates => "acronym",
                BindingPredicate::Aliases => "alias",
                BindingPredicate::Names => "named-operator",
            }
            .to_owned(),
            anchor: short.clone(),
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
            object: ClaimObject::Occurrence(short.clone()),
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
                    source_anchors: Vec::new(),
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
        ("antiderivative", "function"),
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

fn prose_claim_range(
    document: &ProjectDocument,
    formula_ranges: &[SourceRange],
    mut range: SourceRange,
) -> SourceRange {
    let index = crate::SourceIndex::new(&document.content);
    for formula in formula_ranges {
        let content_start = index.byte_for_utf16(formula.start_offset);
        let line_start = document.content[..content_start]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        let opener = document.content[line_start..content_start].trim();
        if range.start_offset < index.utf16_for_byte(line_start)
            && index.utf16_for_byte(line_start) < range.end_offset
            && (matches!(opener, "\\[" | "$$") || opener.starts_with("\\begin{"))
        {
            range.end_offset = index.utf16_for_byte(line_start);
            break;
        }
    }
    let mut start = index.byte_for_utf16(range.start_offset);
    let mut end = index.byte_for_utf16(range.end_offset);
    while start < end
        && document.content[start..end]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        start += document.content[start..end]
            .chars()
            .next()
            .unwrap()
            .len_utf8();
    }
    while start < end
        && document.content[start..end]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        end -= document.content[start..end]
            .chars()
            .next_back()
            .unwrap()
            .len_utf8();
    }
    SourceRange {
        start_offset: index.utf16_for_byte(start),
        end_offset: index.utf16_for_byte(end),
    }
}

fn is_formula_trailing_boundary(
    document: &ProjectDocument,
    formula_range: &SourceRange,
    offset: u32,
) -> bool {
    formula_range.start_offset <= offset
        && offset <= formula_range.end_offset
        && source_text(
            document,
            &SourceRange {
                start_offset: offset,
                end_offset: formula_range.end_offset,
            },
        )
        .chars()
        .all(char::is_whitespace)
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
            | Query::References {
                file_id, offset, ..
            }
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
        let focus = query_offset.and_then(|offset| self.index.cursor_focus_at(file_id, offset));
        let cursor_offset = focus.as_ref().map_or_else(
            || {
                parsed.map_or(offset, |math| {
                    interior_offset(&math.region.content_range, offset)
                })
            },
            |focus| interior_offset(&focus.range, offset),
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
                    focus.as_ref(),
                    (cursor_offset, offset),
                    hygiene_enabled,
                )),
            },
            Query::Definition { .. } => self.definition_value(focus.as_ref()),
            Query::References {
                include_declaration,
                ..
            } => self.references_value(focus.as_ref(), include_declaration),
            Query::PrepareRename { .. } => self.prepare_entity_rename(focus.as_ref()),
            Query::Rename { new_name, .. } => {
                self.entity_rename_proposal(focus.as_ref(), &new_name)
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

    fn visible_definitions(&self, focus: &CursorFocus) -> Vec<DefinitionInfo> {
        self.resolved_entity(&focus.occurrence_id)
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

    fn resolved_entity(&self, occurrence_id: &SourceOccurrenceId) -> Option<EntityId> {
        match self.index.semantic.entity_decision(occurrence_id) {
            EntityEvidenceDecision::Established(entity) => Some(entity),
            EntityEvidenceDecision::Ambiguous
            | EntityEvidenceDecision::Conflicting
            | EntityEvidenceDecision::Unsupported
            | EntityEvidenceDecision::EngineLimited => None,
        }
    }

    fn entity_surface(
        &self,
        focus: Option<&CursorFocus>,
    ) -> Result<AuthorizedEntitySurface, EntitySurfaceRefusal> {
        let Some(focus) = focus else {
            return Err(refusal(
                EntitySurfaceRefusalKind::Unsupported,
                "The cursor does not own a real semantic source occurrence.",
            ));
        };
        let decision = self.index.semantic.entity_decision(&focus.occurrence_id);
        let occurrences = match &decision {
            EntityEvidenceDecision::Established(entity) => self
                .index
                .semantic
                .bounded_established_occurrences_for_entity(entity),
            EntityEvidenceDecision::Ambiguous
            | EntityEvidenceDecision::Conflicting
            | EntityEvidenceDecision::Unsupported
            | EntityEvidenceDecision::EngineLimited => Ok(Vec::new()),
        };
        let declaration = match &decision {
            EntityEvidenceDecision::Established(entity) => self
                .index
                .semantic
                .bounded_authoritative_declaration_for_entity(entity),
            EntityEvidenceDecision::Ambiguous
            | EntityEvidenceDecision::Conflicting
            | EntityEvidenceDecision::Unsupported
            | EntityEvidenceDecision::EngineLimited => Ok(None),
        };
        authorize_entity_surface(&focus.occurrence_id, decision, occurrences, declaration)
    }

    fn definition_value(&self, focus: Option<&CursorFocus>) -> QueryValue {
        let surface = match self.entity_surface(focus) {
            Ok(surface) => surface,
            Err(reason) => return locations_refusal(reason),
        };
        let authorization = surface.authorization();
        let locations = self
            .definition_occurrence(&surface)
            .filter(|definition| definition.id != surface.focus_occurrence_id)
            .map(|definition| vec![self.location_for_occurrence(definition)])
            .unwrap_or_default();
        QueryValue::Locations {
            authorization,
            locations,
        }
    }

    fn references_value(
        &self,
        focus: Option<&CursorFocus>,
        include_declaration: bool,
    ) -> QueryValue {
        let surface = match self.entity_surface(focus) {
            Ok(surface) => surface,
            Err(reason) => return locations_refusal(reason),
        };
        let authorization = surface.authorization();
        let declaration = self
            .definition_occurrence(&surface)
            .map(|occurrence| occurrence.id.clone());
        let mut locations = surface
            .occurrences
            .iter()
            .filter(|occurrence| {
                include_declaration || declaration.as_ref() != Some(&occurrence.id)
            })
            .map(|occurrence| self.location_for_occurrence(occurrence))
            .collect::<Vec<_>>();
        locations.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.range.start_offset.cmp(&right.range.start_offset))
        });
        QueryValue::Locations {
            authorization,
            locations,
        }
    }

    fn definition_occurrence<'a>(
        &'a self,
        surface: &'a AuthorizedEntitySurface,
    ) -> Option<&'a SourceOccurrence> {
        surface
            .occurrences
            .iter()
            .find(|occurrence| occurrence.id == surface.declaration_occurrence_id)
    }

    fn location_for_occurrence(&self, occurrence: &SourceOccurrence) -> Location {
        let document = &self.index.documents[&occurrence.id.file_id];
        Location {
            file_id: occurrence.id.file_id.clone(),
            path: document.document.path.clone(),
            range: occurrence.range.clone(),
        }
    }

    fn prepare_entity_rename(&self, focus: Option<&CursorFocus>) -> QueryValue {
        let surface = match self.entity_surface(focus) {
            Ok(surface) => surface,
            Err(reason) => return rename_preparation_refusal(reason),
        };
        let Some(focus_occurrence) = self.index.semantic.occurrence(&surface.focus_occurrence_id)
        else {
            return rename_preparation_refusal(refusal(
                EntitySurfaceRefusalKind::IncompleteSource,
                "The focused occurrence is no longer present in the project index.",
            ));
        };
        if !crate::entity_policy::rename_focus_is_complete(focus_occurrence) {
            return rename_preparation_refusal(refusal(
                EntitySurfaceRefusalKind::NonEditable,
                "The cursor owns only a non-editable part of a composite identity.",
            ));
        }
        let occurrences = surface
            .occurrences
            .iter()
            .map(|occurrence| self.rename_occurrence(occurrence))
            .collect::<Vec<_>>();
        let Some(first) = occurrences.first() else {
            return rename_preparation_refusal(refusal(
                EntitySurfaceRefusalKind::IncompleteSource,
                "The complete entity has no source occurrences.",
            ));
        };
        let old_name = focus_occurrence.selection_text.clone();
        let replacement = alternate_name(&old_name, first.family);
        match plan_entity_rename(
            EntityEvidenceDecision::Established(surface.entity_id.clone()),
            &old_name,
            &replacement,
            occurrences,
        ) {
            Ok(plan) => QueryValue::RenamePreparation {
                authorization: surface.authorization(),
                range: Some(focus_occurrence.selection_range.clone()),
                placeholder: Some(plan.old_name),
            },
            Err(reason) => rename_preparation_refusal(reason),
        }
    }

    fn entity_rename_proposal(&self, focus: Option<&CursorFocus>, new_name: &str) -> QueryValue {
        let surface = match self.entity_surface(focus) {
            Ok(surface) => surface,
            Err(reason) => return edit_proposal_refusal(reason),
        };
        let Some(focus_occurrence) = self.index.semantic.occurrence(&surface.focus_occurrence_id)
        else {
            return edit_proposal_refusal(refusal(
                EntitySurfaceRefusalKind::IncompleteSource,
                "The focused occurrence is no longer present in the project index.",
            ));
        };
        if !crate::entity_policy::rename_focus_is_complete(focus_occurrence) {
            return edit_proposal_refusal(refusal(
                EntitySurfaceRefusalKind::NonEditable,
                "The cursor owns only a non-editable part of a composite identity.",
            ));
        }
        let old_name = focus_occurrence.selection_text.clone();
        let occurrences = surface
            .occurrences
            .iter()
            .map(|occurrence| self.rename_occurrence(occurrence))
            .collect::<Vec<_>>();
        let plan = match plan_entity_rename(
            EntityEvidenceDecision::Established(surface.entity_id.clone()),
            &old_name,
            new_name,
            occurrences,
        ) {
            Ok(plan) => plan,
            Err(reason) => return edit_proposal_refusal(reason),
        };
        match self.index.semantic.established_selection_would_merge(
            &plan.entity_id,
            new_name,
            &surface.occurrences,
        ) {
            Ok(true) => {
                return edit_proposal_refusal(refusal(
                    EntitySurfaceRefusalKind::Capture,
                    "The replacement would capture or merge another visible established identity.",
                ));
            }
            Err(()) => {
                return edit_proposal_refusal(refusal(
                    EntitySurfaceRefusalKind::EngineLimit,
                    "The replacement collision frontier exceeds the surface safety cap.",
                ));
            }
            Ok(false) => {}
        }
        let mut by_file = BTreeMap::<String, Vec<RenameSourceOccurrence>>::new();
        for occurrence in plan.occurrences {
            by_file
                .entry(occurrence.occurrence_id.file_id.clone())
                .or_default()
                .push(occurrence);
        }
        let mut files = by_file
            .into_iter()
            .map(|(file_id, occurrences)| {
                let analyzed = &self.index.documents[&file_id];
                SemanticEditFile {
                    file_id,
                    path: analyzed.document.path.clone(),
                    document_version: analyzed.document.document_version,
                    edits: occurrences
                        .into_iter()
                        .map(|occurrence| SemanticTextEdit {
                            range: occurrence.range,
                            expected_text: plan.old_name.clone(),
                            replacement_text: plan.new_name.clone(),
                        })
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        QueryValue::EditProposal {
            authorization: surface.authorization(),
            proposal: Some(SemanticEditProposal {
                title: format!("Rename `{}` to `{}`", plan.old_name, plan.new_name),
                safety: "deterministic".into(),
                evidence: vec![Evidence {
                    rule_id: "semath/established-entity-rename".into(),
                    kind: "semantic-identity".into(),
                    strength: "hard".into(),
                    source_ranges: files
                        .iter()
                        .flat_map(|file| file.edits.iter().map(|edit| edit.range.clone()))
                        .collect(),
                    source_anchors: Vec::new(),
                }],
                files,
            }),
        }
    }

    fn rename_occurrence(&self, occurrence: &SourceOccurrence) -> RenameSourceOccurrence {
        let current_text = occurrence.selection_text.clone();
        let family = if current_text.starts_with('\\') {
            RenameNotationFamily::ControlSequence
        } else {
            RenameNotationFamily::PlainIdentifier
        };
        RenameSourceOccurrence {
            occurrence_id: occurrence.id.clone(),
            range: occurrence.selection_range.clone(),
            current_text,
            family,
            editable: occurrence.kind == OccurrenceKind::Notation
                && occurrence.selection_range.start_offset < occurrence.selection_range.end_offset
                && occurrence.range.start_offset <= occurrence.selection_range.start_offset
                && occurrence.selection_range.end_offset <= occurrence.range.end_offset,
        }
    }

    fn semantic_context(
        &self,
        observations: &DocumentSemanticObservations,
        focus: Option<&CursorFocus>,
        meaning_entity: Option<&EntityId>,
        offset: u32,
        formulas: &[crate::LawRecognition],
    ) -> (SemanticContextInfo, Vec<SemanticCandidateInfo>) {
        let (entity_id, symbol_name) = focus
            .map(|focus| {
                (
                    meaning_entity
                        .cloned()
                        .or_else(|| self.resolved_entity(&focus.occurrence_id)),
                    Some(focus.name.clone()),
                )
            })
            .unwrap_or((None, None));
        let mut context =
            observations.context(symbol_name, entity_id.clone(), offset, formulas.to_vec());
        let claim_entity_id = focus.and_then(|focus| self.resolved_entity(&focus.occurrence_id));
        let semantic_occurrence =
            focus.and_then(|focus| self.index.semantic.occurrence(&focus.occurrence_id));
        if let (Some(entity), Some(semantic_occurrence)) = (&claim_entity_id, semantic_occurrence) {
            let context_symbol = context.symbol.clone();
            context.quantities.extend(derived_quantity_infos(
                &self.index.semantic,
                &self.index.documents,
                entity,
                context_symbol.as_deref(),
                semantic_occurrence,
            ));
            normalize_quantities(&mut context.quantities);
            append_index_claims(
                &self.index.semantic,
                &self.index.documents,
                entity,
                semantic_occurrence,
                &mut context,
            );
        }
        let interpretation_candidates = if let Some(focus) = focus {
            let candidates = self
                .index
                .semantic
                .candidates_for(&focus.occurrence_id)
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
            context.candidates = candidates
                .iter()
                .take(MAX_VIEW_CANDIDATES)
                .cloned()
                .collect();
            candidates
        } else {
            context.candidates.clone()
        };
        (context, interpretation_candidates)
    }

    fn formula_meaning_owner(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        focus: &CursorFocus,
        relation: &SemanticExpr,
        offset: u32,
    ) -> Option<(EntityId, bool)> {
        let occurrence = self.index.semantic.occurrence(&focus.occurrence_id)?;
        let matches_occurrence = |seed: &&SemanticOccurrenceSeed| {
            seed.kind == occurrence.kind
                && seed.range == occurrence.range
                && seed.selection_range == occurrence.selection_range
                && seed.surface == occurrence.surface
        };
        let structurally_composite = document
            .semantic_occurrences
            .iter()
            .filter(matches_occurrence)
            .flat_map(|seed| &seed.notation)
            .any(|component| {
                matches!(
                    component,
                    NotationComponent::Subscript { .. }
                        | NotationComponent::Superscript
                        | NotationComponent::Argument { .. }
                        | NotationComponent::Delimiter { .. }
                )
            });
        let application_end = document
            .semantic_occurrences
            .iter()
            .filter(matches_occurrence)
            .filter_map(|seed| seed.application_end_offset)
            .max();
        let attached_relation_fact = offset == relation.range.end_offset
            && focus.range.end_offset == relation.range.end_offset
            && observations
                .formula_meanings
                .iter()
                .any(|fact| fact.target_range == relation.range);
        let owner = if attached_relation_fact {
            relation
        } else {
            canonical_expression_owner(
                relation,
                &focus.range,
                structurally_composite,
                application_end,
            )?
        };
        let occurrence_id = self
            .index
            .occurrence_id_for_range(&document.document.file_id, &owner.range)?;
        let occurrence = self.index.semantic.occurrence(&occurrence_id)?;
        let entity_id = canonical_expression_entity(owner, occurrence);
        self.index
            .semantic
            .contains_entity(&entity_id)
            .then_some(())?;
        let carries_formula_fact = attached_relation_fact || expression_carries_formula_fact(owner);
        Some((entity_id, carries_formula_fact))
    }

    fn canonical_meaning_owner(
        &self,
        document: &AnalyzedDocument,
        expression: &SemanticExpr,
    ) -> Option<EntityId> {
        let occurrence_id = self
            .index
            .occurrence_id_for_range(&document.document.file_id, &expression.range)?;
        let occurrence = self.index.semantic.occurrence(&occurrence_id)?;
        let entity_id = canonical_expression_entity(expression, occurrence);
        self.index
            .semantic
            .contains_entity(&entity_id)
            .then_some(())?;
        Some(entity_id)
    }

    fn semantic_view(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        parsed: Option<&ParsedMath>,
        focus: Option<&CursorFocus>,
        offsets: (u32, u32),
        hygiene_enabled: bool,
    ) -> SemanticViewInfo {
        let (offset, source_offset) = offsets;
        let formula_boundary = focus.is_none()
            || parsed.is_some_and(|math| {
                is_formula_trailing_boundary(&document.document, &math.region.content_range, offset)
            });
        let queried_relation = parsed.and_then(|math| {
            relation_expression_at_cursor(
                &document.canonical_expressions,
                &document.document,
                &math.region.content_range,
                focus.map(|focus| &focus.range),
                offset,
            )
        });
        let queried_formula_range = parsed.map(|math| &math.region.content_range);
        let queried_formula_is_rejected = queried_formula_range
            .is_some_and(|range| observations.semantic_evidence().formula_is_rejected(range));
        let mut projected_formulas = observations.laws.at(offset);
        if projected_formulas.is_empty()
            && formula_boundary
            && let Some(math) = parsed
        {
            projected_formulas = observations.laws.overlapping(&math.region.content_range);
        }
        let focus_is_relation_head = focus.is_some_and(|focus| {
            queried_relation
                .and_then(relation_head)
                .is_some_and(|(_, range)| ranges_overlap(&range, &focus.range))
        });
        if focus_is_relation_head {
            projected_formulas.retain(|formula| {
                formula.relation.as_ref().is_some_and(|relation| {
                    focus.is_some_and(|focus| ranges_overlap(&relation.range, &focus.range))
                })
            });
        }
        let formula_retracted = projected_formulas
            .iter()
            .any(|formula| self.formula_is_retracted(document, formula));
        projected_formulas.retain(|formula| !self.formula_is_retracted(document, formula));
        let (conventional_candidates, conventional_candidates_truncated) =
            conventional_candidates(&projected_formulas);
        let local_formulas = projected_formulas
            .into_iter()
            .filter(|formula| !formula.non_authoritative)
            .collect::<Vec<_>>();
        let display_focus = focus.cloned().or_else(|| {
            let (name, range) = queried_relation.and_then(relation_head)?;
            let occurrence_id = self
                .index
                .occurrence_id_for_range(&document.document.file_id, &range)?;
            Some(CursorFocus {
                name,
                range,
                occurrence_id,
            })
        });
        let semantic_focus = (!parsed
            .is_some_and(|math| cursor_is_structural_environment_marker(&math.root, offset)))
        .then_some(display_focus.as_ref())
        .flatten();
        let formula_meaning_owner = if focus.is_none() {
            queried_relation
                .and_then(|relation| self.canonical_meaning_owner(document, relation))
                .map(|entity_id| (entity_id, false))
        } else {
            display_focus.as_ref().and_then(|focus| {
                if semantic_focus.is_some() && self.resolved_entity(&focus.occurrence_id).is_some()
                {
                    return None;
                }
                queried_relation.and_then(|relation| {
                    self.formula_meaning_owner(
                        document,
                        observations,
                        focus,
                        relation,
                        source_offset,
                    )
                })
            })
        };
        let exact_formula_meaning = formula_meaning_owner
            .as_ref()
            .is_some_and(|(_, exactly_owned)| *exactly_owned);
        let meaning_entity = formula_meaning_owner
            .map(|(entity_id, _)| entity_id)
            .or_else(|| {
                semantic_focus.and_then(|focus| self.resolved_entity(&focus.occurrence_id))
            });
        let linked_formulas = if local_formulas.is_empty() {
            Vec::new()
        } else {
            self.source_linked_preceding_formulas(document, observations, &local_formulas)
        };
        let mut context_formulas = local_formulas.clone();
        if !local_formulas.is_empty() {
            for formula in &linked_formulas {
                if !context_formulas.iter().any(|existing| {
                    existing.pack_id == formula.recognition.pack_id
                        && existing.law_id == formula.recognition.law_id
                        && existing.range == formula.recognition.range
                }) {
                    context_formulas.push(formula.recognition.clone());
                }
            }
        }
        context_formulas.retain(|formula| !self.formula_is_retracted(document, formula));
        let symbol_info = display_focus.as_ref().and_then(|focus| {
            self.symbol_info(document, observations, focus, offset, hygiene_enabled)
        });
        let (context, interpretation_structural_candidates) = self.semantic_context(
            observations,
            semantic_focus,
            meaning_entity.as_ref(),
            offset,
            &context_formulas,
        );
        let symbol_definition_may_establish = !queried_formula_is_rejected
            && queried_relation.is_none_or(|relation| {
                observations
                    .semantic_evidence()
                    .formula_is_asserted(&relation.range)
            });
        let mut symbol_proof = if symbol_definition_may_establish {
            symbol_info.as_ref().map_or_else(Vec::new, |symbol| {
                asserted_definition_evidence(&self.index.semantic, &self.index.documents, symbol)
            })
        } else {
            Vec::new()
        };
        if symbol_definition_may_establish && let Some(focus) = display_focus.as_ref() {
            symbol_proof.extend(
                observations
                    .formula_meanings
                    .iter()
                    .filter(|fact| fact.target_range == focus.range)
                    .map(|fact| fact.evidence.clone()),
            );
        }
        if exact_formula_meaning
            && symbol_definition_may_establish
            && let Some(relation) = queried_relation
        {
            symbol_proof.push(Evidence {
                rule_id: "semath/asserted-formula-meaning".into(),
                kind: "source-claim".into(),
                strength: "hard".into(),
                source_ranges: vec![relation.range.clone()],
                source_anchors: Vec::new(),
            });
        }
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
        let relevant_to_query = |range: &SourceRange, evidence: &[Evidence]| {
            range.contains(offset)
                || evidence.iter().any(|evidence| {
                    evidence.source_ranges.iter().any(|range| {
                        range.contains(offset)
                            || queried_relation
                                .is_some_and(|relation| ranges_overlap(range, &relation.range))
                    })
                })
        };
        let mut typed_conflicts = self
            .index
            .semantic
            .constraint_conflicts_for(&document.document.file_id)
            .into_iter()
            .filter_map(|conflict| {
                let focused_entity = meaning_entity.as_ref().or_else(|| {
                    symbol_info
                        .as_ref()
                        .and_then(|symbol| symbol.entity_id.as_ref())
                });
                let focused_occurrence = display_focus
                    .as_ref()
                    .and_then(|focus| self.index.semantic.occurrence(&focus.occurrence_id));
                let entity_relevant = focused_entity.is_some_and(|entity_id| {
                    entity_id == &conflict.subject
                        || focused_occurrence.is_some_and(|occurrence| {
                            conflict.binding_key.as_deref()
                                == Some(occurrence_binding_key(occurrence).as_str())
                                && conflict.subject.component_id == occurrence.component_id
                                && conflict.subject.scope_path == occurrence.scope_path
                        })
                });
                let (range, conflict) = meaning_conflict(&self.index.semantic, conflict)?;
                (entity_relevant || relevant_to_query(&range, &conflict.evidence))
                    .then_some(conflict)
            })
            .collect::<Vec<_>>();
        if let Some(relation) = queried_relation {
            typed_conflicts.extend(rejected_formula_sign_conflicts(
                relation,
                observations.semantic_evidence(),
            ));
        }
        typed_conflicts.sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
        typed_conflicts.dedup_by(|left, right| left.conflict_id == right.conflict_id);
        let mut diagnostics = document_diagnostics(
            document,
            observations,
            &self.index.semantic,
            hygiene_enabled,
        )
        .into_iter()
        .filter(|diagnostic| relevant_to_query(&diagnostic.range, &diagnostic.evidence))
        .collect::<Vec<_>>();
        diagnostics.extend(
            symbol_info
                .as_ref()
                .into_iter()
                .flat_map(|symbol| symbol.diagnostics.iter().cloned()),
        );
        diagnostics.sort_by(|left, right| {
            left.code.cmp(&right.code).then_with(|| {
                diagnostic_has_typed_constraint_evidence(right)
                    .cmp(&diagnostic_has_typed_constraint_evidence(left))
            })
        });
        diagnostics.dedup_by(|left, right| left.code == right.code);
        let diagnostics_truncated = diagnostics.len() > MAX_VIEW_DIAGNOSTICS;
        diagnostics.truncate(MAX_VIEW_DIAGNOSTICS);
        let (domains, domains_truncated) = observations.domains.at(offset);
        let interpretation_domains = observations.domains.all_at(offset);
        let truncated = declarations_truncated
            || diagnostics_truncated
            || domains_truncated
            || conventional_candidates_truncated
            || context.truncated
            || symbol_info.as_ref().is_some_and(|info| info.truncated);
        let engine_limited = document
            .engine_limited_ranges
            .iter()
            .any(|range| range.contains(offset))
            || symbol_info.as_ref().is_some_and(|symbol| {
                !symbol_has_source_meaning(symbol)
                    && local_formulas.is_empty()
                    && context.candidates.is_empty()
                    && unresolved_control_sequence(symbol)
            });
        let unsupported_relation_context = local_formulas.is_empty()
            && (queried_formula_is_rejected
                || queried_relation.is_some_and(|relation| {
                    observations
                        .semantic_evidence()
                        .formula_is_rejected(&relation.range)
                        || observations
                            .semantic_evidence()
                            .formula_is_asserted(&relation.range)
                            && domain_has_correlated_evidence(&domains)
                            && context.candidates.is_empty()
                            && !self.formula_has_source_meaning(document, &relation.range)
                }))
            || queried_relation.is_none()
                && symbol_info.as_ref().is_some_and(|symbol| {
                    !symbol_has_source_meaning(symbol)
                        && (self
                            .index
                            .semantic
                            .has_future_external_binding_evidence(&symbol.occurrence_id)
                            || self.has_prior_excluding_occurrence(
                                observations,
                                &symbol.occurrence_id,
                            )
                            || explicitly_excludes_external_evidence(&context, symbol))
                });
        let decision = decide_meaning(MeaningDecisionInput {
            formulas: &local_formulas,
            symbol: semantic_focus.and(symbol_info.as_ref()),
            symbol_proof: &symbol_proof,
            candidates: &context.candidates,
            conflicts: &typed_conflicts,
            engine_limited,
            unsupported_relation_context,
            truncated,
        });
        let authoring_context = self.math_authoring_context(
            document,
            parsed,
            display_focus.as_ref(),
            queried_relation,
            &local_formulas,
            &linked_formulas,
            &context,
            &interpretation_domains,
            &interpretation_structural_candidates,
            &decision,
            conventional_candidates,
            formula_retracted,
            engine_limited,
            truncated,
        );
        SemanticViewInfo {
            decision,
            symbol: symbol_info,
            context,
            authoring_context,
            declarations,
            diagnostics,
            domains,
            truncated,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn math_authoring_context(
        &self,
        document: &AnalyzedDocument,
        parsed: Option<&ParsedMath>,
        focus: Option<&CursorFocus>,
        queried_relation: Option<&SemanticExpr>,
        local_formulas: &[LawRecognition],
        linked_formulas: &[SourceLinkedFormula],
        context: &SemanticContextInfo,
        interpretation_domains: &[DomainActivation],
        interpretation_structural_candidates: &[SemanticCandidateInfo],
        decision: &MeaningDecision,
        conventional_candidates: Vec<ConventionalCandidateInfo>,
        retracted: bool,
        engine_limited: bool,
        view_truncated: bool,
    ) -> MathAuthoringContext {
        let provenance =
            queried_relation.map_or_else(Vec::new, |relation| relation.provenance.clone());
        let formula = parsed.map(|math| {
            self.math_formula_anchor(document, &math.region.content_range, provenance.clone())
        });
        let mut truncated = view_truncated;
        let mut requirements = self.math_authoring_requirements(
            focus,
            local_formulas,
            decision,
            &conventional_candidates,
            context,
        );
        let discriminator_set_capped = requirements.len() > MAX_INTERPRETATION_DISCRIMINATORS;
        truncated |= discriminator_set_capped;
        requirements.truncate(MAX_INTERPRETATION_DISCRIMINATORS);

        let mut conditions = local_formulas
            .iter()
            .flat_map(|formula| formula.conditions.iter().cloned())
            .collect::<Vec<_>>();
        conditions.sort_by(|left, right| {
            evidence_order_key(&left.evidence)
                .cmp(&evidence_order_key(&right.evidence))
                .then(left.condition_id.cmp(&right.condition_id))
                .then(left.subjects.cmp(&right.subjects))
        });
        conditions.dedup_by(|left, right| {
            left.condition_id == right.condition_id && left.subjects == right.subjects
        });
        truncated |= conditions.len() > MAX_AUTHORING_ITEMS;
        conditions.truncate(MAX_AUTHORING_ITEMS);

        let mut equation_links = formula.as_ref().map_or_else(Vec::new, |target| {
            self.math_equation_links(document, target, local_formulas, linked_formulas)
        });
        truncated |= equation_links.len() > MAX_AUTHORING_ITEMS;
        equation_links.truncate(MAX_AUTHORING_ITEMS);

        let approximation = queried_relation
            .and_then(|relation| self.math_approximation(relation, local_formulas, context));
        let mut claim_evidence = self.math_claim_evidence(context);
        truncated |= claim_evidence.len() > MAX_AUTHORING_ITEMS;
        claim_evidence.truncate(MAX_AUTHORING_ITEMS);

        let (mut notation_occurrences, notation_truncated) = self.math_notation_occurrences(focus);
        truncated |= notation_truncated || notation_occurrences.len() > MAX_AUTHORING_ITEMS;
        notation_occurrences.truncate(MAX_AUTHORING_ITEMS);

        let generated = formula
            .as_ref()
            .is_some_and(|anchor| !anchor.provenance.is_empty());
        let focus_editable = focus
            .and_then(|focus| self.index.semantic.occurrence(&focus.occurrence_id))
            .is_some_and(|occurrence| {
                occurrence.kind == OccurrenceKind::Notation
                    && occurrence.selection_range.start_offset
                        < occurrence.selection_range.end_offset
            });
        let complete = parsed.is_none_or(|math| math.region.closed);
        let editable = !generated
            && !retracted
            && !engine_limited
            && complete
            && (formula.is_some() || focus_editable);
        let disposition = math_authoring_disposition(
            decision,
            formula.is_some(),
            !conventional_candidates.is_empty(),
            engine_limited,
        );
        let mut lifecycle = MathSourceLifecycleInfo {
            document_version: document.document.document_version,
            generation: if generated {
                MathSourceGeneration::Generated
            } else {
                MathSourceGeneration::Authored
            },
            freshness: MathSourceFreshness::Current,
            editable,
            retracted,
            capped: truncated,
            engine_limited,
        };
        let interpretation_scope_path = formula
            .as_ref()
            .map(|formula| formula.scope_path.clone())
            .or_else(|| {
                focus
                    .and_then(|focus| self.index.semantic.occurrence(&focus.occurrence_id))
                    .map(|occurrence| occurrence.scope_path.clone())
            })
            .unwrap_or_default();
        let interpretation_source_range = formula
            .as_ref()
            .map(|formula| &formula.location.range)
            .or_else(|| focus.map(|focus| &focus.range));
        let resolve_evidence = |evidence: &Evidence| {
            self.resolve_interpretation_evidence(
                document,
                &lifecycle,
                interpretation_source_range,
                evidence,
            )
        };
        let interpretations = project_math_interpretations(MathInterpretationInput {
            decision,
            formulas: local_formulas,
            conventional_candidates: &conventional_candidates,
            domains: interpretation_domains,
            structural_candidates: interpretation_structural_candidates,
            context,
            requirements: &requirements,
            formula: formula.as_ref(),
            focus_range: focus.map(|focus| &focus.range),
            file_id: &document.document.file_id,
            path: &document.document.path,
            scope_path: &interpretation_scope_path,
            lifecycle: &lifecycle,
            discriminator_set_capped,
            resolve_evidence: &resolve_evidence,
        });
        if interpretations.candidate_cap.is_some() {
            lifecycle.capped = true;
            truncated = true;
        }
        let projected_requirements = interpretations.missing_discriminators.clone();
        let public_conventional_candidates = conventional_candidates
            .into_iter()
            .take(MAX_CONVENTIONAL_CANDIDATES)
            .collect();
        MathAuthoringContext {
            disposition,
            formula,
            requirements: projected_requirements,
            conditions,
            conventional_candidates: public_conventional_candidates,
            equation_links,
            approximation,
            claim_evidence,
            notation_occurrences,
            interpretations,
            lifecycle,
            truncated,
        }
    }

    fn math_formula_anchor(
        &self,
        document: &AnalyzedDocument,
        range: &SourceRange,
        mut provenance: Vec<SourceRange>,
    ) -> MathFormulaAnchorInfo {
        provenance.sort_by_key(|range| (range.start_offset, range.end_offset));
        provenance.dedup();
        MathFormulaAnchorInfo {
            location: Location {
                file_id: document.document.file_id.clone(),
                path: document.document.path.clone(),
                range: range.clone(),
            },
            document_version: document.document.document_version,
            scope_path: document.scopes.path_at(range.start_offset),
            source_notation: source_text(&document.document, range),
            provenance,
        }
    }

    fn math_formula_anchor_for_range(
        &self,
        document: &AnalyzedDocument,
        range: &SourceRange,
    ) -> MathFormulaAnchorInfo {
        let formula_range = document
            .formula_ranges
            .iter()
            .find(|candidate| {
                candidate.start_offset <= range.start_offset
                    && range.end_offset <= candidate.end_offset
            })
            .unwrap_or(range);
        let provenance = document
            .canonical_expressions
            .iter()
            .find(|expression| {
                expression.range.start_offset <= range.start_offset
                    && range.end_offset <= expression.range.end_offset
            })
            .map_or_else(Vec::new, |expression| expression.provenance.clone());
        self.math_formula_anchor(document, formula_range, provenance)
    }

    fn resolve_interpretation_evidence(
        &self,
        queried_document: &AnalyzedDocument,
        queried_lifecycle: &MathSourceLifecycleInfo,
        queried_source_range: Option<&SourceRange>,
        evidence: &Evidence,
    ) -> ResolvedInterpretationEvidence {
        let mut anchors = evidence.source_anchors.clone();
        for anchor in &mut anchors {
            if anchor.location.file_id == queried_document.document.file_id
                && queried_lifecycle.retracted
                && queried_source_range.is_some_and(|source| {
                    ranges_overlap(source, &anchor.location.range)
                        || source == &anchor.location.range
                })
            {
                anchor.lifecycle = MathInterpretationSourceLifecycle::Retracted;
            }
        }
        if anchors.is_empty() {
            for range in &evidence.source_ranges {
                let source_document = queried_document;
                anchors.push(MathInterpretationEvidenceSourceAnchorInfo {
                    location: Location {
                        file_id: source_document.document.file_id.clone(),
                        path: source_document.document.path.clone(),
                        range: range.clone(),
                    },
                    document_version: source_document.document.document_version,
                    scope_path: source_document.scopes.path_at(range.start_offset),
                    lifecycle: if source_document.document.file_id
                        == queried_document.document.file_id
                        && queried_lifecycle.retracted
                        && queried_source_range
                            .is_some_and(|source| ranges_overlap(source, range) || source == range)
                    {
                        MathInterpretationSourceLifecycle::Retracted
                    } else {
                        MathInterpretationSourceLifecycle::Current
                    },
                    generation: evidence_source_generation(source_document, range),
                });
            }
        }
        normalize_source_anchors(&mut anchors);
        let authority = if evidence.kind.contains("derived") {
            InterpretationEvidenceAuthority::Derived
        } else if matches!(
            evidence.kind.as_str(),
            "definition"
                | "explicit-math"
                | "source-claim"
                | "source-definition"
                | "source-relation"
        ) {
            InterpretationEvidenceAuthority::Explicit
        } else {
            InterpretationEvidenceAuthority::Observational
        };
        ResolvedInterpretationEvidence { anchors, authority }
    }

    fn math_authoring_requirements(
        &self,
        focus: Option<&CursorFocus>,
        formulas: &[LawRecognition],
        decision: &MeaningDecision,
        conventional_candidates: &[ConventionalCandidateInfo],
        context: &SemanticContextInfo,
    ) -> Vec<MathAuthoringRequirementInfo> {
        let mut requirements = Vec::new();
        for formula in formulas {
            requirements.extend(
                formula
                    .bindings
                    .iter()
                    .filter(|binding| {
                        matches!(
                            binding.proof,
                            LawBindingProof::Asserted | LawBindingProof::Candidate
                        )
                    })
                    .map(|binding| MathAuthoringRequirementInfo::RoleDeclaration {
                        requirement_id: format!("{}/binding/{}", formula.law_id, binding.parameter),
                        parameter: binding.parameter.clone(),
                        symbol: binding.symbol.clone(),
                        constraint: binding.constraint.clone(),
                        evidence: vec![binding.evidence.clone()],
                    }),
            );
            requirements.extend(
                formula
                    .conditions
                    .iter()
                    .filter(|condition| condition.status != crate::ConstraintStatus::Verified)
                    .map(|condition| MathAuthoringRequirementInfo::Condition {
                        requirement_id: format!(
                            "{}/condition/{}",
                            formula.law_id, condition.condition_id
                        ),
                        condition: condition.clone(),
                    }),
            );
        }
        for candidate in conventional_candidates {
            requirements.extend(candidate.requirements.iter().map(
                |requirement| match requirement {
                    ConventionalRequirementInfo::RoleDeclaration {
                        requirement_id,
                        parameter,
                        symbol,
                        constraint,
                        evidence,
                    } => MathAuthoringRequirementInfo::RoleDeclaration {
                        requirement_id: requirement_id.clone(),
                        parameter: parameter.clone(),
                        symbol: symbol.clone(),
                        constraint: constraint.clone(),
                        evidence: evidence.clone(),
                    },
                    ConventionalRequirementInfo::Condition {
                        requirement_id,
                        condition,
                    } => MathAuthoringRequirementInfo::Condition {
                        requirement_id: requirement_id.clone(),
                        condition: condition.clone(),
                    },
                },
            ));
        }
        if let Some(focus) = focus
            && self.resolved_entity(&focus.occurrence_id).is_none()
        {
            requirements.push(MathAuthoringRequirementInfo::Declaration {
                requirement_id: format!(
                    "declaration/{}/{}/{}",
                    focus.occurrence_id.file_id,
                    focus.occurrence_id.document_version,
                    focus.occurrence_id.local_id
                ),
                symbol: focus.name.clone(),
                occurrence_id: focus.occurrence_id.clone(),
                evidence: vec![Evidence {
                    rule_id: "semath/authoring/unresolved-occurrence".into(),
                    kind: "source-occurrence".into(),
                    strength: "weak".into(),
                    source_ranges: vec![focus.range.clone()],
                    source_anchors: Vec::new(),
                }],
            });
        }
        if let MeaningDecision::Ambiguous { alternatives, .. } = decision {
            requirements.push(MathAuthoringRequirementInfo::Disambiguation {
                requirement_id: "meaning/disambiguation".into(),
                alternatives: alternatives.clone(),
                evidence: alternatives
                    .iter()
                    .flat_map(|alternative| alternative.evidence.iter().cloned())
                    .collect(),
            });
        } else {
            let mut groups = BTreeMap::<(u32, u32), Vec<&SemanticCandidateInfo>>::new();
            for candidate in &context.candidates {
                groups
                    .entry((candidate.range.start_offset, candidate.range.end_offset))
                    .or_default()
                    .push(candidate);
            }
            for ((start, end), candidates) in groups {
                if candidates.len() < 2 {
                    continue;
                }
                let alternatives = candidates
                    .into_iter()
                    .map(|candidate| MeaningAlternative {
                        alternative_id: candidate.candidate_id.clone(),
                        label: candidate.interpretation.clone(),
                        range: candidate.range.clone(),
                        evidence: vec![Evidence {
                            rule_id: "semath/authoring/structural-alternative".into(),
                            kind: "source-structure".into(),
                            strength: "contextual".into(),
                            source_ranges: vec![candidate.range.clone()],
                            source_anchors: Vec::new(),
                        }],
                        relevance: None,
                    })
                    .collect::<Vec<_>>();
                requirements.push(MathAuthoringRequirementInfo::Disambiguation {
                    requirement_id: format!("meaning/structural-disambiguation/{start}-{end}"),
                    evidence: alternatives
                        .iter()
                        .flat_map(|alternative| alternative.evidence.iter().cloned())
                        .collect(),
                    alternatives,
                });
            }
        }
        let mut seen = HashSet::new();
        requirements.retain(|requirement| seen.insert(authoring_requirement_id(requirement)));
        requirements
    }

    fn math_equation_links(
        &self,
        document: &AnalyzedDocument,
        target: &MathFormulaAnchorInfo,
        current: &[LawRecognition],
        preceding: &[SourceLinkedFormula],
    ) -> Vec<MathEquationLinkInfo> {
        let mut links = preceding
            .iter()
            .map(|formula| {
                let source =
                    self.math_formula_anchor_for_range(document, &formula.recognition.range);
                let mut evidence = current
                    .iter()
                    .flat_map(|formula| {
                        formula
                            .evidence
                            .iter()
                            .chain(formula.bindings.iter().map(|binding| &binding.evidence))
                    })
                    .filter(|evidence| {
                        evidence
                            .source_ranges
                            .iter()
                            .any(|range| ranges_overlap(range, &source.location.range))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let kind = if evidence
                    .iter()
                    .any(|item| item.rule_id.starts_with("law-chain/"))
                {
                    MathEquationLinkKind::DerivedLaw
                } else {
                    MathEquationLinkKind::SharedEntity
                };
                if evidence.is_empty() {
                    evidence.push(Evidence {
                        rule_id: "semath/authoring/shared-entity".into(),
                        kind: "semantic-identity".into(),
                        strength: "strong".into(),
                        source_ranges: vec![
                            source.location.range.clone(),
                            target.location.range.clone(),
                        ],
                        source_anchors: Vec::new(),
                    });
                }
                MathEquationLinkInfo {
                    link_id: format!(
                        "equation-link/{}:{}-{}:{}",
                        source.location.range.start_offset,
                        source.location.range.end_offset,
                        target.location.range.start_offset,
                        target.location.range.end_offset
                    ),
                    kind,
                    source,
                    target: target.clone(),
                    shared_entities: formula.shared_entities.clone(),
                    evidence,
                }
            })
            .collect::<Vec<_>>();
        links.sort_by_key(|link| {
            (
                link.source.location.range.start_offset,
                link.source.location.range.end_offset,
            )
        });
        links.dedup_by(|left, right| left.link_id == right.link_id);
        links
    }

    fn math_approximation(
        &self,
        relation: &SemanticExpr,
        formulas: &[LawRecognition],
        context: &SemanticContextInfo,
    ) -> Option<MathApproximationInfo> {
        let SemanticExprKind::Relation { operator, .. } = &relation.kind else {
            return None;
        };
        (operator.as_str() == "approximately-equals").then(|| {
            let mut evidence = formulas
                .iter()
                .flat_map(|formula| {
                    formula.evidence.iter().chain(
                        formula
                            .relation
                            .iter()
                            .flat_map(|relation| relation.evidence.iter()),
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            if evidence.is_empty() {
                evidence.push(Evidence {
                    rule_id: "semath/canonical-approximation".into(),
                    kind: "source-relation".into(),
                    strength: "hard".into(),
                    source_ranges: vec![relation.range.clone()],
                    source_anchors: Vec::new(),
                });
            }
            evidence.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
            evidence.dedup();
            let related_fact_ids = context
                .claims
                .iter()
                .filter(|claim| {
                    claim.evidence.iter().any(|evidence| {
                        evidence
                            .source_ranges
                            .iter()
                            .any(|range| ranges_overlap(range, &relation.range))
                    })
                })
                .map(|claim| claim.claim_id.clone())
                .take(MAX_AUTHORING_ITEMS)
                .collect();
            MathApproximationInfo {
                exactness: MathExactness::Approximate,
                relation_range: relation.range.clone(),
                evidence,
                related_fact_ids,
            }
        })
    }

    fn math_claim_evidence(&self, context: &SemanticContextInfo) -> Vec<MathClaimEvidenceLinkInfo> {
        let mut projected_evidence_ids = BTreeSet::new();
        let mut links = context
            .claims
            .iter()
            .filter_map(|claim| {
                let indexed_claim = self
                    .index
                    .semantic
                    .claim(&ClaimId(claim.claim_id.clone()))?;
                // One authored claim may lower to multiple internal facts (for example,
                // Defines and HasType). The authoring projection describes the shared
                // source evidence, so emit that evidence record only once.
                if !projected_evidence_ids.insert(indexed_claim.evidence_id.clone()) {
                    return None;
                }
                let record = self.index.semantic.evidence(&indexed_claim.evidence_id)?;
                let claim_occurrence = self.index.semantic.occurrence(&record.source)?;
                let claim_document = self.index.documents.get(&claim_occurrence.id.file_id)?;
                let claim_range = claim_document
                    .observations
                    .semantic_evidence()
                    .source_clause_range_for(&claim_occurrence.range)
                    .map(|range| {
                        prose_claim_range(
                            &claim_document.document,
                            &claim_document.formula_ranges,
                            range,
                        )
                    })
                    .or_else(|| {
                        claim
                            .evidence
                            .iter()
                            .flat_map(|evidence| evidence.source_ranges.iter())
                            .min_by_key(|range| (range.start_offset, range.end_offset))
                            .cloned()
                    })
                    .unwrap_or_else(|| claim_occurrence.range.clone());
                let supporting_formulas = record
                    .provenance
                    .iter()
                    .filter_map(|id| self.index.semantic.occurrence(id))
                    .filter_map(|occurrence| {
                        let document = self.index.documents.get(&occurrence.id.file_id)?;
                        let formula_range = document.formula_ranges.iter().find(|range| {
                            range.start_offset <= occurrence.range.start_offset
                                && occurrence.range.end_offset <= range.end_offset
                        })?;
                        let expression = canonical_expression_at_range(
                            &document.canonical_expressions,
                            formula_range,
                        )?;
                        expression_contains_relation(expression).then(|| {
                            self.math_formula_anchor_for_range(document, &occurrence.range)
                        })
                    })
                    .take(MAX_AUTHORING_ITEMS)
                    .collect::<Vec<_>>();
                let polarity = match record.polarity {
                    EvidencePolarity::Positive => MathClaimPolarity::Positive,
                    EvidencePolarity::Negative => MathClaimPolarity::Negative,
                };
                let modality = match record.modality {
                    EvidenceModality::Asserted => MathClaimModality::Asserted,
                    EvidenceModality::Hypothetical => MathClaimModality::Hypothetical,
                    EvidenceModality::Hedged => MathClaimModality::Hedged,
                    EvidenceModality::Quoted => MathClaimModality::Quoted,
                    EvidenceModality::Cited => MathClaimModality::Cited,
                };
                let strength_ceiling = match (record.polarity, record.modality) {
                    (EvidencePolarity::Positive, EvidenceModality::Asserted) => {
                        MathClaimStrengthCeiling::Asserted
                    }
                    (EvidencePolarity::Positive, _) => MathClaimStrengthCeiling::Qualified,
                    (EvidencePolarity::Negative, _) => MathClaimStrengthCeiling::Unusable,
                };
                Some(MathClaimEvidenceLinkInfo {
                    claim_id: claim.claim_id.clone(),
                    claim: Location {
                        file_id: claim_occurrence.id.file_id.clone(),
                        path: claim_document.document.path.clone(),
                        range: claim_range,
                    },
                    polarity,
                    modality,
                    strength_ceiling,
                    supporting_claim_ids: record
                        .parent_claims
                        .iter()
                        .map(|id| id.0.clone())
                        .collect(),
                    supporting_formulas,
                    evidence: claim.evidence.clone(),
                })
            })
            .collect::<Vec<_>>();
        links.sort_by(|left, right| {
            self.index
                .order
                .position(&left.claim.file_id, left.claim.range.start_offset)
                .unwrap_or(u64::MAX)
                .cmp(
                    &self
                        .index
                        .order
                        .position(&right.claim.file_id, right.claim.range.start_offset)
                        .unwrap_or(u64::MAX),
                )
                .then(left.claim.file_id.cmp(&right.claim.file_id))
                .then(
                    left.claim
                        .range
                        .start_offset
                        .cmp(&right.claim.range.start_offset),
                )
                .then(left.claim_id.cmp(&right.claim_id))
        });
        links.dedup_by(|left, right| left.claim_id == right.claim_id);
        links
    }

    fn math_notation_occurrences(
        &self,
        focus: Option<&CursorFocus>,
    ) -> (Vec<MathNotationOccurrenceInfo>, bool) {
        let Some(entity_id) = focus.and_then(|focus| self.resolved_entity(&focus.occurrence_id))
        else {
            return (Vec::new(), false);
        };
        let Ok(occurrences) = self
            .index
            .semantic
            .bounded_established_occurrences_for_entity(&entity_id)
        else {
            return (Vec::new(), true);
        };
        let mut projected = occurrences
            .into_iter()
            .filter(|occurrence| occurrence.kind == OccurrenceKind::Notation)
            .filter_map(|occurrence| {
                let document = self.index.documents.get(&occurrence.id.file_id)?;
                Some(MathNotationOccurrenceInfo {
                    occurrence_id: occurrence.id,
                    entity_id: entity_id.clone(),
                    location: Location {
                        file_id: document.document.file_id.clone(),
                        path: document.document.path.clone(),
                        range: occurrence.range,
                    },
                    scope_path: occurrence.scope_path,
                    source_notation: occurrence.source_text,
                })
            })
            .collect::<Vec<_>>();
        projected.sort_by(|left, right| {
            self.index
                .order
                .position(&left.location.file_id, left.location.range.start_offset)
                .unwrap_or(u64::MAX)
                .cmp(
                    &self
                        .index
                        .order
                        .position(&right.location.file_id, right.location.range.start_offset)
                        .unwrap_or(u64::MAX),
                )
                .then(left.location.path.cmp(&right.location.path))
                .then(
                    left.location
                        .range
                        .start_offset
                        .cmp(&right.location.range.start_offset),
                )
        });
        (projected, false)
    }

    fn formula_is_retracted(
        &self,
        document: &AnalyzedDocument,
        formula: &crate::LawRecognition,
    ) -> bool {
        let Some(expression) =
            canonical_expression_at_range(&document.canonical_expressions, &formula.range)
        else {
            return false;
        };
        let Some((_, subject_range)) = relation_head(expression) else {
            return false;
        };
        let Some(occurrence) = self
            .index
            .occurrence_id_for_range(&document.document.file_id, &subject_range)
        else {
            return false;
        };
        self.index.semantic.relation_is_retracted(
            &stable_text_digest(&render_canonical(expression)),
            &occurrence,
        )
    }

    fn source_linked_preceding_formulas(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        current: &[crate::LawRecognition],
    ) -> Vec<SourceLinkedFormula> {
        let current_start = current
            .iter()
            .map(|formula| formula.range.start_offset)
            .min()
            .unwrap_or(0);
        let current_entities = current
            .iter()
            .flat_map(|formula| self.entities_in_formula(document, &formula.range))
            .collect::<BTreeSet<_>>();
        if current_entities.is_empty() {
            return Vec::new();
        }
        observations
            .laws
            .all()
            .iter()
            .filter(|formula| formula.range.end_offset <= current_start)
            .filter_map(|formula| {
                let shared_entities = self
                    .entities_in_formula(document, &formula.range)
                    .intersection(&current_entities)
                    .cloned()
                    .collect::<Vec<_>>();
                (!shared_entities.is_empty()).then(|| SourceLinkedFormula {
                    recognition: formula.clone(),
                    shared_entities,
                })
            })
            .collect()
    }

    fn entities_in_formula(
        &self,
        document: &AnalyzedDocument,
        range: &SourceRange,
    ) -> BTreeSet<EntityId> {
        let start = document.semantic_occurrences.partition_point(|occurrence| {
            occurrence.selection_range.start_offset < range.start_offset
        });
        let end = document.semantic_occurrences.partition_point(|occurrence| {
            occurrence.selection_range.start_offset < range.end_offset
        });
        document.semantic_occurrences[start..end]
            .iter()
            .filter(|occurrence| {
                range.start_offset <= occurrence.selection_range.start_offset
                    && occurrence.selection_range.end_offset <= range.end_offset
            })
            .filter_map(|occurrence| {
                self.index
                    .cursor_focus(&document.document.file_id, occurrence)
            })
            .flat_map(|focus| self.index.semantic.resolve(&focus.occurrence_id).candidates)
            .filter(|candidate| {
                !candidate.supporting_claims.is_empty() && candidate.rejecting_claims.is_empty()
            })
            .map(|candidate| candidate.entity_id)
            .collect()
    }

    fn formula_has_source_meaning(&self, document: &AnalyzedDocument, range: &SourceRange) -> bool {
        document
            .semantic_occurrences
            .iter()
            .filter(|occurrence| {
                range.start_offset <= occurrence.selection_range.start_offset
                    && occurrence.selection_range.end_offset <= range.end_offset
            })
            .take(32)
            .filter_map(|occurrence| {
                self.index
                    .cursor_focus(&document.document.file_id, occurrence)
            })
            .any(|focus| {
                self.index
                    .semantic
                    .occurrence_has_source_meaning(&focus.occurrence_id)
            })
    }

    fn has_prior_excluding_occurrence(
        &self,
        observations: &DocumentSemanticObservations,
        current_id: &SourceOccurrenceId,
    ) -> bool {
        let Some(current) = self.index.semantic.occurrence(current_id) else {
            return false;
        };
        let binding = occurrence_binding_key(current);
        self.index.semantic.occurrences().any(|candidate| {
            if candidate.id.file_id != current.id.file_id
                || candidate.range.start_offset >= current.range.start_offset
                || occurrence_binding_key(candidate) != binding
            {
                return false;
            }
            let (polarity, modality) = observations
                .semantic_evidence()
                .formula_disposition(&candidate.range);
            polarity == EvidencePolarity::Negative
                || matches!(
                    modality,
                    EvidenceModality::Hypothetical | EvidenceModality::Cited
                )
        })
    }

    fn symbol_info(
        &self,
        document: &AnalyzedDocument,
        observations: &DocumentSemanticObservations,
        focus: &CursorFocus,
        offset: u32,
        hygiene_enabled: bool,
    ) -> Option<SymbolInfo> {
        let semantic_name = focus.name.trim_start_matches('\\');
        let mut definitions = self.visible_definitions(focus);
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
        let semantic_occurrence = self.index.semantic.occurrence(&focus.occurrence_id)?;
        let entity_id = self.resolved_entity(&focus.occurrence_id);
        if let Some(entity) = &entity_id {
            shapes.extend(derived_shape_infos(
                &self.index.semantic,
                &self.index.documents,
                entity,
                &semantic_occurrence.surface,
                semantic_occurrence,
            ));
            quantities.extend(derived_quantity_infos(
                &self.index.semantic,
                &self.index.documents,
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
            occurrence_id: focus.occurrence_id.clone(),
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
                let mut activation = activation.clone();
                ground_evidence_in_document(document, &mut activation.evidence);
                activations
                    .entry(document.component_id.clone())
                    .or_default()
                    .push(IndexedLawActivation {
                        source_offset: evidence_anchor(&activation.evidence),
                        activation,
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
                let source_document = document;
                let roles = document
                    .observations
                    .roles
                    .exported()
                    .into_iter()
                    .filter(|role| target_symbols.contains(role.symbol.as_str()))
                    .map({
                        let component_id = component_id.clone();
                        let file_id = file_id.clone();
                        move |mut role| {
                            ground_evidence_in_document(source_document, &mut role.evidence);
                            IndexedTypeFact {
                                component_id: component_id.clone(),
                                source_offset: evidence_anchor(&role.evidence),
                                file_id: file_id.clone(),
                                fact: ExportedTypeFact::Role(role),
                            }
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
                        move |mut quantity| {
                            ground_evidence_in_document(source_document, &mut quantity.evidence);
                            IndexedTypeFact {
                                component_id: component_id.clone(),
                                source_offset: evidence_anchor(&quantity.evidence),
                                file_id: file_id.clone(),
                                fact: ExportedTypeFact::Quantity(quantity),
                            }
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
                        move |mut shape| {
                            ground_evidence_in_document(source_document, &mut shape.evidence);
                            IndexedTypeFact {
                                component_id: component_id.clone(),
                                source_offset: evidence_anchor(&shape.evidence),
                                file_id: file_id.clone(),
                                fact: ExportedTypeFact::Shape(shape),
                            }
                        }
                    });
                roles.chain(quantities).chain(shapes)
            })
            .collect::<Vec<_>>();
        let mut assumptions = self
            .index
            .documents
            .iter()
            .filter(|(_, document)| target_components.contains(&document.component_id))
            .flat_map(|(file_id, document)| {
                document
                    .observations
                    .assumptions()
                    .iter()
                    .filter(|assumption| !assumption.subjects.is_empty())
                    .cloned()
                    .map({
                        let component_id = document.component_id.clone();
                        let file_id = file_id.clone();
                        move |mut assumption| {
                            ground_evidence_in_document(document, &mut assumption.evidence);
                            IndexedAssumption {
                                component_id: component_id.clone(),
                                source_offset: evidence_anchor(&assumption.evidence),
                                file_id: file_id.clone(),
                                assumption,
                            }
                        }
                    })
            })
            .collect::<Vec<_>>();
        assumptions.sort_by(|left, right| {
            left.component_id
                .cmp(&right.component_id)
                .then(left.file_id.cmp(&right.file_id))
                .then(left.source_offset.cmp(&right.source_offset))
                .then(left.assumption.kind.cmp(&right.assumption.kind))
                .then(left.assumption.value.cmp(&right.assumption.value))
                .then(left.assumption.subjects.cmp(&right.assumption.subjects))
        });
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
                    environment.begin_formula(&math.region.content_range);
                    let order_offset = math
                        .symbols
                        .first()
                        .map_or(semantic_offset, |(_, range)| range.start_offset);
                    let symbols = math
                        .symbols
                        .iter()
                        .map(|(symbol, _)| symbol.as_str())
                        .collect::<HashSet<_>>();
                    for assumption in &assumptions {
                        if assumption.file_id != *file_id
                            && assumption.component_id == target.component_id
                            && assumption
                                .assumption
                                .subjects
                                .iter()
                                .all(|subject| symbols.contains(subject.as_str()))
                            && self.index.order.precedes(
                                &assumption.file_id,
                                assumption.source_offset,
                                file_id,
                                order_offset,
                            )
                        {
                            environment
                                .add_assumption(semantic_offset, assumption.assumption.clone());
                        }
                    }
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
            let formula_ranges = document.formula_ranges.clone();
            self.index.observations_mut(&file_id).refresh_laws(
                &source,
                &canonical_expressions,
                &formula_ranges,
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

fn math_authoring_disposition(
    decision: &MeaningDecision,
    formula_context: bool,
    has_conventional_candidate: bool,
    engine_limited: bool,
) -> MathAuthoringDisposition {
    match decision {
        MeaningDecision::Established { meaning, .. }
            if formula_context && meaning.relation_id.is_none() =>
        {
            MathAuthoringDisposition::Partial
        }
        MeaningDecision::Established { .. } => MathAuthoringDisposition::Established,
        MeaningDecision::Partial { .. } if has_conventional_candidate => {
            MathAuthoringDisposition::Conventional
        }
        MeaningDecision::Partial { .. } => MathAuthoringDisposition::Partial,
        MeaningDecision::Ambiguous { .. } => MathAuthoringDisposition::Ambiguous,
        MeaningDecision::Conflicting { .. } => MathAuthoringDisposition::Conflicting,
        MeaningDecision::Unsupported { .. } if engine_limited => {
            MathAuthoringDisposition::EngineLimited
        }
        MeaningDecision::Unsupported { .. } if has_conventional_candidate => {
            MathAuthoringDisposition::Conventional
        }
        MeaningDecision::Unsupported { .. } => MathAuthoringDisposition::Unsupported,
    }
}

fn authoring_requirement_id(requirement: &MathAuthoringRequirementInfo) -> String {
    match requirement {
        MathAuthoringRequirementInfo::Declaration { requirement_id, .. }
        | MathAuthoringRequirementInfo::RoleDeclaration { requirement_id, .. }
        | MathAuthoringRequirementInfo::Condition { requirement_id, .. }
        | MathAuthoringRequirementInfo::Disambiguation { requirement_id, .. } => {
            requirement_id.clone()
        }
    }
}

fn evidence_order_key(evidence: &[Evidence]) -> (u32, u32) {
    evidence
        .iter()
        .flat_map(|item| item.source_ranges.iter())
        .map(|range| (range.start_offset, range.end_offset))
        .min()
        .unwrap_or((u32::MAX, u32::MAX))
}

fn conventional_candidates(formulas: &[LawRecognition]) -> (Vec<ConventionalCandidateInfo>, bool) {
    let mut candidates = formulas
        .iter()
        .filter(|formula| formula.conventional_candidate)
        .filter(|formula| formula.status != LawRecognitionStatus::Conflicting)
        .filter_map(|formula| {
            let relevance = formula.relevance.clone()?;
            let relation = formula.relation.clone()?;
            let unresolved_bindings = formula
                .bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.proof,
                        LawBindingProof::Asserted | LawBindingProof::Candidate
                    )
                })
                .collect::<Vec<_>>();
            if unresolved_bindings.is_empty() {
                return None;
            }
            let mut requirements = unresolved_bindings
                .into_iter()
                .map(|binding| ConventionalRequirementInfo::RoleDeclaration {
                    requirement_id: format!("{}/binding/{}", formula.law_id, binding.parameter),
                    parameter: binding.parameter.clone(),
                    symbol: binding.symbol.clone(),
                    constraint: binding.constraint.clone(),
                    evidence: vec![binding.evidence.clone()],
                })
                .chain(
                    formula
                        .conditions
                        .iter()
                        .filter(|condition| condition.status != crate::ConstraintStatus::Verified)
                        .cloned()
                        .map(|condition| ConventionalRequirementInfo::Condition {
                            requirement_id: format!(
                                "{}/condition/{}",
                                formula.law_id, condition.condition_id
                            ),
                            condition,
                        }),
                )
                .collect::<Vec<_>>();
            requirements.truncate(MAX_CONVENTIONAL_REQUIREMENTS);
            let mut evidence = formula
                .evidence
                .iter()
                .chain(&relevance.evidence)
                .cloned()
                .collect::<Vec<_>>();
            evidence.sort_by_key(|item| {
                (
                    item.rule_id.clone(),
                    item.source_ranges
                        .first()
                        .map_or(0, |range| range.start_offset),
                )
            });
            evidence.dedup();
            Some(ConventionalCandidateInfo {
                candidate_id: format!(
                    "conventional/{}/{}/{}:{}",
                    formula.pack_id,
                    formula.law_id,
                    formula.range.start_offset,
                    formula.range.end_offset
                ),
                disposition: ConventionalCandidateDisposition::ConventionalCandidate,
                pack_id: formula.pack_id.clone(),
                pack_version: formula.pack_version.clone(),
                law_id: formula.law_id.clone(),
                title: formula.title.clone(),
                relation,
                bindings: formula.bindings.clone(),
                requirements,
                relevance,
                evidence,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        crate::domain::support_rank(left.relevance.support)
            .cmp(&crate::domain::support_rank(right.relevance.support))
            .then(left.pack_id.cmp(&right.pack_id))
            .then(left.law_id.cmp(&right.law_id))
    });
    candidates.dedup_by(|left, right| left.candidate_id == right.candidate_id);
    let truncated = candidates.len() > MAX_CONVENTIONAL_CANDIDATES;
    (candidates, truncated)
}

fn asserted_definition_evidence(
    index: &ProjectSemanticIndex,
    documents: &HashMap<String, AnalyzedDocument>,
    symbol: &SymbolInfo,
) -> Vec<Evidence> {
    let (Some(entity), Some(occurrence)) = (
        symbol.entity_id.as_ref(),
        index.occurrence(&symbol.occurrence_id),
    ) else {
        return Vec::new();
    };
    let claims = index
        .claims_for_entity_at(entity, occurrence)
        .into_iter()
        .filter(|claim| {
            matches!(
                claim.predicate,
                ClaimPredicate::Defines | ClaimPredicate::Abbreviates
            )
        })
        .collect::<Vec<_>>();
    let mut evidence = claims
        .iter()
        .filter_map(|claim| {
            let record = index.evidence(&claim.evidence_id)?;
            let opposed = claims.iter().any(|other| {
                other.predicate == claim.predicate
                    && other.object == claim.object
                    && index
                        .evidence(&other.evidence_id)
                        .is_some_and(|other_record| other_record.polarity != record.polarity)
            });
            (decide_fact(record, opposed) == EntityFactDisposition::Certain)
                .then(|| semantic_evidence(index, documents, record, "source-definition", "hard"))
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    evidence.dedup();
    evidence
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

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

fn equation_node_count(node: &crate::EquationNode) -> u32 {
    1 + node.children.iter().map(equation_node_count).sum::<u32>()
}

fn cursor_is_structural_environment_marker(node: &crate::EquationNode, offset: u32) -> bool {
    if !node.range.contains(offset) {
        return false;
    }
    node.children
        .iter()
        .find(|child| child.range.contains(offset))
        .map_or(node.kind == "environment", |child| {
            cursor_is_structural_environment_marker(child, offset)
        })
}

fn unresolved_control_sequence(symbol: &SymbolInfo) -> bool {
    symbol.source_notation.starts_with('\\')
        && !symbol.source_notation.contains('{')
        && matches!(
            symbol.notation.as_slice(),
            [NotationComponent::NamedSurface { .. }]
        )
}

fn domain_has_correlated_evidence(domains: &[DomainActivation]) -> bool {
    domains.iter().any(|domain| {
        let has_prose = domain
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "prose-domain-prior");
        let has_structure = domain
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "structural-domain-prior");
        has_prose && has_structure
    })
}

fn explicitly_excludes_external_evidence(
    context: &SemanticContextInfo,
    symbol: &SymbolInfo,
) -> bool {
    context.assumptions.iter().any(|assumption| {
        assumption.kind == "project-reachability"
            && assumption.value == "not-included"
            && assumption
                .subjects
                .iter()
                .any(|subject| subject == &symbol.symbol)
    })
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
    documents: &HashMap<String, AnalyzedDocument>,
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
                evidence: semantic_evidence(
                    index,
                    documents,
                    evidence,
                    "derived-constraint",
                    "strong",
                ),
                derived_from,
            })
        })
        .collect()
}

fn derived_shape_infos(
    index: &ProjectSemanticIndex,
    documents: &HashMap<String, AnalyzedDocument>,
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
                evidence: semantic_evidence(
                    index,
                    documents,
                    evidence,
                    "derived-constraint",
                    "strong",
                ),
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
    documents: &HashMap<String, AnalyzedDocument>,
    evidence: &EvidenceRecord,
    kind: &str,
    strength: &str,
) -> Evidence {
    let mut source_anchors = evidence
        .provenance
        .iter()
        .filter_map(|source| index.occurrence(source))
        .filter_map(|occurrence| {
            let document = documents.get(&occurrence.id.file_id)?;
            Some(evidence_source_anchor(document, &occurrence.range))
        })
        .collect::<Vec<_>>();
    normalize_source_anchors(&mut source_anchors);
    let source_ranges = source_anchors
        .iter()
        .map(|anchor| anchor.location.range.clone())
        .collect();
    Evidence {
        rule_id: evidence.rule_id.clone(),
        kind: kind.to_owned(),
        strength: strength.to_owned(),
        source_ranges,
        source_anchors,
    }
}

fn evidence_source_generation(
    document: &AnalyzedDocument,
    range: &SourceRange,
) -> MathSourceGeneration {
    if document.canonical_expressions.iter().any(|expression| {
        expression.range.start_offset <= range.start_offset
            && range.end_offset <= expression.range.end_offset
            && !expression.provenance.is_empty()
    }) {
        MathSourceGeneration::Generated
    } else {
        MathSourceGeneration::Authored
    }
}

fn ground_evidence_in_document(document: &AnalyzedDocument, evidence: &mut Evidence) {
    let mut matched_anchors = vec![false; evidence.source_anchors.len()];
    for range in &evidence.source_ranges {
        if let Some(index) = evidence
            .source_anchors
            .iter()
            .enumerate()
            .position(|(index, anchor)| !matched_anchors[index] && anchor.location.range == *range)
        {
            matched_anchors[index] = true;
        } else {
            evidence
                .source_anchors
                .push(evidence_source_anchor(document, range));
            matched_anchors.push(true);
        }
    }
    normalize_source_anchors(&mut evidence.source_anchors);
    evidence.source_ranges = evidence
        .source_anchors
        .iter()
        .map(|anchor| anchor.location.range.clone())
        .collect();
}

fn evidence_source_anchor(
    document: &AnalyzedDocument,
    range: &SourceRange,
) -> MathInterpretationEvidenceSourceAnchorInfo {
    MathInterpretationEvidenceSourceAnchorInfo {
        location: Location {
            file_id: document.document.file_id.clone(),
            path: document.document.path.clone(),
            range: range.clone(),
        },
        document_version: document.document.document_version,
        scope_path: document.scopes.path_at(range.start_offset),
        lifecycle: MathInterpretationSourceLifecycle::Current,
        generation: evidence_source_generation(document, range),
    }
}

fn append_index_claims(
    index: &ProjectSemanticIndex,
    documents: &HashMap<String, AnalyzedDocument>,
    entity: &EntityId,
    occurrence: &SourceOccurrence,
    context: &mut SemanticContextInfo,
) {
    let raw = index
        .claims_for_entity_at(entity, occurrence)
        .into_iter()
        .filter_map(|claim| {
            let evidence = index.evidence(&claim.evidence_id)?;
            let value = claim_object_display(index, &claim.object)?;
            Some((claim, evidence, value))
        })
        .collect::<Vec<_>>();
    let mut claims = raw
        .iter()
        .map(|(claim, evidence, value)| {
            let conflicts = raw
                .iter()
                .filter(|(other, other_evidence, other_value)| {
                    claim.predicate == other.predicate
                        && value == other_value
                        && evidence.polarity != other_evidence.polarity
                })
                .map(|(other, _, _)| other.id.0.clone())
                .collect::<Vec<_>>();
            let status = match decide_fact(evidence, !conflicts.is_empty()) {
                EntityFactDisposition::Certain => SemanticClaimStatus::Certain,
                EntityFactDisposition::Supported => SemanticClaimStatus::Supported,
                EntityFactDisposition::Speculative => SemanticClaimStatus::Speculative,
                EntityFactDisposition::Conflicting => SemanticClaimStatus::Conflicting,
            };
            let kind = match evidence.origin {
                EvidenceOrigin::Explicit => "source-claim",
                EvidenceOrigin::Derived => "derived-claim",
            };
            let strength = match status {
                SemanticClaimStatus::Certain => "hard",
                SemanticClaimStatus::Supported => "strong",
                SemanticClaimStatus::Speculative | SemanticClaimStatus::Conflicting => "weak",
            };
            crate::SemanticClaimInfo {
                claim_id: claim.id.0.clone(),
                predicate: claim_predicate_name(&claim.predicate).into(),
                value: value.clone(),
                status,
                evidence: vec![semantic_evidence(
                    index, documents, evidence, kind, strength,
                )],
                conflicts,
            }
        })
        .collect::<Vec<_>>();
    claims.sort_by(|left, right| {
        left.predicate
            .cmp(&right.predicate)
            .then(left.value.cmp(&right.value))
            .then(left.claim_id.cmp(&right.claim_id))
    });
    context.claims = claims;
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
    context.concepts = context
        .claims
        .iter()
        .filter(|claim| claim.predicate == "concept")
        .filter_map(|claim| {
            let evidence = claim.evidence.first()?.clone();
            Some(ConceptInfo {
                concept_id: claim.value.clone(),
                label: concept_label(&claim.value),
                description: claim.value.clone(),
                evidence,
            })
        })
        .collect();
    context
        .concepts
        .sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
    context
        .concepts
        .dedup_by(|left, right| left.concept_id == right.concept_id);
}

fn claim_object_display(index: &ProjectSemanticIndex, object: &ClaimObject) -> Option<String> {
    match object {
        ClaimObject::Value(value) => claim_value_display(value),
        ClaimObject::Occurrence(occurrence) => index
            .occurrence(occurrence)
            .map(|source| source.surface.clone()),
        ClaimObject::Entity(entity) => index
            .occurrence(&entity.anchor)
            .map(|source| source.surface.clone()),
    }
}

fn concept_label(concept_id: &str) -> String {
    concept_id
        .split(':')
        .next_back()
        .unwrap_or(concept_id)
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

fn diagnostic_has_typed_constraint_evidence(diagnostic: &SemanticDiagnostic) -> bool {
    diagnostic.evidence.iter().any(|evidence| {
        matches!(
            evidence.kind.as_str(),
            "derived-constraint" | "explicit-constraint"
        )
    })
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
            let evidence = constraint_conflict_evidence(semantic, conflict);
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

fn meaning_conflict(
    semantic: &ProjectSemanticIndex,
    conflict: &PlannedConflict,
) -> Option<(SourceRange, MeaningConflict)> {
    let anchor = semantic.occurrence(&conflict.subject.anchor)?;
    Some((
        anchor.range.clone(),
        MeaningConflict {
            conflict_id: conflict.code.clone(),
            label: conflict.summary.clone(),
            evidence: constraint_conflict_evidence(semantic, conflict),
        },
    ))
}

fn constraint_conflict_evidence(
    semantic: &ProjectSemanticIndex,
    conflict: &PlannedConflict,
) -> Vec<Evidence> {
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
                source_anchors: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    evidence.dedup();
    evidence
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

fn locations_refusal(reason: EntitySurfaceRefusal) -> QueryValue {
    QueryValue::Locations {
        authorization: refused_authorization(reason),
        locations: Vec::new(),
    }
}

fn rename_preparation_refusal(reason: EntitySurfaceRefusal) -> QueryValue {
    QueryValue::RenamePreparation {
        authorization: refused_authorization(reason),
        range: None,
        placeholder: None,
    }
}

fn alternate_name(current: &str, family: RenameNotationFamily) -> String {
    match family {
        RenameNotationFamily::PlainIdentifier => if current == "z" { "y" } else { "z" }.into(),
        RenameNotationFamily::ControlSequence => if current == "\\zeta" {
            "\\eta"
        } else {
            "\\zeta"
        }
        .into(),
    }
}

fn edit_proposal_refusal(reason: EntitySurfaceRefusal) -> QueryValue {
    QueryValue::EditProposal {
        authorization: refused_authorization(reason),
        proposal: None,
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
