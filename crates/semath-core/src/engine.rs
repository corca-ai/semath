use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::binder::{binder_at, binders, bound_occurrences, rename_rejection};
use crate::consistency::{ConsistencyAnalysis, analyze_consistency};
use crate::domain::{DomainAnalysis, analyze_domains};
use crate::hygiene::{HygieneAnalysis, analyze_hygiene};
use crate::parser::{ParsedMath, deepest_node, math_regions, parse_regions, selection_path};
use crate::pattern::{FormulaAnalysis, analyze_formulas, formula_completions};
use crate::prose::analyze_prose;
use crate::rewrite::formula_rewrites;
use crate::scope::ScopeGraph;
use crate::shape::{ShapeAnalysis, analyze_shapes};
use crate::{
    ChangeEnvelope, DefinitionInfo, DocumentLanguage, EquationNode, EquationNodeSummary, Evidence,
    InspectionInfo, Location, PROTOCOL_VERSION, ProjectChange, ProjectDocument, ProjectSnapshot,
    Query, QueryEnvelope, QueryResult, QueryValue, RenamePreparation, SemanticDiagnostic,
    SemanticEditFile, SemanticEditProposal, SemanticTextEdit, ShapeInfo, SourceRange, SymbolInfo,
    UpdateResult,
};

const MAX_SYMBOL_DEFINITIONS: usize = 8;
const MAX_SYMBOL_DIAGNOSTICS: usize = 8;
const MAX_INSPECTION_DIAGNOSTICS: usize = 8;
const MAX_INSPECTION_REFERENCES: usize = 32;
const MAX_INSPECTION_SELECTION_DEPTH: usize = 16;
const MAX_INSPECTION_TREE_DEPTH: usize = 12;
const MAX_INSPECTION_TREE_NODES: usize = 128;

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
    definitions: Vec<DefinitionInfo>,
    shapes: ShapeAnalysis,
    consistency: ConsistencyAnalysis,
    hygiene: HygieneAnalysis,
    formulas: FormulaAnalysis,
    domains: DomainAnalysis,
    scopes: ScopeGraph,
    component_id: String,
}

impl AnalyzedDocument {
    fn analyze(mut document: ProjectDocument) -> Self {
        if document.math_regions.is_empty() && document.language != DocumentLanguage::Bibtex {
            document.math_regions = math_regions(&document.content, document.language);
        }
        let parsed = parse_regions(&document.content, &document.math_regions);
        let scopes = ScopeGraph::new(&document);
        let prose = analyze_prose(&document, &parsed);
        let shapes = analyze_shapes(&document, &parsed, &prose.shapes);
        let consistency = analyze_consistency(&document, &prose.definitions, &shapes);
        let hygiene = analyze_hygiene(&document, &parsed, &prose.definitions);
        let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
        let domains = analyze_domains(&document, &formulas);
        Self {
            component_id: document.file_id.clone(),
            document,
            parsed,
            definitions: prose.definitions,
            shapes,
            consistency,
            hygiene,
            formulas,
            domains,
            scopes,
        }
    }
}

#[derive(Default)]
pub struct SemathEngine {
    epoch: String,
    inventory_version: u64,
    project_id: String,
    main_file_id: Option<String>,
    analysis_generation: u64,
    documents: HashMap<String, AnalyzedDocument>,
}

impl SemathEngine {
    pub fn reset(&mut self, snapshot: ProjectSnapshot) -> Result<UpdateResult, EngineError> {
        check_protocol(snapshot.protocol_version)?;
        let changed_file_ids = snapshot
            .documents
            .iter()
            .map(|doc| doc.file_id.clone())
            .collect();
        self.epoch = snapshot.epoch;
        self.inventory_version = snapshot.inventory_version;
        self.project_id = snapshot.project_id;
        self.main_file_id = snapshot.main_file_id;
        self.analysis_generation = 0;
        self.documents = snapshot
            .documents
            .into_iter()
            .map(|document| {
                (
                    document.file_id.clone(),
                    AnalyzedDocument::analyze(document),
                )
            })
            .collect();
        self.refresh_semantic_identities();
        Ok(self.update_result(changed_file_ids))
    }

