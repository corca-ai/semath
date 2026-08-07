use std::collections::HashMap;

use regex::Regex;
use thiserror::Error;

use crate::parser::{ParsedMath, deepest_node, math_regions, parse_regions, selection_path};
use crate::{
    ChangeEnvelope, DefinitionInfo, DocumentLanguage, Evidence, Location, PROTOCOL_VERSION,
    ProjectChange, ProjectDocument, ProjectSnapshot, Query, QueryEnvelope, QueryResult, QueryValue,
    SourceIndex, SourceRange, UpdateResult,
};

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
}

impl AnalyzedDocument {
    fn analyze(mut document: ProjectDocument) -> Self {
        if document.math_regions.is_empty() && document.language != DocumentLanguage::Bibtex {
            document.math_regions = math_regions(&document.content, document.language);
        }
        let parsed = parse_regions(&document.content, &document.math_regions);
        let definitions = extract_definitions(&document, &parsed);
        Self {
            document,
            parsed,
            definitions,
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
        let (file_id, offset) = match &envelope.query {
            Query::Selection { file_id, offset }
            | Query::EquationTree { file_id, offset }
            | Query::Hover { file_id, offset }
            | Query::Definition { file_id, offset }
            | Query::References { file_id, offset } => (file_id, *offset),
        };
        let document = self
            .documents
            .get(file_id)
            .ok_or_else(|| EngineError::MissingDocument(file_id.clone()))?;
        if document.document.document_version != envelope.document_version {
            return Err(EngineError::DocumentVersionMismatch);
        }
        let parsed = document
            .parsed
            .iter()
            .find(|math| math.region.full_range.contains(offset));
        let symbol = parsed.and_then(|math| {
            math.symbols
                .iter()
                .find(|(_, range)| range.contains(offset))
                .map(|(symbol, range)| (symbol.clone(), range.clone()))
        });

        let value = match envelope.query {
            Query::Selection { .. } => {
                let mut ranges = Vec::new();
                if let Some(math) = parsed {
                    selection_path(&math.root, offset, &mut ranges);
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
                let definitions = symbol
                    .as_ref()
                    .map(|(name, _)| self.definitions_for(name))
                    .unwrap_or_default();
                QueryValue::Hover {
                    symbol: symbol.map(|(name, _)| name),
                    equation_kind: parsed
                        .and_then(|math| deepest_node(&math.root, offset))
                        .map(|node| node.kind.clone()),
                    definitions,
                }
            }
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

    fn resolve_definition(
        &self,
        file_id: &str,
        occurrence: &SourceRange,
        symbol: &str,
    ) -> Option<DefinitionInfo> {
        let definitions = self.definitions_for(symbol);
        definitions
            .iter()
            .filter(|definition| {
                definition.location.file_id == file_id
                    && definition.location.range.start_offset <= occurrence.start_offset
            })
            .max_by_key(|definition| definition.location.range.start_offset)
            .cloned()
            .or_else(|| (definitions.len() == 1).then(|| definitions[0].clone()))
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
                        .is_some_and(|resolved| resolved.location == definition.location)
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
}

fn check_protocol(version: u32) -> Result<(), EngineError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(EngineError::UnsupportedProtocol(version))
    }
}

fn extract_definitions(document: &ProjectDocument, parsed: &[ParsedMath]) -> Vec<DefinitionInfo> {
    let prefix = Regex::new(r"(?i)(?:let|where)\s*$").unwrap();
    let suffix =
        Regex::new(r"(?i)^\s*(?:denote(?:s)?|be|is|represent(?:s)?)\s+([^.;\n]+)").unwrap();
    let source_index = SourceIndex::new(&document.content);
    let mut definitions = Vec::new();

    for math in parsed {
        let Some((symbol, symbol_range)) = math.symbols.first() else {
            continue;
        };
        let start_byte = source_index.byte_for_utf16(math.region.full_range.start_offset);
        let end_byte = source_index.byte_for_utf16(math.region.full_range.end_offset);
        let before_start = document.content[..start_byte]
            .char_indices()
            .rev()
            .nth(80)
            .map_or(0, |(offset, _)| offset);
        let before = &document.content[before_start..start_byte];
        let after_end = document.content[end_byte..]
            .char_indices()
            .nth(180)
            .map_or(document.content.len(), |(offset, _)| end_byte + offset);
        let after = &document.content[end_byte..after_end];
        if prefix.is_match(before) {
            if let Some(captures) = suffix.captures(after) {
                definitions.push(definition(
                    document,
                    symbol,
                    symbol_range,
                    captures.get(1).unwrap().as_str().trim(),
                    "english-let-definition",
                ));
                continue;
            }
            if math.symbols.len() > 1 {
                definitions.push(definition(
                    document,
                    symbol,
                    symbol_range,
                    "explicit mathematical declaration",
                    "english-let-math-declaration",
                ));
            }
        }

        let line_start = document.content[..start_byte]
            .rfind('\n')
            .map_or(0, |offset| offset + 1);
        let line_end = document.content[end_byte..]
            .find('\n')
            .map_or(document.content.len(), |offset| end_byte + offset);
        let line = &document.content[line_start..line_end];
        if line.contains('|') {
            let math_end_in_line = end_byte - line_start;
            let tail = &line[math_end_in_line..];
            if let Some(cell_start) = tail.find('|').map(|offset| offset + 1)
                && let Some(cell_end) = tail[cell_start..].find('|')
            {
                let description = tail[cell_start..cell_start + cell_end].trim();
                if !description.is_empty() && !description.chars().all(|ch| ch == '-' || ch == ':')
                {
                    definitions.push(definition(
                        document,
                        symbol,
                        symbol_range,
                        description,
                        "notation-table-definition",
                    ));
                }
            }
        }
    }

    definitions
}

fn definition(
    document: &ProjectDocument,
    symbol: &str,
    range: &SourceRange,
    description: &str,
    rule_id: &str,
) -> DefinitionInfo {
    DefinitionInfo {
        symbol: symbol.to_string(),
        description: description.to_string(),
        location: Location {
            file_id: document.file_id.clone(),
            path: document.path.clone(),
            range: range.clone(),
        },
        evidence: Evidence {
            rule_id: rule_id.to_string(),
            kind: "explicit-prose".into(),
            strength: "strong".into(),
            source_ranges: vec![range.clone()],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::SemathEngine;
    use crate::{
        DocumentLanguage, PROTOCOL_VERSION, ProjectDocument, ProjectSnapshot, Query, QueryEnvelope,
        QueryValue,
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
}
