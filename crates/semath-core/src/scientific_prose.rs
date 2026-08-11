use crate::DocumentLanguage;
use crate::semantic_index::{EvidenceModality, EvidencePolarity};

const MAX_CLAUSE_BYTES: usize = 640;
const MAX_CLAUSES: usize = 256;
const MAX_ASSUMPTIONS_PER_CLAUSE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommunicativeAct {
    Statement,
    Definition,
    Assumption,
    Result,
    Relation,
    Alternative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Attribution {
    Author,
    Cited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Conditionality {
    Unconditional,
    Conditional,
    Counterfactual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscourseFeatureKind {
    Act,
    Polarity,
    Modality,
    Attribution,
    Conditionality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscourseFeatureEvidence {
    pub kind: DiscourseFeatureKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscourseFrame {
    pub act: CommunicativeAct,
    pub polarity: EvidencePolarity,
    pub modality: EvidenceModality,
    pub attribution: Attribution,
    pub conditionality: Conditionality,
    pub evidence: Vec<DiscourseFeatureEvidence>,
}

impl DiscourseFrame {
    pub(crate) fn establishes(&self) -> bool {
        self.polarity == EvidencePolarity::Positive
            && self.modality == EvidenceModality::Asserted
            && self.attribution == Attribution::Author
            && self.conditionality == Conditionality::Unconditional
            && self.act != CommunicativeAct::Alternative
    }

    pub(crate) fn evidence_modality(&self) -> EvidenceModality {
        if self.conditionality != Conditionality::Unconditional {
            EvidenceModality::Hypothetical
        } else if self.modality != EvidenceModality::Asserted {
            self.modality
        } else if self.attribution == Attribution::Cited {
            EvidenceModality::Cited
        } else {
            EvidenceModality::Asserted
        }
    }
}

pub(crate) fn asserted_author_frame() -> DiscourseFrame {
    DiscourseFrame {
        act: CommunicativeAct::Statement,
        polarity: EvidencePolarity::Positive,
        modality: EvidenceModality::Asserted,
        attribution: Attribution::Author,
        conditionality: Conditionality::Unconditional,
        evidence: Vec::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScientificClause<'a> {
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
    pub frame: DiscourseFrame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScientificMention {
    pub symbol: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssumptionCandidate {
    pub kind: String,
    pub value: String,
    pub subjects: Vec<ScientificMention>,
    pub phrase_start: usize,
    pub phrase_end: usize,
}

pub(crate) fn segment_scientific_clauses<'a>(
    source: &'a str,
    language: DocumentLanguage,
    citation_ranges: &[(usize, usize)],
) -> Vec<ScientificClause<'a>> {
    let mut ranges = Vec::new();
    let mut byte_offset = 0;
    let mut fenced = false;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if language == DocumentLanguage::Markdown && trimmed.starts_with("```") {
            fenced = !fenced;
            byte_offset += line.len();
            continue;
        }
        if fenced {
            byte_offset += line.len();
            continue;
        }
        let visible = if language == DocumentLanguage::Latex {
            line.split('%').next().unwrap_or("")
        } else {
            line
        };
        split_visible_line(byte_offset, visible, &mut ranges);
        byte_offset += line.len();
        if ranges.len() >= MAX_CLAUSES {
            break;
        }
    }
    ranges
        .into_iter()
        .take(MAX_CLAUSES)
        .filter_map(|(start, end)| {
            let (start, end) = trim_range(source, start, end);
            (start < end && end - start <= MAX_CLAUSE_BYTES).then(|| {
                let text = &source[start..end];
                ScientificClause {
                    start,
                    end,
                    text,
                    frame: classify_discourse_frame(text, start, end, citation_ranges),
                }
            })
        })
        .collect()
}

pub(crate) fn extract_assumptions(
    clause: &ScientificClause<'_>,
    mentions: &[ScientificMention],
) -> Vec<AssumptionCandidate> {
    if !clause.frame.establishes() {
        return Vec::new();
    }
    let lower = clause.text.to_ascii_lowercase().replace('-', " ");
    let mut matches = assumption_phrases()
        .iter()
        .filter_map(|(phrase, kind, value)| {
            let offset = lower.find(phrase)?;
            let prefix = &lower[..offset];
            if negates_phrase(prefix) {
                return None;
            }
            let phrase_start = clause.start + offset;
            let phrase_end = phrase_start + phrase.len();
            let subjects = nearest_subjects(mentions, clause, phrase_start);
            Some((
                offset,
                phrase.len(),
                AssumptionCandidate {
                    kind: (*kind).into(),
                    value: (*value).into(),
                    subjects,
                    phrase_start,
                    phrase_end,
                },
            ))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)));
    let mut occupied = Vec::<(usize, usize)>::new();
    matches
        .into_iter()
        .filter_map(|(start, length, candidate)| {
            let end = start + length;
            (!occupied
                .iter()
                .any(|(used_start, used_end)| start < *used_end && *used_start < end))
            .then(|| {
                occupied.push((start, end));
                candidate
            })
        })
        .take(MAX_ASSUMPTIONS_PER_CLAUSE)
        .collect()
}

pub(crate) fn clause_at<'a>(
    clauses: &'a [ScientificClause<'a>],
    byte_offset: usize,
) -> Option<&'a ScientificClause<'a>> {
    clauses
        .iter()
        .find(|clause| clause.start <= byte_offset && byte_offset < clause.end)
}

pub(crate) fn align_ordered_descriptions(value: &str, arity: usize) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_math = false;
    let mut brace_depth = 0usize;
    let mut parenthesis_depth = 0usize;
    for (offset, character) in value.char_indices() {
        match character {
            '$' => in_math = !in_math,
            '{' if !in_math => brace_depth += 1,
            '}' if !in_math => brace_depth = brace_depth.saturating_sub(1),
            '(' if !in_math => parenthesis_depth += 1,
            ')' if !in_math => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            ',' if !in_math && brace_depth == 0 && parenthesis_depth == 0 => {
                parts.push(&value[start..offset]);
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    if parts.len() != arity {
        if arity == 2 && parts.len() == 1 {
            let normalized = parts[0].trim();
            let split = normalized.rfind(" and ")?;
            parts = vec![&normalized[..split], &normalized[split + 5..]];
        } else {
            return None;
        }
    }
    let last = parts.last_mut()?;
    *last = last.trim().strip_prefix("and ").unwrap_or(last.trim());
    let normalized = parts
        .into_iter()
        .map(|part| strip_article(part.trim()))
        .collect::<Vec<_>>();
    normalized
        .iter()
        .all(|part| !part.is_empty())
        .then_some(normalized)
}

fn split_visible_line(line_start: usize, line: &str, ranges: &mut Vec<(usize, usize)>) {
    let mut start = 0;
    let mut in_math = false;
    for (index, character) in line.char_indices() {
        if character == '$' {
            in_math = !in_math;
        }
        if !in_math && matches!(character, '.' | '!' | '?' | ';' | '\n') {
            let end = index + character.len_utf8();
            ranges.push((line_start + start, line_start + end));
            start = end;
        }
    }
    if start < line.len() {
        ranges.push((line_start + start, line_start + line.len()));
    }
}

fn classify_discourse_frame(
    text: &str,
    clause_start: usize,
    clause_end: usize,
    citation_ranges: &[(usize, usize)],
) -> DiscourseFrame {
    let lower = text.trim().to_ascii_lowercase();
    let words = lower
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let mut evidence = Vec::new();
    let (conditionality, conditional_marker) =
        if starts_with_any(&lower, &["were ", "had ", "imagine ", "counterfactually"])
            || words
                .iter()
                .any(|word| matches!(*word, "would" | "were" | "had"))
        {
            (
                Conditionality::Counterfactual,
                first_marker(
                    &lower,
                    &["were", "had", "imagine", "counterfactually", "would"],
                ),
            )
        } else if starts_with_any(&lower, &["if ", "when ", "provided ", "assuming "])
            || words.contains(&"could")
        {
            (
                Conditionality::Conditional,
                first_marker(&lower, &["if", "when", "provided", "assuming", "could"]),
            )
        } else {
            (Conditionality::Unconditional, None)
        };
    push_marker_evidence(
        &mut evidence,
        DiscourseFeatureKind::Conditionality,
        clause_start,
        conditional_marker,
    );

    let hedged_marker = words
        .iter()
        .find(|word| {
            matches!(
                **word,
                "might" | "may" | "perhaps" | "possibly" | "apparently"
            )
        })
        .and_then(|word| lower.find(word).map(|start| (start, start + word.len())))
        .or_else(|| first_marker(&lower, &["seems to", "appears to", "is likely to"]));
    let modality = if hedged_marker.is_some() {
        EvidenceModality::Hedged
    } else {
        EvidenceModality::Asserted
    };
    push_marker_evidence(
        &mut evidence,
        DiscourseFeatureKind::Modality,
        clause_start,
        hedged_marker,
    );

    let negative_marker = if starts_with_any(
        &lower,
        &["not ", "do not ", "does not ", "we do not ", "never "],
    ) || lower.starts_with("without ")
        || lower.contains(" is not ")
        || lower.contains(" are not ")
        || lower.contains(" must not ")
        || [" not define", " not denote", " not represent", " not mean"]
            .iter()
            .any(|marker| lower.contains(marker))
    {
        first_marker(&lower, &["not", "never", "without"])
    } else {
        None
    };
    let polarity = if negative_marker.is_some() {
        EvidencePolarity::Negative
    } else {
        EvidencePolarity::Positive
    };
    push_marker_evidence(
        &mut evidence,
        DiscourseFeatureKind::Polarity,
        clause_start,
        negative_marker,
    );

    let citation = citation_ranges
        .iter()
        .copied()
        .find(|(start, end)| *start < clause_end && clause_start < *end);
    let lexical_attribution =
        first_marker(&lower, &["according to", "as reported"]).or_else(|| {
            starts_with_any(&lower, &["the citation ", "the reference "])
                .then(|| first_marker(&lower, &["the citation", "the reference"]))
                .flatten()
        });
    let attribution = if citation.is_some() || lexical_attribution.is_some() {
        Attribution::Cited
    } else {
        Attribution::Author
    };
    if let Some((start, end)) = citation {
        evidence.push(DiscourseFeatureEvidence {
            kind: DiscourseFeatureKind::Attribution,
            start,
            end,
        });
    } else {
        push_marker_evidence(
            &mut evidence,
            DiscourseFeatureKind::Attribution,
            clause_start,
            lexical_attribution,
        );
    }

    let (act, act_marker) = classify_act(&lower);
    push_marker_evidence(
        &mut evidence,
        DiscourseFeatureKind::Act,
        clause_start,
        act_marker,
    );

    DiscourseFrame {
        act,
        polarity,
        modality,
        attribution,
        conditionality,
        evidence,
    }
}

fn classify_act(lower: &str) -> (CommunicativeAct, Option<(usize, usize)>) {
    if let Some(marker) = first_marker(lower, &["alternatively", "otherwise", "either"]) {
        return (CommunicativeAct::Alternative, Some(marker));
    }
    const DEFINITIONS: &[&str] = &[
        "denote",
        "represent",
        "stand for",
        "define",
        "write",
        "call",
        "mean",
    ];
    const ASSUMPTIONS: &[&str] = &["assume", "suppose", "given", "subject to", "take"];
    const RESULTS: &[&str] = &["therefore", "thus", "hence", "we obtain", "it follows"];
    const RELATIONS: &[&str] = &["compared with", "in contrast", "whereas", "while"];
    for (act, markers) in [
        (CommunicativeAct::Definition, DEFINITIONS),
        (CommunicativeAct::Assumption, ASSUMPTIONS),
        (CommunicativeAct::Result, RESULTS),
        (CommunicativeAct::Relation, RELATIONS),
    ] {
        if let Some(marker) = first_marker(lower, markers) {
            return (act, Some(marker));
        }
    }
    (CommunicativeAct::Statement, None)
}

fn first_marker(value: &str, markers: &[&str]) -> Option<(usize, usize)> {
    markers
        .iter()
        .filter_map(|marker| {
            value
                .find(marker)
                .map(|start| (start, start + marker.len()))
        })
        .min_by_key(|(start, _)| *start)
}

fn push_marker_evidence(
    evidence: &mut Vec<DiscourseFeatureEvidence>,
    kind: DiscourseFeatureKind,
    clause_start: usize,
    marker: Option<(usize, usize)>,
) {
    if let Some((start, end)) = marker {
        evidence.push(DiscourseFeatureEvidence {
            kind,
            start: clause_start + start,
            end: clause_start + end,
        });
    }
}

fn assumption_phrases() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        ("strictly positive", "sign", "strictly-positive"),
        ("non-zero", "sign", "nonzero"),
        ("nonzero", "sign", "nonzero"),
        ("nonnegative", "sign", "nonnegative"),
        (
            "positive semidefinite",
            "definiteness",
            "positive-semidefinite",
        ),
        ("positive definite", "definiteness", "positive-definite"),
        ("negative definite", "definiteness", "negative-definite"),
        ("symmetric", "structure", "symmetric"),
        ("continuous", "regularity", "continuous"),
        ("differentiable", "regularity", "differentiable"),
        ("invertible", "algebraic-property", "invertible"),
        ("independent", "statistical-relation", "independent"),
        ("steady state", "regime", "steady-state"),
        ("steady-state", "regime", "steady-state"),
        ("small signal", "regime", "small-signal"),
        ("small-signal", "regime", "small-signal"),
        ("time invariant", "regime", "time-invariant"),
        ("time-invariant", "regime", "time-invariant"),
        ("idealized", "regime", "idealized"),
        ("ideal ", "regime", "ideal"),
        ("positive", "sign", "positive"),
    ]
}

fn nearest_subjects(
    mentions: &[ScientificMention],
    clause: &ScientificClause<'_>,
    phrase_start: usize,
) -> Vec<ScientificMention> {
    let mut candidates = mentions
        .iter()
        .filter(|mention| clause.start <= mention.start && mention.end <= clause.end)
        .cloned()
        .collect::<Vec<_>>();
    let preceding = candidates
        .iter()
        .filter(|mention| mention.end <= phrase_start)
        .map(|mention| mention.end)
        .max();
    if let Some(nearest) = preceding {
        candidates.retain(|mention| mention.end <= phrase_start && nearest - mention.end <= 96);
    }
    candidates.truncate(4);
    candidates
}

fn negates_phrase(prefix: &str) -> bool {
    prefix
        .split_whitespace()
        .rev()
        .take(3)
        .any(|word| matches!(word, "not" | "never" | "neither" | "without"))
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

fn trim_range(source: &str, mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end
        && source[start..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        start += source[start..].chars().next().unwrap().len_utf8();
    }
    while start < end
        && source[..end]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        end -= source[..end].chars().next_back().unwrap().len_utf8();
    }
    (start, end)
}

fn strip_article(value: &str) -> &str {
    ["a ", "an ", "the "]
        .into_iter()
        .find_map(|article| value.strip_prefix(article))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_visible_clauses_and_classifies_non_evidence() {
        let source = "Assume $A$ is symmetric. If $B$ were invertible, continue. $C$ might denote a matrix. Without adopting the convention, inspect $D$. % Assume steady state\n```\nAssume idealized operation.\n```";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Markdown, &[]);
        assert_eq!(clauses.len(), 5);
        assert!(clauses[0].frame.establishes());
        assert_eq!(
            clauses[1].frame.conditionality,
            Conditionality::Counterfactual
        );
        assert_eq!(clauses[2].frame.modality, EvidenceModality::Hedged);
        assert_eq!(clauses[3].frame.polarity, EvidencePolarity::Negative);
    }

    #[test]
    fn aligns_arbitrary_arity_without_splitting_nested_descriptions() {
        assert_eq!(
            align_ordered_descriptions("input, state (in $R^n$), and output", 3),
            Some(vec!["input", "state (in $R^n$)", "output"]),
        );
        assert_eq!(align_ordered_descriptions("input and output", 3), None);
    }

    #[test]
    fn extracts_bounded_assumptions_with_subject_spans_and_refuses_negation() {
        let source = "Assume $A$ is symmetric and positive definite.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let mentions = vec![ScientificMention {
            symbol: "A".into(),
            start: 7,
            end: 10,
        }];
        let assumptions = extract_assumptions(&clause, &mentions);
        assert_eq!(assumptions.len(), 2);
        assert!(
            assumptions
                .iter()
                .all(|item| item.subjects[0].symbol == "A")
        );
        let symmetric = assumptions
            .iter()
            .find(|item| item.value == "symmetric")
            .unwrap();
        assert_eq!(
            &source[symmetric.phrase_start..symmetric.phrase_end],
            "symmetric"
        );
        assert_eq!(
            &source[symmetric.subjects[0].start..symmetric.subjects[0].end],
            "$A$"
        );

        let negated = "Assume $A$ is not symmetric.";
        let clause = segment_scientific_clauses(negated, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        assert!(extract_assumptions(&clause, &mentions).is_empty());
    }

    #[test]
    fn preserves_cited_hedged_negated_and_conditional_features_together() {
        let source = "If prior work      might not define $A$ as a matrix.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[(14, 19)]);
        let frame = &clauses[0].frame;

        assert_eq!(frame.conditionality, Conditionality::Conditional);
        assert_eq!(frame.attribution, Attribution::Cited);
        assert_eq!(frame.modality, EvidenceModality::Hedged);
        assert_eq!(frame.polarity, EvidencePolarity::Negative);
        assert_eq!(frame.act, CommunicativeAct::Definition);
        assert!(!frame.establishes());
        assert!(frame.evidence.len() >= 5);
    }
}
