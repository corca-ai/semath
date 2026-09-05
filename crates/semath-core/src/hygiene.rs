use std::collections::BTreeMap;

use crate::binder::{binder_at, binders};
use crate::parser::ParsedMath;
use crate::scope::ScopeGraph;
use crate::{
    DefinitionInfo, Evidence, ProjectDocument, SemanticDiagnostic, SourceIndex, SourceRange,
};

const MAX_HYGIENE_DIAGNOSTICS: usize = 8;

#[derive(Clone, Debug)]
struct HygieneEntry {
    symbol: String,
    scope_id: usize,
    diagnostic: SemanticDiagnostic,
}

#[derive(Clone, Debug)]
pub(crate) struct HygieneAnalysis {
    entries: Vec<HygieneEntry>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    scopes: ScopeGraph,
}

impl HygieneAnalysis {
    pub fn diagnostics_for(&self, symbol: &str, offset: u32) -> (Vec<SemanticDiagnostic>, bool) {
        let diagnostics = self
            .entries
            .iter()
            .filter(|entry| {
                entry.symbol == symbol
                    && entry.diagnostic.range.contains(offset)
                    && self.scopes.visible(entry.scope_id, offset)
            })
            .collect::<Vec<_>>();
        let truncated = diagnostics.len() > MAX_HYGIENE_DIAGNOSTICS;
        (
            diagnostics
                .into_iter()
                .take(MAX_HYGIENE_DIAGNOSTICS)
                .map(|entry| entry.diagnostic.clone())
                .collect(),
            truncated,
        )
    }

    pub fn diagnostic(&self, code: &str, offset: u32) -> Option<SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code && diagnostic.range.contains(offset))
            .cloned()
    }
}

pub(crate) fn analyze_hygiene(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    definitions: &[DefinitionInfo],
) -> HygieneAnalysis {
    let scopes = ScopeGraph::new(document);
    let mut definitions_by_symbol = BTreeMap::<&str, Vec<&DefinitionInfo>>::new();
    for definition in definitions {
        definitions_by_symbol
            .entry(&definition.symbol)
            .or_default()
            .push(definition);
    }

    let mut entries = Vec::new();
    for (symbol, candidates) in definitions_by_symbol {
        let [definition] = candidates.as_slice() else {
            continue;
        };
        let Some(notation) = source_notation(document, &definition.location.range) else {
            continue;
        };
        if !eligible_definition(document, definition, parsed)
            || has_unclosed_occurrence(notation, parsed)
        {
            continue;
        }
        let scope_id = scopes.id_at(definition.location.range.start_offset);
        let mut occurrences = free_occurrences(notation, parsed)
            .into_iter()
            .filter(|range| {
                *range != definition.location.range && scopes.visible(scope_id, range.start_offset)
            })
            .collect::<Vec<_>>();
        occurrences.sort_by_key(|range| range.start_offset);

        let before = occurrences
            .iter()
            .filter(|range| range.start_offset < definition.location.range.start_offset)
            .cloned()
            .collect::<Vec<_>>();
        if !before.is_empty() {
            entries.push(HygieneEntry {
                symbol: symbol.into(),
                scope_id,
                diagnostic: used_before_definition(symbol, definition, before),
            });
        } else if occurrences.is_empty() {
            entries.push(HygieneEntry {
                symbol: symbol.into(),
                scope_id,
                diagnostic: defined_but_unused(symbol, definition),
            });
        }
    }

    entries.sort_by_key(|entry| entry.diagnostic.range.start_offset);
    let diagnostics = entries
        .iter()
        .map(|entry| entry.diagnostic.clone())
        .collect();
    HygieneAnalysis {
        entries,
        diagnostics,
        scopes,
    }
}

fn eligible_definition(
    document: &ProjectDocument,
    definition: &DefinitionInfo,
    parsed: &[ParsedMath],
) -> bool {
    definition.evidence.kind == "explicit-prose"
        && definition.evidence.strength == "strong"
        && definition.evidence.rule_id != "notation-table-definition"
        && atomic_source_notation(document, &definition.location.range)
        && parsed.iter().any(|math| {
            math.region.closed
                && math
                    .region
                    .content_range
                    .contains(definition.location.range.start_offset)
        })
}

fn source_notation<'a>(document: &'a ProjectDocument, range: &SourceRange) -> Option<&'a str> {
    let index = SourceIndex::new(&document.content);
    let start = index.byte_for_utf16(range.start_offset);
    let end = index.byte_for_utf16(range.end_offset);
    document.content.get(start..end)
}