    pub fn apply(&mut self, envelope: ChangeEnvelope) -> Result<UpdateResult, EngineError> {
        check_protocol(envelope.protocol_version)?;
        if envelope.epoch != self.epoch {
            return Err(EngineError::EpochMismatch);
        }
        if envelope.inventory_version <= self.inventory_version {
            return Err(EngineError::StaleInventory);
        }
        let mut changed = Vec::new();
        for change in envelope.changes {
            match change {
                ProjectChange::Upsert { document } => {
                    let file_id = document.file_id.clone();
                    let accept = self.documents.get(&file_id).is_none_or(|current| {
                        document.document_version > current.document.document_version
                    });
                    if accept {
                        self.documents
                            .insert(file_id.clone(), AnalyzedDocument::analyze(document));
                        changed.push(file_id);
                    }
                }
                ProjectChange::PathChange { file_id, path } => {
                    let document = self
                        .documents
                        .get_mut(&file_id)
                        .ok_or_else(|| EngineError::MissingDocument(file_id.clone()))?;
                    document.document.path = path;
                    changed.push(file_id);
                }
                ProjectChange::Remove { file_id } => {
                    self.documents.remove(&file_id);
                    changed.push(file_id);
                }
            }
        }
        self.inventory_version = envelope.inventory_version;
        self.analysis_generation = envelope.analysis_generation;
        self.refresh_semantic_identities();
        Ok(self.update_result(changed))
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
            | Query::EquationTree { file_id, offset }
            | Query::Hover { file_id, offset }
            | Query::SymbolInfo { file_id, offset }
            | Query::Definition { file_id, offset }
            | Query::References { file_id, offset }
            | Query::PrepareRename { file_id, offset }
            | Query::Rename {
                file_id, offset, ..
            }
            | Query::ExplainDiagnostic {
                file_id, offset, ..
            }
            | Query::FormulaRecognition { file_id, offset }
            | Query::FormulaCompletion { file_id, offset }
            | Query::FormulaRewrite { file_id, offset }
            | Query::DomainEvidence { file_id, offset }
            | Query::Inspection { file_id, offset } => (file_id, Some(*offset)),
            Query::Diagnostics { file_id } => (file_id, None),
        };
        let document = self
            .documents
            .get(file_id)
            .ok_or_else(|| EngineError::MissingDocument(file_id.clone()))?;
        if document.document.document_version != envelope.document_version {
            return Err(EngineError::DocumentVersionMismatch);
        }
        let offset = query_offset.unwrap_or(0);
        let parsed =
            query_offset.and_then(|offset| parsed_math_at_cursor(&document.parsed, offset));
        let symbol = parsed.and_then(|math| symbol_at_cursor(math, offset));
        let cursor_offset = symbol
            .as_ref()
            .map_or(offset, |(_, range)| offset.min(range.end_offset - 1));

        let hygiene_enabled = self.documents.len() == 1;
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
            Query::EquationTree { .. } => QueryValue::EquationTree {
                tree: parsed.map(|math| math.root.clone()),
            },
            Query::Hover { .. } => {
                let roles = symbol
                    .as_ref()
                    .map(|(name, _)| document.consistency.roles_at(name, cursor_offset).0)
                    .unwrap_or_default();
                let definitions = symbol
                    .as_ref()
                    .map(|(name, occurrence)| self.visible_definitions(file_id, occurrence, name))
                    .unwrap_or_default();
                QueryValue::Hover {
                    shape: symbol
                        .as_ref()
                        .and_then(|(name, _)| document.shapes.shape_at(name, cursor_offset)),
                    symbol: symbol.map(|(name, _)| name),
                    equation_kind: parsed
                        .and_then(|math| deepest_node(&math.root, cursor_offset))
                        .map(|node| node.kind.clone()),
                    definitions,
                    formulas: document.formulas.at(cursor_offset),
                    roles,
                }
            }
            Query::SymbolInfo { .. } => QueryValue::SymbolInfo {
                info: symbol.as_ref().map(|(name, occurrence)| {
                    self.symbol_info(document, name, occurrence, cursor_offset, hygiene_enabled)
                }),
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
                diagnostics: document_diagnostics(document, hygiene_enabled),
            },
            Query::ExplainDiagnostic { code, .. } => QueryValue::DiagnosticExplanation {
                diagnostic: document
                    .shapes
                    .diagnostic(&code, cursor_offset)
                    .or_else(|| document.consistency.diagnostic(&code, cursor_offset))
                    .or_else(|| {
                        hygiene_enabled
                            .then(|| document.hygiene.diagnostic(&code, cursor_offset))
                            .flatten()
                    }),
            },
            Query::FormulaRecognition { .. } => QueryValue::FormulaRecognitions {
                recognitions: document.formulas.at(cursor_offset),
            },
            Query::FormulaCompletion { .. } => QueryValue::FormulaCompletions {
                completions: formula_completions(
                    &document.document,
                    &document.parsed,
                    &document.shapes,
                    &document.consistency,
                    offset,
                ),
            },
            Query::FormulaRewrite { .. } => QueryValue::FormulaRewrites {
                rewrites: formula_rewrites(&document.document, &document.formulas, cursor_offset),
            },
            Query::DomainEvidence { .. } => {
                let (activations, truncated) = document.domains.at(cursor_offset);
                QueryValue::DomainActivations {
                    activations,
                    truncated,
                }
            }
            Query::Inspection { .. } => {
                let mut tree_budget = MAX_INSPECTION_TREE_NODES;
                let mut tree_truncated = false;
                let equation = parsed.and_then(|math| {
                    bounded_equation_tree(&math.root, 0, &mut tree_budget, &mut tree_truncated)
                });
                let mut selection_path = Vec::new();
                if let Some(math) = parsed {
                    equation_selection_path(&math.root, cursor_offset, &mut selection_path);
                }
                let selection_truncated = selection_path.len() > MAX_INSPECTION_SELECTION_DEPTH;
                selection_path.truncate(MAX_INSPECTION_SELECTION_DEPTH);

                let symbol_info = symbol.as_ref().map(|(name, occurrence)| {
                    self.symbol_info(document, name, occurrence, cursor_offset, hygiene_enabled)
                });
                let mut references = symbol
                    .as_ref()
                    .and_then(|(name, occurrence)| {
                        self.resolve_definition(file_id, occurrence, name)
                    })
                    .map(|definition| self.references_for(&definition))
                    .unwrap_or_default();
                let references_truncated = references.len() > MAX_INSPECTION_REFERENCES;
                references.truncate(MAX_INSPECTION_REFERENCES);

                let mut diagnostics = document_diagnostics(document, hygiene_enabled)
                    .into_iter()
                    .filter(|diagnostic| {
                        parsed.is_some_and(|math| {
                            ranges_overlap(&diagnostic.range, &math.region.content_range)
                        }) || diagnostic.range.contains(cursor_offset)
                    })
                    .collect::<Vec<_>>();
                let diagnostics_truncated = diagnostics.len() > MAX_INSPECTION_DIAGNOSTICS;
                diagnostics.truncate(MAX_INSPECTION_DIAGNOSTICS);

                let (domains, domains_truncated) = document.domains.at(cursor_offset);
                let recognitions = document.formulas.at(cursor_offset);
                let completions = formula_completions(
                    &document.document,
                    &document.parsed,
                    &document.shapes,
                    &document.consistency,
                    offset,
                );
                let rewrites =
                    formula_rewrites(&document.document, &document.formulas, cursor_offset);
                let rename = prepare_rename_info(parsed, cursor_offset);
                let truncated = tree_truncated
                    || selection_truncated
                    || references_truncated
                    || diagnostics_truncated
                    || domains_truncated
                    || symbol_info.as_ref().is_some_and(|info| info.truncated);

                QueryValue::Inspection {
                    inspection: Box::new(InspectionInfo {
                        equation,
                        selection_path,
                        symbol: symbol_info,
                        references,
                        diagnostics,
                        recognitions,
                        domains,
                        completions,
                        rewrites,
                        rename,
                        truncated,
                    }),
                }
            }
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

