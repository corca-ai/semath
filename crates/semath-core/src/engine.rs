use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::binder::{binder_at, binders, bound_occurrences, rename_rejection};
use crate::cursor::{interior_offset, item_at_cursor_with_trailing_edge};
use crate::hygiene::{HygieneAnalysis, analyze_hygiene};
use crate::law::ExternalTypeEnvironment;
use crate::parser::{ParsedMath, parse_regions, selection_path};
use crate::project_order::{ProjectOrder, ProjectOrderDocument};
use crate::scope::ScopeGraph;
use crate::semantic::SemanticFactStore;
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
}

#[derive(Clone)]
enum ExportedTypeFact {
    Role(RoleInfo),
    Quantity(QuantityInfo),
    Shape(ShapeInfo),
}

#[derive(Clone)]
struct IndexedTypeFact {
    component_id: String,
    fact: ExportedTypeFact,
    file_id: String,
    source_offset: u32,
}

impl AnalyzedDocument {
    fn analyze(document: ProjectDocument) -> (Self, SemanticFactStore) {
        let parsed = parse_regions(&document.content, &document.math_regions);
        let scopes = ScopeGraph::new(&document);
        let facts = SemanticFactStore::build(&document, &parsed);
        let hygiene = analyze_hygiene(&document, &parsed, &facts.definitions);
        (
            Self {
                component_id: document.file_id.clone(),
                document,
                parsed,
                hygiene,
                scopes,
            },
            facts,
        )
    }
}

#[derive(Default)]
struct ProjectSemanticIndex {
    documents: HashMap<String, AnalyzedDocument>,
    facts: HashMap<String, SemanticFactStore>,
    order: ProjectOrder,
    external_types: HashMap<String, ExternalTypeEnvironment>,
}

impl ProjectSemanticIndex {
    fn replace(&mut self, document: ProjectDocument) {
        let file_id = document.file_id.clone();
        let (document, facts) = AnalyzedDocument::analyze(document);
        self.documents.insert(file_id.clone(), document);
        self.facts.insert(file_id, facts);
    }

    fn remove(&mut self, file_id: &str) {
        self.documents.remove(file_id);
        self.facts.remove(file_id);
        self.external_types.remove(file_id);
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
}

#[derive(Default)]
pub struct SemathEngine {
    epoch: String,
    inventory_version: u64,
    project_id: String,
    main_file_id: Option<String>,
    analysis_generation: u64,
    index: ProjectSemanticIndex,
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
            self.index.replace(document);
        }
        self.refresh_semantic_identities();
        self.refresh_project_laws(&changed_file_ids.iter().cloned().collect());
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
        let mut analyzed_directly = HashSet::new();
        for change in envelope.changes {
            match change {
                ProjectChange::Upsert { document } => {
                    let file_id = document.file_id.clone();
                    let accept = self.index.documents.get(&file_id).is_none_or(|current| {
                        document.document_version > current.document.document_version
                    });
                    if accept {
                        self.index.replace(document);
                        analyzed_directly.insert(file_id.clone());
                        changed.push(file_id);
                    }
                }
                ProjectChange::PathChange { file_id, path } => {
                    let document = self.index.documents.get_mut(&file_id).unwrap();
                    document.document.path = path;
                    changed.push(file_id);
                }
                ProjectChange::Remove { file_id } => {
                    self.index.remove(&file_id);
                    changed.push(file_id);
                }
            }
        }
        self.inventory_version = envelope.inventory_version;
        self.analysis_generation = envelope.analysis_generation;
        self.refresh_semantic_identities();
        affected.extend(self.index.order.affected_by(&requested));
        let dependents = affected
            .iter()
            .filter(|file_id| !analyzed_directly.contains(*file_id))
            .filter_map(|file_id| {
                self.index
                    .documents
                    .get(file_id)
                    .map(|document| (file_id.clone(), document.document.clone()))
            })
            .collect::<Vec<_>>();
        for (_, document) in dependents {
            self.index.replace(document);
        }
        self.refresh_semantic_identities();
        let mut analyzed = affected
            .into_iter()
            .filter(|file_id| self.index.documents.contains_key(file_id))
            .collect::<Vec<_>>();
        analyzed.sort();
        self.refresh_project_laws(&analyzed.iter().cloned().collect());
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

    fn definitions_for(&self, symbol: &str) -> Vec<DefinitionInfo> {
        let mut definitions: Vec<_> = self
            .index
            .facts
            .values()
            .flat_map(|facts| facts.definitions.iter())
            .filter(|definition| definition.symbol == symbol)
            .cloned()
            .collect();
        definitions.sort_by(|left, right| {
            left.location.path.cmp(&right.location.path).then(
                left.location
                    .range
                    .start_offset
                    .cmp(&right.location.range.start_offset),
            )
        });
        definitions
    }

