use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use crate::canonical::declared_symbols;
use crate::parser::ParsedMath;
use crate::scientific_prose::{
    ClauseDisposition, ScientificClause, ScientificMention, align_ordered_descriptions, clause_at,
    extract_assumptions, segment_scientific_clauses,
};
use crate::{
    AssumptionInfo, DefinitionInfo, Evidence, Location, ProjectDocument, SourceIndex, SourceRange,
};

static LET_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:let|where|take|given|suppose|assume|subject\s+to)\s*$").unwrap()
});
static DEFINITION_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:denote(?:s)?|stand(?:s)?\s+for|to\s+be|be|is|are|as|represent(?:s)?)\s+([^$.;\n]+)").unwrap()
});
static DIRECT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[.!?]\s*|\n\s*)$").unwrap());
static APPOSITION_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*,\s*(?:(?:an?|the)\s+)?([^$,.;\n]+)\s*,").unwrap());
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
static COORDINATED_LET_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*|,\s*)(?:let|take|given|suppose|assume)\s*$").unwrap()
});
static COORDINATED_DIRECT_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)(?:here\s+)?(?:the\s+(?:symbols|notations)\s+)?$").unwrap()
});
static COORDINATED_WRITE_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)we\s+write\s*$").unwrap());
static COORDINATED_DENOTE_BY_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)denote\s+by\s*$").unwrap());
static COORDINATED_MAPPING_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s+(?:denote|represent|mean|stand\s+for)\s+(.+?)(?:,\s*|\s+)(?:respectively|in\s+that\s+order)\s*[.;]",
    )
    .unwrap()
});
static COORDINATED_WRITE_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s+for\s+(.+?)(?:,\s*|\s+)(?:respectively|in\s+that\s+order)\s*[.;]").unwrap()
});
static COORDINATED_DENOTE_BY_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s+(.+?)(?:,\s*|\s+)(?:respectively|in\s+that\s+order)\s*[.;]").unwrap()
});
static COORDINATED_SHARED_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s+(?:denote|represent|stand\s+for|to\s+be|be|are)\s+([^,.;\n]+)[,.;]")
        .unwrap()
});
static FRONTED_SHARED_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)((?:the\s+)?[a-z][a-z-]*(?:\s+[a-z][a-z-]*){0,3})\s*$")
        .unwrap()
});
static FRONTED_SHARED_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s+(?:belong(?:s)?\s+to|are\s+(?:in|on|defined\s+(?:in|on)|drawn\s+from|measured\s+in)|share)\b[^.;\n]*[.;]",
    )
    .unwrap()
});
static CONTEXTUAL_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[.!?]\s*|\n\s*)(?:here|throughout,?|with|given|suppose|assume|subject\s+to)\s*$",
    )
    .unwrap()
});
static CONTEXTUAL_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:denot(?:e|es|ing)|designate(?:s)?|be|is|represent(?:s)?)\s+([^,.;\n]+)")
        .unwrap()
});
static DIRECT_EXTENDED_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)(?:the\s+(?:symbol|notation)\s+)?$").unwrap()
});
static DIRECT_EXTENDED_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(?:means|stands\s+for|refers\s+to|will\s+denote|shall\s+be|designates)\s+([^.;\n]+)",
    )
    .unwrap()
});
static WRITE_FOR_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)we\s+write\s*$").unwrap());
static WRITE_FOR_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+for\s+([^.;\n]+)").unwrap());
static DEFINE_AS_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)define\s*$").unwrap());
static DEFINE_AS_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+as\s+([^.;\n]+)").unwrap());
static DENOTE_BY_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)denote\s+by\s*$").unwrap());
static DENOTE_BY_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+(?:(?:an?|the)\s+)?([^.;\n]+)").unwrap());
static SET_EQUAL_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)set\s*$").unwrap());
static SET_EQUAL_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+equal\s+to\s+([^.;\n]+)").unwrap());
static USE_REPRESENT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)we\s+use\s*$").unwrap());
static USE_REPRESENT_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+to\s+represent\s+([^.;\n]+)").unwrap());
static CALL_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[.!?]\s*|\n\s*)call\s*$").unwrap());
static CALL_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+(?:(?:an?|the)\s+)?([^.;\n]+)").unwrap());
static EXPRESSION_DEFINES_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s+defines?\s+(?:(?:an?|the)\s+)?([^.;\n]+)").unwrap());
static VECTOR_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)(?:[ -]dimensional|[ -])\s*(?:(?:real|normalized)\s+)*(?:state\s+|control\s+)?(?:vectors?|states?|inputs?|controls?)(?:\s+of\s+[a-z -]+)?\s*(?:,?\s+and)?\s*$")
        .unwrap()
});
static MATRIX_DIMENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9]+)(?:\s+by\s+|\s+x\s+|\s*×\s*|\s*\\times\s*)([a-z0-9]+)(?:\s+(?:(?:real|symmetric|diagonal|orthogonal|positive[ -]definite|positive[ -]semidefinite|state|input|system)\s+)*(?:matrix|matrices))?\s*(?:,?\s+and)?\s*$",
    )
    .unwrap()
});
static SQUARE_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bsquare(?:\s+matrices?)?(?:\s+(?:of|with))?(?:\s+(?:the\s+)?(?:same|common))?\s*(?:size|order|dimension)?\s*([a-z0-9]+)\s*$").unwrap()
});
static INLINE_VECTOR_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:be|is)\s+an?\s+\$([a-z0-9]+)\$[ -]dimensional\s+(?:real\s+)?vector")
        .unwrap()
});
static PASSIVE_DEFINITION_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[.!?]\s*|\n\s*)(?:(?:an?|the)\s+)?([a-z][a-z0-9 -]{0,119})\s+(?:is|are)\s+(?:denoted|represented|written)\s+by\s*$",
    )
    .unwrap()
});
static PASSIVE_DEFINITION_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[,.;:]").unwrap());

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
pub(crate) struct ProseObservations {
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: Vec<ProseShapeClaim>,
    pub assumptions: Vec<AssumptionInfo>,
}