    fn update_result(&self, changed_file_ids: Vec<String>) -> UpdateResult {
        UpdateResult {
            protocol_version: PROTOCOL_VERSION,
            epoch: self.epoch.clone(),
            inventory_version: self.inventory_version,
            analysis_generation: self.analysis_generation,
            changed_file_ids,
        }
    }

    fn definitions_for(&self, symbol: &str) -> Vec<DefinitionInfo> {
        let mut definitions: Vec<_> = self
            .documents
            .values()
            .flat_map(|document| document.definitions.iter())
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
        let Some(document) = self.documents.get(file_id) else {
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
        for document in self.documents.values() {
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

    fn symbol_info(
        &self,
        document: &AnalyzedDocument,
        name: &str,
        occurrence: &SourceRange,
        offset: u32,
        hygiene_enabled: bool,
    ) -> SymbolInfo {
        let mut definitions =
            self.visible_definitions(&document.document.file_id, occurrence, name);
        let definitions_truncated = definitions.len() > MAX_SYMBOL_DEFINITIONS;
        definitions.truncate(MAX_SYMBOL_DEFINITIONS);
        let (shapes, shapes_truncated) = document.shapes.claims_at(name, offset);
        let (roles, roles_truncated) = document.consistency.roles_at(name, offset);
        let (diagnostics, diagnostics_truncated) =
            symbol_diagnostics(document, name, offset, &shapes, hygiene_enabled);
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
            roles,
            formulas: document.formulas.at(offset),
            diagnostics,
            truncated: definitions_truncated
                || shapes_truncated
                || roles_truncated
                || diagnostics_truncated,
        }
    }

    fn refresh_semantic_identities(&mut self) {
        let path_to_id = self
            .documents
            .values()
            .map(|document| {
                (
                    normalize_project_path(&document.document.path),
                    document.document.file_id.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut adjacency = self
            .documents
            .keys()
            .map(|file_id| (file_id.clone(), HashSet::new()))
            .collect::<HashMap<_, _>>();
        for document in self.documents.values() {
            for include in &document.document.includes {
                let Some(target) =
                    resolve_include(&document.document.path, &include.path, &path_to_id)
                else {
                    continue;
                };
                adjacency
                    .entry(document.document.file_id.clone())
                    .or_default()
                    .insert(target.clone());
                adjacency
                    .entry(target)
                    .or_default()
                    .insert(document.document.file_id.clone());
            }
        }

        let mut component_by_file = HashMap::new();
        let mut remaining = self.documents.keys().cloned().collect::<HashSet<_>>();
        while let Some(seed) = remaining.iter().next().cloned() {
            let mut pending = vec![seed.clone()];
            let mut members = Vec::new();
            remaining.remove(&seed);
            while let Some(file_id) = pending.pop() {
                members.push(file_id.clone());
                for neighbor in adjacency.get(&file_id).into_iter().flatten() {
                    if remaining.remove(neighbor) {
                        pending.push(neighbor.clone());
                    }
                }
            }
            members.sort();
            let component_id = self
                .main_file_id
                .as_ref()
                .filter(|main| members.contains(main))
                .cloned()
                .unwrap_or_else(|| members[0].clone());
            for file_id in members {
                component_by_file.insert(file_id, component_id.clone());
            }
        }

        for (file_id, document) in &mut self.documents {
            document.component_id = component_by_file
                .get(file_id)
                .cloned()
                .unwrap_or_else(|| file_id.clone());
            for definition in &mut document.definitions {
                if let Some(identity) = &mut definition.semantic_id {
                    identity.component_id.clone_from(&document.component_id);
                }
            }
        }
    }
}

fn scope_visible(definition: &[u32], occurrence: &[u32]) -> bool {
    definition.len() <= occurrence.len()
        && definition
            .iter()
            .zip(occurrence)
            .all(|(left, right)| left == right)
}

fn resolve_include(
    source_path: &str,
    include_path: &str,
    path_to_id: &HashMap<String, String>,
) -> Option<String> {
    let parent = source_path
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let joined = if include_path.starts_with('/') || parent.is_empty() {
        include_path.trim_start_matches('/').to_string()
    } else {
        format!("{parent}/{include_path}")
    };
    let normalized = normalize_project_path(&joined);
    path_to_id.get(&normalized).cloned().or_else(|| {
        (!normalized.contains('.'))
            .then(|| path_to_id.get(&format!("{normalized}.tex")).cloned())
            .flatten()
    })
}

fn normalize_project_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn parsed_math_at_cursor(parsed: &[ParsedMath], offset: u32) -> Option<&ParsedMath> {
    parsed
        .iter()
        .find(|math| math.region.full_range.contains(offset))
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
    symbols
        .iter()
        .find(|(_, range)| range.contains(offset))
        .or_else(|| {
            symbols
                .iter()
                .filter(|(_, range)| {
                    range.start_offset < range.end_offset && range.end_offset == offset
                })
                .max_by_key(|(_, range)| range.start_offset)
        })
        .map(|(symbol, range)| (symbol, range))
}

fn bounded_equation_tree(
    node: &EquationNode,
    depth: usize,
    budget: &mut usize,
    truncated: &mut bool,
) -> Option<EquationNode> {
    if *budget == 0 {
        *truncated = true;
        return None;
    }
    *budget -= 1;
    let mut children = Vec::new();
    if depth >= MAX_INSPECTION_TREE_DEPTH {
        *truncated |= !node.children.is_empty();
    } else {
        for child in &node.children {
            let Some(child) = bounded_equation_tree(child, depth + 1, budget, truncated) else {
                break;
            };
            children.push(child);
        }
    }
    Some(EquationNode {
        kind: node.kind.clone(),
        label: node.label.clone(),
        range: node.range.clone(),
        children,
    })
}

fn equation_selection_path(
    node: &EquationNode,
    offset: u32,
    output: &mut Vec<EquationNodeSummary>,
) {
    if !node.range.contains(offset) {
        return;
    }
    output.push(EquationNodeSummary {
        kind: node.kind.clone(),
        label: node.label.clone(),
        range: node.range.clone(),
    });
    if let Some(child) = node
        .children
        .iter()
        .find(|child| child.range.contains(offset))
    {
        equation_selection_path(child, offset, output);
    }
}

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

fn document_diagnostics(
    document: &AnalyzedDocument,
    hygiene_enabled: bool,
) -> Vec<SemanticDiagnostic> {
    let mut diagnostics = document.shapes.diagnostics.clone();
    diagnostics.extend(document.consistency.diagnostics.iter().cloned());
    if hygiene_enabled {
        diagnostics.extend(document.hygiene.diagnostics.iter().cloned());
    }
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start_offset);
    diagnostics
}

fn symbol_diagnostics(
    document: &AnalyzedDocument,
    symbol: &str,
    offset: u32,
    shapes: &[ShapeInfo],
    hygiene_enabled: bool,
) -> (Vec<SemanticDiagnostic>, bool) {
    let (mut diagnostics, shape_truncated) = document.shapes.diagnostics_for(offset, shapes);
    let (role_diagnostics, role_truncated) = document.consistency.diagnostics_for(symbol, offset);
    diagnostics.extend(role_diagnostics);
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
mod tests {
    use super::{SemathEngine, symbol_range_at_cursor};
    use crate::{
        DocumentLanguage, PROTOCOL_VERSION, ProjectDocument, ProjectInclude, ProjectSnapshot,
        Query, QueryEnvelope, QueryValue, SourceRange,
    };

    fn snapshot(content: &str) -> ProjectSnapshot {
        ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![ProjectDocument {
                file_id: "main".into(),
                path: "main.tex".into(),
                language: DocumentLanguage::Latex,
                content: content.into(),
                document_version: 1,
                math_regions: Vec::new(),
                includes: Vec::new(),
            }],
        }
    }

    fn query(kind: Query) -> QueryEnvelope {
        QueryEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            document_version: 1,
            analysis_generation: 1,
            query: kind,
        }
    }

    #[test]
    fn links_explicit_definition_to_later_reference() {
        let content = "Let $x$ denote the input vector.\nThen $y = x$.";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let second_x = content.encode_utf16().collect::<Vec<_>>().len() as u32 - 3;
        let result = engine
            .query(query(Query::Definition {
                file_id: "main".into(),
                offset: second_x,
            }))
            .unwrap();
        let QueryValue::Locations { locations } = result.value else {
            panic!("expected locations")
        };
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].path, "main.tex");
    }

    #[test]
    fn resolves_a_definition_from_the_end_boundary_of_a_symbol() {
        let content = concat!(
            "Let $A$ denote an event of positive probability.\n",
            "Let $B$ denote an event of positive probability.\n",
            "$p = \\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}$.",
        );
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let after_numerator_a = content.rfind("A \\cap").unwrap() as u32 + 1;

        let result = engine
            .query(query(Query::Definition {
                file_id: "main".into(),
                offset: after_numerator_a,
            }))
            .unwrap();

        let QueryValue::Locations { locations } = result.value else {
            panic!("expected locations")
        };
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start_offset, 5);
    }

