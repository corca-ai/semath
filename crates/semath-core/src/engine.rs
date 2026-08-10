use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use thiserror::Error;

use crate::binder::{binder_at, binders, bound_occurrences, rename_rejection};
use crate::canonical::{SemanticExpr, lower_document_region};
use crate::cursor::{interior_offset, item_at_cursor_with_trailing_edge};
use crate::hygiene::{HygieneAnalysis, analyze_hygiene};
use crate::law::ExternalTypeEnvironment;
use crate::parser::{ParsedMath, parse_snapshot, selection_path};
use crate::project_order::{ProjectOrder, ProjectOrderDocument};
use crate::scope::ScopeGraph;
use crate::semantic::SemanticFactStore;
use crate::semantic_index::{
    Claim, ClaimId, ClaimObject, ClaimPredicate, DocumentSemanticFacts, EntityId, EvidenceId,
    EvidenceModality, EvidenceOrigin, EvidencePolarity, EvidenceRecord, InferenceTier, Mention,
    MentionModality, NotationComponent, OccurrenceKind, ProjectSemanticIndex, ResolutionStatus,
    SourceOccurrence, SourceOccurrenceId,
};
use crate::{
    AnalysisStats, ChangeEnvelope, DefinitionInfo, Evidence, Location, PROTOCOL_VERSION,
    ProjectChange, ProjectDocument, ProjectSnapshot, QuantityInfo, Query, QueryEnvelope,
    QueryResult, QueryValue, RenamePreparation, RoleInfo, SemanticContextInfo, SemanticDiagnostic,
    SemanticEditFile, SemanticEditProposal, SemanticTextEdit, SemanticViewInfo, ShapeInfo,
    SourceRange, SymbolInfo, UpdateResult,
};

const MAX_SYMBOL_DEFINITIONS: usize = 8;
const MAX_SYMBOL_DIAGNOSTICS: usize = 8;
const MAX_VIEW_DIAGNOSTICS: usize = 8;
const MAX_VIEW_DECLARATIONS: usize = 16;

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
}

#[derive(Clone, Debug)]
struct SemanticOccurrenceSeed {
    surface: String,
    selection_range: SourceRange,
    range: SourceRange,
    structural_path: Vec<u32>,
    source_text: String,
    notation: Vec<NotationComponent>,
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

impl AnalyzedDocument {
    fn analyze(mut document: ProjectDocument) -> Result<(Self, SemanticFactStore), EngineError> {
        #[cfg(test)]
        let parsed = if document.nodes.is_empty() {
            crate::parser::parse_regions(&document.content, &document.math_regions)
        } else {
            parse_snapshot(&document).map_err(EngineError::InvalidSyntaxSnapshot)?
        };
        #[cfg(not(test))]
        let parsed = parse_snapshot(&document).map_err(EngineError::InvalidSyntaxSnapshot)?;
        let scopes = ScopeGraph::new(&document);
        let facts = SemanticFactStore::build(&document, &parsed);
        let hygiene = analyze_hygiene(&document, &parsed, &facts.definitions);
        let semantic_occurrences = parsed
            .iter()
            .flat_map(|math| &math.symbols)
            .map(|(surface, selection_range)| {
                let range = notation_occurrence_range(&document, selection_range);
                SemanticOccurrenceSeed {
                    surface: surface.clone(),
                    selection_range: selection_range.clone(),
                    structural_path: notation_path(&document, selection_range),
                    source_text: source_text(&document, &range),
                    notation: notation_components(&document, selection_range, surface),
                    range,
                }
            })
            .collect();
        let canonical_expressions = parsed
            .iter()
            .map(|math| {
                let mut expression = lower_document_region(&document, &math.region.content_range);
                expression.range = math.region.content_range.clone();
                expression
            })
            .collect();
        let analysis_fingerprint = analysis_fingerprint(&document);
        compact_analyzed_document(&mut document);
        Ok((
            Self {
                component_id: document.file_id.clone(),
                document,
                parsed,
                hygiene,
                scopes,
                analysis_fingerprint,
                canonical_expressions,
                semantic_occurrences,
            },
            facts,
        ))
    }
}

#[derive(Default)]
struct ProjectState {
    documents: HashMap<String, AnalyzedDocument>,
    facts: HashMap<String, SemanticFactStore>,
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
        let (mut document, facts) = AnalyzedDocument::analyze(document)?;
        if let Some(component_id) = previous_component {
            document.component_id = component_id;
        }
        self.documents.insert(file_id.clone(), document);
        self.facts.insert(file_id, facts);
        Ok(())
    }

