use std::sync::LazyLock;

use regex::Regex;

use crate::parser::ParsedMath;
use crate::{DefinitionInfo, Evidence, Location, ProjectDocument, SourceIndex, SourceRange};

static LET_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:let|where)\s*$").unwrap());
static DEFINITION_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:denote(?:s)?|be|is|represent(?:s)?)\s+([^.;\n]+)").unwrap()
});
static DIRECT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[.!?]\s*|\n\s*)$").unwrap());
static APPOSITION_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*,\s*(?:(?:an?|the)\s+)?([^,.;\n]+)\s*,").unwrap());
static PARENTHETICAL_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:(?:an?|the)\s+)([a-z][a-z0-9 -]{0,79})\s*\(\s*$").unwrap()
});
static PARENTHETICAL_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\)").unwrap());
static QUANTIFIED_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[.!?]\s+)for\s+(?:each|every)\s+(?:(?:an?|the)\s+)?([a-z][a-z0-9 -]{0,79})\s+$",
    )
    .unwrap()
});
static QUANTIFIED_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[,.;:]").unwrap());
static RESPECTIVELY_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s+)let\s*$").unwrap());
static RESPECTIVELY_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s+denote\s+(?:(?:an?|the)\s+)?([^,.;\n]+?)\s+and\s+(?:(?:an?|the)\s+)?([^,.;\n]+?),\s*respectively\s*[.;]",
    )
    .unwrap()
});
static VECTOR_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z0-9]+)[ -]dimensional\s+(?:(?:real|normalized)\s+)*vector\s*$").unwrap()
});
static MATRIX_DIMENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9]+)\s*(?:by|x|×)\s*([a-z0-9]+)\s+(?:(?:real|symmetric|diagonal|orthogonal|positive[ -]definite|positive[ -]semidefinite)\s+)*matrix\s*$",
    )
    .unwrap()
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProseShape {
    Scalar,
    Vector(String),
    Matrix(String, String),
    Tensor(Vec<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct ProseShapeClaim {
    pub symbol: String,
    pub symbol_range: SourceRange,
    pub available_from: u32,
    pub evidence: Evidence,
    pub shape: ProseShape,
    pub refinements: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProseAnalysis {
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: Vec<ProseShapeClaim>,
}

pub(crate) fn analyze_prose(document: &ProjectDocument, parsed: &[ParsedMath]) -> ProseAnalysis {
    let index = SourceIndex::new(&document.content);
    let mut analysis = ProseAnalysis::default();

    collect_respectively(document, parsed, &index, &mut analysis);
    for math in parsed {
        let Some((symbol, symbol_range)) = math.symbols.first() else {
            continue;
        };
        let start_byte = index.byte_for_utf16(math.region.full_range.start_offset);
        let end_byte = index.byte_for_utf16(math.region.full_range.end_offset);
        let before_start = bounded_start(&document.content, start_byte, 160);
        let after_end = bounded_end(&document.content, end_byte, 240);
        let before = &document.content[before_start..start_byte];
        let after = &document.content[end_byte..after_end];

        if let Some(prefix) = LET_PREFIX.find(before)
            && let Some(captures) = DEFINITION_SUFFIX.captures(after)
        {
            let description = captures.get(1).unwrap().as_str().trim();
            let end = end_byte + captures.get(1).unwrap().end();
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                description,
                "english-let-definition",
                before_start + prefix.start(),
                end,
            );
        } else if let Some(captures) = APPOSITION_SUFFIX.captures(after) {
            let description = captures.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                description,
                "english-apposition-definition",
                start_byte,
                end_byte + captures.get(0).unwrap().end(),
            );
        } else if let (Some(prefix), Some(suffix)) = (
            PARENTHETICAL_PREFIX.captures(before),
            PARENTHETICAL_SUFFIX.find(after),
        ) {
            let description = prefix.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                description,
                "english-parenthetical-definition",
                before_start + prefix.get(0).unwrap().start(),
                end_byte + suffix.end(),
            );
        } else if let (Some(prefix), Some(suffix)) = (
            QUANTIFIED_PREFIX.captures(before),
            QUANTIFIED_SUFFIX.find(after),
        ) {
            let description = prefix.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                description,
                "english-quantified-definition",
                before_start + prefix.get(0).unwrap().start(),
                end_byte + suffix.end(),
            );
        } else if DIRECT_PREFIX.is_match(before)
            && let Some(captures) = DEFINITION_SUFFIX.captures(after)
        {
            let description = captures.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                description,
                "english-relational-definition",
                start_byte,
                end_byte + captures.get(1).unwrap().end(),
            );
        } else if LET_PREFIX.is_match(before) && math.symbols.len() > 1 {
            push_claim(
                &mut analysis,
                document,
                &index,
                symbol,
                symbol_range,
                "explicit mathematical declaration",
                "english-let-math-declaration",
                start_byte,
                end_byte,
            );
        }

        collect_notation_table(
            document,
            &index,
            symbol,
            symbol_range,
            start_byte,
            end_byte,
            &mut analysis,
        );
    }
    deduplicate(&mut analysis);
    analysis
}