    #[test]
    fn keeps_cursor_queries_stable_at_a_symbol_end_boundary() {
        let content = concat!(
            "Let $A$ denote an event of positive probability.\n",
            "Let $B$ denote an event of positive probability.\n",
            "$p = \\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}$.",
        );
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let numerator_a = content.rfind("A \\cap").unwrap() as u32;
        let queries_at = |offset| {
            vec![
                Query::Selection {
                    file_id: "main".into(),
                    offset,
                },
                Query::EquationTree {
                    file_id: "main".into(),
                    offset,
                },
                Query::Hover {
                    file_id: "main".into(),
                    offset,
                },
                Query::SymbolInfo {
                    file_id: "main".into(),
                    offset,
                },
                Query::Definition {
                    file_id: "main".into(),
                    offset,
                },
                Query::References {
                    file_id: "main".into(),
                    offset,
                },
                Query::PrepareRename {
                    file_id: "main".into(),
                    offset,
                },
                Query::FormulaRecognition {
                    file_id: "main".into(),
                    offset,
                },
                Query::FormulaRewrite {
                    file_id: "main".into(),
                    offset,
                },
                Query::DomainEvidence {
                    file_id: "main".into(),
                    offset,
                },
                Query::Inspection {
                    file_id: "main".into(),
                    offset,
                },
            ]
            .into_iter()
            .map(|kind| engine.query(query(kind)).unwrap().value)
            .collect::<Vec<_>>()
        };

        assert_eq!(queries_at(numerator_a), queries_at(numerator_a + 1));
    }

