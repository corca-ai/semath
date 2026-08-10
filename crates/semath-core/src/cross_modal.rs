use std::sync::LazyLock;

use regex::Regex;

use crate::prose::{citation_byte_ranges, visible_prose_source};
use crate::scientific_prose::segment_scientific_clauses;
use crate::semantic_index::{EvidenceModality, EvidencePolarity, OccurrenceKind};
use crate::{MathRootState, ProjectDocument, SourceIndex, SourceRange, StructuralDeclaration};

const MAX_BINDINGS: usize = 512;

static LONG_SHORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b([A-Za-z][A-Za-z0-9]*(?:[ -][A-Za-z][A-Za-z0-9]*){1,11})\s*\(\s*([A-Z][A-Z0-9-]{1,11})\s*\)",
    )
    .unwrap()
});
static SHORT_LONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b([A-Z][A-Z0-9-]{1,11})\s*\(\s*([A-Za-z][A-Za-z0-9]*(?:[ -][A-Za-z][A-Za-z0-9]*){1,11})\s*\)",
    )
    .unwrap()
});
static SHORT_DEFINES_LONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b([A-Z][A-Z0-9-]{1,11})\s+(?:(?:does?|did)\s+not\s+|(?:might|may|could|would)\s+)?(?:stands?\s+for|means?|meant|is\s+(?:not\s+)?short\s+for|is\s+(?:not\s+)?an?\s+abbreviation\s+for|denotes?|represents?)\s+([A-Za-z][A-Za-z0-9]*(?:[ -][A-Za-z][A-Za-z0-9]*){0,11})",
    )
    .unwrap()
});
static LONG_DEFINES_SHORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b([A-Za-z][A-Za-z0-9]*(?:[ -][A-Za-z][A-Za-z0-9]*){1,11})\s*,?\s+(?:abbreviated\s+as|hereafter|denoted\s+by|written\s+as|called)\s+([A-Z][A-Z0-9-]{1,11})\b",
    )
    .unwrap()
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingPredicate {
    Abbreviates,
    Aliases,
    Names,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CrossModalBinding {
    pub(crate) short: String,
    pub(crate) long: String,
    pub(crate) short_range: SourceRange,
    pub(crate) long_range: SourceRange,
    pub(crate) evidence_range: SourceRange,
    pub(crate) polarity: EvidencePolarity,
    pub(crate) modality: EvidenceModality,
    pub(crate) predicate: BindingPredicate,
    pub(crate) occurrence_kind: OccurrenceKind,
    pub(crate) rule_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ByteBinding<'a> {
    short: &'a str,
    long: &'a str,
    short_start: usize,
    short_end: usize,
    long_start: usize,
    long_end: usize,
    evidence_start: usize,
    evidence_end: usize,
    polarity: EvidencePolarity,
    modality: EvidenceModality,
    rule_id: &'static str,
}

pub(crate) fn extract_cross_modal_bindings(document: &ProjectDocument) -> Vec<CrossModalBinding> {
    let visible = visible_prose_source(document);
    let index = SourceIndex::new(&document.content);
    let citations = citation_byte_ranges(document, &index);
    let mut output = extract_prose_bindings(&visible, document.language, &citations)
        .into_iter()
        .map(|binding| CrossModalBinding {
            short: binding.short.to_owned(),
            long: binding.long.to_owned(),
            short_range: byte_range(&index, binding.short_start, binding.short_end),
            long_range: byte_range(&index, binding.long_start, binding.long_end),
            evidence_range: byte_range(&index, binding.evidence_start, binding.evidence_end),
            polarity: binding.polarity,
            modality: binding.modality,
            predicate: BindingPredicate::Abbreviates,
            occurrence_kind: OccurrenceKind::Prose,
            rule_id: binding.rule_id.to_owned(),
        })
        .collect::<Vec<_>>();
    append_structural_bindings(document, &mut output);
    output.sort_by_key(|binding| {
        (
            binding.short_range.start_offset,
            binding.long_range.start_offset,
            binding.rule_id.clone(),
        )
    });
    output.dedup_by(|left, right| {
        left.short_range == right.short_range
            && left.long_range == right.long_range
            && left.predicate == right.predicate
    });
    output.truncate(MAX_BINDINGS);
    output
}

fn extract_prose_bindings<'a>(
    source: &'a str,
    language: crate::DocumentLanguage,
    citation_ranges: &[(usize, usize)],
) -> Vec<ByteBinding<'a>> {
    let mut output = Vec::new();
    for clause in segment_scientific_clauses(source, language, citation_ranges) {
        for captures in LONG_SHORT.captures_iter(clause.text) {
            let full = captures.get(0).unwrap();
            let long = captures.get(1).unwrap();
            let short = captures.get(2).unwrap();
            let Some((long_start, long_end)) = initialism_suffix(long.as_str(), short.as_str())
            else {
                continue;
            };
            push_byte_binding(
                &mut output,
                &clause,
                short.as_str(),
                long.as_str().get(long_start..long_end).unwrap(),
                clause.start + short.start(),
                clause.start + short.end(),
                clause.start + long.start() + long_start,
                clause.start + long.start() + long_end,
                clause.start + full.start(),
                clause.start + full.end(),
                "english-long-short-parenthetical",
            );
        }
        for captures in SHORT_LONG.captures_iter(clause.text) {
            let full = captures.get(0).unwrap();
            let short = captures.get(1).unwrap();
            let long = captures.get(2).unwrap();
            let Some((long_start, long_end)) = initialism_prefix(long.as_str(), short.as_str())
            else {
                continue;
            };
            push_byte_binding(
                &mut output,
                &clause,
                short.as_str(),
                long.as_str().get(long_start..long_end).unwrap(),
                clause.start + short.start(),
                clause.start + short.end(),
                clause.start + long.start() + long_start,
                clause.start + long.start() + long_end,
                clause.start + full.start(),
                clause.start + full.end(),
                "english-short-long-parenthetical",
            );
        }
        for (pattern, rule_id, reverse) in [
            (&*SHORT_DEFINES_LONG, "english-short-defines-long", false),
            (&*LONG_DEFINES_SHORT, "english-long-defines-short", true),
        ] {
            for captures in pattern.captures_iter(clause.text) {
                let full = captures.get(0).unwrap();
                let (short, long) = if reverse {
                    (captures.get(2).unwrap(), captures.get(1).unwrap())
                } else {
                    (captures.get(1).unwrap(), captures.get(2).unwrap())
                };
                let (long_start, long_end) = initialism_span(long.as_str(), short.as_str())
                    .unwrap_or_else(|| trim_long_phrase(long.as_str()));
                if long_end <= long_start {
                    continue;
                }
                push_byte_binding(
                    &mut output,
                    &clause,
                    short.as_str(),
                    long.as_str().get(long_start..long_end).unwrap(),
                    clause.start + short.start(),
                    clause.start + short.end(),
                    clause.start + long.start() + long_start,
                    clause.start + long.start() + long_end,
                    clause.start + full.start(),
                    clause.start + full.end(),
                    rule_id,
                );
            }
        }
        if output.len() >= MAX_BINDINGS {
            break;
        }
    }
    output.truncate(MAX_BINDINGS);
    output
}