fn primary_symbol(document: &ProjectDocument, math: &ParsedMath) -> Option<(String, SourceRange)> {
    declared_symbols(document, &math.region.content_range)
        .into_iter()
        .next()
        .or_else(|| math.symbols.first().cloned())
}

pub(crate) fn observe_prose(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
) -> ProseObservations {
    let visible_source = visible_prose_source(document);
    let source = visible_source.as_ref();
    let index = SourceIndex::new(source);
    let mut analysis = ProseObservations::default();
    let clauses = segment_scientific_clauses(source, document.language);
    let mentions = parsed
        .iter()
        .filter_map(|math| {
            let (symbol, _) = primary_symbol(document, math)?;
            Some(ScientificMention {
                symbol,
                start: index.byte_for_utf16(math.region.full_range.start_offset),
                end: index.byte_for_utf16(math.region.full_range.end_offset),
            })
        })
        .collect::<Vec<_>>();
    collect_assumptions(&index, &clauses, &mentions, &mut analysis);

    collect_coordinated_definitions(document, source, parsed, &index, &clauses, &mut analysis);
    collect_clause_definitions(document, source, parsed, &index, &clauses, &mut analysis);
    for math in parsed {
        let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
            continue;
        };
        let start_byte = index.byte_for_utf16(math.region.full_range.start_offset);
        let end_byte = index.byte_for_utf16(math.region.full_range.end_offset);
        if clause_at(&clauses, start_byte)
            .is_some_and(|clause| clause.disposition != ClauseDisposition::Establishing)
        {
            continue;
        }
        let before_start = bounded_start(source, start_byte, 160);
        let after_end = bounded_end(source, end_byte, 240);
        let before = &source[before_start..start_byte];
        let after = &source[end_byte..after_end];
        let trimmed_after = after.trim_start().to_ascii_lowercase();
        if let Some(captures) = INLINE_VECTOR_SUFFIX.captures(after) {
            let evidence_end = end_byte + captures.get(0).unwrap().end();
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                &format!("{}-dimensional vector", &captures[1]),
                "english-inline-dimension-definition",
                start_byte,
                evidence_end,
            );
            continue;
        }
        if [
            "-dimensional",
            "dimensional",
            "-vector",
            "-state",
            "-input",
            " by ",
            "\\times",
        ]
        .iter()
        .any(|prefix| trimmed_after.starts_with(prefix))
        {
            continue;
        }

        if let (Some(prefix), Some(suffix)) = (
            PASSIVE_DEFINITION_PREFIX.captures(before),
            PASSIVE_DEFINITION_SUFFIX.find(after),
        ) {
            let description = prefix.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                description,
                "english-passive-definition",
                before_start + prefix.get(0).unwrap().start(),
                end_byte + suffix.end(),
            );
        } else if let Some(explicit) = explicit_single_definition(before, after, math) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                explicit.description,
                explicit.rule_id,
                before_start + explicit.prefix_start,
                end_byte + explicit.suffix_end,
            );
        } else if let Some(prefix) = LET_PREFIX.find(before)
            && let Some(captures) = DEFINITION_SUFFIX.captures(after)
        {
            let description = captures.get(1).unwrap().as_str().trim();
            let end = end_byte + captures.get(1).unwrap().end();
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                description,
                "english-let-definition",
                before_start + prefix.start(),
                end,
            );
        } else if let Some(captures) = APPOSITION_SUFFIX.captures(after)
            && !captures.get(1).unwrap().as_str().contains("\\(")
            && !captures.get(1).unwrap().as_str().contains("\\[")
        {
            let description = captures.get(1).unwrap().as_str().trim();
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
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
                &symbol,
                &symbol_range,
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
                &symbol,
                &symbol_range,
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
                &symbol,
                &symbol_range,
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
                &symbol,
                &symbol_range,
                "explicit mathematical declaration",
                "english-let-math-declaration",
                start_byte,
                end_byte,
            );
        }

        collect_notation_table(
            document,
            source,
            &index,
            &symbol,
            &symbol_range,
            start_byte,
            end_byte,
            &mut analysis,
        );
    }
    deduplicate(&mut analysis);
    analysis
}