    fn remove(&mut self, file_id: &str) {
        self.documents.remove(file_id);
        self.facts.remove(file_id);
        self.external_types.remove(file_id);
        self.semantic.remove_document(file_id);
        self.definitions_by_entity
            .retain(|entity, _| entity.anchor.file_id != file_id);
        self.occurrences_by_range
            .retain(|(candidate, _, _), _| candidate != file_id);
    }

    fn facts(&self, file_id: &str) -> &SemanticFactStore {
        self.facts
            .get(file_id)
            .expect("every analyzed document must have one semantic fact store")
    }

    fn facts_mut(&mut self, file_id: &str) -> &mut SemanticFactStore {
        self.facts
            .get_mut(file_id)
            .expect("every analyzed document must have one semantic fact store")
    }

    fn order_document(&self, file_id: &str) -> Option<ProjectOrderDocument> {
        let document = self.documents.get(file_id)?;
        let facts = self.facts(file_id);
        Some(ProjectOrderDocument {
            file_id: file_id.to_owned(),
            includes: document.document.includes.clone(),
            occurrence_offsets: document
                .parsed
                .iter()
                .flat_map(|math| math.symbols.iter().map(|(_, range)| range.start_offset))
                .chain(
                    facts
                        .definitions
                        .iter()
                        .map(|definition| definition.location.range.start_offset),
                )
                .chain(
                    facts
                        .roles
                        .exported()
                        .into_iter()
                        .flat_map(|role| role.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .chain(
                    facts
                        .quantities
                        .exported()
                        .into_iter()
                        .flat_map(|quantity| quantity.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .chain(
                    facts
                        .shapes
                        .exported()
                        .into_iter()
                        .flat_map(|shape| shape.evidence.source_ranges)
                        .map(|range| range.start_offset),
                )
                .collect(),
            path: document.document.path.clone(),
        })
    }

    fn rebuild_semantic_index(&mut self) -> Result<(), EngineError> {
        self.semantic = ProjectSemanticIndex::default();
        self.definitions_by_entity.clear();
        self.occurrences_by_range.clear();
        let mut file_ids = self.documents.keys().cloned().collect::<Vec<_>>();
        file_ids.sort();
        for file_id in file_ids {
            self.replace_semantic_document(&file_id)?;
        }
        Ok(())
    }

    fn replace_semantic_document(&mut self, file_id: &str) -> Result<(), EngineError> {
        let document = self
            .documents
            .get(file_id)
            .expect("semantic lowering requires an analyzed document");
        let facts = self.facts(file_id);
        let lowered = lower_semantic_document(document, facts, &self.order);
        self.semantic
            .replace_document(lowered.facts)
            .map_err(EngineError::InvalidSemanticFacts)?;
        self.definitions_by_entity
            .retain(|entity, _| entity.anchor.file_id != file_id);
        self.occurrences_by_range
            .retain(|(candidate, _, _), _| candidate != file_id);
        self.definitions_by_entity.extend(lowered.definitions);
        self.occurrences_by_range.extend(lowered.occurrences);
        Ok(())
    }
}

struct LoweredSemanticDocument {
    facts: DocumentSemanticFacts,
    definitions: BTreeMap<EntityId, DefinitionInfo>,
    occurrences: HashMap<(String, u32, u32), SourceOccurrenceId>,
}

fn lower_semantic_document(
    document: &AnalyzedDocument,
    facts: &SemanticFactStore,
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
        occurrences.push(SourceOccurrence {
            id,
            component_id: document.component_id.clone(),
            kind: OccurrenceKind::Notation,
            range: seed.range.clone(),
            selection_range: seed.selection_range.clone(),
            scope_path: document.scopes.path_at(seed.selection_range.start_offset),
            structural_path: seed.structural_path.clone(),
            availability_order: order
                .position(&source.file_id, seed.selection_range.start_offset)
                .unwrap_or(u64::MAX),
            surface: seed.surface.clone(),
            source_text: seed.source_text.clone(),
            notation: seed.notation.clone(),
        });
    }
    occurrences.sort_by_key(|item| {
        (
            item.range.start_offset,
            item.range.end_offset,
            item.id.local_id,
        )
    });
    occurrences.dedup_by(|left, right| left.range == right.range && left.surface == right.surface);
    occurrences_by_range.clear();
    for (local_id, occurrence) in occurrences.iter_mut().enumerate() {
        occurrence.id.local_id = local_id as u32;
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

    let mut entities = Vec::new();
    let mut evidence = Vec::new();
    let mut claims = Vec::new();
    let mut definitions = BTreeMap::new();
    for (definition_index, definition) in facts.definitions.iter().enumerate() {
        let key = (
            source.file_id.clone(),
            definition.location.range.start_offset,
            definition.location.range.end_offset,
        );
        let Some(anchor) = occurrences_by_range.get(&key).cloned() else {
            continue;
        };
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
            .position(&source.file_id, definition.location.range.start_offset)
            .unwrap_or(u64::MAX)
            .max(
                occurrences
                    .iter()
                    .find(|occurrence| occurrence.id == anchor)
                    .map_or(0, |occurrence| occurrence.availability_order),
            );
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
            evidence_id,
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        });
        definitions.insert(entity.clone(), definition.clone());
        entities.push(entity);
    }
    let mentions = occurrences
        .iter()
        .map(|occurrence| Mention {
            occurrence_id: occurrence.id.clone(),
            modality: MentionModality::Notation,
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
        },
        definitions,
        occurrences: occurrences_by_range,
    }
}

fn notation_occurrence_range(document: &ProjectDocument, selection: &SourceRange) -> SourceRange {
    document
        .nodes
        .iter()
        .filter(|node| {
            let identity_range = match node.kind {
                crate::NotationNodeKind::NamedOperator => node.ranges.name.as_ref(),
                crate::NotationNodeKind::Modifier
                | crate::NotationNodeKind::Style
                | crate::NotationNodeKind::Script => node.ranges.nucleus.as_ref(),
                _ => None,
            };
            identity_range.is_some_and(|identity| {
                identity.start_offset <= selection.start_offset
                    && selection.end_offset <= identity.end_offset
            })
        })
        .max_by_key(|node| node.ranges.full.end_offset - node.ranges.full.start_offset)
        .map_or_else(|| selection.clone(), |node| node.ranges.full.clone())
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
                Some("subscript") => components.push(NotationComponent::Subscript),
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

fn source_text(document: &ProjectDocument, range: &SourceRange) -> String {
    let index = crate::SourceIndex::new(&document.content);
    let start = index.byte_for_utf16(range.start_offset);
    let end = index.byte_for_utf16(range.end_offset);
    document.content.get(start..end).unwrap_or("").to_owned()
}

fn analysis_fingerprint(document: &ProjectDocument) -> u64 {
    let encoded = serde_json::to_vec(&(
        document.schema_version,
        document.language,
        &document.nodes,
        &document.math_roots,
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
    document.scopes.clear();
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
        check_protocol(snapshot.protocol_version)?;
        let changed_file_ids = snapshot
            .documents
            .iter()
            .map(|doc| doc.file_id.clone())
            .collect::<Vec<_>>();
        self.epoch = snapshot.epoch;
        self.inventory_version = snapshot.inventory_version;
        self.project_id = snapshot.project_id;
        self.main_file_id = snapshot.main_file_id;
        self.analysis_generation = 0;
        self.index.documents.clear();
        self.index.facts.clear();
        for document in snapshot.documents {
            self.index.replace(document)?;
        }
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
        let mut semantic_changed = false;
        for change in envelope.changes {
            match change {
                ProjectChange::Upsert { document } => {
                    let file_id = document.file_id.clone();
                    let accept = self.index.documents.get(&file_id).is_none_or(|current| {
                        document.document_version > current.document.document_version
                    });
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
                        order_changed |= previous_order != self.index.order_document(&file_id);
                        changed.push(file_id);
                    }
                }
                ProjectChange::PathChange { file_id, path } => {
                    let document = self.index.documents.get_mut(&file_id).unwrap();
                    order_changed |= document.document.path != path;
                    semantic_changed = true;
                    document.document.path = path;
                    changed.push(file_id);
                }
                ProjectChange::Remove { file_id } => {
                    order_changed = true;
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
            if order_changed {
                self.index.rebuild_semantic_index()?;
            } else {
                for file_id in &analyzed {
                    self.index.replace_semantic_document(file_id)?;
                }
            }
        }
        if !order_changed {
            for file_id in &revision_only {
                self.index.replace_semantic_document(file_id)?;
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
        let facts = self.index.facts(file_id);
        if document.document.document_version != envelope.document_version {
            return Err(EngineError::DocumentVersionMismatch);
        }
        let offset = query_offset.unwrap_or(0);
        let parsed =
            query_offset.and_then(|offset| parsed_math_at_cursor(&document.parsed, offset));
        let symbol = parsed.and_then(|math| symbol_at_cursor(math, offset));
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
                    facts,
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
                diagnostics: document_diagnostics(document, facts, hygiene_enabled),
            },
            Query::ExplainDiagnostic { code, .. } => QueryValue::DiagnosticExplanation {
                diagnostic: facts
                    .shapes
                    .diagnostic(&code, cursor_offset)
                    .or_else(|| facts.roles.diagnostic(&code, cursor_offset))
                    .or_else(|| facts.quantities.diagnostic(&code, cursor_offset))
                    .or_else(|| {
                        hygiene_enabled
                            .then(|| document.hygiene.diagnostic(&code, cursor_offset))
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
                    .facts
                    .values()
                    .map(|facts| facts.laws.all().len() as u32)
                    .sum(),
                semantic_nodes: analyzed_file_ids
                    .iter()
                    .filter_map(|file_id| self.index.documents.get(file_id))
                    .flat_map(|document| &document.parsed)
                    .map(|math| equation_node_count(&math.root))
                    .sum(),
                constraints: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.facts(file_id).constraint_count())
                    .sum(),
                law_rules_visited: analyzed_file_ids
                    .iter()
                    .map(|file_id| self.index.facts(file_id).laws.visited_rules())
                    .sum(),
                semantic_occurrences: semantic_stats.occurrences,
                semantic_entities: semantic_stats.entities,
                semantic_claims: semantic_stats.claims,
                semantic_evidence: semantic_stats.evidence,
                semantic_dependency_edges: semantic_stats.dependency_edges,
                invalidated_semantic_claims: semantic_stats.invalidated_claims,
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
        for ((file_id, start_offset, end_offset), occurrence_id) in &self.index.occurrences_by_range
        {
            let resolution = self.index.semantic.resolve(occurrence_id);
            if resolution.status == ResolutionStatus::Established
                && resolution.candidates.len() == 1
                && resolution.candidates[0].entity_id == *entity
            {
                let document = &self.index.documents[file_id];
                locations.push(Location {
                    file_id: file_id.clone(),
                    path: document.document.path.clone(),
                    range: SourceRange {
                        start_offset: *start_offset,
                        end_offset: *end_offset,
                    },
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

    fn semantic_context(
        &self,
        facts: &SemanticFactStore,
        file_id: &str,
        symbol: Option<&(String, SourceRange)>,
        offset: u32,
    ) -> SemanticContextInfo {
        let (definitions, entity_id, symbol_name) = symbol
            .map(|(name, occurrence)| {
                (
                    self.visible_definitions(file_id, occurrence, name),
                    self.resolved_entity(file_id, occurrence),
                    Some(name.clone()),
                )
            })
            .unwrap_or_else(|| (Vec::new(), None, None));
        facts.context(
            definitions,
            symbol_name,
            entity_id,
            offset,
            self.index.external_types.get(file_id),
        )
    }

    fn semantic_view(
        &self,
        document: &AnalyzedDocument,
        facts: &SemanticFactStore,
        parsed: Option<&ParsedMath>,
        symbol: Option<&(String, SourceRange)>,
        offset: u32,
        hygiene_enabled: bool,
    ) -> SemanticViewInfo {
        let symbol_info = symbol.as_ref().and_then(|(name, occurrence)| {
            self.symbol_info(document, facts, name, occurrence, offset, hygiene_enabled)
        });
        let context = self.semantic_context(facts, &document.document.file_id, symbol, offset);
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
        let mut diagnostics = document_diagnostics(document, facts, hygiene_enabled)
            .into_iter()
            .filter(|diagnostic| {
                parsed.is_some_and(|math| {
                    ranges_overlap(&diagnostic.range, &math.region.content_range)
                }) || diagnostic.range.contains(offset)
            })
            .collect::<Vec<_>>();
        let diagnostics_truncated = diagnostics.len() > MAX_VIEW_DIAGNOSTICS;
        diagnostics.truncate(MAX_VIEW_DIAGNOSTICS);
        let (domains, domains_truncated) = facts.domains.at(offset);
        let conflicting = diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.severity.as_str(), "error" | "warning"));
        let (status, summary, refusal) = if conflicting {
            (
                "conflicting",
                "Conflicting semantic evidence",
                Some(
                    "Resolve the source-linked conflicts before applying this interpretation."
                        .into(),
                ),
            )
        } else if context.relations.len() > 1 {
            (
                "ambiguous",
                "Multiple semantic interpretations remain",
                Some(
                    "Add type, quantity, shape, or role declarations to disambiguate the formula."
                        .into(),
                ),
            )
        } else if let Some(relation) = context.relations.first() {
            ("established", relation.title.as_str(), None)
        } else if let Some(info) = &symbol_info {
            (
                "partial",
                info.definitions
                    .first()
                    .map_or(info.symbol.as_str(), |definition| {
                        definition.description.as_str()
                    }),
                None,
            )
        } else {
            (
                "unsupported",
                "No supported semantic interpretation",
                Some(
                    "Semath could not establish a typed meaning from the available evidence."
                        .into(),
                ),
            )
        };
        SemanticViewInfo {
            status: status.into(),
            summary: summary.into(),
            symbol: symbol_info,
            context,
            declarations,
            diagnostics,
            domains,
            refusal,
            truncated: declarations_truncated || diagnostics_truncated || domains_truncated,
        }
    }

    fn symbol_info(
        &self,
        document: &AnalyzedDocument,
        facts: &SemanticFactStore,
        name: &str,
        occurrence: &SourceRange,
        offset: u32,
        hygiene_enabled: bool,
    ) -> Option<SymbolInfo> {
        let mut definitions =
            self.visible_definitions(&document.document.file_id, occurrence, name);
        let definitions_truncated = definitions.len() > MAX_SYMBOL_DEFINITIONS;
        definitions.truncate(MAX_SYMBOL_DEFINITIONS);
        let external = self.index.external_types.get(&document.document.file_id);
        let (mut shapes, shapes_truncated) = facts.shapes.claims_at(name, offset);
        let (mut roles, roles_truncated) = facts.roles.roles_at(name, offset);
        let (mut quantities, quantities_truncated) = facts.quantities.at(name, offset);
        if let Some(external) = external {
            shapes.extend(external.shapes_at(offset, name));
            roles.extend(external.roles_at(offset, name));
            quantities.extend(external.quantities_at(offset, name));
            shapes.sort_by(|left, right| left.kind.cmp(&right.kind));
            shapes.dedup();
            roles.sort_by(|left, right| left.concept_id.cmp(&right.concept_id));
            roles.dedup();
            quantities.sort_by(|left, right| left.display.cmp(&right.display));
            quantities.dedup();
        }
        let (diagnostics, diagnostics_truncated) =
            symbol_diagnostics(document, facts, name, offset, &shapes, hygiene_enabled);
        let occurrence_id = self.index.occurrences_by_range.get(&(
            document.document.file_id.clone(),
            occurrence.start_offset,
            occurrence.end_offset,
        ))?;
        let semantic_occurrence = self.index.semantic.occurrence(occurrence_id)?;
        Some(SymbolInfo {
            symbol: name.into(),
            occurrence_id: occurrence_id.clone(),
            notation: semantic_occurrence.notation.clone(),
            source_notation: semantic_occurrence.source_text.clone(),
            entity_id: self.resolved_entity(&document.document.file_id, occurrence),
            location: Location {
                file_id: document.document.file_id.clone(),
                path: document.document.path.clone(),
                range: occurrence.clone(),
            },
            definitions,
            shapes,
            quantities,
            roles,
            diagnostics,
            truncated: definitions_truncated
                || shapes_truncated
                || quantities_truncated
                || roles_truncated
                || diagnostics_truncated,
        })
    }

    fn refresh_project_laws(&mut self, targets: &HashSet<String>) {
        let facts = self
            .index
            .facts
            .iter()
            .flat_map(|(file_id, semantic)| {
                let document = self.index.documents.get(file_id).unwrap();
                let component_id = document.component_id.clone();
                let roles = semantic.roles.exported().into_iter().map({
                    let component_id = component_id.clone();
                    let file_id = file_id.clone();
                    move |role| IndexedTypeFact {
                        component_id: component_id.clone(),
                        source_offset: evidence_anchor(&role.evidence),
                        file_id: file_id.clone(),
                        fact: ExportedTypeFact::Role(role),
                    }
                });
                let quantities = semantic.quantities.exported().into_iter().map({
                    let component_id = component_id.clone();
                    let file_id = file_id.clone();
                    move |quantity| IndexedTypeFact {
                        component_id: component_id.clone(),
                        source_offset: evidence_anchor(&quantity.evidence),
                        file_id: file_id.clone(),
                        fact: ExportedTypeFact::Quantity(quantity),
                    }
                });
                let shapes = semantic.shapes.exported().into_iter().map({
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
            self.index.facts_mut(&file_id).refresh_laws(
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

fn symbol_at_cursor(math: &ParsedMath, offset: u32) -> Option<(String, SourceRange)> {
    symbol_range_at_cursor(&math.symbols, offset)
        .map(|(symbol, range)| (symbol.clone(), range.clone()))
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

fn document_diagnostics(
    document: &AnalyzedDocument,
    facts: &SemanticFactStore,
    hygiene_enabled: bool,
) -> Vec<SemanticDiagnostic> {
    let mut diagnostics = facts.shapes.diagnostics.clone();
    diagnostics.extend(facts.quantities.diagnostics.iter().cloned());
    diagnostics.extend(facts.roles.diagnostics.iter().cloned());
    if hygiene_enabled {
        diagnostics.extend(document.hygiene.diagnostics.iter().cloned());
    }
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start_offset);
    diagnostics
}

fn symbol_diagnostics(
    document: &AnalyzedDocument,
    facts: &SemanticFactStore,
    symbol: &str,
    offset: u32,
    shapes: &[ShapeInfo],
    hygiene_enabled: bool,
) -> (Vec<SemanticDiagnostic>, bool) {
    let (mut diagnostics, shape_truncated) = facts.shapes.diagnostics_for(offset, shapes);
    let (role_diagnostics, role_truncated) = facts.roles.diagnostics_for(symbol, offset);
    diagnostics.extend(role_diagnostics);
    diagnostics.extend(
        facts
            .quantities
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.range.contains(offset))
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