#[allow(clippy::too_many_arguments)]
fn push_byte_binding<'a>(
    output: &mut Vec<ByteBinding<'a>>,
    clause: &crate::scientific_prose::ScientificClause<'a>,
    short: &'a str,
    long: &'a str,
    short_start: usize,
    short_end: usize,
    long_start: usize,
    long_end: usize,
    evidence_start: usize,
    evidence_end: usize,
    rule_id: &'static str,
) {
    if output.len() == MAX_BINDINGS || !valid_short(short) || !valid_long(long) {
        return;
    }
    let (polarity, modality) = claim_disposition(clause, evidence_start, evidence_end);
    output.push(ByteBinding {
        short,
        long,
        short_start,
        short_end,
        long_start,
        long_end,
        evidence_start,
        evidence_end,
        polarity,
        modality,
        rule_id,
    });
}

fn claim_disposition(
    clause: &crate::scientific_prose::ScientificClause<'_>,
    evidence_start: usize,
    evidence_end: usize,
) -> (EvidencePolarity, EvidenceModality) {
    let local_start = evidence_start.saturating_sub(clause.start);
    let local_end = evidence_end.saturating_sub(clause.start);
    let prefix = &clause.text[..local_start.min(clause.text.len())];
    let segment_start = [
        prefix.rfind(';'),
        prefix.to_ascii_lowercase().rfind(" but "),
    ]
    .into_iter()
    .flatten()
    .max()
    .map_or(0, |offset| offset + 1);
    let segment = clause.text[segment_start..local_end.min(clause.text.len())]
        .trim()
        .to_ascii_lowercase();
    let quote_count = clause.text[..local_start.min(clause.text.len())]
        .chars()
        .filter(|character| matches!(character, '"' | '“' | '”'))
        .count();
    if quote_count % 2 == 1 {
        return (EvidencePolarity::Positive, EvidenceModality::Quoted);
    }
    if [
        " does not ",
        " is not ",
        " never ",
        " not stand",
        " not mean",
    ]
    .iter()
    .any(|marker| format!(" {segment} ").contains(marker))
    {
        return (EvidencePolarity::Negative, EvidenceModality::Asserted);
    }
    if ["if ", "would ", "could ", "were "]
        .iter()
        .any(|marker| segment.starts_with(marker) || segment.contains(&format!(" {marker}")))
    {
        return (EvidencePolarity::Positive, EvidenceModality::Hypothetical);
    }
    if ["might ", "may ", "perhaps", "possibly", "seems to"]
        .iter()
        .any(|marker| segment.contains(marker))
    {
        return (EvidencePolarity::Positive, EvidenceModality::Hedged);
    }
    if ["according to", "as reported"]
        .iter()
        .any(|marker| segment.contains(marker))
    {
        return (EvidencePolarity::Positive, EvidenceModality::Cited);
    }
    (clause.frame.polarity, clause.frame.evidence_modality())
}