fn collect_clause_definitions(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    output: &mut ProseObservations,
) {
    for clause in clauses {
        if clause.disposition != ClauseDisposition::Establishing {
            continue;
        }
        let sentence_start = clause.start;
        let sentence_end = clause.end;
        let sentence = clause.text;
        let sentence_lower = sentence.to_ascii_lowercase();
        if sentence_lower.contains("respectively") || sentence_lower.contains("in that order") {
            collect_ordered_clause_definition(
                document,
                source,
                parsed,
                index,
                output,
                sentence_start,
                sentence_end,
            );
            continue;
        }
        let regions = parsed
            .iter()
            .filter(|math| {
                let start = index.byte_for_utf16(math.region.full_range.start_offset);
                sentence_start <= start
                    && start < sentence_end
                    && !is_description_parameter(source, math, sentence_start, sentence_end, index)
            })
            .collect::<Vec<_>>();
        if regions.len() < 2 {
            continue;
        }
        let prefix_end = index.byte_for_utf16(regions[0].region.full_range.start_offset);
        let prefix = &source[sentence_start..prefix_end];
        let contextual = [
            "let",
            "where",
            "here",
            "throughout",
            "symbols",
            "notations",
            "declares",
        ]
        .iter()
        .any(|word| prefix.to_ascii_lowercase().contains(word));
        let explicit = regions
            .iter()
            .filter(|math| {
                let end = index.byte_for_utf16(math.region.full_range.end_offset);
                definition_clause(&source[end..sentence_end]).1
            })
            .count();
        if !contextual && explicit < 2 {
            continue;
        }
        for (position, math) in regions.iter().enumerate() {
            let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
                continue;
            };
            let end = index.byte_for_utf16(math.region.full_range.end_offset);
            let next = regions.get(position + 1).map_or(sentence_end, |next| {
                index.byte_for_utf16(next.region.full_range.start_offset)
            });
            let (description, _) = definition_clause(&source[end..next]);
            let Some(description) = description else {
                continue;
            };
            push_claim(
                output,
                document,
                index,
                &symbol,
                &symbol_range,
                description,
                "english-clause-definition",
                sentence_start,
                next,
            );
        }
    }
}

fn collect_ordered_clause_definition(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    output: &mut ProseObservations,
    sentence_start: usize,
    sentence_end: usize,
) {
    let regions = parsed
        .iter()
        .filter(|math| {
            let start = index.byte_for_utf16(math.region.full_range.start_offset);
            sentence_start <= start
                && start < sentence_end
                && !is_description_parameter(source, math, sentence_start, sentence_end, index)
        })
        .filter(|math| primary_symbol(document, math).is_some())
        .collect::<Vec<_>>();
    if regions.len() < 2 {
        return;
    }
    let last_end = index.byte_for_utf16(regions.last().unwrap().region.full_range.end_offset);
    let suffix = source[last_end..sentence_end].trim();
    let suffix = suffix
        .trim_end_matches(|character: char| character.is_whitespace() || character == '.')
        .trim_end_matches("respectively")
        .trim_end_matches("in that order")
        .trim_end_matches(|character: char| character.is_whitespace() || character == ',');
    let (description, explicit) = definition_clause(suffix);
    if !explicit {
        return;
    }
    let Some(descriptions) =
        description.and_then(|description| align_ordered_descriptions(description, regions.len()))
    else {
        return;
    };
    for (math, description) in regions.into_iter().zip(descriptions) {
        let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
            return;
        };
        push_claim(
            output,
            document,
            index,
            &symbol,
            &symbol_range,
            description,
            "english-clause-ordered-definition",
            sentence_start,
            sentence_end,
        );
    }
}