fn atomic_source_notation(document: &ProjectDocument, range: &SourceRange) -> bool {
    let Some(notation) = source_notation(document, range) else {
        return false;
    };
    if let Some(command) = notation.strip_prefix('\\') {
        !command.is_empty() && command.chars().all(char::is_alphabetic)
    } else {
        !notation.is_empty() && notation.chars().all(char::is_alphanumeric)
    }
}

fn has_unclosed_occurrence(symbol: &str, parsed: &[ParsedMath]) -> bool {
    parsed.iter().any(|math| {
        !math.region.closed
            && math
                .symbols
                .iter()
                .any(|(candidate, _)| candidate == symbol)
    })
}

fn free_occurrences(symbol: &str, parsed: &[ParsedMath]) -> Vec<SourceRange> {
    parsed
        .iter()
        .filter(|math| math.region.closed)
        .flat_map(|math| {
            let found = binders(math);
            math.symbols
                .iter()
                .filter(move |(candidate, range)| {
                    candidate == symbol && binder_at(math, &found, range.start_offset).is_none()
                })
                .map(|(_, range)| range.clone())
        })
        .collect()
}

fn used_before_definition(
    symbol: &str,
    definition: &DefinitionInfo,
    occurrences: Vec<SourceRange>,
) -> SemanticDiagnostic {
    let range = occurrences[0].clone();
    SemanticDiagnostic {
        code: "used-before-explicit-definition".into(),
        severity: "hint".into(),
        message: format!("Notation `{symbol}` is used before its explicit definition."),
        explanation: "A free occurrence appears earlier than the symbol's only strong explicit definition in this scope. This is a review hint, not a correctness warning.".into(),
        range,
        evidence: vec![
            Evidence {
                rule_id: "definition-hygiene/free-occurrence-before-definition".into(),
                kind: "structural-order".into(),
                strength: "calibrated-hint".into(),
                source_ranges: occurrences,
                source_anchors: Vec::new(),
            },
            definition.evidence.clone(),
        ],
    }
}

fn defined_but_unused(symbol: &str, definition: &DefinitionInfo) -> SemanticDiagnostic {
    SemanticDiagnostic {
        code: "defined-but-unused".into(),
        severity: "hint".into(),
        message: format!("Notation `{symbol}` is explicitly defined but not used."),
        explanation: "No other free occurrence resolves inside the definition's analyzable scope. This is a review hint, not a correctness warning.".into(),
        range: definition.location.range.clone(),
        evidence: vec![definition.evidence.clone()],
    }
}

#[cfg(test)]
mod tests {
    use crate::canonical::lower_document_region;
    use serde::Deserialize;

    use super::{analyze_hygiene, atomic_source_notation};
    use crate::parser::{parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::{DocumentLanguage, ProjectDocument};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        false_positive_budget: usize,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        id: String,
        content: String,
        expected: Vec<String>,
    }

    #[test]
    fn hygiene_hints_require_stably_atomic_source_notation() {
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: "x \\rho ECE \\Delta t x_i \\hat{y}".into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: Vec::new(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let ranges = [(0, 1), (2, 6), (7, 10), (11, 19), (20, 23), (24, 31)];
        assert_eq!(
            ranges.map(|(start_offset, end_offset)| atomic_source_notation(
                &document,
                &crate::SourceRange {
                    start_offset,
                    end_offset,
                },
            )),
            [true, true, true, false, false, false]
        );
    }

    #[test]
    fn matches_the_labeled_definition_hygiene_corpus() {
        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../fixtures/definition-hygiene-corpus.json"
        ))
        .unwrap();
        assert_eq!(corpus.false_positive_budget, 0);

        for case in corpus.cases {
            let regions = test_math_regions(&case.content, DocumentLanguage::Markdown);
            let document = ProjectDocument {
                prose_annotations: vec![],
                file_id: "main".into(),
                path: "main.md".into(),
                language: DocumentLanguage::Markdown,
                content: case.content.clone(),
                document_version: 1,
                schema_version: 8,
                nodes: Vec::new(),
                math_roots: Vec::new(),
                visible_prose: Vec::new(),
                scopes: Vec::new(),
                blocks: Vec::new(),
                declarations: Vec::new(),
                math_regions: regions.clone(),
                macros: Vec::new(),
                includes: Vec::new(),
            };
            let parsed = parse_regions(&case.content, &regions);
            let canonical = parsed
                .iter()
                .map(|math| lower_document_region(&document, &math.region.content_range))
                .collect::<Vec<_>>();
            let prose = observe_prose(&document, &parsed, &canonical);
            let analysis = analyze_hygiene(&document, &parsed, &prose.definitions);
            let actual = analysis
                .entries
                .iter()
                .map(|entry| format!("{}:{}", entry.diagnostic.code, entry.symbol))
                .collect::<Vec<_>>();
            assert_eq!(actual, case.expected, "corpus case {}", case.id);
        }
    }
}