fn initialism_suffix(long: &str, short: &str) -> Option<(usize, usize)> {
    let words = word_ranges(long);
    (0..words.len()).rev().find_map(|start| {
        initials(long, &words[start..])
            .eq_ignore_ascii_case(&normalized_short(short))
            .then_some((words[start].0, words.last()?.1))
    })
}

fn initialism_prefix(long: &str, short: &str) -> Option<(usize, usize)> {
    let words = word_ranges(long);
    (2..=words.len()).find_map(|end| {
        initials(long, &words[..end])
            .eq_ignore_ascii_case(&normalized_short(short))
            .then_some((words[0].0, words[end - 1].1))
    })
}

fn initialism_span(long: &str, short: &str) -> Option<(usize, usize)> {
    let words = word_ranges(long);
    for start in 0..words.len() {
        for end in start + 2..=words.len() {
            if initials(long, &words[start..end]).eq_ignore_ascii_case(&normalized_short(short)) {
                return Some((words[start].0, words[end - 1].1));
            }
        }
    }
    None
}

fn initials(value: &str, words: &[(usize, usize)]) -> String {
    words
        .iter()
        .filter_map(|(start, end)| value[*start..*end].chars().next())
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
}

fn normalized_short(value: &str) -> String {
    value.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn word_ranges(value: &str) -> Vec<(usize, usize)> {
    let mut output = Vec::new();
    let mut start = None;
    for (offset, character) in value.char_indices() {
        if character.is_ascii_alphanumeric() {
            start.get_or_insert(offset);
        } else if let Some(word_start) = start.take() {
            output.push((word_start, offset));
        }
    }
    if let Some(word_start) = start {
        output.push((word_start, value.len()));
    }
    output
}

fn trim_long_phrase(value: &str) -> (usize, usize) {
    let mut start = 0;
    let mut end = value.len();
    while start < end
        && value[start..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        start += value[start..].chars().next().unwrap().len_utf8();
    }
    while start < end
        && value[..end]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        end -= value[..end].chars().next_back().unwrap().len_utf8();
    }
    (start, end)
}

fn valid_short(value: &str) -> bool {
    let normalized = normalized_short(value);
    (2..=12).contains(&normalized.len())
        && normalized
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        && normalized
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
}

fn valid_long(value: &str) -> bool {
    let words = word_ranges(value);
    (2..=12).contains(&words.len()) && value.len() <= 120
}

fn byte_range(index: &SourceIndex, start: usize, end: usize) -> SourceRange {
    SourceRange {
        start_offset: index.utf16_for_byte(start),
        end_offset: index.utf16_for_byte(end),
    }
}