fn collect_assumptions(
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    output: &mut ProseObservations,
) {
    for clause in clauses {
        for assumption in extract_assumptions(clause, mentions) {
            let mut source_ranges = assumption
                .subjects
                .iter()
                .map(|subject| SourceRange {
                    start_offset: index.utf16_for_byte(subject.start),
                    end_offset: index.utf16_for_byte(subject.end),
                })
                .collect::<Vec<_>>();
            source_ranges.push(SourceRange {
                start_offset: index.utf16_for_byte(assumption.phrase_start),
                end_offset: index.utf16_for_byte(assumption.phrase_end),
            });
            output.assumptions.push(AssumptionInfo {
                kind: assumption.kind,
                value: assumption.value,
                subjects: assumption
                    .subjects
                    .into_iter()
                    .map(|subject| subject.symbol)
                    .collect(),
                evidence: Evidence {
                    rule_id: "english-scientific-assumption".into(),
                    kind: "explicit-prose".into(),
                    strength: "strong".into(),
                    source_ranges,
                },
            });
        }
    }
    output.assumptions.sort_by(|left, right| {
        left.evidence
            .source_ranges
            .last()
            .map(|range| range.start_offset)
            .cmp(
                &right
                    .evidence
                    .source_ranges
                    .last()
                    .map(|range| range.start_offset),
            )
            .then(left.kind.cmp(&right.kind))
            .then(left.value.cmp(&right.value))
    });
    output.assumptions.dedup();
}

fn is_description_parameter(
    source: &str,
    math: &ParsedMath,
    sentence_start: usize,
    sentence_end: usize,
    index: &SourceIndex,
) -> bool {
    let start = index.byte_for_utf16(math.region.full_range.start_offset);
    let end = index.byte_for_utf16(math.region.full_range.end_offset);
    if end > sentence_end {
        return false;
    }
    let before = source[sentence_start..start]
        .trim_end()
        .to_ascii_lowercase();
    let after = source[end..sentence_end].trim_start().to_ascii_lowercase();
    after.starts_with("-dimensional")
        || after.starts_with("dimensional")
        || after.starts_with("by ")
        || after.starts_with("\\times")
        || before.ends_with(" by")
        || before.ends_with("\\times")
}

fn definition_clause(segment: &str) -> (Option<&str>, bool) {
    let mut clause = segment.trim();
    clause = clause.trim_start_matches([',', ';', ':']).trim_start();
    for connector in ["and ", "while ", "whereas "] {
        if clause.to_ascii_lowercase().starts_with(connector) {
            clause = clause[connector.len()..].trim_start();
            break;
        }
    }
    let lower = clause.to_ascii_lowercase();
    let mut explicit = false;
    for verb in [
        "denotes ",
        "denote ",
        "represents ",
        "represent ",
        "stands for ",
        "stand for ",
        "is ",
        "are ",
        "be ",
        "in ",
    ] {
        if lower.starts_with(verb) {
            clause = clause[verb.len()..].trim_start();
            explicit = true;
            break;
        }
    }
    clause = clause
        .trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, ',' | ';' | ':' | '.')
        })
        .trim_end_matches(" and")
        .trim_end_matches(" while")
        .trim();
    let lower = clause.to_ascii_lowercase();
    for prefix in ["a ", "an ", "the "] {
        if lower.starts_with(prefix) {
            clause = clause[prefix.len()..].trim_start();
            break;
        }
    }
    let valid = !clause.is_empty()
        && !matches!(lower.as_str(), "and" | "while" | "whereas")
        && clause.len() <= 120
        && !clause.contains('=')
        && !clause.contains("\\[")
        && !clause.contains("$$");
    (valid.then_some(clause), explicit)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoordinationLead {
    Let,
    Direct,
    Write,
    DenoteBy,
}

#[derive(Debug, PartialEq, Eq)]
struct SingleDefinitionMatch<'a> {
    description: &'a str,
    rule_id: &'static str,
    prefix_start: usize,
    suffix_end: usize,
}