fn collect_respectively(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    analysis: &mut ProseAnalysis,
) {
    for pair in parsed.windows(2) {
        let [left, right] = pair else { continue };
        let (Some((left_symbol, left_range)), Some((right_symbol, right_range))) =
            (left.symbols.first(), right.symbols.first())
        else {
            continue;
        };
        let left_start = index.byte_for_utf16(left.region.full_range.start_offset);
        let left_end = index.byte_for_utf16(left.region.full_range.end_offset);
        let right_start = index.byte_for_utf16(right.region.full_range.start_offset);
        let right_end = index.byte_for_utf16(right.region.full_range.end_offset);
        if !document.content[left_end..right_start]
            .trim()
            .eq_ignore_ascii_case("and")
        {
            continue;
        }
        let before_start = bounded_start(&document.content, left_start, 80);
        let after_end = bounded_end(&document.content, right_end, 240);
        let before = &document.content[before_start..left_start];
        let after = &document.content[right_end..after_end];
        let Some(prefix) = RESPECTIVELY_PREFIX.find(before) else {
            continue;
        };
        let Some(captures) = RESPECTIVELY_SUFFIX.captures(after) else {
            continue;
        };
        let statement_start = before_start + prefix.start();
        let statement_end = right_end + captures.get(0).unwrap().end();
        for (symbol, range, description) in [
            (left_symbol, left_range, captures.get(1).unwrap().as_str()),
            (right_symbol, right_range, captures.get(2).unwrap().as_str()),
        ] {
            push_claim(
                analysis,
                document,
                index,
                symbol,
                range,
                description.trim(),
                "english-respectively-definition",
                statement_start,
                statement_end,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_notation_table(
    document: &ProjectDocument,
    index: &SourceIndex,
    symbol: &str,
    symbol_range: &SourceRange,
    start_byte: usize,
    end_byte: usize,
    analysis: &mut ProseAnalysis,
) {
    let line_start = document.content[..start_byte]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let line_end = document.content[end_byte..]
        .find('\n')
        .map_or(document.content.len(), |offset| end_byte + offset);
    let line = &document.content[line_start..line_end];
    if !line.contains('|') {
        return;
    }
    let math_end_in_line = end_byte - line_start;
    let tail = &line[math_end_in_line..];
    if let Some(cell_start) = tail.find('|').map(|offset| offset + 1)
        && let Some(cell_end) = tail[cell_start..].find('|')
    {
        let description = tail[cell_start..cell_start + cell_end].trim();
        if !description.is_empty() && !description.chars().all(|ch| ch == '-' || ch == ':') {
            push_claim(
                analysis,
                document,
                index,
                symbol,
                symbol_range,
                description,
                "notation-table-definition",
                line_start,
                line_end,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_claim(
    analysis: &mut ProseAnalysis,
    document: &ProjectDocument,
    index: &SourceIndex,
    symbol: &str,
    symbol_range: &SourceRange,
    description: &str,
    rule_id: &str,
    evidence_start: usize,
    evidence_end: usize,
) {
    let evidence_range = SourceRange {
        start_offset: index.utf16_for_byte(evidence_start),
        end_offset: index.utf16_for_byte(evidence_end),
    };
    let legacy_definition_range = matches!(
        rule_id,
        "english-let-definition" | "english-let-math-declaration" | "notation-table-definition"
    );
    let definition_evidence = Evidence {
        rule_id: rule_id.into(),
        kind: "explicit-prose".into(),
        strength: "strong".into(),
        source_ranges: vec![if legacy_definition_range {
            symbol_range.clone()
        } else {
            evidence_range.clone()
        }],
    };
    analysis.definitions.push(DefinitionInfo {
        symbol: symbol.into(),
        description: description.into(),
        location: Location {
            file_id: document.file_id.clone(),
            path: document.path.clone(),
            range: symbol_range.clone(),
        },
        evidence: definition_evidence.clone(),
    });
    if let Some((shape, refinements)) = shape_claim(description) {
        analysis.shapes.push(ProseShapeClaim {
            symbol: symbol.into(),
            symbol_range: symbol_range.clone(),
            available_from: evidence_range.end_offset,
            evidence: Evidence {
                source_ranges: vec![evidence_range],
                ..definition_evidence
            },
            shape,
            refinements,
        });
    }
}

fn shape_claim(description: &str) -> Option<(ProseShape, Vec<String>)> {
    let normalized = description.to_ascii_lowercase().replace('-', " ");
    let shape = if let Some(captures) = MATRIX_DIMENSIONS.captures(description) {
        ProseShape::Matrix(
            captures.get(1).unwrap().as_str().into(),
            captures.get(2).unwrap().as_str().into(),
        )
    } else if let Some(captures) = VECTOR_DIMENSION.captures(description) {
        ProseShape::Vector(captures.get(1).unwrap().as_str().into())
    } else if last_word(&normalized) == Some("matrix") {
        ProseShape::Matrix("?".into(), "?".into())
    } else if last_word(&normalized) == Some("vector") {
        ProseShape::Vector("?".into())
    } else if last_word(&normalized) == Some("scalar") {
        ProseShape::Scalar
    } else if last_word(&normalized) == Some("tensor") {
        ProseShape::Tensor(vec!["?".into()])
    } else {
        return None;
    };
    let refinements = [
        ("positive semidefinite", "positive-semidefinite"),
        ("positive definite", "positive-definite"),
        ("symmetric", "symmetric"),
        ("diagonal", "diagonal"),
        ("orthogonal", "orthogonal"),
        ("normalized", "normalized"),
    ]
    .into_iter()
    .filter(|(phrase, _)| normalized.contains(phrase))
    .map(|(_, refinement)| refinement.into())
    .collect();
    Some((shape, refinements))
}

fn last_word(value: &str) -> Option<&str> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .rfind(|word| !word.is_empty())
}

fn bounded_start(source: &str, end: usize, characters: usize) -> usize {
    source[..end]
        .char_indices()
        .rev()
        .nth(characters)
        .map_or(0, |(offset, _)| offset)
}

fn bounded_end(source: &str, start: usize, characters: usize) -> usize {
    source[start..]
        .char_indices()
        .nth(characters)
        .map_or(source.len(), |(offset, _)| start + offset)
}

fn deduplicate(analysis: &mut ProseAnalysis) {
    analysis.definitions.sort_by_key(|definition| {
        (
            definition.location.range.start_offset,
            definition.evidence.rule_id.clone(),
        )
    });
    analysis.definitions.dedup_by(|left, right| {
        left.location == right.location && left.evidence.rule_id == right.evidence.rule_id
    });
    analysis.shapes.sort_by_key(|claim| {
        (
            claim.symbol_range.start_offset,
            claim.evidence.rule_id.clone(),
        )
    });
    analysis.shapes.dedup_by(|left, right| {
        left.symbol_range == right.symbol_range && left.evidence.rule_id == right.evidence.rule_id
    });
}

#[cfg(test)]
mod tests {
    use super::{ProseShape, analyze_prose};
    use crate::parser::{math_regions, parse_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::ProseAnalysis {
        let regions = math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
        };
        analyze_prose(&document, &parse_regions(source, &regions))
    }

    #[test]
    fn links_respectively_apposition_parenthetical_and_quantified_prose() {
        let source = "Let $x$ and $A$ denote an n-dimensional vector and an m by n matrix, respectively.\n$S$, the symmetric matrix, is fixed.\nThe normalized vector ($z$) is observed.\nFor every scalar $t$, the result is finite.\n$D$ is a positive-definite diagonal matrix.";
        let analysis = analyze(source);
        assert_eq!(analysis.definitions.len(), 6);
        assert!(matches!(analysis.shapes[0].shape, ProseShape::Vector(_)));
        assert!(matches!(analysis.shapes[1].shape, ProseShape::Matrix(_, _)));
        assert_eq!(analysis.shapes[2].refinements, ["symmetric"]);
        assert_eq!(analysis.shapes[3].refinements, ["normalized"]);
        assert!(matches!(analysis.shapes[4].shape, ProseShape::Scalar));
        assert_eq!(
            analysis.shapes[5].refinements,
            ["positive-definite", "diagonal"]
        );
        assert_eq!(
            analysis.shapes[5].evidence.rule_id,
            "english-relational-definition"
        );
    }

    #[test]
    fn ignores_unbounded_nearby_type_words() {
        let analysis = analyze(
            "The vector near $x$ is only an example.\nWe compare $A$ with a matrix.\n$v$ is a vector field.\n$G$ is a matrix group.",
        );
        assert_eq!(analysis.definitions.len(), 2);
        assert!(analysis.shapes.is_empty());
    }
}