    #[test]
    fn resolves_a_symbol_at_the_end_of_an_unfinished_math_region() {
        let content = "Let $A$ denote an event.\nUse $A";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let result = engine
            .query(query(Query::Definition {
                file_id: "main".into(),
                offset: content.len() as u32,
            }))
            .unwrap();

        let QueryValue::Locations { locations } = result.value else {
            panic!("expected locations")
        };
        assert_eq!(locations.len(), 1);
    }

    #[test]
    fn an_exact_symbol_start_wins_over_the_previous_symbol_end() {
        let symbols = vec![
            (
                "A".into(),
                SourceRange {
                    start_offset: 2,
                    end_offset: 3,
                },
            ),
            (
                "B".into(),
                SourceRange {
                    start_offset: 3,
                    end_offset: 4,
                },
            ),
        ];

        let (symbol, range) = symbol_range_at_cursor(&symbols, 3).unwrap();

        assert_eq!(symbol, "B");
        assert_eq!(range.start_offset, 3);
    }

    #[test]
    fn keeps_bound_variable_rename_available_after_the_symbol() {
        let content = "$\\sum_{i=1}^n i$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let use_i = content.rfind(" i$").unwrap() as u32 + 1;

        let prepare_at = |offset| {
            engine
                .query(query(Query::PrepareRename {
                    file_id: "main".into(),
                    offset,
                }))
                .unwrap()
                .value
        };
        let rename_at = |offset| {
            engine
                .query(query(Query::Rename {
                    file_id: "main".into(),
                    new_name: "j".into(),
                    offset,
                }))
                .unwrap()
                .value
        };

        let preparation = prepare_at(use_i);
        assert_eq!(preparation, prepare_at(use_i + 1));
        let QueryValue::RenamePreparation {
            placeholder: Some(placeholder),
            rejection: None,
            ..
        } = preparation
        else {
            panic!("expected a renameable bound variable")
        };
        assert_eq!(placeholder, "i");
        assert_eq!(rename_at(use_i), rename_at(use_i + 1));
    }