fn explicit_single_definition<'a>(
    before: &str,
    after: &'a str,
    math: &ParsedMath,
) -> Option<SingleDefinitionMatch<'a>> {
    let trimmed_after = after.trim_start();
    if trimmed_after.starts_with(',') || trimmed_after.starts_with("and ") {
        return None;
    }
    let rules = [
        (
            &*WRITE_FOR_PREFIX,
            &*WRITE_FOR_SUFFIX,
            "english-write-for-definition",
        ),
        (
            &*DEFINE_AS_PREFIX,
            &*DEFINE_AS_SUFFIX,
            "english-imperative-definition",
        ),
        (
            &*DENOTE_BY_PREFIX,
            &*DENOTE_BY_SUFFIX,
            "english-imperative-definition",
        ),
        (
            &*SET_EQUAL_PREFIX,
            &*SET_EQUAL_SUFFIX,
            "english-imperative-definition",
        ),
        (
            &*USE_REPRESENT_PREFIX,
            &*USE_REPRESENT_SUFFIX,
            "english-use-definition",
        ),
        (
            &*CALL_PREFIX,
            &*CALL_SUFFIX,
            "english-imperative-definition",
        ),
        (
            &*CONTEXTUAL_PREFIX,
            &*CONTEXTUAL_SUFFIX,
            "english-contextual-definition",
        ),
        (
            &*DIRECT_EXTENDED_PREFIX,
            &*DIRECT_EXTENDED_SUFFIX,
            "english-relational-definition",
        ),
    ];
    for (prefix_pattern, suffix_pattern, rule_id) in rules {
        let Some(prefix) = prefix_pattern.find(before) else {
            continue;
        };
        let Some(captures) = suffix_pattern.captures(after) else {
            continue;
        };
        let description = captures.get(1).unwrap();
        return Some(SingleDefinitionMatch {
            description: description.as_str().trim(),
            rule_id,
            prefix_start: prefix.start(),
            suffix_end: description.end(),
        });
    }

    if DIRECT_PREFIX.is_match(before)
        && contains_assignment(&math.root)
        && let Some(captures) = EXPRESSION_DEFINES_SUFFIX.captures(after)
    {
        let description = captures.get(1).unwrap();
        return Some(SingleDefinitionMatch {
            description: description.as_str().trim(),
            rule_id: "english-math-assignment-definition",
            prefix_start: before.len(),
            suffix_end: description.end(),
        });
    }
    None
}

fn contains_assignment(node: &crate::EquationNode) -> bool {
    let mut labels = Vec::new();
    collect_equation_labels(node, &mut labels);
    labels.windows(2).any(|pair| pair == [Some(":"), Some("=")])
}

fn collect_equation_labels<'a>(node: &'a crate::EquationNode, output: &mut Vec<Option<&'a str>>) {
    output.push(node.label.as_deref());
    for child in &node.children {
        collect_equation_labels(child, output);
    }
}

fn collect_coordinated_definitions(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    analysis: &mut ProseObservations,
) {
    for arity in (2..=8).rev() {
        for group in parsed.windows(arity) {
            let Some(definitions) = coordinated_group(document, source, group, index, clauses)
            else {
                continue;
            };
            for definition in definitions {
                push_claim(
                    analysis,
                    document,
                    index,
                    &definition.symbol,
                    &definition.range,
                    &definition.description,
                    definition.rule_id,
                    definition.statement_start,
                    definition.statement_end,
                );
            }
        }
    }
}

struct CoordinatedDefinition {
    symbol: String,
    range: SourceRange,
    description: String,
    rule_id: &'static str,
    statement_start: usize,
    statement_end: usize,
}

fn coordinated_group(
    document: &ProjectDocument,
    source: &str,
    group: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
) -> Option<Vec<CoordinatedDefinition>> {
    group.first()?;
    group.last()?;
    if group
        .iter()
        .any(|math| primary_symbol(document, math).is_none())
    {
        return None;
    }
    let starts = group
        .iter()
        .map(|math| index.byte_for_utf16(math.region.full_range.start_offset))
        .collect::<Vec<_>>();
    let ends = group
        .iter()
        .map(|math| index.byte_for_utf16(math.region.full_range.end_offset))
        .collect::<Vec<_>>();
    if !valid_symbol_separators(source, &starts, &ends) {
        return None;
    }

    let first_start = starts[0];
    let last_end = *ends.last()?;
    let clause = clause_at(clauses, first_start)?;
    if clause.disposition != ClauseDisposition::Establishing || last_end > clause.end {
        return None;
    }
    let before_start = bounded_start(source, first_start, 120);
    let after_end = bounded_end(source, last_end, 360);
    let before = &source[before_start..first_start];
    let after = &source[last_end..after_end];
    if let Some((description, prefix_start, suffix_end)) = fronted_shared_description(before, after)
    {
        let mut definitions = Vec::with_capacity(group.len());
        for math in group {
            let (symbol, range) = primary_symbol(document, math)?;
            definitions.push(CoordinatedDefinition {
                symbol,
                range,
                description: description.into(),
                rule_id: "english-fronted-shared-definition",
                statement_start: before_start + prefix_start,
                statement_end: last_end + suffix_end,
            });
        }
        return Some(definitions);
    }
    let (lead, prefix_start) = coordination_lead(before)?;
    let (descriptions, rule_id, suffix_end) = coordinated_descriptions(lead, after, group.len())?;
    let statement_start = before_start + prefix_start;
    let statement_end = last_end + suffix_end;

    let mut definitions = Vec::with_capacity(group.len());
    for (math, description) in group.iter().zip(descriptions) {
        let (symbol, range) = primary_symbol(document, math)?;
        definitions.push(CoordinatedDefinition {
            symbol,
            range,
            description: description.into(),
            rule_id,
            statement_start,
            statement_end,
        });
    }
    Some(definitions)
}