fn append_structural_bindings(document: &ProjectDocument, output: &mut Vec<CrossModalBinding>) {
    for declaration in &document.declarations {
        if output.len() == MAX_BINDINGS {
            return;
        }
        match declaration {
            StructuralDeclaration::Acronym {
                short,
                long,
                short_source,
                long_source,
                source,
                state: MathRootState::Complete,
                ..
            } if short_source.file_id == document.file_id
                && long_source.file_id == document.file_id =>
            {
                output.push(structural_binding(
                    short,
                    long,
                    &short_source.range,
                    &long_source.range,
                    &source.range,
                    BindingPredicate::Abbreviates,
                    "latex-acronym-declaration",
                ));
            }
            StructuralDeclaration::Glossary {
                key,
                fields,
                source,
                state: MathRootState::Complete,
                ..
            } if source.file_id == document.file_id => {
                let short = fields
                    .iter()
                    .find(|field| field.name.eq_ignore_ascii_case("name"));
                let long = fields
                    .iter()
                    .find(|field| field.name.eq_ignore_ascii_case("description"));
                if let Some(long) = long {
                    let (short_value, short_range) = short.map_or_else(
                        || (key.as_str(), source.range.clone()),
                        |field| (field.value.as_str(), field.source.range.clone()),
                    );
                    output.push(structural_binding(
                        short_value,
                        &long.value,
                        &short_range,
                        &long.source.range,
                        &source.range,
                        BindingPredicate::Aliases,
                        "latex-glossary-declaration",
                    ));
                    if let Some(plural) = fields
                        .iter()
                        .find(|field| field.name.eq_ignore_ascii_case("plural"))
                    {
                        output.push(structural_binding(
                            &plural.value,
                            &long.value,
                            &plural.source.range,
                            &long.source.range,
                            &source.range,
                            BindingPredicate::Aliases,
                            "latex-glossary-plural",
                        ));
                    }
                }
            }
            StructuralDeclaration::Operator {
                name,
                surface,
                source,
                name_source,
                surface_source,
                state: MathRootState::Complete,
                ..
            } if source.file_id == document.file_id => {
                output.push(structural_binding(
                    surface,
                    name.trim_start_matches('\\'),
                    &surface_source.range,
                    &name_source.range,
                    &source.range,
                    BindingPredicate::Names,
                    "latex-math-operator-declaration",
                ));
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn structural_binding(
    short: &str,
    long: &str,
    short_range: &SourceRange,
    long_range: &SourceRange,
    evidence_range: &SourceRange,
    predicate: BindingPredicate,
    rule_id: &str,
) -> CrossModalBinding {
    let occurrence_kind = match predicate {
        BindingPredicate::Names => OccurrenceKind::MacroDeclaration,
        BindingPredicate::Abbreviates | BindingPredicate::Aliases => {
            OccurrenceKind::ResourceDeclaration
        }
    };
    CrossModalBinding {
        short: short.to_owned(),
        long: long.to_owned(),
        short_range: short_range.clone(),
        long_range: long_range.clone(),
        evidence_range: evidence_range.clone(),
        polarity: EvidencePolarity::Positive,
        modality: EvidenceModality::Asserted,
        predicate,
        occurrence_kind,
        rule_id: rule_id.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_operator_and_resource_declarations_by_semantic_kind() {
        let range = SourceRange {
            start_offset: 0,
            end_offset: 1,
        };
        let operator = structural_binding(
            "ECE",
            "expected calibration error",
            &range,
            &range,
            &range,
            BindingPredicate::Names,
            "operator",
        );
        let acronym = structural_binding(
            "ECE",
            "expected calibration error",
            &range,
            &range,
            &range,
            BindingPredicate::Abbreviates,
            "acronym",
        );

        assert_eq!(operator.occurrence_kind, OccurrenceKind::MacroDeclaration);
        assert_eq!(acronym.occurrence_kind, OccurrenceKind::ResourceDeclaration);
    }

    #[test]
    fn extracts_both_parenthetical_directions_without_absorbing_leading_prose() {
        let source =
            "Report expected calibration error (ECE). RMSE (root mean squared error) follows.";
        let bindings = extract_prose_bindings(source, crate::DocumentLanguage::Markdown, &[]);
        assert_eq!(bindings.len(), 2);
        assert_eq!(
            (bindings[0].long, bindings[0].short),
            ("expected calibration error", "ECE")
        );
        assert_eq!(
            (bindings[1].long, bindings[1].short),
            ("root mean squared error", "RMSE")
        );
    }

    #[test]
    fn trims_explicit_claim_context_when_the_initialism_identifies_the_name() {
        let source = "ECE means expected calibration error in this report.";
        let bindings = extract_prose_bindings(source, crate::DocumentLanguage::Markdown, &[]);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].long, "expected calibration error");
    }

    #[test]
    fn classifies_mixed_negative_and_positive_claim_spans_independently() {
        let source = "ECE does not mean electrical computer engineering; but ECE means expected calibration error.";
        let bindings = extract_prose_bindings(source, crate::DocumentLanguage::Markdown, &[]);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].polarity, EvidencePolarity::Negative);
        assert_eq!(bindings[1].polarity, EvidencePolarity::Positive);
        assert_eq!(bindings[1].modality, EvidenceModality::Asserted);
    }

    #[test]
    fn preserves_non_establishing_modalities_as_evidence_without_promotion() {
        for (source, modality) in [
            (
                "If ECE meant expected calibration error, continue.",
                EvidenceModality::Hypothetical,
            ),
            (
                "ECE might mean expected calibration error.",
                EvidenceModality::Hedged,
            ),
            (
                "According to the reference, ECE means expected calibration error.",
                EvidenceModality::Cited,
            ),
            (
                "The phrase \"ECE means expected calibration error\" is quoted.",
                EvidenceModality::Quoted,
            ),
        ] {
            let bindings = extract_prose_bindings(source, crate::DocumentLanguage::Markdown, &[]);
            assert_eq!(bindings.len(), 1, "{source}");
            assert_eq!(bindings[0].modality, modality, "{source}");
        }
    }

    #[test]
    fn refuses_parenthetical_pairs_without_initialism_evidence() {
        assert!(extract_prose_bindings(
            "Electrical computer engineering (ECE) is one meaning, but random words (ECE) are not.",
            crate::DocumentLanguage::Markdown,
            &[],
        )
        .iter()
        .all(|binding| binding.long != "random words"));
    }
}