    #[test]
    fn separates_same_glyph_definitions_by_scope_and_include_component() {
        let chapter = concat!(
            "\\section{First}\nLet $x$ denote the first value.\nUse $x$.\n",
            "\\section{Second}\nLet $x$ denote the second value.\nUse $x$.",
        );
        let mut project = snapshot("\\input{chapter}");
        project.documents[0].includes = vec![ProjectInclude {
            path: "chapter".into(),
            source_range: SourceRange {
                start_offset: 0,
                end_offset: 15,
            },
        }];
        project.documents.push(ProjectDocument {
            file_id: "chapter".into(),
            path: "chapter.tex".into(),
            language: DocumentLanguage::Latex,
            content: chapter.into(),
            document_version: 1,
            math_regions: Vec::new(),
            includes: Vec::new(),
        });
        project.documents.push(ProjectDocument {
            file_id: "orphan".into(),
            path: "orphan.tex".into(),
            language: DocumentLanguage::Latex,
            content: "Let $x$ denote an unrelated value.".into(),
            document_version: 1,
            math_regions: Vec::new(),
            includes: Vec::new(),
        });
        let mut engine = SemathEngine::default();
        engine.reset(project).unwrap();

        let first_use = chapter.find("Use $x$").unwrap() as u32 + 5;
        let second_use = chapter.rfind("Use $x$").unwrap() as u32 + 5;
        let definition_at = |offset| {
            let result = engine
                .query(query(Query::SymbolInfo {
                    file_id: "chapter".into(),
                    offset,
                }))
                .unwrap();
            let QueryValue::SymbolInfo { info: Some(info) } = result.value else {
                panic!("expected symbol info")
            };
            info
        };
        let first = definition_at(first_use);
        let second = definition_at(second_use);
        assert_eq!(first.definitions.len(), 1);
        assert_eq!(first.definitions[0].description, "the first value");
        assert_eq!(second.definitions.len(), 1);
        assert_eq!(second.definitions[0].description, "the second value");
        assert_ne!(first.semantic_id, second.semantic_id);
        assert_eq!(first.semantic_id.as_ref().unwrap().component_id, "main");
    }

    #[test]
    fn accepts_the_camel_case_json_protocol() {
        let mut engine = SemathEngine::default();
        let reset = serde_json::json!({
            "protocolVersion": 1,
            "epoch": "project:1",
            "inventoryVersion": 1,
            "projectId": "project",
            "documents": [{
                "fileId": "main",
                "path": "main.tex",
                "language": "latex",
                "content": "$x$",
                "documentVersion": 1
            }]
        });
        engine
            .reset_json(&serde_json::to_vec(&reset).unwrap())
            .unwrap();
        let query = serde_json::json!({
            "protocolVersion": 1,
            "epoch": "project:1",
            "inventoryVersion": 1,
            "documentVersion": 1,
            "analysisGeneration": 1,
            "query": { "kind": "hover", "fileId": "main", "offset": 1 }
        });
        let result = engine
            .query_json(&serde_json::to_vec(&query).unwrap())
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(result["value"]["symbol"], "x");
    }

    #[test]
    fn inspects_a_formula_from_one_coherent_snapshot() {
        let content = "Let $A$ denote an event of positive probability.\nLet $B$ denote an event of positive probability.\n$p = \\mathbb{P}(A \\mid B)$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let offset = content.rfind("A \\mid").unwrap() as u32;
        let result = engine
            .query(query(Query::Inspection {
                file_id: "main".into(),
                offset,
            }))
            .unwrap();
        let QueryValue::Inspection { inspection } = result.value else {
            panic!("expected inspection")
        };

        assert!(inspection.equation.is_some());
        assert!(inspection.selection_path.len() >= 2);
        assert_eq!(
            inspection.symbol.as_ref().map(|info| info.symbol.as_str()),
            Some("A")
        );
        assert!(!inspection.references.is_empty());
        assert!(
            inspection
                .domains
                .iter()
                .any(|domain| domain.pack_id == "probability")
        );
        assert_eq!(
            inspection.recognitions[0].pattern_id,
            "conditional-probability"
        );
        assert_eq!(inspection.rewrites.len(), 2);
        assert!(inspection.rename.range.is_none());
        assert!(!inspection.truncated);
    }

    #[test]
    fn bounds_large_inspection_trees_and_exposes_rename_availability() {
        let large_formula = format!("$x={}$", "a+".repeat(140));
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(&large_formula)).unwrap();
        let result = engine
            .query(query(Query::Inspection {
                file_id: "main".into(),
                offset: 1,
            }))
            .unwrap();
        let QueryValue::Inspection { inspection } = result.value else {
            panic!("expected inspection")
        };
        assert!(inspection.truncated);