fn fronted_shared_description<'a>(before: &'a str, after: &str) -> Option<(&'a str, usize, usize)> {
    let prefix = FRONTED_SHARED_PREFIX.captures(before)?;
    let description = prefix.get(1)?.as_str().trim();
    let final_word = description
        .split_whitespace()
        .next_back()?
        .trim_end_matches('-')
        .to_ascii_lowercase();
    if !final_word.ends_with('s') || !shared_description_is_unambiguous(description) {
        return None;
    }
    let suffix = FRONTED_SHARED_SUFFIX.find(after)?;
    Some((description, prefix.get(1)?.start(), suffix.end()))
}

fn valid_symbol_separators(source: &str, starts: &[usize], ends: &[usize]) -> bool {
    starts
        .iter()
        .skip(1)
        .zip(ends)
        .all(|(start, end)| matches!(source[*end..*start].trim(), "," | "and" | ", and"))
}

fn coordination_lead(before: &str) -> Option<(CoordinationLead, usize)> {
    for (pattern, lead) in [
        (&*COORDINATED_LET_PREFIX, CoordinationLead::Let),
        (&*COORDINATED_WRITE_PREFIX, CoordinationLead::Write),
        (&*COORDINATED_DENOTE_BY_PREFIX, CoordinationLead::DenoteBy),
        (&*COORDINATED_DIRECT_PREFIX, CoordinationLead::Direct),
    ] {
        if let Some(prefix) = pattern.find(before) {
            return Some((lead, prefix.start()));
        }
    }
    None
}

fn coordinated_descriptions(
    lead: CoordinationLead,
    after: &str,
    arity: usize,
) -> Option<(Vec<&str>, &'static str, usize)> {
    let mapping_pattern = match lead {
        CoordinationLead::Let | CoordinationLead::Direct => &*COORDINATED_MAPPING_SUFFIX,
        CoordinationLead::Write => &*COORDINATED_WRITE_SUFFIX,
        CoordinationLead::DenoteBy => &*COORDINATED_DENOTE_BY_SUFFIX,
    };
    if let Some(captures) = mapping_pattern.captures(after) {
        let descriptions = align_ordered_descriptions(captures.get(1)?.as_str(), arity)?;
        return Some((
            descriptions,
            "english-respectively-definition",
            captures.get(0)?.end(),
        ));
    }
    if lead == CoordinationLead::Let
        && let Some(captures) = COORDINATED_SHARED_SUFFIX.captures(after)
    {
        let description = captures.get(1)?.as_str().trim();
        if !shared_description_is_unambiguous(description) {
            return None;
        }
        return Some((
            vec![description; arity],
            "english-coordinated-definition",
            captures.get(0)?.end(),
        ));
    }
    None
}

fn shared_description_is_unambiguous(description: &str) -> bool {
    !description.contains(',')
        && !description.to_ascii_lowercase().contains(" and ")
        && !description.to_ascii_lowercase().contains(" or ")
}

