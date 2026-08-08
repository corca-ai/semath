use std::collections::HashSet;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::pattern::FormulaAnalysis;
use crate::{
    Evidence, FormulaRecognition, FormulaRewrite, ProjectDocument, SemanticEditFile,
    SemanticEditProposal, SemanticTextEdit, SourceIndex,
};

const REWRITE_SCHEMA_VERSION: u32 = 1;
const MAX_REWRITES: usize = 4;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRewritePack {
    schema_version: u32,
    pack_id: String,
    pack_version: String,
    rewrites: Vec<RewriteRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewriteRule {
    id: String,
    title: String,
    source_pattern: String,
    required_refinements: Vec<RequiredRefinement>,
    replacement_template: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequiredRefinement {
    parameter: String,
    refinement: String,
}

static PROBABILITY_REWRITES: LazyLock<RawRewritePack> = LazyLock::new(|| {
    let pack: RawRewritePack =
        serde_json::from_str(include_str!("../../../packs/probability/v1.json"))
            .expect("probability rewrite pack must be valid JSON");
    validate_pack(&pack).expect("probability rewrite pack must satisfy the rewrite schema");
    pack
});

pub(crate) fn formula_rewrites(
    document: &ProjectDocument,
    formulas: &FormulaAnalysis,
    offset: u32,
) -> Vec<FormulaRewrite> {
    let Some(recognition) = rewrite_recognition_at(document, formulas, offset) else {
        return Vec::new();
    };
    let index = SourceIndex::new(&document.content);
    let start = index.byte_for_utf16(recognition.range.start_offset);
    let end = index.byte_for_utf16(recognition.range.end_offset);
    let Some(expected_text) = document.content.get(start..end) else {
        return Vec::new();
    };
    if expected_text.is_empty() {
        return Vec::new();
    }

    PROBABILITY_REWRITES
        .rewrites
        .iter()
        .filter(|rule| rule.source_pattern == recognition.pattern_id)
        .filter(|rule| satisfies_side_conditions(rule, &recognition))
        .take(MAX_REWRITES)
        .enumerate()
        .map(|(rank, rule)| rewrite(document, &recognition, expected_text, rule, rank))
        .collect()
}

fn rewrite_recognition_at(
    document: &ProjectDocument,
    formulas: &FormulaAnalysis,
    offset: u32,
) -> Option<FormulaRecognition> {
    if let Some(recognition) = formulas.at(offset).into_iter().next() {
        return Some(recognition);
    }

    let region = document
        .math_regions
        .iter()
        .find(|region| region.closed && region.full_range.contains(offset))
        .or_else(|| {
            document
                .math_regions
                .iter()
                .find(|region| region.closed && region.full_range.end_offset == offset)
        })?;
    let mut candidates = formulas.all().iter().filter(|recognition| {
        region.content_range.start_offset <= recognition.range.start_offset
            && recognition.range.end_offset <= region.content_range.end_offset
    });
    let recognition = candidates.next()?.clone();
    candidates.next().is_none().then_some(recognition)
}

fn satisfies_side_conditions(rule: &RewriteRule, recognition: &FormulaRecognition) -> bool {
    rule.required_refinements.iter().all(|required| {
        recognition.bindings.iter().any(|binding| {
            binding.parameter == required.parameter
                && binding
                    .constraint
                    .refinements
                    .contains(&required.refinement)
        })
    })
}

fn rewrite(
    document: &ProjectDocument,
    recognition: &FormulaRecognition,
    expected_text: &str,
    rule: &RewriteRule,
    rank: usize,
) -> FormulaRewrite {
    let mut replacement_text = rule.replacement_template.clone();
    for binding in &recognition.bindings {
        replacement_text =
            replacement_text.replace(&format!("{{{{{}}}}}", binding.parameter), &binding.symbol);
    }

    let mut evidence = recognition
        .bindings
        .iter()
        .map(|binding| binding.evidence.clone())
        .collect::<Vec<_>>();
    let mut source_ranges = recognition
        .bindings
        .iter()
        .flat_map(|binding| binding.evidence.source_ranges.iter().cloned())
        .chain(std::iter::once(recognition.range.clone()))
        .collect::<Vec<_>>();
    source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    source_ranges.dedup();
    evidence.push(Evidence {
        rule_id: format!("{}/rewrite/{}", PROBABILITY_REWRITES.pack_id, rule.id),
        kind: "derived-constraint".into(),
        strength: "strong".into(),
        source_ranges,
    });

    FormulaRewrite {
        rule_id: rule.id.clone(),
        title: rule.title.clone(),
        detail: format!(
            "{} · {} {} · review required",
            rule.title, PROBABILITY_REWRITES.pack_id, PROBABILITY_REWRITES.pack_version
        ),
        rank: rank as u32,
        proposal: SemanticEditProposal {
            title: rule.title.clone(),
            safety: "review-required".into(),
            evidence,
            files: vec![SemanticEditFile {
                file_id: document.file_id.clone(),
                path: document.path.clone(),
                document_version: document.document_version,
                edits: vec![SemanticTextEdit {
                    range: recognition.range.clone(),
                    expected_text: expected_text.into(),
                    replacement_text,
                }],
            }],
        },
    }
}

fn validate_pack(pack: &RawRewritePack) -> Result<(), String> {
    if pack.schema_version != REWRITE_SCHEMA_VERSION {
        return Err("unsupported rewrite schema".into());
    }
    let mut ids = HashSet::new();
    for rule in &pack.rewrites {
        if !ids.insert(rule.id.as_str())
            || rule.id.is_empty()
            || rule.title.is_empty()
            || rule.source_pattern.is_empty()
            || rule.required_refinements.is_empty()
            || rule.replacement_template.is_empty()
        {
            return Err(format!("incomplete or duplicate rewrite {}", rule.id));
        }
        for required in &rule.required_refinements {
            if required.parameter.is_empty()
                || required.refinement.is_empty()
                || !rule
                    .replacement_template
                    .contains(&format!("{{{{{}}}}}", required.parameter))
            {
                return Err(format!("invalid side condition in rewrite {}", rule.id));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::formula_rewrites;
    use crate::consistency::analyze_consistency;
    use crate::parser::{math_regions, parse_regions};
    use crate::pattern::analyze_formulas;
    use crate::prose::analyze_prose;
    use crate::shape::analyze_shapes;
    use crate::{DocumentLanguage, ProjectDocument};
    use serde::Deserialize;

    fn rewrites(source: &str, needle: &str) -> Vec<crate::FormulaRewrite> {
        rewrites_at(
            source,
            source.find(needle).expect("needle must exist") as u32,
        )
    }

    fn rewrites_at(source: &str, offset: u32) -> Vec<crate::FormulaRewrite> {
        let regions = math_regions(source, DocumentLanguage::Markdown);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.md".into(),
            language: DocumentLanguage::Markdown,
            content: source.into(),
            document_version: 7,
            math_regions: regions.clone(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = analyze_prose(&document, &parsed);
        let shapes = analyze_shapes(&document, &parsed, &prose.shapes);
        let consistency = analyze_consistency(&document, &prose.definitions, &shapes);
        let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
        formula_rewrites(&document, &formulas, offset)
    }

    #[test]
    fn offers_definition_expansion_when_the_condition_is_proven_nonzero() {
        let source = "Let $A$ denote an event.\nLet $B$ denote an event of positive probability.\n$p = \\mathbb{P}\\left(A \\mid B\\right)$";
        let rewrites = rewrites(source, "mathbb");

        assert_eq!(rewrites.len(), 1);
        assert_eq!(rewrites[0].rule_id, "conditional-probability-definition");
        assert_eq!(
            rewrites[0].proposal.files[0].edits[0].expected_text,
            "\\mathbb{P}\\left(A \\mid B\\right)"
        );
        assert_eq!(
            rewrites[0].proposal.files[0].edits[0].replacement_text,
            "\\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}"
        );
        assert_eq!(rewrites[0].proposal.files[0].document_version, 7);
        assert_eq!(rewrites[0].proposal.safety, "review-required");
    }

    #[test]
    fn adds_bayes_only_when_both_conditional_probabilities_are_defined() {
        let source = "Let $A$ denote an event of positive probability.\nLet $B$ denote an event of positive probability.\n$p = \\mathbb{P}(A \\mid B)$";
        let rewrites = rewrites(source, "mathbb");

        assert_eq!(
            rewrites
                .iter()
                .map(|rewrite| rewrite.rule_id.as_str())
                .collect::<Vec<_>>(),
            ["conditional-probability-definition", "bayes-theorem"]
        );
        assert_eq!(
            rewrites[1].proposal.files[0].edits[0].replacement_text,
            "\\frac{\\mathbb{P}(B \\mid A)\\mathbb{P}(A)}{\\mathbb{P}(B)}"
        );
    }

    #[test]
    fn offers_the_unique_rewrite_from_any_cursor_position_in_its_math_region() {
        let source = "Let $A$ denote an event of positive probability.\nLet $B$ denote an event of positive probability.\n$p = \\mathbb{P}(A \\mid B)$";
        let expected = ["conditional-probability-definition", "bayes-theorem"];
        let offsets = [
            source.find("$p").unwrap(),
            source.find(" = ").unwrap() + 1,
            source.len() - 1,
            source.len(),
        ];

        for offset in offsets {
            let actual = rewrites_at(source, offset as u32)
                .into_iter()
                .map(|rewrite| rewrite.rule_id)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "cursor offset {offset}");
        }
    }

    #[test]
    fn suppresses_rewrites_without_explicit_side_condition_evidence() {
        let source =
            "Let $A$ denote an event.\nLet $B$ denote an event.\n$p = \\mathbb{P}(A \\mid B)$";
        assert!(rewrites(source, "mathbb").is_empty());
    }

    #[test]
    fn suppresses_notation_only_and_unfinished_formulas() {
        assert!(rewrites("$p = \\mathbb{P}(A \\mid B)$", "mathbb").is_empty());
        assert!(
            rewrites(
                "Let $A$ denote an event.\nLet $B$ denote an event of positive probability.\n$p = \\mathbb{P}(A \\mid B$",
                "mathbb"
            )
            .is_empty()
        );
    }

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
        expected_rules: Vec<String>,
    }

    #[test]
    fn matches_the_labeled_rewrite_corpus() {
        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../fixtures/v0.7/formula-rewrite-corpus.json"
        ))
        .unwrap();
        assert_eq!(corpus.false_positive_budget, 0);

        for case in corpus.cases {
            let actual = rewrites(&case.content, "mathbb")
                .into_iter()
                .map(|rewrite| rewrite.rule_id)
                .collect::<Vec<_>>();
            assert_eq!(actual, case.expected_rules, "corpus case {}", case.id);
        }
    }
}