        let content = "$\\sum_{i=1}^{n} x_i$";
        engine.reset(snapshot(content)).unwrap();
        let offset = content.find('i').unwrap() as u32;
        let result = engine
            .query(query(Query::Inspection {
                file_id: "main".into(),
                offset,
            }))
            .unwrap();
        let QueryValue::Inspection { inspection } = result.value else {
            panic!("expected inspection")
        };
        assert_eq!(inspection.rename.placeholder.as_deref(), Some("i"));
        assert!(inspection.rename.range.is_some());
        assert!(inspection.rename.rejection.is_none());
    }

    #[test]
    fn extracts_notation_table_definitions() {
        let content = "| Symbol | Meaning |\n| --- | --- |\n| $W$ | weight matrix |\n\nUse $W$.";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let offset = content.rfind('W').unwrap() as u32;
        let result = engine
            .query(query(Query::Hover {
                file_id: "main".into(),
                offset,
            }))
            .unwrap();
        let QueryValue::Hover { definitions, .. } = result.value else {
            panic!("expected hover")
        };
        assert_eq!(definitions[0].description, "weight matrix");
    }

    #[test]
    fn explains_a_symbol_with_bounded_evidence_backed_claims() {
        let content = "Let $A$ denote the transformation.\n$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}, y \\in \\mathbb{R}^{m}$\n$y = Ax$\n$A \\in \\mathbb{R}^{k}$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let formula_result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: content.find("Ax").unwrap() as u32,
            }))
            .unwrap();
        let QueryValue::SymbolInfo {
            info: Some(formula_info),
        } = formula_result.value
        else {
            panic!("expected symbol info")
        };
        assert_eq!(formula_info.symbol, "A");
        assert_eq!(formula_info.definitions.len(), 1);
        assert_eq!(formula_info.shapes[0].display, "Matrix[m × n]");
        assert_eq!(formula_info.formulas[0].pattern_id, "matrix-vector-product");

        let redeclaration = content.rfind("$A").unwrap() as u32 + 1;
        let conflict_result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: redeclaration,
            }))
            .unwrap();
        let QueryValue::SymbolInfo {
            info: Some(conflict_info),
        } = conflict_result.value
        else {
            panic!("expected symbol info")
        };
        assert_eq!(conflict_info.shapes.len(), 2);
        assert_eq!(conflict_info.shapes[0].display, "Vector[k]");
        assert_eq!(conflict_info.shapes[1].display, "Matrix[m × n]");
        assert_eq!(conflict_info.diagnostics[0].code, "notation-shape-conflict");
        assert!(!conflict_info.truncated);
    }

    #[test]
    fn uses_scoped_prose_shapes_for_formulas_and_conflicts() {
        let content = "Let $x$ and $A$ denote an n-dimensional normalized vector and an m by n symmetric matrix, respectively.\n$y \\in \\mathbb{R}^{m}$\n$y = Ax$\n$A \\in \\mathbb{R}^{n}$\n$A$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let formula_result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: content.find("Ax").unwrap() as u32,
            }))
            .unwrap();
        let QueryValue::SymbolInfo {
            info: Some(formula_info),
        } = formula_result.value
        else {
            panic!("expected symbol info")
        };
        assert_eq!(formula_info.shapes[0].display, "Matrix[m × n]");
        assert_eq!(formula_info.shapes[0].refinements, ["symmetric"]);
        assert_eq!(
            formula_info.shapes[0].evidence.rule_id,
            "english-respectively-definition"
        );
        assert_eq!(formula_info.formulas[0].pattern_id, "matrix-vector-product");
        assert_eq!(
            formula_info.formulas[0].bindings[0].constraint.refinements,
            ["symmetric"]
        );

        let final_a = content.rfind("$A$").unwrap() as u32 + 1;
        let conflict_result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: final_a,
            }))
            .unwrap();
        let QueryValue::SymbolInfo {
            info: Some(conflict_info),
        } = conflict_result.value
        else {
            panic!("expected symbol info")
        };
        assert_eq!(conflict_info.shapes.len(), 2);
        assert_eq!(conflict_info.shapes[0].display, "Vector[n]");
        assert_eq!(conflict_info.shapes[1].display, "Matrix[m × n]");
        assert_eq!(conflict_info.diagnostics[0].code, "notation-shape-conflict");
        assert_eq!(
            conflict_info.diagnostics[0].evidence[0].kind,
            "explicit-prose"
        );
        assert_eq!(
            conflict_info.diagnostics[0].evidence[1].kind,
            "explicit-math"
        );
    }

    #[test]
    fn exposes_domain_priors_without_creating_definitions_or_warnings() {
        let content =
            "\\section{Mixed model}\nA probability distribution and matrix terminology.\n$p$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let offset = content.rfind('p').unwrap() as u32;

        let hover = engine
            .query(query(Query::Hover {
                file_id: "main".into(),
                offset,
            }))
            .unwrap();
        let QueryValue::Hover { definitions, .. } = hover.value else {
            panic!("expected hover")
        };
        assert!(definitions.is_empty());

        let domains = engine
            .query(query(Query::DomainEvidence {
                file_id: "main".into(),
                offset,
            }))
            .unwrap();
        let QueryValue::DomainActivations {
            activations,
            truncated,
        } = domains.value
        else {
            panic!("expected domain activations")
        };
        assert!(!truncated);
        assert_eq!(
            activations
                .iter()
                .map(|domain| domain.pack_id.as_str())
                .collect::<Vec<_>>(),
            ["linear-algebra", "probability"]
        );
        assert!(activations.iter().all(|domain| domain.strength == "weak"));

        let diagnostics = engine
            .query(query(Query::Diagnostics {
                file_id: "main".into(),
            }))
            .unwrap();
        let QueryValue::Diagnostics { diagnostics } = diagnostics.value else {
            panic!("expected diagnostics")
        };
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn explains_scoped_role_and_type_conflicts() {
        let content = "Let $p$ denote a probability distribution.\n$p$ is a random variable.\n$p$\nLet $S$ denote a set.\n$S \\in \\mathbb{R}^{n}$\n$S$";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let final_p = content.find("\n$p$\n").unwrap() as u32 + 2;
        let result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: final_p,
            }))
            .unwrap();
        let QueryValue::SymbolInfo { info: Some(info) } = result.value else {
            panic!("expected symbol info")
        };
        assert_eq!(info.roles.len(), 2);
        assert_eq!(info.roles[0].role, "random-variable");
        assert_eq!(info.roles[1].role, "distribution");
        assert_eq!(info.diagnostics[0].code, "notation-role-conflict");
        assert_eq!(info.diagnostics[0].evidence.len(), 2);

        let final_s = content.rfind("$S$").unwrap() as u32 + 1;
        let result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: final_s,
            }))
            .unwrap();
        let QueryValue::SymbolInfo { info: Some(info) } = result.value else {
            panic!("expected symbol info")
        };
        assert_eq!(info.roles[0].role, "set");
        assert_eq!(info.shapes[0].display, "Vector[n]");
        assert_eq!(info.diagnostics[0].code, "notation-role-type-conflict");
        assert_eq!(info.diagnostics[0].evidence.len(), 2);

        let result = engine
            .query(query(Query::ExplainDiagnostic {
                file_id: "main".into(),
                code: "notation-role-type-conflict".into(),
                offset: content.find("S \\in").unwrap() as u32,
            }))
            .unwrap();
        let QueryValue::DiagnosticExplanation {
            diagnostic: Some(diagnostic),
        } = result.value
        else {
            panic!("expected diagnostic explanation")
        };
        assert_eq!(diagnostic.evidence.len(), 2);
    }

    #[test]
    fn exposes_definition_hygiene_only_as_targeted_hints() {
        let content = "$x+1$ appears first.\nLet $x$ denote a scalar.\nLater $x$ is used.";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let diagnostics = engine
            .query(query(Query::Diagnostics {
                file_id: "main".into(),
            }))
            .unwrap();
        let QueryValue::Diagnostics { diagnostics } = diagnostics.value else {
            panic!("expected diagnostics")
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "used-before-explicit-definition");
        assert_eq!(diagnostics[0].severity, "hint");
        assert_eq!(diagnostics[0].evidence.len(), 2);

        let early_x = content.find('x').unwrap() as u32;
        let result = engine
            .query(query(Query::SymbolInfo {
                file_id: "main".into(),
                offset: early_x,
            }))
            .unwrap();
        let QueryValue::SymbolInfo { info: Some(info) } = result.value else {
            panic!("expected symbol info")
        };
        assert_eq!(info.diagnostics[0].code, "used-before-explicit-definition");

        let explanation = engine
            .query(query(Query::ExplainDiagnostic {
                file_id: "main".into(),
                code: "used-before-explicit-definition".into(),
                offset: early_x,
            }))
            .unwrap();
        let QueryValue::DiagnosticExplanation {
            diagnostic: Some(diagnostic),
        } = explanation.value
        else {
            panic!("expected diagnostic explanation")
        };
        assert_eq!(diagnostic.severity, "hint");
    }

    #[test]
    fn disables_hygiene_hints_when_project_scope_is_uncertain() {
        let mut project = snapshot("Let $z$ denote a scalar.");
        project.documents.push(ProjectDocument {
            file_id: "appendix".into(),
            path: "appendix.tex".into(),
            language: DocumentLanguage::Latex,
            content: "$z$ may be used here.".into(),
            document_version: 1,
            math_regions: Vec::new(),
            includes: Vec::new(),
        });
        let mut engine = SemathEngine::default();
        engine.reset(project).unwrap();

        let result = engine
            .query(query(Query::Diagnostics {
                file_id: "main".into(),
            }))
            .unwrap();
        let QueryValue::Diagnostics { diagnostics } = result.value else {
            panic!("expected diagnostics")
        };
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn proposes_a_capture_avoiding_bound_variable_rename() {
        let content = "$i$ is external. $\\sum_{i=1}^n x_i$.";
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let use_offset = content.rfind("x_i").unwrap() as u32 + 2;
        let result = engine
            .query(query(Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "j".into(),
            }))
            .unwrap();
        let QueryValue::EditProposal {
            proposal: Some(proposal),
            rejection: None,
        } = result.value
        else {
            panic!("expected edit proposal")
        };
        assert_eq!(proposal.safety, "deterministic");
        assert_eq!(proposal.files[0].edits.len(), 2);
        assert!(
            proposal.files[0]
                .edits
                .iter()
                .all(|edit| edit.expected_text == "i" && edit.replacement_text == "j")
        );
    }

    #[test]
    fn refuses_an_unfinished_or_capturing_rename() {
        for content in ["$\\sum_{i=1}^n (x_i + j)$", "$\\sum_{i=1}^n x_i"] {
            let mut engine = SemathEngine::default();
            engine.reset(snapshot(content)).unwrap();
            let offset = content.find("i=1").unwrap() as u32;
            let result = engine
                .query(query(Query::Rename {
                    file_id: "main".into(),
                    offset,
                    new_name: "j".into(),
                }))
                .unwrap();
            let QueryValue::EditProposal {
                proposal: None,
                rejection: Some(_),
            } = result.value
            else {
                panic!("expected rename rejection")
            };
        }
    }
}