#[allow(clippy::too_many_arguments)]
fn collect_notation_table(
    document: &ProjectDocument,
    source: &str,
    index: &SourceIndex,
    symbol: &str,
    symbol_range: &SourceRange,
    start_byte: usize,
    end_byte: usize,
    analysis: &mut ProseObservations,
) {
    let line_start = source[..start_byte]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let line_end = source[end_byte..]
        .find('\n')
        .map_or(source.len(), |offset| end_byte + offset);
    let line = &source[line_start..line_end];
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
    analysis: &mut ProseObservations,
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
    let definition_evidence = Evidence {
        rule_id: rule_id.into(),
        kind: "explicit-prose".into(),
        strength: "strong".into(),
        source_ranges: vec![evidence_range.clone()],
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
        entity_id: None,
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
    let description = description
        .split_once(", and let")
        .map_or(description, |(description, _)| description);
    let shape_source = description.replace('$', "");
    let normalized = shape_source.to_ascii_lowercase().replace('-', " ");
    let shape = if let Some(captures) = MATRIX_DIMENSIONS.captures(&shape_source) {
        ProseShape::Matrix(
            captures.get(1).unwrap().as_str().into(),
            captures.get(2).unwrap().as_str().into(),
        )
    } else if let Some(captures) = VECTOR_DIMENSION.captures(&shape_source) {
        ProseShape::Vector(captures.get(1).unwrap().as_str().into())
    } else if let Some(captures) = SQUARE_DIMENSION.captures(&shape_source) {
        let dimension = captures.get(1).unwrap().as_str().to_owned();
        if matches!(dimension.as_str(), "size" | "order" | "dimension") {
            return None;
        }
        ProseShape::Matrix(dimension.clone(), dimension)
    } else if matches!(last_word(&normalized), Some("matrix" | "matrices")) {
        ProseShape::Matrix("?".into(), "?".into())
    } else if matches!(last_word(&normalized), Some("vector" | "vectors")) {
        ProseShape::Vector("?".into())
    } else if normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| matches!(word, "scalar" | "scalars"))
    {
        ProseShape::Scalar
    } else if matches!(last_word(&normalized), Some("tensor" | "tensors")) {
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

pub(crate) fn visible_prose_source(document: &ProjectDocument) -> Cow<'_, str> {
    #[cfg(test)]
    if document.visible_prose.is_empty() {
        return Cow::Borrowed(&document.content);
    }
    let index = SourceIndex::new(&document.content);
    let mut visible = vec![false; document.content.len()];
    let mut math_boundary = vec![false; document.content.len()];
    for span in &document.visible_prose {
        let start = index.byte_for_utf16(span.range.start_offset);
        let end = index.byte_for_utf16(span.range.end_offset);
        visible[start..end].fill(true);
    }
    // Inline math is part of a scientific sentence. Preserve its source-sized
    // surface so clauses such as "real $n$ by $n$ matrices" keep the symbols
    // that carry their dimensions. wasmtex has already established these
    // ranges as math, so this does not make comments or control prose visible.
    for root in &document.math_roots {
        if root.delimiter != "$" && root.delimiter != "\\(" {
            continue;
        }
        let start = index.byte_for_utf16(root.content_range.start_offset);
        let end = index.byte_for_utf16(root.content_range.end_offset);
        visible[start..end].fill(true);
    }
    for root in &document.math_roots {
        let start = index.byte_for_utf16(root.full_range.start_offset);
        let end = index.byte_for_utf16(root.full_range.end_offset);
        if start < math_boundary.len() {
            math_boundary[start] = true;
        }
        if end > start && end - 1 < math_boundary.len() {
            math_boundary[end - 1] = true;
        }
    }
    let mut output = String::with_capacity(document.content.len());
    for (offset, character) in document.content.char_indices() {
        if math_boundary[offset] {
            output.push('$');
        } else if character == '\n' || character == '\r' || visible[offset] {
            output.push(character);
        } else {
            output.push(match character.len_utf8() {
                1 => ' ',
                2 => '\u{00a0}',
                3 => '\u{2000}',
                4 => '\u{10000}',
                _ => unreachable!("UTF-8 scalar width is at most four bytes"),
            });
        }
    }
    debug_assert_eq!(output.len(), document.content.len());
    debug_assert_eq!(
        output.encode_utf16().count(),
        document.content.encode_utf16().count()
    );
    Cow::Owned(output)
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

fn deduplicate(analysis: &mut ProseObservations) {
    analysis.definitions.sort_by_key(|definition| {
        (
            definition.location.range.start_offset,
            definition.evidence.rule_id.clone(),
        )
    });
    analysis
        .definitions
        .dedup_by(|left, right| left.location == right.location);
    analysis.shapes.sort_by_key(|claim| {
        (
            claim.symbol_range.start_offset,
            claim.evidence.rule_id.clone(),
        )
    });
    analysis
        .shapes
        .dedup_by(|left, right| left.symbol_range == right.symbol_range);
}

#[cfg(test)]
mod tests {
    use super::{ProseShape, observe_prose};
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::ProseObservations {
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 4,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        observe_prose(&document, &parse_regions(source, &regions))
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
        assert!(analysis.shapes.is_empty(), "{:?}", analysis.shapes);
    }

    #[test]
    fn recognizes_extended_single_declaration_families() {
        let source = "We write $x$ for the input scalar.\nThe symbol $G$ stands for the graph.\nDefine $p$ as the empirical probability.\nDenote by $d$ the distance.\nSet $r$ equal to the residual norm.\nWe use $I$ to represent the identity matrix.\nCall $e$ the identity element.\nHere $T$ denotes the linear operator.\nWith $m$ denoting the row count, continue.\n$f := g+h$ defines the combined function.";
        let analysis = analyze(source);
        assert_eq!(analysis.definitions.len(), 10);
        assert_eq!(analysis.definitions[0].description, "the input scalar");
        assert_eq!(analysis.definitions[1].description, "the graph");
        assert_eq!(
            analysis.definitions[2].description,
            "the empirical probability"
        );
        assert_eq!(analysis.definitions[3].description, "distance");
        assert_eq!(analysis.definitions[4].description, "the residual norm");
        assert_eq!(analysis.definitions[5].description, "the identity matrix");
        assert_eq!(analysis.definitions[6].description, "identity element");
        assert_eq!(analysis.definitions[7].description, "the linear operator");
        assert_eq!(analysis.definitions[8].description, "the row count");
        assert_eq!(analysis.definitions[9].description, "combined function");
    }

    #[test]
    fn maps_coordinated_declarations_by_arity_and_refuses_mismatches() {
        let source = "Let $a$ and $b$ denote a lower bound and an upper bound, respectively.\nLet $x$, $y$, and $z$ denote the input, state, and output, respectively.\nThe symbols $p$, $q$, and $r$ stand for $d$, $e$, and $f$, respectively.\nLet $U$ and $V$ be vector spaces.\nLet $i$, $j$, and $k$ denote row and column indices, respectively.";
        let analysis = analyze(source);
        let descriptions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        assert!(descriptions.contains(&("a", "lower bound")));
        assert!(descriptions.contains(&("b", "upper bound")));
        assert!(descriptions.contains(&("x", "input")));
        assert!(descriptions.contains(&("y", "state")));
        assert!(descriptions.contains(&("z", "output")));
        assert!(descriptions.contains(&("p", "$d$")));
        assert!(descriptions.contains(&("q", "$e$")));
        assert!(descriptions.contains(&("r", "$f$")));
        assert!(
            descriptions.contains(&("U", "vector spaces")),
            "{descriptions:?}"
        );
        assert!(descriptions.contains(&("V", "vector spaces")));
        assert!(
            !descriptions
                .iter()
                .any(|(symbol, _)| ["i", "j", "k"].contains(symbol))
        );
    }

    #[test]
    fn maps_shared_declarations_after_an_introductory_clause() {
        let analysis = analyze(
            "During optimization, let $x$ and $y$ be n-dimensional iterates, $g$ the gradient.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        assert!(definitions.contains(&("x", "n-dimensional iterates")));
        assert!(definitions.contains(&("y", "n-dimensional iterates")));
    }

    #[test]
    fn maps_fronted_plural_types_without_domain_specific_grammar() {
        let analysis = analyze(
            "Events $C$ and $D$ belong to one probability space. Sets $S$ and $T$ are defined on a common universe. Vectors $u$ and $v$ share one coordinate frame.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        for expected in [
            ("C", "Events"),
            ("D", "Events"),
            ("S", "Sets"),
            ("T", "Sets"),
            ("u", "Vectors"),
            ("v", "Vectors"),
        ] {
            assert!(definitions.contains(&expected), "{definitions:?}");
        }
    }

    #[test]
    fn composes_active_passive_and_arbitrary_arity_declarations() {
        let source = "Given $A$ as the system matrix.\nTake $x$ to be the state vector.\nThe control input is denoted by $u$.\nLet $a$, $b$, $c$, and $d$ denote gain, bias, scale, and offset, respectively.";
        let analysis = analyze(source);
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        assert!(
            definitions.contains(&("A", "the system matrix")),
            "{definitions:?}"
        );
        assert!(
            definitions.contains(&("x", "the state vector")),
            "{definitions:?}"
        );
        assert!(
            definitions.contains(&("u", "control input")),
            "{definitions:?}"
        );
        assert!(definitions.contains(&("a", "gain")), "{definitions:?}");
        assert!(definitions.contains(&("b", "bias")), "{definitions:?}");
        assert!(definitions.contains(&("c", "scale")), "{definitions:?}");
        assert!(definitions.contains(&("d", "offset")), "{definitions:?}");
    }

    #[test]
    fn records_assumptions_but_refuses_non_evidence() {
        let source = "Assume $A$ is symmetric and positive definite.\nIf $B$ were invertible, the solve would be unique.\nAccording to \\cite{prior}, $C$ is continuous.\n$D$ may be differentiable.\n$E$ is not independent.";
        let analysis = analyze(source);
        let assumptions = analysis
            .assumptions
            .iter()
            .map(|assumption| (assumption.kind.as_str(), assumption.value.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            assumptions,
            vec![
                ("structure", "symmetric"),
                ("definiteness", "positive-definite")
            ]
        );
        assert!(
            analysis
                .definitions
                .iter()
                .all(|definition| { !["B", "C", "D", "E"].contains(&definition.symbol.as_str()) })
        );
    }
}