    fn visible_definitions(
        &self,
        file_id: &str,
        occurrence: &SourceRange,
        symbol: &str,
    ) -> Vec<DefinitionInfo> {
        let Some(document) = self.index.documents.get(file_id) else {
            return Vec::new();
        };
        let occurrence_scope = document.scopes.path_at(occurrence.start_offset);
        let candidates = self
            .definitions_for(symbol)
            .into_iter()
            .filter(|definition| {
                definition
                    .semantic_id
                    .as_ref()
                    .is_some_and(|identity| identity.component_id == document.component_id)
            })
            .filter(|definition| {
                if definition.location.file_id == file_id {
                    definition.location.range.start_offset <= occurrence.start_offset
                        && definition.semantic_id.as_ref().is_some_and(|identity| {
                            scope_visible(&identity.scope_path, &occurrence_scope)
                        })
                } else {
                    self.index.order.precedes(
                        &definition.location.file_id,
                        definition.location.range.start_offset,
                        file_id,
                        occurrence.start_offset,
                    )
                }
            })
            .collect::<Vec<_>>();
        let mut local = candidates
            .iter()
            .filter(|definition| {
                definition.location.file_id == file_id
                    && definition.location.range.start_offset <= occurrence.start_offset
                    && definition.semantic_id.as_ref().is_some_and(|identity| {
                        scope_visible(&identity.scope_path, &occurrence_scope)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        local.sort_by_key(|definition| {
            let identity = definition.semantic_id.as_ref().unwrap();
            (
                identity.scope_path.len(),
                definition.location.range.start_offset,
            )
        });
        if !local.is_empty() {
            return local;
        }
        if candidates.len() == 1 {
            return candidates;
        }
        let document_scoped = candidates
            .into_iter()
            .filter(|definition| {
                definition
                    .semantic_id
                    .as_ref()
                    .is_some_and(|identity| identity.scope_path.is_empty())
            })
            .collect::<Vec<_>>();
        (document_scoped.len() == 1)
            .then(|| document_scoped[0].clone())
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
            .max_by_key(|definition| {
                let identity = definition.semantic_id.as_ref().unwrap();
                (
                    identity.scope_path.len(),
                    definition.location.range.start_offset,
                )
            })
    }

    fn references_for(&self, definition: &DefinitionInfo) -> Vec<Location> {
        let mut locations = Vec::new();
        for document in self.index.documents.values() {
            for math in &document.parsed {
                for (symbol, range) in &math.symbols {
                    if symbol != &definition.symbol {
                        continue;
                    }
                    if self
                        .resolve_definition(&document.document.file_id, range, symbol)
                        .as_ref()
                        .is_some_and(|resolved| resolved.semantic_id == definition.semantic_id)
                    {
                        locations.push(Location {
                            file_id: document.document.file_id.clone(),
                            path: document.document.path.clone(),
                            range: range.clone(),
                        });
                    }
                }
            }
        }
        locations.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.range.start_offset.cmp(&right.range.start_offset))
        });
        locations
    }

    fn semantic_context(
        &self,
        facts: &SemanticFactStore,
        file_id: &str,
        symbol: Option<&(String, SourceRange)>,
        offset: u32,
    ) -> SemanticContextInfo {
        let (definitions, semantic_id, symbol_name) = symbol
            .map(|(name, occurrence)| {
                (
                    self.visible_definitions(file_id, occurrence, name),
                    self.resolve_definition(file_id, occurrence, name)
                        .and_then(|definition| definition.semantic_id),
                    Some(name.clone()),
                )
            })
            .unwrap_or_else(|| (Vec::new(), None, None));
        facts.context(
            definitions,
            symbol_name,
            semantic_id,
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
        let symbol_info = symbol.as_ref().map(|(name, occurrence)| {
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
    ) -> SymbolInfo {
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
        SymbolInfo {
            symbol: name.into(),
            semantic_id: self
                .resolve_definition(&document.document.file_id, occurrence, name)
                .and_then(|definition| definition.semantic_id),
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
        }
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
                    for fact in &facts {
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
                Some((file_id.clone(), environment))
            })
            .collect::<Vec<_>>();
        for (file_id, environment) in environments {
            let document = self.index.documents.get(&file_id).unwrap();
            let source = document.document.clone();
            let parsed = document.parsed.clone();
            self.index
                .facts_mut(&file_id)
                .refresh_laws(&source, &parsed, &environment);
            self.index.external_types.insert(file_id, environment);
        }
    }

    fn refresh_semantic_identities(&mut self) {
        let project_order = ProjectOrder::new(
            self.index
                .documents
                .values()
                .map(|document| {
                    let facts = self.index.facts(&document.document.file_id);
                    ProjectOrderDocument {
                        file_id: document.document.file_id.clone(),
                        includes: document.document.includes.clone(),
                        occurrence_offsets: document
                            .parsed
                            .iter()
                            .flat_map(|math| {
                                math.symbols.iter().map(|(_, range)| range.start_offset)
                            })
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
                    }
                })
                .collect(),
            self.main_file_id.as_deref(),
        );
        for (file_id, document) in &mut self.index.documents {
            document.component_id = project_order
                .component_for(file_id)
                .map(str::to_owned)
                .unwrap_or_else(|| file_id.clone());
        }
        for (file_id, facts) in &mut self.index.facts {
            let component_id = &self.index.documents.get(file_id).unwrap().component_id;
            for definition in &mut facts.definitions {
                if let Some(identity) = &mut definition.semantic_id {
                    identity.component_id.clone_from(component_id);
                }
            }
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

fn scope_visible(definition: &[u32], occurrence: &[u32]) -> bool {
    definition.len() <= occurrence.len()
        && definition
            .iter()
            .zip(occurrence)
            .all(|(left, right)| left == right)
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
