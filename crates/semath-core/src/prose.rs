use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::canonical::{
    SemanticExpr, SemanticExprKind, declared_symbols, expression_children, expression_name,
    lower_document_region, relation_head_symbol,
};
use crate::concept::classify_role;
use crate::construction::{
    coordinated_descriptions, coordination_lead, defines_by_formula, fronted_labeled_descriptions,
    fronted_shared_description, is_declaration_lead, match_apposition, match_definition,
    match_parenthetical, match_passive_definition, match_quantified, role_first_nominal_candidates,
};
use crate::pack::{PackActivationStructure, PackConditionKind, built_in_packs};
use crate::parser::ParsedMath;
use crate::scientific_prose::{
    AnaphorKind, CommunicativeAct, DefinitionAction, DiscourseConnective, DiscourseConstruction,
    DiscourseFrame, MAX_ATTACHMENT_DISTANCE_BYTES, ProseEvent, ProseEventKind, ProseEventStream,
    ScientificClause, ScientificMention, align_ordered_descriptions, clause_at,
    extract_assumptions_with_phrases, normalize_prose_events, segment_scientific_clauses,
};
use crate::scope::{AttachmentGraph, ScopeGraph, scope_visible};
use crate::{
    AssumptionInfo, DefinitionInfo, Evidence, Location, ProjectDocument, SourceIndex, SourceRange,
};

static VECTOR_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)[ -]dimensional\s*(?:[a-z][a-z-]*\s+){0,4}(?:vectors?|states?|inputs?|controls?)(?:\s+of\s+[a-z -]+)?\s*(?:,?\s+and)?\s*$")
        .unwrap()
});
static COMPACT_VECTOR_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)[ -]vectors?(?:\s+of\s+[a-z -]+)?\s*(?:,?\s+and)?\s*$")
        .unwrap()
});
static MATRIX_DIMENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)(?:\s+by\s+|\s+x\s+|\s*×\s*|\s*\\times\s*)([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)(?:\s+(?:[a-z][a-z-]*\s+){0,4}(?:matrix|matrices))?\s*(?:,?\s+and)?\s*$",
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

pub(crate) fn definition_available_from(definition: &DefinitionInfo) -> u32 {
    let attached = definition.evidence.kind == "attached-prose";
    let ranges = definition.evidence.source_ranges.iter();
    if attached {
        ranges.map(|range| range.start_offset).min()
    } else {
        ranges.map(|range| range.end_offset).max()
    }
    .unwrap_or(definition.location.range.end_offset)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProseObservations {
    pub definitions: Vec<DefinitionInfo>,
    pub shapes: Vec<ProseShapeClaim>,
    pub assumptions: Vec<AssumptionInfo>,
    pub semantic_evidence: ScientificSemanticEvidence,
    pub match_stats: ProseMatchStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProseMatchStats {
    pub clauses: u32,
    pub construction_candidates: u32,
    pub matcher_work: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScientificClauseEvidence {
    pub range: SourceRange,
    pub frame: DiscourseFrame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomainPriorEvidence {
    pub pack_id: String,
    pub pack_version: String,
    pub title: String,
    pub frame: DiscourseFrame,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LawActivationEvidence {
    pub pack_id: String,
    pub law_id: String,
    pub clause_range: SourceRange,
    pub attached_formula_ranges: Vec<SourceRange>,
    pub identifies_attached_formula: bool,
    pub frame: DiscourseFrame,
    pub evidence: Evidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FormulaOperationKind {
    VectorDotProduct,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormulaOperationEvidence {
    pub clause_range: SourceRange,
    pub operation: FormulaOperationKind,
    pub frame: DiscourseFrame,
    pub evidence: Evidence,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScientificSemanticEvidence {
    pub clauses: Vec<ScientificClauseEvidence>,
    pub domain_priors: Vec<DomainPriorEvidence>,
    pub law_activations: Vec<LawActivationEvidence>,
    pub formula_operations: Vec<FormulaOperationEvidence>,
    attachment: AttachmentGraph,
}

impl ScientificSemanticEvidence {
    pub(crate) fn formula_disposition(
        &self,
        range: &SourceRange,
    ) -> (
        crate::semantic_index::EvidencePolarity,
        crate::semantic_index::EvidenceModality,
    ) {
        self.formula_frame(range).map_or(
            (
                crate::semantic_index::EvidencePolarity::Positive,
                crate::semantic_index::EvidenceModality::Asserted,
            ),
            |clause| (clause.frame.polarity, clause.frame.evidence_modality()),
        )
    }

    pub fn formula_is_asserted(&self, range: &SourceRange) -> bool {
        self.formula_disposition(range)
            == (
                crate::semantic_index::EvidencePolarity::Positive,
                crate::semantic_index::EvidenceModality::Asserted,
            )
    }

    pub(crate) fn formula_is_rejected(&self, range: &SourceRange) -> bool {
        self.formula_disposition(range).0 == crate::semantic_index::EvidencePolarity::Negative
    }

    pub fn law_activation(
        &self,
        pack_id: &str,
        law_id: &str,
        range: &SourceRange,
    ) -> Option<&LawActivationEvidence> {
        let matching = |activation: &&LawActivationEvidence| {
            activation.pack_id == pack_id
                && activation.law_id == law_id
                && activation.frame.establishes()
        };
        if let Some(activation) = self
            .law_activations
            .iter()
            .filter(matching)
            .filter(|activation| {
                ranges_overlap(&activation.clause_range, range)
                    && self.attachment.permits(&activation.clause_range, range)
            })
            .max_by_key(|activation| activation.identifies_attached_formula)
        {
            return Some(activation);
        }

        if let Some(activation) = self
            .law_activations
            .iter()
            .filter(matching)
            .filter(|activation| {
                activation
                    .attached_formula_ranges
                    .iter()
                    .any(|target| ranges_overlap(target, range))
            })
            .max_by_key(|activation| activation.identifies_attached_formula)
        {
            return Some(activation);
        }

        // Scientific prose commonly names a law or model, declares its roles,
        // and puts the asserted equation in the immediately following clause.
        // Keep this attachment local: do not let a law name leak across another
        // clause, section, or unrelated formula later in the document.
        let formula_start = self
            .clause_for(range)
            .map_or(range.start_offset, |clause| clause.range.start_offset);
        let previous_clause = self
            .clauses
            .iter()
            .filter(|clause| {
                clause.range.end_offset <= formula_start
                    && clause
                        .range
                        .end_offset
                        .saturating_sub(clause.range.start_offset)
                        > 1
            })
            .max_by_key(|clause| clause.range.end_offset)?;
        self.law_activations
            .iter()
            .filter(matching)
            .filter(|activation| {
                activation.clause_range == previous_clause.range
                    && previous_clause.frame.establishes()
                    && self.attachment.permits(&activation.clause_range, range)
            })
            .max_by_key(|activation| activation.identifies_attached_formula)
    }

    pub fn formula_operations(
        &self,
        range: &SourceRange,
    ) -> impl Iterator<Item = &FormulaOperationEvidence> {
        self.formula_operations.iter().filter(|operation| {
            operation.frame.establishes() && ranges_overlap(&operation.clause_range, range)
        })
    }

    fn clause_for(&self, range: &SourceRange) -> Option<&ScientificClauseEvidence> {
        self.clauses
            .iter()
            .filter(|clause| ranges_overlap(&clause.range, range))
            .min_by_key(|clause| clause.range.end_offset - clause.range.start_offset)
    }

    fn formula_frame(&self, range: &SourceRange) -> Option<&ScientificClauseEvidence> {
        let direct = self.clause_for(range);
        if direct.is_some_and(|clause| !clause.frame.evidence.is_empty()) {
            return direct;
        }
        self.clauses
            .iter()
            .filter(|clause| {
                clause.range.end_offset <= range.start_offset
                    && range.start_offset - clause.range.end_offset <= 32
                    && !clause.frame.evidence.is_empty()
                    && self.attachment.permits(&clause.range, range)
            })
            .max_by_key(|clause| clause.range.end_offset)
            .or(direct)
    }
}

fn primary_symbol(document: &ProjectDocument, math: &ParsedMath) -> Option<(String, SourceRange)> {
    declared_symbols(document, &math.region.content_range)
        .into_iter()
        .next()
        .or_else(|| {
            math.symbols
                .first()
                .map(|(symbol, range)| (symbol.trim_start_matches('\\').to_owned(), range.clone()))
        })
}

pub(crate) fn observe_prose(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    canonical_expressions: &[SemanticExpr],
) -> ProseObservations {
    let visible_source = visible_prose_source(document);
    let source = visible_source.as_ref();
    let index = SourceIndex::new(source);
    let mut analysis = ProseObservations::default();
    let citation_ranges = citation_byte_ranges(document, &index);
    let clauses = segment_scientific_clauses(source, document.language, &citation_ranges);
    analysis.match_stats.clauses = clauses.len() as u32;
    let mentions = parsed
        .iter()
        .enumerate()
        .filter_map(|(math_index, math)| {
            let (symbol, _) = primary_symbol(document, math)?;
            Some(ScientificMention {
                symbol,
                start: index.byte_for_utf16(math.region.full_range.start_offset),
                end: index.byte_for_utf16(math.region.full_range.end_offset),
                math_index,
            })
        })
        .collect::<Vec<_>>();
    let events = normalize_prose_events(source, &clauses, &mentions);
    let discourse_constructions = events.discourse_constructions(source, &mentions, &clauses);
    let construction_targets = construction_formula_targets(
        document,
        &index,
        &clauses,
        &events,
        &discourse_constructions,
        &mentions,
    );
    analysis.semantic_evidence = collect_semantic_evidence(
        document,
        source,
        &index,
        &clauses,
        canonical_expressions,
        &construction_targets,
    );
    analysis.match_stats.matcher_work += analysis.semantic_evidence.attachment.candidate_edges();
    collect_assumptions(source, &index, &clauses, &mentions, &events, &mut analysis);

    collect_role_first_nominal_definitions(
        document,
        source,
        parsed,
        canonical_expressions,
        &index,
        &clauses,
        &mentions,
        &events,
        &mut analysis,
    );

    collect_coordinated_definitions(document, source, parsed, &index, &clauses, &mut analysis);
    collect_anaphoric_definitions(
        document,
        parsed,
        &index,
        &clauses,
        &mentions,
        &events,
        &discourse_constructions,
        &mut analysis,
    );
    collect_equation_flow_definitions(
        document,
        source,
        parsed,
        &index,
        &clauses,
        &mentions,
        &events,
        &discourse_constructions,
        &mut analysis,
    );
    collect_clause_definitions(
        document,
        source,
        parsed,
        &index,
        &clauses,
        &events,
        &mut analysis,
    );
    for (math_index, math) in parsed.iter().enumerate() {
        analysis.match_stats.matcher_work += 1;
        let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
            continue;
        };
        let start_byte = index.byte_for_utf16(math.region.full_range.start_offset);
        let end_byte = index.byte_for_utf16(math.region.full_range.end_offset);
        let clause = clause_at(&clauses, start_byte);
        if clause.is_some_and(|clause| !clause.frame.establishes()) {
            continue;
        }
        let before_start = bounded_start(source, start_byte, 160);
        let after_end = clause
            .and_then(|clause| {
                parsed
                    .iter()
                    .filter_map(|candidate| {
                        let candidate_start =
                            index.byte_for_utf16(candidate.region.full_range.start_offset);
                        (end_byte < candidate_start
                            && candidate_start < clause.end
                            && !is_description_parameter(
                                source,
                                candidate,
                                clause.start,
                                clause.end,
                                &index,
                            ))
                        .then_some(candidate_start)
                    })
                    .min()
            })
            .unwrap_or_else(|| bounded_end(source, end_byte, 240));
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

        if defines_by_formula(before, after) {
            let evidence_end = end_byte
                + after
                    .to_ascii_lowercase()
                    .find("by")
                    .map_or(0, |offset| offset + 2);
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                "function",
                "english-formula-definition",
                before_start,
                evidence_end,
            );
        } else if let Some(passive) = match_passive_definition(before, after) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                passive.description,
                passive.rule_id,
                before_start + passive.prefix_start,
                end_byte + passive.suffix_end,
            );
        } else if math.symbols.len() > 1
            && let Some((description, prefix_start, suffix_end)) =
                fronted_shared_description(before, after)
        {
            for (listed_symbol, listed_range) in &math.symbols {
                push_claim(
                    &mut analysis,
                    document,
                    &index,
                    listed_symbol,
                    listed_range,
                    description,
                    "english-fronted-inline-list-definition",
                    before_start + prefix_start,
                    end_byte + suffix_end,
                );
            }
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
        } else if let Some(active) = mentions
            .iter()
            .position(|mention| mention.math_index == math_index)
            .and_then(|mention_index| {
                discourse_constructions.iter().find_map(|construction| {
                    let DiscourseConstruction::Definition(definition) = construction else {
                        return None;
                    };
                    (definition.mention_index == mention_index).then_some(definition)
                })
            })
        {
            if !active.frame.establishes() || active.coordinated {
                continue;
            }
            let rule_id = match active.action {
                DefinitionAction::Call
                | DefinitionAction::Define
                | DefinitionAction::Denote
                | DefinitionAction::Represent
                | DefinitionAction::Mean => "english-relational-definition",
                DefinitionAction::Write | DefinitionAction::Compute | DefinitionAction::Produce => {
                    unreachable!()
                }
            };
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                &source[active.description_start..active.description_end],
                rule_id,
                active.evidence_start,
                active.evidence_end,
            );
        } else if let Some(apposition) = match_apposition(after) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                apposition.description,
                apposition.rule_id,
                start_byte,
                end_byte + apposition.suffix_end,
            );
        } else if let Some(parenthetical) = match_parenthetical(before, after) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                parenthetical.description,
                parenthetical.rule_id,
                before_start + parenthetical.prefix_start,
                end_byte + parenthetical.suffix_end,
            );
        } else if let Some(quantified) = match_quantified(before, after) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                quantified.description,
                quantified.rule_id,
                before_start + quantified.prefix_start,
                end_byte + quantified.suffix_end,
            );
        } else if is_declaration_lead(before) && math.symbols.len() > 1 {
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
    let document_index = SourceIndex::new(&document.content);
    attach_equation_reference_definitions(
        &document.content,
        &document_index,
        parsed,
        &clauses,
        &mut analysis,
    );
    deduplicate(&mut analysis);
    analysis
}

#[allow(clippy::too_many_arguments)]
fn collect_role_first_nominal_definitions(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    canonical_expressions: &[SemanticExpr],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    events: &ProseEventStream,
    output: &mut ProseObservations,
) {
    for (clause_index, clause) in clauses.iter().enumerate() {
        if !clause.frame.establishes() {
            continue;
        }
        for mention_index in events.mentions_in_clause(clause_index) {
            output.match_stats.matcher_work += 1;
            let Some(mention) = mentions.get(*mention_index) else {
                continue;
            };
            let Some(math) = parsed
                .get(mention.math_index)
                .filter(|math| is_definition_slot_math(math))
            else {
                continue;
            };
            if !canonical_expressions
                .get(mention.math_index)
                .is_some_and(is_nominal_mention)
            {
                continue;
            }
            let Some((span_start, span_end)) =
                events.description_before(source, clause_index, mention.start)
            else {
                continue;
            };
            let candidates = role_first_nominal_candidates(&source[span_start..span_end]);
            let Some(base) = candidates.last() else {
                continue;
            };
            // An explicit verb before the noun phrase (for example “denotes”)
            // owns its coordinated clause. A word inside the noun phrase can
            // share that spelling (“mean speed”) without becoming an action.
            if events
                .last_definition_action(span_start, span_end)
                .is_some_and(|action| {
                    matches!(
                        action.kind,
                        ProseEventKind::DefinitionAction(
                            DefinitionAction::Define
                                | DefinitionAction::Denote
                                | DefinitionAction::Represent
                                | DefinitionAction::Write
                                | DefinitionAction::Call
                        )
                    ) || matches!(
                        action.kind,
                        ProseEventKind::DefinitionAction(DefinitionAction::Mean)
                    ) && action.end <= span_start + base.relative_start
                })
            {
                continue;
            }
            let Some(role) = classify_role(base.description) else {
                continue;
            };
            let selected = candidates
                .iter()
                .find(|candidate| classify_role(candidate.description).as_deref() == Some(&role))
                .unwrap_or(base);
            let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
                continue;
            };
            push_claim(
                output,
                document,
                index,
                &symbol,
                &symbol_range,
                selected.description,
                "english-role-first-nominal-definition",
                span_start + selected.relative_start,
                mention.end,
            );
        }
    }
}

fn is_nominal_mention(expression: &SemanticExpr) -> bool {
    matches!(
        expression.kind,
        SemanticExprKind::Symbol(_)
            | SemanticExprKind::Index { .. }
            | SemanticExprKind::Apply { .. }
    )
}

fn attach_equation_reference_definitions(
    source: &str,
    index: &SourceIndex,
    parsed: &[ParsedMath],
    clauses: &[ScientificClause<'_>],
    analysis: &mut ProseObservations,
) {
    let mut search_from = 0;
    while let Some(relative) = source[search_from..].find("\\label{") {
        let label_start = search_from + relative;
        let key_start = label_start + "\\label{".len();
        let Some(relative_end) = source[key_start..].find('}') else {
            break;
        };
        let key_end = key_start + relative_end;
        let key = &source[key_start..key_end];
        search_from = key_end + 1;
        let label_offset = index.utf16_for_byte(label_start);
        let Some(formula) = parsed.iter().find(|math| {
            math.region.full_range.start_offset <= label_offset
                && label_offset < math.region.full_range.end_offset
        }) else {
            continue;
        };
        let Some(reference_relative) = [format!("\\ref{{{key}}}"), format!("\\eqref{{{key}}}")]
            .iter()
            .filter_map(|reference| source[search_from..].find(reference))
            .min()
        else {
            continue;
        };
        let reference_start = search_from + reference_relative;
        let Some(clause) = clause_at(clauses, reference_start) else {
            continue;
        };
        let clause_range = SourceRange {
            start_offset: index.utf16_for_byte(clause.start),
            end_offset: index.utf16_for_byte(clause.end),
        };
        for definition in &mut analysis.definitions {
            if !definition
                .evidence
                .source_ranges
                .iter()
                .any(|range| ranges_overlap(range, &clause_range))
            {
                continue;
            }
            definition.evidence.rule_id = "english-equation-reference-definition".into();
            definition.evidence.kind = "attached-prose".into();
            definition
                .evidence
                .source_ranges
                .push(formula.region.content_range.clone());
            definition
                .evidence
                .source_ranges
                .sort_by_key(|range| (range.start_offset, range.end_offset));
            definition.evidence.source_ranges.dedup();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_anaphoric_definitions(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    events: &ProseEventStream,
    constructions: &[DiscourseConstruction],
    output: &mut ProseObservations,
) {
    for construction in constructions {
        let DiscourseConstruction::Anaphoric {
            antecedent_clause_index: symbols_index,
            description_clause_index: description_index,
            candidate,
            frame,
        } = construction
        else {
            continue;
        };
        let symbols_clause = &clauses[*symbols_index];
        let description_clause = &clauses[*description_index];
        if !symbols_clause.frame.establishes() || !frame.establishes() {
            continue;
        }

        let mut antecedents = candidate
            .mention_indices
            .iter()
            .filter_map(|mention_index| mentions.get(*mention_index))
            .filter(|mention| is_definition_slot_math(&parsed[mention.math_index]))
            .collect::<Vec<_>>();
        let mut seen_symbols = std::collections::BTreeSet::new();
        antecedents.retain(|mention| {
            primary_symbol(document, &parsed[mention.math_index])
                .is_some_and(|(symbol, _)| seen_symbols.insert(symbol))
        });
        if antecedents.is_empty() || antecedents.len() > 8 {
            continue;
        }
        let description_range = SourceRange {
            start_offset: index.utf16_for_byte(description_clause.start),
            end_offset: index.utf16_for_byte(description_clause.end),
        };
        let antecedent_range = SourceRange {
            start_offset: index.utf16_for_byte(symbols_clause.start),
            end_offset: index.utf16_for_byte(symbols_clause.end),
        };
        if !output
            .semantic_evidence
            .attachment
            .permits(&antecedent_range, &description_range)
        {
            continue;
        }

        let text = description_clause.text.trim();
        let has_former = events.has_anaphor_kind(*description_index, AnaphorKind::Former);
        let has_latter = events.has_anaphor_kind(*description_index, AnaphorKind::Latter);
        if has_former || has_latter {
            if antecedents.len() != 2 {
                continue;
            }
            let paired = former_latter_descriptions(text).map(|(former, latter)| {
                (former.to_owned(), latter.to_owned(), description_clause.end)
            });
            let split = paired.or_else(|| {
                let next = clauses.get(*description_index + 1)?;
                if !has_former
                    || next.start.saturating_sub(description_clause.end) > 160
                    || !next.frame.establishes()
                    || !events.has_anaphor_kind(*description_index + 1, AnaphorKind::Latter)
                {
                    return None;
                }
                let former = single_anaphor_description(text, "the former")?;
                let latter = single_anaphor_description(next.text.trim(), "the latter")?;
                Some((former.to_owned(), latter.to_owned(), next.end))
            });
            let Some((former, latter, evidence_end)) = split else {
                continue;
            };
            let expanded_description_range = SourceRange {
                start_offset: description_range.start_offset,
                end_offset: index.utf16_for_byte(evidence_end),
            };
            if !output
                .semantic_evidence
                .attachment
                .permits(&antecedent_range, &expanded_description_range)
            {
                continue;
            }
            for (mention, description) in antecedents.into_iter().zip([former, latter]) {
                let math = &parsed[mention.math_index];
                let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
                    continue;
                };
                push_claim(
                    output,
                    document,
                    index,
                    &symbol,
                    &symbol_range,
                    &description,
                    "english-former-latter-definition",
                    symbols_clause.start,
                    evidence_end,
                );
            }
            continue;
        }

        let ordered = events.has_connective(
            *description_index,
            &[
                DiscourseConnective::Respectively,
                DiscourseConnective::InThatOrder,
            ],
        );
        let Some(body) = anaphoric_description_body(text, antecedents.len()) else {
            continue;
        };
        let descriptions = if antecedents.len() == 1 {
            vec![strip_description_article(body)]
        } else if ordered {
            let Some(descriptions) = align_ordered_descriptions(body, antecedents.len()) else {
                continue;
            };
            descriptions
        } else {
            continue;
        };
        for (mention, description) in antecedents.into_iter().zip(descriptions) {
            let math = &parsed[mention.math_index];
            let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
                continue;
            };
            push_claim(
                output,
                document,
                index,
                &symbol,
                &symbol_range,
                description,
                "english-anaphoric-definition",
                symbols_clause.start,
                description_clause.end,
            );
        }
    }
}

fn anaphoric_description_body(text: &str, arity: usize) -> Option<&str> {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let leads: &[&str] = if arity == 1 {
        &["this quantity", "this symbol", "this variable"]
    } else {
        &[
            "they",
            "these",
            "these quantities",
            "these symbols",
            "those quantities",
            "those symbols",
        ]
    };
    let lead = leads.iter().find(|lead| lower.starts_with(**lead))?;
    let tail = trimmed[lead.len()..].trim_start();
    let tail_lower = tail.to_ascii_lowercase();
    let action = [
        "denotes ",
        "denote ",
        "represents ",
        "represent ",
        "means ",
        "mean ",
        "is ",
        "are ",
    ]
    .into_iter()
    .find(|action| tail_lower.starts_with(action))?;
    let mut body = tail[action.len()..].trim();
    if arity == 1
        && let Some((primary, _)) = body.split_once(", while ")
    {
        body = primary.trim();
    }
    body = trim_order_marker(body);
    (!body.is_empty()).then_some(body)
}

fn former_latter_descriptions(text: &str) -> Option<(&str, &str)> {
    let lower = text.to_ascii_lowercase();
    let former_start = lower.find("the former")? + "the former".len();
    let latter_marker = lower[former_start..].find("the latter")? + former_start;
    let mut former = text[former_start..latter_marker]
        .trim()
        .trim_end_matches([',', ';'])
        .trim_end();
    former = former.strip_suffix("and").map_or(former, str::trim_end);
    let latter_start = latter_marker + "the latter".len();
    let latter = text[latter_start..].trim();
    let former = strip_definition_action(former)?;
    let latter = strip_definition_action(latter).unwrap_or(latter);
    Some((
        strip_description_article(trim_terminal_punctuation(former)),
        strip_description_article(trim_terminal_punctuation(latter)),
    ))
}

fn single_anaphor_description<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let description = strip_definition_action(text[start..].trim())?;
    Some(strip_description_article(trim_terminal_punctuation(
        description,
    )))
}

fn strip_definition_action(value: &str) -> Option<&str> {
    let lower = value.to_ascii_lowercase();
    [
        "denotes ",
        "denote ",
        "represents ",
        "represent ",
        "means ",
        "mean ",
        "is ",
        "are ",
    ]
    .into_iter()
    .find_map(|action| {
        lower
            .starts_with(action)
            .then(|| value[action.len()..].trim())
    })
}

fn trim_order_marker(value: &str) -> &str {
    let value = trim_terminal_punctuation(value);
    let lower = value.to_ascii_lowercase();
    for marker in [
        ", respectively",
        " respectively",
        ", in that order",
        " in that order",
    ] {
        if lower.ends_with(marker) {
            return value[..value.len() - marker.len()].trim_end();
        }
    }
    value
}

fn trim_terminal_punctuation(value: &str) -> &str {
    value
        .trim()
        .trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, '.' | ';' | ':')
        })
        .trim()
}

fn strip_description_article(value: &str) -> &str {
    let lower = value.to_ascii_lowercase();
    ["a ", "an ", "the "]
        .into_iter()
        .find_map(|article| {
            lower
                .starts_with(article)
                .then(|| value[article.len()..].trim())
        })
        .unwrap_or(value)
}

#[allow(clippy::too_many_arguments)]
fn collect_equation_flow_definitions(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    events: &ProseEventStream,
    constructions: &[DiscourseConstruction],
    output: &mut ProseObservations,
) {
    let scopes = ScopeGraph::new(document);
    let mut resolved_mentions = std::collections::BTreeSet::new();
    for construction in constructions {
        let DiscourseConstruction::OutputDefinition {
            producer_mention_index,
            result_mention_index,
            description_start,
            description_end,
            candidate,
            frame,
        } = construction
        else {
            continue;
        };
        let Some((producer, result)) = mentions
            .get(*producer_mention_index)
            .zip(mentions.get(*result_mention_index))
        else {
            continue;
        };
        let result_math = &parsed[result.math_index];
        let formula_range = result_math.region.content_range.clone();
        let evidence_range = SourceRange {
            start_offset: index.utf16_for_byte(candidate.evidence_start),
            end_offset: index.utf16_for_byte(candidate.evidence_end),
        };
        if !frame.establishes()
            || !output.semantic_evidence.formula_is_asserted(&formula_range)
            || candidate.distance_bytes > MAX_ATTACHMENT_DISTANCE_BYTES
            || !output
                .semantic_evidence
                .attachment
                .permits(&evidence_range, &formula_range)
        {
            continue;
        }
        let producer_math = &parsed[producer.math_index];
        let Some((symbol, symbol_range)) =
            opaque_application_equation_result(document, producer_math, result_math)
        else {
            continue;
        };
        let description =
            strip_description_article(source[*description_start..*description_end].trim());
        if !valid_flow_description(description) {
            continue;
        }
        if conflicts_with_existing_role(output, &scopes, &symbol, &symbol_range, description) {
            continue;
        }
        push_claim(
            output,
            document,
            index,
            &symbol,
            &symbol_range,
            description,
            "english-output-definition",
            candidate.evidence_start,
            candidate.evidence_end,
        );
        resolved_mentions.insert(*result_mention_index);
    }
    for construction in constructions {
        let DiscourseConstruction::EquationFlow {
            mention_index,
            prose_start,
            prose_end,
            precedes_formula,
            candidate,
            frame,
        } = construction
        else {
            continue;
        };
        if resolved_mentions.contains(mention_index) {
            continue;
        }
        let mention = &mentions[*mention_index];
        let math = &parsed[mention.math_index];
        if is_definition_slot_math(math) {
            continue;
        }
        let formula_range = SourceRange {
            start_offset: index.utf16_for_byte(mention.start),
            end_offset: index.utf16_for_byte(mention.end),
        };
        output.match_stats.matcher_work += 1;
        let prose_range = SourceRange {
            start_offset: index.utf16_for_byte(*prose_start),
            end_offset: index.utf16_for_byte(*prose_end),
        };
        if !frame.establishes()
            || !output
                .semantic_evidence
                .attachment
                .permits(&prose_range, &formula_range)
        {
            continue;
        }
        debug_assert_eq!(candidate.mention_indices, [*mention_index]);
        let window = &source[*prose_start..*prose_end];
        let action = events.last_definition_action(*prose_start, *prose_end);
        let description = equation_flow_description(window)
            .or_else(|| precedes_formula.then(|| equation_flow_nominal_description(window))?)
            .or_else(|| {
                precedes_formula.then_some(())?;
                action_equation_flow_description(source, *prose_start, *prose_end, action?)
            });
        let Some(description) = description else {
            continue;
        };
        let description_start = description.as_ptr() as usize - source.as_ptr() as usize;
        if !clause_at(clauses, description_start).is_some_and(|clause| clause.frame.establishes())
            || action.is_some_and(|action| {
                !clauses
                    .get(action.clause_index)
                    .is_some_and(|clause| clause.frame.establishes())
            })
        {
            continue;
        }
        let Some((symbol, symbol_range)) =
            relation_head_symbol(document, &math.region.content_range)
                .or_else(|| primary_symbol(document, math))
        else {
            continue;
        };
        let description = description.split_whitespace().collect::<Vec<_>>().join(" ");
        if conflicts_with_existing_role(output, &scopes, &symbol, &symbol_range, &description) {
            continue;
        }
        push_claim(
            output,
            document,
            index,
            &symbol,
            &symbol_range,
            &description,
            "english-equation-flow-definition",
            candidate.evidence_start,
            candidate.evidence_end,
        );
        resolved_mentions.insert(*mention_index);
    }
}

fn opaque_application_equation_result(
    document: &ProjectDocument,
    producer: &ParsedMath,
    result: &ParsedMath,
) -> Option<(String, SourceRange)> {
    let producer_expression = lower_document_region(document, &producer.region.content_range);
    let result_expression = lower_document_region(document, &result.region.content_range);
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &result_expression.kind
    else {
        return None;
    };
    if !matches!(operator.as_str(), "=" | "equals") {
        return None;
    }
    if !same_transformation_application(
        document,
        &producer_expression,
        &producer.region.content_range,
        right,
    ) {
        return None;
    }
    match &left.kind {
        SemanticExprKind::Symbol(symbol) => Some((symbol.clone(), left.range.clone())),
        SemanticExprKind::Index { .. } => {
            relation_head_symbol(document, &result.region.content_range)
        }
        _ => None,
    }
}

fn same_transformation_application(
    document: &ProjectDocument,
    producer: &SemanticExpr,
    producer_range: &SourceRange,
    result: &SemanticExpr,
) -> bool {
    if let (
        SemanticExprKind::Apply {
            operator: producer_operator,
            arguments: producer_arguments,
        },
        SemanticExprKind::Apply {
            operator: result_operator,
            arguments: result_arguments,
        },
    ) = (&producer.kind, &result.kind)
    {
        return producer_operator == result_operator
            && producer_arguments.len() == 1
            && result_arguments.len() == 1
            && expression_name(&producer_arguments[0]) == expression_name(&result_arguments[0]);
    }

    macro_application_signature(document, producer_range)
        .zip(macro_application_signature(document, &result.range))
        .is_some_and(|(producer, result)| producer == result)
}

fn macro_application_signature(document: &ProjectDocument, range: &SourceRange) -> Option<String> {
    document.macros.iter().find_map(|event| {
        let input = event.expansion.input_range.as_ref()?;
        let surface = event.expansion.surface.as_ref()?;
        (event.kind == crate::ProjectMacroKind::Call
            && event.expansion.status == crate::ProjectMacroExpansionStatus::Expanded
            && input == range)
            .then(|| format!("{}:{surface}", event.name))
    })
}

fn conflicts_with_existing_role(
    observations: &ProseObservations,
    scopes: &ScopeGraph,
    symbol: &str,
    symbol_range: &SourceRange,
    description: &str,
) -> bool {
    let Some(role) = classify_role(description) else {
        return false;
    };
    let target_scope = scopes.path_at(symbol_range.start_offset);
    observations.definitions.iter().any(|definition| {
        definition.symbol == symbol
            && definition_available_from(definition) <= symbol_range.start_offset
            && scope_visible(
                &scopes.path_at(definition.location.range.start_offset),
                &target_scope,
            )
            && classify_role(&definition.description).is_some_and(|existing| existing != role)
    })
}

fn action_equation_flow_description<'a>(
    source: &'a str,
    window_start: usize,
    window_end: usize,
    action: &ProseEvent,
) -> Option<&'a str> {
    let ProseEventKind::DefinitionAction(kind) = action.kind else {
        return None;
    };
    if !matches!(kind, DefinitionAction::Compute | DefinitionAction::Write) {
        return None;
    }

    let after = trim_terminal_punctuation(&source[action.end..window_end]);
    let after_lower = after.to_ascii_lowercase();
    for marker in [
        " as follows",
        " as",
        " by the following equation",
        " by the following formula",
    ] {
        if after_lower.ends_with(marker) {
            let description = strip_description_article(after[..after.len() - marker.len()].trim());
            if valid_flow_description(description) && classify_role(description).is_some() {
                return Some(description);
            }
        }
    }

    if kind != DefinitionAction::Write {
        return None;
    }
    let before = trim_terminal_punctuation(&source[window_start..action.start]);
    let phrase = strip_trailing_flow_copula(before);
    role_first_nominal_candidates(phrase)
        .into_iter()
        .find(|candidate| {
            valid_flow_description(candidate.description)
                && classify_role(candidate.description).is_some()
                && !is_formula_metadescription(candidate.description)
        })
        .map(|candidate| candidate.description)
}

fn is_formula_metadescription(description: &str) -> bool {
    matches!(
        description
            .split(|character: char| !character.is_ascii_alphabetic())
            .rfind(|word| !word.is_empty())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "balance"
                | "check"
                | "condition"
                | "equation"
                | "expression"
                | "formula"
                | "identity"
                | "law"
                | "model"
                | "relation"
        )
    )
}

fn strip_trailing_flow_copula(mut value: &str) -> &str {
    let mut words = value.split_whitespace().rev();
    let last = words.next();
    let previous = words.next();
    let remove = if last == Some("be") && previous == Some("to") {
        2
    } else if matches!(last, Some("is" | "are" | "was" | "were" | "be")) {
        1
    } else {
        0
    };
    for _ in 0..remove {
        value = value.trim_end();
        let start = value
            .rfind(char::is_whitespace)
            .map_or(0, |offset| offset + 1);
        value = &value[..start];
    }
    value.trim()
}

fn equation_flow_nominal_description(value: &str) -> Option<&str> {
    let trimmed = trim_terminal_punctuation(value);
    let without_copula = strip_trailing_flow_copula(trimmed);
    let lower = trimmed.to_ascii_lowercase();
    let has_introductory_relation = without_copula.len() < trimmed.len()
        || [" has ", " have ", " had "]
            .iter()
            .any(|marker| format!(" {lower} ").contains(marker));
    has_introductory_relation
        .then(|| role_first_nominal_candidates(without_copula))?
        .into_iter()
        .find(|candidate| {
            valid_flow_description(candidate.description)
                && classify_role(candidate.description).is_some()
                && !is_formula_metadescription(candidate.description)
        })
        .map(|candidate| candidate.description)
}

fn equation_flow_description(text: &str) -> Option<&str> {
    let trimmed = trim_terminal_punctuation(text)
        .trim_start_matches(|character: char| {
            character.is_whitespace() || matches!(character, '.' | ';' | ':')
        })
        .trim_start();
    let lower = trimmed.to_ascii_lowercase();
    for lead in [
        "the following equation defines ",
        "the following formula defines ",
        "this equation defines ",
        "this formula defines ",
    ] {
        if lower.starts_with(lead) {
            let description = strip_description_article(trimmed[lead.len()..].trim());
            return valid_flow_description(description).then_some(description);
        }
    }
    for lead in ["we define ", "define "] {
        if lower.starts_with(lead) {
            let tail = &trimmed[lead.len()..];
            let tail_lower = tail.to_ascii_lowercase();
            let marker = [" by the following", " by", " as follows"]
                .into_iter()
                .filter_map(|marker| tail_lower.find(marker))
                .min()?;
            let description = strip_description_article(tail[..marker].trim());
            return valid_flow_description(description).then_some(description);
        }
    }
    for marker in [
        " is defined by the following equation",
        " is defined by the following formula",
    ] {
        if let Some(end) = lower.find(marker) {
            let description = strip_description_article(trimmed[..end].trim());
            return valid_flow_description(description).then_some(description);
        }
    }
    None
}

fn valid_flow_description(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.split_whitespace().count() <= 12
        && !value.contains(['$', '=', '\\'])
}

fn collect_semantic_evidence(
    document: &ProjectDocument,
    source: &str,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    canonical_expressions: &[SemanticExpr],
    construction_targets: &BTreeMap<usize, Vec<ConstructionFormulaTarget>>,
) -> ScientificSemanticEvidence {
    let clause_evidence = clauses
        .iter()
        .map(|clause| ScientificClauseEvidence {
            range: SourceRange {
                start_offset: index.utf16_for_byte(clause.start),
                end_offset: index.utf16_for_byte(clause.end),
            },
            frame: clause.frame.clone(),
        })
        .collect::<Vec<_>>();
    let mut domain_priors = Vec::new();
    let mut law_activations = Vec::new();
    let reference_targets = explicit_equation_reference_targets(
        document,
        source,
        index,
        clauses,
        canonical_expressions,
    );
    let mut formula_operations = Vec::new();
    for clause in clauses {
        for range in phrase_ranges(source, index, clause, "vector dot product") {
            formula_operations.push(FormulaOperationEvidence {
                clause_range: SourceRange {
                    start_offset: index.utf16_for_byte(clause.start),
                    end_offset: index.utf16_for_byte(clause.end),
                },
                operation: FormulaOperationKind::VectorDotProduct,
                frame: clause.frame.clone(),
                evidence: Evidence {
                    rule_id: "scientific-prose/vector-dot-product".into(),
                    kind: "typed-operation-constraint".into(),
                    strength: "strong".into(),
                    source_ranges: vec![range],
                },
            });
        }
    }
    for pack in built_in_packs() {
        for rule in &pack.activation_rules {
            for clause in clauses {
                for phrase in &rule.phrases {
                    for range in phrase_ranges(source, index, clause, phrase) {
                        domain_priors.push(DomainPriorEvidence {
                            pack_id: pack.pack_id.clone(),
                            pack_version: pack.pack_version.clone(),
                            title: pack.title.clone(),
                            frame: clause.frame.clone(),
                            evidence: Evidence {
                                rule_id: format!("{}/activation/{}", pack.pack_id, rule.id),
                                kind: "prose-domain-prior".into(),
                                strength: "weak".into(),
                                source_ranges: vec![range],
                            },
                        });
                    }
                }
            }
            for structure in &rule.structures {
                for range in
                    structural_activation_ranges(document, canonical_expressions, *structure)
                {
                    domain_priors.push(DomainPriorEvidence {
                        pack_id: pack.pack_id.clone(),
                        pack_version: pack.pack_version.clone(),
                        title: pack.title.clone(),
                        frame: frame_for_range(&clause_evidence, &range),
                        evidence: Evidence {
                            rule_id: format!("{}/activation/{}", pack.pack_id, rule.id),
                            kind: "structural-domain-prior".into(),
                            strength: "weak".into(),
                            source_ranges: vec![range],
                        },
                    });
                }
            }
        }
        for law in &pack.laws {
            for clause in clauses {
                let mut phrases = law
                    .activation_phrases
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(law.title.as_str()))
                    .collect::<Vec<_>>();
                if let Some(head) = law_title_head(&law.title) {
                    phrases.push(head);
                }
                phrases.sort_unstable();
                phrases.dedup();
                for phrase in phrases {
                    for range in phrase_ranges(source, index, clause, phrase) {
                        let mut attached_formula_ranges = reference_targets
                            .get(&clause.start)
                            .cloned()
                            .unwrap_or_default();
                        let construction_targets = construction_targets
                            .get(&clause.start)
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        attached_formula_ranges.extend(
                            construction_targets
                                .iter()
                                .map(|target| target.range.clone()),
                        );
                        attached_formula_ranges
                            .sort_by_key(|range| (range.start_offset, range.end_offset));
                        attached_formula_ranges.dedup();
                        law_activations.push(LawActivationEvidence {
                            pack_id: pack.pack_id.clone(),
                            law_id: law.id.clone(),
                            clause_range: SourceRange {
                                start_offset: index.utf16_for_byte(clause.start),
                                end_offset: index.utf16_for_byte(clause.end),
                            },
                            identifies_attached_formula: !reference_targets
                                .get(&clause.start)
                                .map(Vec::is_empty)
                                .unwrap_or(true)
                                || construction_targets
                                    .iter()
                                    .any(|target| target.identifies_formula)
                                || activation_identifies_formula(
                                    source,
                                    index,
                                    clause,
                                    &range,
                                    canonical_expressions,
                                ),
                            attached_formula_ranges,
                            frame: clause.frame.clone(),
                            evidence: Evidence {
                                rule_id: format!(
                                    "{}/law/{}/activation-phrase",
                                    pack.pack_id, law.id
                                ),
                                kind: "explicit-prose".into(),
                                strength: "strong".into(),
                                source_ranges: vec![range],
                            },
                        });
                    }
                }
            }
        }
    }
    domain_priors.sort_by(|left, right| {
        left.pack_id
            .cmp(&right.pack_id)
            .then(left.evidence.rule_id.cmp(&right.evidence.rule_id))
            .then(evidence_range_key(&left.evidence).cmp(&evidence_range_key(&right.evidence)))
    });
    domain_priors.dedup();
    law_activations.sort_by(|left, right| {
        left.pack_id
            .cmp(&right.pack_id)
            .then(left.law_id.cmp(&right.law_id))
            .then(evidence_range_key(&left.evidence).cmp(&evidence_range_key(&right.evidence)))
    });
    law_activations.dedup();
    ScientificSemanticEvidence {
        clauses: clause_evidence,
        domain_priors,
        law_activations,
        formula_operations,
        attachment: AttachmentGraph::new(document),
    }
}

fn explicit_equation_reference_targets(
    document: &ProjectDocument,
    source: &str,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    canonical_expressions: &[SemanticExpr],
) -> BTreeMap<usize, Vec<SourceRange>> {
    let commands = scan_reference_commands(source);
    let labels = commands
        .iter()
        .filter(|command| command.kind == ReferenceCommandKind::Label)
        .filter_map(|command| {
            let offset = index.utf16_for_byte(command.start);
            let target = document
                .math_roots
                .iter()
                .filter(|root| root.full_range.contains(offset))
                .min_by_key(|root| root.full_range.end_offset - root.full_range.start_offset)
                .map(|root| root.content_range.clone())
                .or_else(|| {
                    canonical_expressions
                        .iter()
                        .filter(|expression| expression.range.end_offset <= offset)
                        .filter(|expression| offset - expression.range.end_offset <= 128)
                        .max_by_key(|expression| expression.range.end_offset)
                        .map(|expression| expression.range.clone())
                })?;
            Some((command.key.as_str(), target))
        })
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::<usize, Vec<SourceRange>>::new();
    for command in commands.iter().filter(|command| {
        matches!(
            command.kind,
            ReferenceCommandKind::Reference | ReferenceCommandKind::EquationReference
        )
    }) {
        let Some(target) = labels.get(command.key.as_str()) else {
            continue;
        };
        let Some(clause) = clause_at(clauses, command.start) else {
            continue;
        };
        let ranges = targets.entry(clause.start).or_default();
        if !ranges.contains(target) {
            ranges.push(target.clone());
        }
    }
    targets
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConstructionFormulaTarget {
    range: SourceRange,
    identifies_formula: bool,
}

fn construction_formula_targets(
    document: &ProjectDocument,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    events: &ProseEventStream,
    constructions: &[DiscourseConstruction],
    mentions: &[ScientificMention],
) -> BTreeMap<usize, Vec<ConstructionFormulaTarget>> {
    let attachment = AttachmentGraph::new(document);
    let mut targets = BTreeMap::<usize, Vec<ConstructionFormulaTarget>>::new();
    let mut nearest_preceding_flow = BTreeMap::<usize, &DiscourseConstruction>::new();
    for construction in constructions {
        if let DiscourseConstruction::EquationFlow {
            mention_index,
            prose_start,
            precedes_formula: true,
            ..
        } = construction
        {
            nearest_preceding_flow
                .entry(*mention_index)
                .and_modify(|current| {
                    let DiscourseConstruction::EquationFlow {
                        prose_start: current_start,
                        ..
                    } = current
                    else {
                        unreachable!()
                    };
                    if current_start < prose_start {
                        *current = construction;
                    }
                })
                .or_insert(construction);
        }
    }
    for construction in nearest_preceding_flow.values().copied() {
        let DiscourseConstruction::EquationFlow {
            mention_index,
            prose_start,
            candidate,
            frame,
            ..
        } = construction
        else {
            unreachable!()
        };
        let Some((clause, mention)) =
            clause_at(clauses, *prose_start).zip(mentions.get(*mention_index))
        else {
            continue;
        };
        let evidence_range = SourceRange {
            start_offset: index.utf16_for_byte(clause.start),
            end_offset: index.utf16_for_byte(clause.end),
        };
        let target_range = SourceRange {
            start_offset: index.utf16_for_byte(mention.start),
            end_offset: index.utf16_for_byte(mention.end),
        };
        let identifies_formula = frame.establishes()
            && matches!(
                frame.act,
                CommunicativeAct::Definition | CommunicativeAct::Result
            );
        if identifies_formula
            && candidate.distance_bytes <= MAX_ATTACHMENT_DISTANCE_BYTES
            && attachment.permits(&evidence_range, &target_range)
        {
            targets
                .entry(clause.start)
                .or_default()
                .push(ConstructionFormulaTarget {
                    range: target_range,
                    identifies_formula,
                });
        }
    }
    for construction in constructions {
        let DiscourseConstruction::Anaphoric {
            description_clause_index,
            candidate,
            frame,
            ..
        } = construction
        else {
            continue;
        };
        let Some(clause) = clauses.get(*description_clause_index) else {
            continue;
        };
        if !frame.establishes()
            || !events.starts_with_anaphor_kind(
                *description_clause_index,
                clause.start,
                AnaphorKind::FormulaDemonstrative,
            )
        {
            continue;
        }
        let evidence_range = SourceRange {
            start_offset: index.utf16_for_byte(clause.start),
            end_offset: index.utf16_for_byte(clause.end),
        };
        for mention in candidate
            .mention_indices
            .iter()
            .filter_map(|index| mentions.get(*index))
        {
            let target_range = SourceRange {
                start_offset: index.utf16_for_byte(mention.start),
                end_offset: index.utf16_for_byte(mention.end),
            };
            if attachment.permits(&target_range, &evidence_range) {
                targets
                    .entry(clause.start)
                    .or_default()
                    .push(ConstructionFormulaTarget {
                        range: target_range,
                        identifies_formula: true,
                    });
            }
        }
    }
    for values in targets.values_mut() {
        values.sort_by_key(|target| {
            (
                target.range.start_offset,
                target.range.end_offset,
                target.identifies_formula,
            )
        });
        values.dedup();
    }
    targets
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceCommandKind {
    Label,
    Reference,
    EquationReference,
}

struct ReferenceCommand {
    kind: ReferenceCommandKind,
    key: String,
    start: usize,
}

fn scan_reference_commands(source: &str) -> Vec<ReferenceCommand> {
    let mut commands = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('\\') {
        let start = cursor + relative;
        let tail = &source[start..];
        let matched = [
            ("\\eqref{", ReferenceCommandKind::EquationReference),
            ("\\label{", ReferenceCommandKind::Label),
            ("\\ref{", ReferenceCommandKind::Reference),
        ]
        .into_iter()
        .find(|(prefix, _)| tail.starts_with(prefix));
        let Some((prefix, kind)) = matched else {
            cursor = start + 1;
            continue;
        };
        let key_start = start + prefix.len();
        let Some(relative_end) = source[key_start..].find('}') else {
            break;
        };
        let key_end = key_start + relative_end;
        if key_start < key_end {
            commands.push(ReferenceCommand {
                kind,
                key: source[key_start..key_end].to_owned(),
                start,
            });
        }
        cursor = key_end + 1;
    }
    commands
}

fn evidence_range_key(evidence: &Evidence) -> (u32, u32) {
    evidence
        .source_ranges
        .first()
        .map_or((0, 0), |range| (range.start_offset, range.end_offset))
}

fn activation_identifies_formula(
    source: &str,
    index: &SourceIndex,
    clause: &ScientificClause<'_>,
    phrase: &SourceRange,
    canonical_expressions: &[SemanticExpr],
) -> bool {
    let phrase_end = index.byte_for_utf16(phrase.end_offset);
    let formula_start = canonical_expressions
        .iter()
        .filter(|expression| {
            let start = index.byte_for_utf16(expression.range.start_offset);
            let end = index.byte_for_utf16(expression.range.end_offset);
            phrase_end <= start && start < clause.end && end <= clause.end
        })
        .map(|expression| index.byte_for_utf16(expression.range.start_offset))
        .min()
        .unwrap_or(clause.end);
    formula_identification_bridge(&source[phrase_end..formula_start])
}

fn formula_identification_bridge(value: &str) -> bool {
    let words = value
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let words = match words.first().map(String::as_str) {
        Some(
            "definition" | "equation" | "expression" | "formula" | "identity" | "law" | "model"
            | "relation",
        ) => &words[1..],
        _ => words.as_slice(),
    };
    let words = match words {
        [prefix @ .., article, following, noun]
            if article == "the"
                && following == "following"
                && matches!(
                    noun.as_str(),
                    "equation" | "expression" | "formula" | "relation"
                ) =>
        {
            prefix
        }
        _ => words,
    };
    matches!(
        words,
        [copula]
            if matches!(copula.as_str(), "is" | "are" | "was" | "were" | "equals" | "reads")
    ) || matches!(
        words,
        [copula, participle, preposition]
            if matches!(copula.as_str(), "is" | "are" | "was" | "were")
                && matches!(participle.as_str(), "defined" | "expressed" | "given" | "written")
                && matches!(preposition.as_str(), "as" | "by")
    ) || matches!(
        words,
        [modal, be_word, participle, preposition]
            if matches!(modal.as_str(), "can" | "may")
                && be_word == "be"
                && matches!(participle.as_str(), "expressed" | "written")
                && preposition == "as"
    ) || matches!(
        words,
        [takes, article, form] if takes == "takes" && article == "the" && form == "form"
    )
}

fn phrase_ranges(
    source: &str,
    index: &SourceIndex,
    clause: &ScientificClause<'_>,
    phrase: &str,
) -> Vec<SourceRange> {
    let lower = clause.text.to_ascii_lowercase().replace('-', " ");
    let phrase = phrase.to_ascii_lowercase().replace('-', " ");
    let mut ranges = Vec::new();
    let mut search_from = 0;
    while let Some(found) = lower[search_from..].find(&phrase) {
        let start = clause.start + search_from + found;
        let end = start + phrase.len();
        ranges.push(SourceRange {
            start_offset: index.utf16_for_byte(start),
            end_offset: index.utf16_for_byte(end),
        });
        search_from += found + phrase.len();
    }
    ranges.retain(|range| range.start_offset < range.end_offset);
    debug_assert!(ranges.iter().all(|range| {
        let start = index.byte_for_utf16(range.start_offset);
        let end = index.byte_for_utf16(range.end_offset);
        source.get(start..end).is_some()
    }));
    ranges
}

fn law_title_head(title: &str) -> Option<&str> {
    [" definition", " relation"].into_iter().find_map(|suffix| {
        title
            .to_ascii_lowercase()
            .ends_with(suffix)
            .then(|| title[..title.len() - suffix.len()].trim_end())
            .filter(|head| !head.is_empty())
    })
}

fn structural_activation_ranges(
    document: &ProjectDocument,
    canonical_expressions: &[SemanticExpr],
    structure: PackActivationStructure,
) -> Vec<SourceRange> {
    let mut ranges = canonical_expressions
        .iter()
        .filter(|expression| expression_has_structure(expression, structure))
        .map(|expression| expression.range.clone())
        .collect::<Vec<_>>();
    ranges.extend(
        document
            .nodes
            .iter()
            .filter(|node| node_has_structure(document, node, structure))
            .map(|node| node.ranges.full.clone()),
    );
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    ranges
}

fn expression_has_structure(expression: &SemanticExpr, structure: PackActivationStructure) -> bool {
    let direct = match (&expression.kind, structure) {
        (SemanticExprKind::Derivative { .. }, PackActivationStructure::Calculus) => true,
        (SemanticExprKind::Apply { operator, .. }, PackActivationStructure::Calculus) => {
            matches!(operator.as_str(), "integral" | "limit" | "nabla")
        }
        (SemanticExprKind::Binder { operator, .. }, PackActivationStructure::Calculus) => {
            matches!(operator.as_str(), "integral" | "limit" | "sum" | "product")
        }
        (SemanticExprKind::Apply { operator, .. }, PackActivationStructure::Discrete) => {
            matches!(operator.as_str(), "intersection" | "union" | "binomial")
        }
        (SemanticExprKind::Relation { operator, .. }, PackActivationStructure::Discrete) => {
            matches!(operator.as_str(), "membership" | "subset")
        }
        (SemanticExprKind::Apply { operator, .. }, PackActivationStructure::Optimization) => {
            matches!(operator.as_str(), "argmin" | "argmax" | "min" | "max")
        }
        _ => false,
    };
    direct
        || expression_children(expression)
            .into_iter()
            .any(|child| expression_has_structure(child, structure))
}

fn node_has_structure(
    document: &ProjectDocument,
    node: &crate::NotationNode,
    structure: PackActivationStructure,
) -> bool {
    let name = node.name.as_deref().unwrap_or_default();
    match structure {
        PackActivationStructure::Calculus => {
            matches!(
                name,
                "partial" | "int" | "iint" | "iiint" | "lim" | "nabla" | "dot" | "ddot"
            )
        }
        PackActivationStructure::Discrete => {
            matches!(
                name,
                "subset" | "in" | "cup" | "cap" | "forall" | "exists" | "binom"
            )
        }
        PackActivationStructure::Optimization => {
            matches!(name, "argmin" | "argmax" | "min" | "max")
                || name == "mathcal" && bounded_node_text(document, node, 0) == "L"
        }
        PackActivationStructure::Probability => {
            (name == "mathbb" && matches!(bounded_node_text(document, node, 0).as_str(), "P" | "E"))
                || name == "Var"
        }
        PackActivationStructure::RealCoordinateSpace => {
            name == "mathbb" && bounded_node_text(document, node, 0) == "R"
        }
    }
}

fn bounded_node_text(document: &ProjectDocument, node: &crate::NotationNode, depth: u8) -> String {
    if depth == 8 {
        return String::new();
    }
    if let Some(text) = &node.text {
        return text.clone();
    }
    node.children
        .iter()
        .filter_map(|child| document.nodes.get(*child as usize))
        .map(|child| bounded_node_text(document, child, depth + 1))
        .collect()
}

fn frame_for_range(clauses: &[ScientificClauseEvidence], range: &SourceRange) -> DiscourseFrame {
    clauses
        .iter()
        .filter(|clause| ranges_overlap(&clause.range, range))
        .min_by_key(|clause| clause.range.end_offset - clause.range.start_offset)
        .map(|clause| clause.frame.clone())
        .unwrap_or_else(crate::scientific_prose::asserted_author_frame)
}

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

pub(crate) fn citation_byte_ranges(
    document: &ProjectDocument,
    index: &SourceIndex,
) -> Vec<(usize, usize)> {
    document
        .prose_annotations
        .iter()
        .filter(|annotation| annotation.kind == "citation")
        .map(|annotation| {
            (
                index.byte_for_utf16(annotation.range.start_offset),
                index.byte_for_utf16(annotation.range.end_offset),
            )
        })
        .collect()
}

fn collect_clause_definitions(
    document: &ProjectDocument,
    source: &str,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    events: &ProseEventStream,
    output: &mut ProseObservations,
) {
    for (clause_index, clause) in clauses.iter().enumerate() {
        output.match_stats.matcher_work += 1;
        if !clause.frame.establishes() {
            continue;
        }
        let sentence_start = clause.start;
        let sentence_end = clause.end;
        if events.has_connective(
            clause_index,
            &[
                DiscourseConnective::Respectively,
                DiscourseConnective::InThatOrder,
            ],
        ) {
            let definitions_before = output
                .definitions
                .iter()
                .filter(|definition| {
                    let start = index.byte_for_utf16(definition.location.range.start_offset);
                    sentence_start <= start && start < sentence_end
                })
                .count();
            collect_ordered_clause_definition(
                document,
                source,
                parsed,
                index,
                output,
                sentence_start,
                sentence_end,
            );
            let definitions_after = output
                .definitions
                .iter()
                .filter(|definition| {
                    let start = index.byte_for_utf16(definition.location.range.start_offset);
                    sentence_start <= start && start < sentence_end
                })
                .count();
            if definitions_after > definitions_before
                || definitions_before >= 2
                || !document.content[sentence_start..sentence_end].contains("\\ref{")
            {
                continue;
            }
        }
        let regions = parsed
            .iter()
            .filter(|math| {
                let start = index.byte_for_utf16(math.region.full_range.start_offset);
                let end = index.byte_for_utf16(math.region.full_range.end_offset);
                sentence_start <= start
                    && start < sentence_end
                    && end <= sentence_end
                    && is_definition_slot_math(math)
                    && !is_description_parameter(source, math, sentence_start, sentence_end, index)
            })
            .collect::<Vec<_>>();
        if regions.len() < 2 {
            continue;
        }
        let starts = regions
            .iter()
            .map(|math| index.byte_for_utf16(math.region.full_range.start_offset))
            .collect::<Vec<_>>();
        let ends = regions
            .iter()
            .map(|math| index.byte_for_utf16(math.region.full_range.end_offset))
            .collect::<Vec<_>>();
        let label_segments = starts
            .iter()
            .enumerate()
            .map(|(position, start)| {
                let segment_start = position
                    .checked_sub(1)
                    .map_or(sentence_start, |previous| ends[previous]);
                &source[segment_start..*start]
            })
            .collect::<Vec<_>>();
        if let Some(descriptions) = fronted_labeled_descriptions(&label_segments) {
            for (math, description) in regions.iter().zip(descriptions) {
                let Some((symbol, symbol_range)) = primary_symbol(document, math) else {
                    continue;
                };
                push_claim(
                    output,
                    document,
                    index,
                    &symbol,
                    &symbol_range,
                    description,
                    "english-fronted-labeled-definition",
                    sentence_start,
                    sentence_end,
                );
            }
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
        let first_is_explicit = regions.first().is_some_and(|math| {
            let end = index.byte_for_utf16(math.region.full_range.end_offset);
            definition_clause(&source[end..sentence_end]).1
        });
        let first_introduces_list = ends.first().zip(starts.get(1)).is_some_and(|(end, start)| {
            let between = source[*end..*start].to_ascii_lowercase();
            between.contains(',') || between.contains(" and ")
        });
        // A leading copular/definition verb scopes over a coordinated list:
        // “q denotes heat flux, k conductivity, and T temperature”.  Requiring
        // the elided verb to be repeated for every item rejects ordinary
        // scientific English and also breaks definitions attached by an
        // equation reference.
        if !contextual && explicit < 2 && (!first_is_explicit || !first_introduces_list) {
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
            let end = index.byte_for_utf16(math.region.full_range.end_offset);
            sentence_start <= start
                && start < sentence_end
                && end <= sentence_end
                && is_definition_slot_math(math)
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
    source: &str,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    events: &ProseEventStream,
    output: &mut ProseObservations,
) {
    let condition_phrases = built_in_packs()
        .iter()
        .flat_map(|pack| &pack.laws)
        .flat_map(|law| &law.conditions)
        .flat_map(|condition| {
            let kind = condition_evidence_kind(condition.kind);
            condition
                .evidence_phrases
                .iter()
                .map(move |phrase| (phrase.as_str(), kind, condition.id.as_str()))
        })
        .collect::<Vec<_>>();
    for (clause_index, clause) in clauses.iter().enumerate() {
        let attached_to_preceding_math =
            clause_attaches_to_preceding_math(clause_index, mentions, events);
        for assumption in extract_assumptions_with_phrases(clause, mentions, &condition_phrases) {
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
                    kind: if attached_to_preceding_math {
                        "attached-prose"
                    } else {
                        "explicit-prose"
                    }
                    .into(),
                    strength: "strong".into(),
                    source_ranges,
                },
            });
        }
    }
    collect_typed_regularity_assumptions(source, index, clauses, mentions, output);
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

fn collect_typed_regularity_assumptions(
    source: &str,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    output: &mut ProseObservations,
) {
    for clause in clauses.iter().filter(|clause| clause.frame.establishes()) {
        for mention in mentions
            .iter()
            .filter(|mention| clause.start <= mention.start && mention.end <= clause.end)
        {
            let typed_expression = &source[mention.start..mention.end];
            if !is_differentiability_membership(typed_expression) {
                continue;
            }
            output.assumptions.push(AssumptionInfo {
                kind: "regularity".into(),
                value: "differentiable".into(),
                subjects: vec![mention.symbol.clone()],
                evidence: Evidence {
                    rule_id: "scientific-prose/function-space-regularity".into(),
                    kind: "typed-math-condition".into(),
                    strength: "strong".into(),
                    source_ranges: vec![SourceRange {
                        start_offset: index.utf16_for_byte(mention.start),
                        end_offset: index.utf16_for_byte(mention.end),
                    }],
                },
            });
        }
    }
}

fn is_differentiability_membership(expression: &str) -> bool {
    let compact = expression
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact.contains("\\in")
        && !compact.contains("\\notin")
        && ["C^1(", "C^{1}(", "C^\\infty(", "C^{\\infty}("]
            .iter()
            .any(|class| compact.contains(class))
}

fn condition_evidence_kind(kind: PackConditionKind) -> &'static str {
    match kind {
        PackConditionKind::Assumption => "law-condition",
        PackConditionKind::Differentiable => "regularity",
        PackConditionKind::DomainMembership => "domain-membership",
        PackConditionKind::Positive => "sign",
        PackConditionKind::SameContext => "context",
        PackConditionKind::ShapeCompatible => "shape-compatibility",
        PackConditionKind::SignConvention => "sign-convention",
        PackConditionKind::Uniform => "uniformity",
    }
}

fn clause_attaches_to_preceding_math(
    clause_index: usize,
    mentions: &[ScientificMention],
    events: &ProseEventStream,
) -> bool {
    events
        .first_connective(
            clause_index,
            &[DiscourseConnective::Where, DiscourseConnective::Here],
        )
        .is_some_and(|connective| {
            mentions.iter().any(|mention| {
                mention.end <= connective.start
                    && connective.start - mention.end <= MAX_ATTACHMENT_DISTANCE_BYTES
            })
        })
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
    let mut copular = false;
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
            copular = matches!(verb, "is " | "are " | "be ");
            break;
        }
    }
    if copular && is_non_defining_passive(clause) {
        return (None, false);
    }
    clause = clause
        .trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, ',' | ';' | ':' | '.')
        })
        .trim_end_matches(" and")
        .trim_end_matches(" while")
        .trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, ',' | ';' | ':' | '.')
        })
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

fn is_non_defining_passive(clause: &str) -> bool {
    let lower = clause.to_ascii_lowercase();
    let participle = lower
        .split(|character: char| !character.is_ascii_alphabetic())
        .find(|word| !word.is_empty());
    matches!(
        participle,
        Some(
            "assumed"
                | "compared"
                | "drawn"
                | "evaluated"
                | "held"
                | "located"
                | "observed"
                | "referenced"
                | "reported"
                | "shown"
                | "used"
        )
    ) || participle == Some("measured") && !lower.starts_with("measured in ")
}

fn explicit_single_definition<'a>(
    before: &str,
    after: &'a str,
    math: &ParsedMath,
) -> Option<crate::construction::DefinitionConstruction<'a>> {
    match_definition(before, after, contains_assignment(&math.root))
}

fn contains_assignment(node: &crate::EquationNode) -> bool {
    let mut labels = Vec::new();
    collect_equation_labels(node, &mut labels);
    labels.windows(2).any(|pair| pair == [Some(":"), Some("=")])
}

fn is_definition_slot_math(math: &ParsedMath) -> bool {
    fn contains_formula_relation(node: &crate::EquationNode) -> bool {
        node.label.as_deref().is_some_and(|label| {
            matches!(
                label,
                "=" | "equals" | "<" | ">" | "≤" | "≥" | "≠" | "≈" | "equation" | "relation"
            )
        }) || node.children.iter().any(contains_formula_relation)
    }

    !contains_formula_relation(&math.root)
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
            analysis.match_stats.matcher_work += 1;
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
    if !clause.frame.establishes() || last_end > clause.end {
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
    let descriptions = expand_shared_shape_head(descriptions);
    let statement_start = before_start + prefix_start;
    let statement_end = last_end + suffix_end;

    let mut definitions = Vec::with_capacity(group.len());
    for (math, description) in group.iter().zip(descriptions) {
        let (symbol, range) = primary_symbol(document, math)?;
        definitions.push(CoordinatedDefinition {
            symbol,
            range,
            description,
            rule_id,
            statement_start,
            statement_end,
        });
    }
    Some(definitions)
}

fn expand_shared_shape_head(descriptions: Vec<&str>) -> Vec<String> {
    let Some(last) = descriptions.last() else {
        return Vec::new();
    };
    let Some(head) = last.split_whitespace().next_back() else {
        return descriptions.into_iter().map(str::to_owned).collect();
    };
    let singular = head.strip_suffix('s').unwrap_or(head);
    if !matches!(singular, "scalar" | "vector" | "matrix" | "tensor")
        || last.split_whitespace().count() < 2
    {
        return descriptions.into_iter().map(str::to_owned).collect();
    }
    descriptions
        .into_iter()
        .map(|description| {
            if has_trailing_shape_head(description) {
                description.to_owned()
            } else {
                format!("{description} {singular}")
            }
        })
        .collect()
}

fn has_trailing_shape_head(description: &str) -> bool {
    description
        .split_whitespace()
        .next_back()
        .map(|word| word.trim_end_matches(|character: char| !character.is_alphabetic()))
        .map(|word| word.strip_suffix('s').unwrap_or(word))
        .is_some_and(|word| matches!(word, "scalar" | "vector" | "matrix" | "tensor"))
}

fn valid_symbol_separators(source: &str, starts: &[usize], ends: &[usize]) -> bool {
    starts
        .iter()
        .skip(1)
        .zip(ends)
        .all(|(start, end)| matches!(source[*end..*start].trim(), "," | "and" | ", and"))
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
    let description = description
        .trim()
        .strip_suffix(" and let")
        .unwrap_or(description.trim())
        .trim();
    let description = if rule_id == "english-construction-definition"
        && let Some(coordinated) = description.strip_suffix(" and")
    {
        strip_description_article(coordinated.trim())
    } else {
        description
    };
    if rule_id.contains("definition") {
        analysis.match_stats.construction_candidates += 1;
    }
    let evidence_range = SourceRange {
        start_offset: index.utf16_for_byte(evidence_start),
        end_offset: index.utf16_for_byte(evidence_end),
    };
    let attached = matches!(
        rule_id,
        "english-anaphoric-definition"
            | "english-clause-definition"
            | "english-clause-ordered-definition"
            | "english-contextual-definition"
            | "english-equation-flow-definition"
            | "english-former-latter-definition"
            | "english-output-definition"
    ) && evidence_range.start_offset < symbol_range.start_offset;
    let definition_evidence = Evidence {
        rule_id: rule_id.into(),
        kind: if attached {
            "attached-prose"
        } else {
            "explicit-prose"
        }
        .into(),
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
            available_from: if attached {
                evidence_range.start_offset
            } else {
                evidence_range.end_offset
            },
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
        .or_else(|| description.split_once(" and let "))
        .map_or(description, |(description, _)| description);
    let shape_source = description.replace('$', "");
    let shape_source = shape_source
        .trim_end_matches(|character: char| character.is_whitespace() || character == ',')
        .strip_suffix(" with")
        .unwrap_or(shape_source.trim());
    let normalized = shape_source.to_ascii_lowercase().replace('-', " ");
    let shape = if let Some(captures) = MATRIX_DIMENSIONS.captures(shape_source) {
        ProseShape::Matrix(
            normalized_extent(captures.get(1).unwrap().as_str()),
            normalized_extent(captures.get(2).unwrap().as_str()),
        )
    } else if let Some(captures) = VECTOR_DIMENSION.captures(shape_source) {
        ProseShape::Vector(normalized_extent(captures.get(1).unwrap().as_str()))
    } else if let Some(captures) = COMPACT_VECTOR_DIMENSION.captures(shape_source) {
        ProseShape::Vector(normalized_extent(captures.get(1).unwrap().as_str()))
    } else if let Some(captures) = SQUARE_DIMENSION.captures(shape_source) {
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

fn normalized_extent(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "one" => "1",
        "two" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        "ten" => "10",
        _ => value,
    }
    .to_owned()
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
    // A transparent macro call is observable syntax approved by wasmtex. Keep
    // its real call-site bytes available so downstream description lowering
    // can resolve the supplied expansion without inventing a source range.
    for event in &document.macros {
        if event.kind != crate::ProjectMacroKind::Call
            || event.expansion.status != crate::ProjectMacroExpansionStatus::Expanded
        {
            continue;
        }
        let Some(range) = &event.expansion.input_range else {
            continue;
        };
        let start = index.byte_for_utf16(range.start_offset);
        let end = index.byte_for_utf16(range.end_offset);
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
    analysis.definitions.sort_by(|left, right| {
        left.location
            .range
            .start_offset
            .cmp(&right.location.range.start_offset)
            .then_with(|| definition_priority(right).cmp(&definition_priority(left)))
            .then_with(|| left.evidence.rule_id.cmp(&right.evidence.rule_id))
            .then_with(|| left.description.len().cmp(&right.description.len()))
    });
    let mut definition_locations = BTreeSet::new();
    analysis.definitions.retain(|definition| {
        definition_locations.insert((
            definition.location.file_id.clone(),
            definition.location.range.start_offset,
            definition.location.range.end_offset,
        ))
    });
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

fn definition_priority(definition: &DefinitionInfo) -> u8 {
    match definition.evidence.rule_id.as_str() {
        "english-output-definition"
        | "english-former-latter-definition"
        | "english-anaphoric-definition" => 6,
        "english-imperative-definition"
        | "english-write-for-definition"
        | "english-use-definition"
        | "english-formula-definition"
        | "english-math-assignment-definition"
        | "english-respectively-definition"
        | "english-coordinated-definition" => 5,
        "english-equation-reference-definition"
        | "english-clause-definition"
        | "english-clause-ordered-definition" => 4,
        "english-apposition-definition"
        | "english-equation-flow-definition"
        | "english-construction-definition"
        | "english-relational-definition"
        | "english-passive-definition"
        | "english-contextual-definition"
        | "english-role-first-nominal-definition" => 3,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProseShape, expand_shared_shape_head, formula_identification_bridge, observe_prose,
        shape_claim,
    };
    use crate::canonical::lower_document_region;
    use crate::concept::classify_role;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{
        DocumentLanguage, ProjectDocument, ProjectMacro, ProjectMacroExpansion,
        ProjectMacroExpansionStatus, ProjectMacroKind, ProjectSourceRef, SourceRange,
    };

    fn analyze(source: &str) -> super::ProseObservations {
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
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
        analyze_document(&document)
    }

    fn analyze_document(document: &ProjectDocument) -> super::ProseObservations {
        let parsed = parse_regions(&document.content, &document.math_regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(document, &math.region.content_range))
            .collect::<Vec<_>>();
        observe_prose(document, &parsed, &canonical)
    }

    #[test]
    fn distinguishes_formula_identification_from_contextual_law_mentions() {
        for bridge in [
            " is $",
            " relation is $",
            " is given by the following equation: ",
            " can be written as \\[",
            " takes the form $",
        ] {
            assert!(formula_identification_bridge(bridge), "{bridge:?}");
        }
        for bridge in [
            " is used in ",
            " is compared with ",
            ". ",
            " may motivate the following equation ",
        ] {
            assert!(!formula_identification_bridge(bridge), "{bridge:?}");
        }
    }

    #[test]
    fn ordinary_by_phrases_are_not_matrix_dimensions() {
        assert_eq!(
            shape_claim("is defined by Here the current is a scalar"),
            Some((ProseShape::Scalar, Vec::new()))
        );
    }

    #[test]
    fn descriptive_passives_do_not_redeclare_their_subjects() {
        let analysis = analyze(
            "Current $i$ is referenced leaving the positive-voltage terminal. We write $i$ for electric current scalar.",
        );
        assert!(analysis.definitions.iter().any(|definition| {
            definition.symbol == "i"
                && classify_role(&definition.description).as_deref()
                    == Some("quantities-units:electric-current")
        }));
        assert!(!analysis.definitions.iter().any(|definition| {
            definition.symbol == "i"
                && classify_role(&definition.description).as_deref()
                    == Some("quantities-units:voltage")
        }));
    }

    #[test]
    fn shared_shape_heads_do_not_overwrite_explicit_mixed_shapes() {
        assert_eq!(
            expand_shared_shape_head(vec![
                "system state",
                "n by n state matrix",
                "control input vectors",
            ]),
            [
                "system state vector",
                "n by n state matrix",
                "control input vectors",
            ]
        );
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
    fn shared_nominal_heads_type_coordinated_assumption_subjects() {
        let analysis = analyze(
            "Assume $A$ and $B$ are finite sets of respondents who selected alpha and beta, respectively.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| {
                (
                    definition.symbol.as_str(),
                    definition.description.as_str(),
                    definition.evidence.rule_id.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            definitions.iter().any(|(symbol, description, _)| {
                *symbol == "A" && *description == "finite sets of respondents"
            }),
            "{definitions:?}"
        );
        assert!(
            definitions.iter().any(|(symbol, description, _)| {
                *symbol == "B" && *description == "finite sets of respondents"
            }),
            "{definitions:?}"
        );
    }

    #[test]
    fn attaches_a_leading_apposition_before_a_later_formula() {
        let analysis = analyze("$x$, the system state vector, evolves according to $\\dot{x}=Ax$.");
        assert!(
            analysis
                .definitions
                .iter()
                .any(|definition| definition.symbol == "x"
                    && definition.description == "system state vector"),
            "{:?}",
            analysis.definitions
        );
    }

    #[test]
    fn carries_an_ordered_pronoun_across_a_semicolon() {
        let analysis = analyze(
            "The quantities are $p$, $V$, and $T$; they denote pressure, volume, and temperature, respectively. Thus $pV/T$ is considered.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        for expected in [("p", "pressure"), ("V", "volume"), ("T", "temperature")] {
            assert!(definitions.contains(&expected), "{definitions:?}");
        }
    }

    #[test]
    fn resolves_bounded_former_latter_and_demonstrative_definitions() {
        let former_latter = analyze(
            "We compare $u$ and $v$. The former denotes the input vector and the latter the output vector.",
        );
        let definitions = former_latter
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        assert!(
            definitions.contains(&("u", "input vector")),
            "{definitions:?}"
        );
        assert!(
            definitions.contains(&("v", "output vector")),
            "{definitions:?}"
        );

        let singular = analyze("Let $r$ be fixed. This quantity represents the residual norm.");
        assert!(singular.definitions.iter().any(|definition| {
            definition.symbol == "r" && definition.description == "residual norm"
        }));

        let semicolon = analyze(
            "Let $x$ and $u$ be introduced. The former is the state vector; the latter is the control input.",
        );
        assert!(
            semicolon.definitions.iter().any(|definition| {
                definition.symbol == "u" && definition.description == "control input"
            }),
            "{:?}",
            semicolon.definitions
        );

        let equation = analyze(
            "Let $J$ be defined by $J=-D\\nabla c$. This quantity represents species flux, while $D$ is diffusivity and $c$ concentration.",
        );
        assert!(
            equation.definitions.iter().any(|definition| {
                definition.symbol == "J"
                    && definition.description.starts_with("species flux")
                    && definition.evidence.kind == "attached-prose"
            }),
            "{:?}",
            equation.definitions
        );
    }

    #[test]
    fn normalizes_spelled_numeric_extents_before_a_following_clause() {
        let analysis = analyze(
            "Let $x$ be a three-dimensional vector and let $x=y$. Inspect $y$ after the equality.",
        );
        assert!(
            analysis.shapes.iter().any(|claim| {
                claim.symbol == "x" && claim.shape == ProseShape::Vector("3".into())
            }),
            "definitions={:?}, shapes={:?}",
            analysis.definitions,
            analysis.shapes
        );
    }

    #[test]
    fn refuses_ambiguous_or_nonasserted_anaphora() {
        let ambiguous = analyze(
            "$a$, $b$, and $c$ are compared. The former denotes the input and the latter the output.",
        );
        assert!(!ambiguous.definitions.iter().any(|definition| {
            definition.evidence.rule_id == "english-former-latter-definition"
        }));

        let hedged =
            analyze("We compare $u$ and $v$. The former might denote input and the latter output.");
        assert!(!hedged.definitions.iter().any(|definition| {
            definition.evidence.rule_id == "english-former-latter-definition"
        }));
    }

    #[test]
    fn attaches_asserted_equation_leads_and_followups() {
        for source in [
            "We define total energy by the following equation.\n$E=mc^2$",
            "$J=J_d+J_r$. This equation defines the total objective.",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis.definitions.iter().any(|definition| {
                    definition.evidence.rule_id == "english-equation-flow-definition"
                        && matches!(definition.symbol.as_str(), "E" | "J")
                }),
                "{source}: {:?}",
                analysis.definitions
            );
        }

        let cited =
            analyze("$J=J_d+J_r$. According to [1], this equation defines the total objective.");
        assert!(!cited.definitions.iter().any(|definition| {
            definition.evidence.rule_id == "english-equation-flow-definition"
        }));
    }

    #[test]
    fn construction_targets_attach_named_result_and_definition_formulas() {
        for source in [
            "The screening Reynolds number is therefore\n\\[\\mathrm{Re}_{D_h}=\\frac{\\rho v D_h}{\\mu}.\\]",
            "The model stage defines the transfer function in the Laplace domain as\n\\[H(s)=\\frac{Y(s)}{X(s)}.\\]",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis
                    .semantic_evidence
                    .law_activations
                    .iter()
                    .any(|activation| activation.identifies_attached_formula
                        && !activation.attached_formula_ranges.is_empty()),
                "{source}: {:?}",
                analysis.semantic_evidence.law_activations
            );
        }
    }

    #[test]
    fn extracts_differentiability_from_typed_function_space_membership() {
        let analysis = analyze("Suppose $y\\in C^1([a,b])$. For $x\\in(a,b)$, inspect $y'(x)$.");
        assert!(analysis.assumptions.iter().any(|assumption| {
            assumption.kind == "regularity"
                && assumption.value == "differentiable"
                && assumption.subjects == ["y"]
                && assumption.evidence.kind == "typed-math-condition"
        }));

        for source in [
            "Suppose $y\\notin C^1([a,b])$.",
            "According to the reference, $y\\in C^1([a,b])$.",
            "The space $C^1([a,b])$ is used for comparison.",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis
                    .assumptions
                    .iter()
                    .all(|assumption| assumption.evidence.kind != "typed-math-condition"),
                "{source}: {:?}",
                analysis.assumptions
            );
        }
    }

    #[test]
    fn construction_targets_preserve_nonestablishing_formula_leads() {
        for source in [
            "The model might define the transfer function as\n\\[H(s)=\\frac{Y(s)}{X(s)}.\\]",
            "According to the reference, the model defines the transfer function as\n\\[H(s)=\\frac{Y(s)}{X(s)}.\\]",
            "The model does not define the transfer function as\n\\[H(s)=\\frac{Y(s)}{X(s)}.\\]",
            "The model defines the transfer function. Background separates the formula.\n\\[H(s)=\\frac{Y(s)}{X(s)}.\\]",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis
                    .semantic_evidence
                    .law_activations
                    .iter()
                    .all(|activation| !activation.identifies_attached_formula),
                "{source}: {:?}",
                analysis.semantic_evidence.law_activations
            );
        }

        let alternative = analyze(
            "At this point the specification permits either\n\\[R=A\\cap B\\qquad\\text{or}\\qquad R=A\\cup B.\\]",
        );
        assert!(
            alternative
                .semantic_evidence
                .law_activations
                .iter()
                .all(|activation| activation.attached_formula_ranges.is_empty()),
            "{:?}",
            alternative.semantic_evidence.law_activations
        );
    }

    #[test]
    fn composes_typed_result_actions_across_soft_wraps_and_display_math() {
        for (source, description) in [
            (
                "We computed\nthe discharged mass per unit time as\n\\[\n\\dot m=\\rho A v.\n\\]",
                "discharged mass per unit time",
            ),
            (
                "The samples allow the corresponding mass rate\nto be written as\n\\[\n\\dot m=\\rho Q.\n\\]",
                "corresponding mass rate",
            ),
            (
                "Density was sampled at the same temperature, allowing the corresponding mass rate\nto be written as\n\\[\n\\dot m=\\rho Q=\\rho A v.\n\\]",
                "corresponding mass rate",
            ),
            (
                "The volume reduction is\n\\[Q=A v.\\]\nDensity was sampled at the same temperature, allowing the corresponding mass rate\nto be written as\n\\[\n\\dot m=\\rho Q=\\rho A v.\n\\]",
                "corresponding mass rate",
            ),
            (
                "We calculated the per-example binary\ncross-entropy as\n\\[\nL=-y\\log p.\n\\]",
                "per-example binary cross-entropy",
            ),
        ] {
            let analysis = analyze(source);
            assert!(
                analysis.definitions.iter().any(|definition| {
                    definition.description == description
                        && definition.evidence.rule_id == "english-equation-flow-definition"
                }),
                "{source}: {:?}",
                analysis.definitions
            );
        }
    }

    #[test]
    fn attaches_typed_transformation_outputs_to_equation_results_only() {
        for action in ["returns", "produces", "yields"] {
            let source = format!(
                "The sealed transformation $F(z)$ {action} calibrated range:\n\\[r=F(z).\\]"
            );
            let analysis = analyze(&source);
            assert!(
                analysis.definitions.iter().any(|definition| {
                    definition.symbol == "r"
                        && definition.description == "calibrated range"
                        && definition.evidence.rule_id == "english-output-definition"
                }),
                "{source}: {:?}",
                analysis.definitions
            );
            assert!(analysis.definitions.iter().all(|definition| {
                definition.evidence.rule_id != "english-output-definition"
                    || definition.symbol == "r"
            }));
        }
    }

    #[test]
    fn attaches_outputs_through_matching_opaque_macro_calls() {
        let source = "The sealed transformation $\\vendorcal{z}$ returns calibrated range:\n\\[r=\\vendorcal{z}.\\]";
        let mut document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: test_math_regions(source, DocumentLanguage::Latex),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let invocation = "\\vendorcal{z}";
        document.macros = source
            .match_indices(invocation)
            .map(|(start, _)| {
                let input_range = SourceRange {
                    start_offset: start as u32,
                    end_offset: (start + invocation.len()) as u32,
                };
                ProjectMacro {
                    kind: ProjectMacroKind::Call,
                    name: "vendorcal".into(),
                    source: ProjectSourceRef {
                        file_id: "main".into(),
                        path: "main.tex".into(),
                        range: SourceRange {
                            start_offset: start as u32,
                            end_offset: (start + "\\vendorcal".len()) as u32,
                        },
                    },
                    definitions: Vec::new(),
                    expansion: ProjectMacroExpansion {
                        status: ProjectMacroExpansionStatus::Expanded,
                        depth: 0,
                        editable: false,
                        surface: Some("\\mathsf{VendorCal}(z)".into()),
                        input_range: Some(input_range),
                        notation: None,
                    },
                }
            })
            .collect();

        let analysis = analyze_document(&document);
        assert!(analysis.definitions.iter().any(|definition| {
            definition.symbol == "r"
                && definition.description == "calibrated range"
                && definition.evidence.rule_id == "english-output-definition"
        }));
    }

    #[test]
    fn refuses_nonasserted_or_ambiguous_transformation_outputs() {
        for source in [
            "According to [1], the transform $F(z)$ returns calibrated range:\n\\[r=F(z).\\]",
            "The transform $F(z)$ might return calibrated range:\n\\[r=F(z).\\]",
            "The transform $F(z)$ returns either calibrated range or bearing:\n\\[r=F(z).\\]",
            "The transform $F(z)$ returns calibrated range:\n\\[r=G(z).\\]",
            "The transform $F(z)$ returns calibrated range:\n\\[r=z+1.\\]",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis.definitions.iter().all(|definition| {
                    definition.evidence.rule_id != "english-output-definition"
                }),
                "{source}: {:?}",
                analysis.definitions
            );
        }
    }

    #[test]
    fn refuses_mismatched_opaque_macro_outputs() {
        let source =
            "The transform $\\vendorcal{z}$ returns calibrated range:\n\\[r=\\vendorcal{x}.\\]";
        let mut document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: test_math_regions(source, DocumentLanguage::Latex),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        document.macros = [
            ("\\vendorcal{z}", "\\mathsf{VendorCal}(z)"),
            ("\\vendorcal{x}", "\\mathsf{VendorCal}(x)"),
        ]
        .into_iter()
        .map(|(invocation, surface)| {
            let start = source.find(invocation).unwrap();
            ProjectMacro {
                kind: ProjectMacroKind::Call,
                name: "vendorcal".into(),
                source: ProjectSourceRef {
                    file_id: "main".into(),
                    path: "main.tex".into(),
                    range: SourceRange {
                        start_offset: start as u32,
                        end_offset: (start + "\\vendorcal".len()) as u32,
                    },
                },
                definitions: Vec::new(),
                expansion: ProjectMacroExpansion {
                    status: ProjectMacroExpansionStatus::Expanded,
                    depth: 0,
                    editable: false,
                    surface: Some(surface.into()),
                    input_range: Some(SourceRange {
                        start_offset: start as u32,
                        end_offset: (start + invocation.len()) as u32,
                    }),
                    notation: None,
                },
            }
        })
        .collect();

        assert!(
            analyze_document(&document)
                .definitions
                .iter()
                .all(|definition| { definition.evidence.rule_id != "english-output-definition" })
        );
    }

    #[test]
    fn composes_a_result_named_before_its_equation() {
        let source = "Dividing by the interval gives the volumetric flow rate $Q$,\n\\[Q=Av.\\]";
        let analysis = analyze(source);

        assert!(
            analysis.definitions.iter().any(|definition| {
                definition.symbol == "Q"
                    && classify_role(&definition.description).as_deref()
                        == Some("quantities-units:volumetric-flow-rate")
            }),
            "{:?}",
            analysis.definitions
        );
    }

    #[test]
    fn does_not_assign_formula_metadescriptions_to_relation_heads() {
        for description in [
            "resistance check",
            "energy balance",
            "stability condition",
            "governing equation",
            "candidate formula",
        ] {
            let source = format!("The {description} is\n\\[V=RI.\\]");
            let analysis = analyze(&source);
            assert!(
                !analysis.definitions.iter().any(|definition| {
                    definition.symbol == "V"
                        && definition.evidence.rule_id == "english-equation-flow-definition"
                }),
                "{description}: {:?}",
                analysis.definitions
            );
        }
    }

    #[test]
    fn composes_active_definition_events_without_collapsing_explicit_ambiguity() {
        let analysis =
            analyze("The notation table calls $K$ both kinetic energy and stiffness; $m$ is mass.");
        assert!(
            analysis.definitions.iter().any(|definition| {
                definition.symbol == "K"
                    && definition.description == "both kinetic energy and stiffness"
                    && definition.evidence.rule_id == "english-relational-definition"
            }),
            "{:#?}",
            analysis.definitions
        );
    }

    #[test]
    fn refuses_untyped_or_nonasserted_result_actions() {
        for source in [
            "We computed the diagnostic summary as\n\\[x=y.\\]",
            "We might compute the mass rate as\n\\[Q=Av.\\]",
            "According to [1], we computed the mass rate as\n\\[Q=Av.\\]",
        ] {
            let analysis = analyze(source);
            assert!(
                !analysis.definitions.iter().any(|definition| {
                    definition.evidence.rule_id == "english-equation-flow-definition"
                }),
                "{source}: {:?}",
                analysis.definitions
            );
        }
    }

    #[test]
    fn marks_explicitly_withdrawn_inline_formulas_as_negative_evidence() {
        let source = "The review no longer publishes the preliminary estimate $Q=Av$.";
        let analysis = analyze(source);
        let formula = test_math_regions(source, DocumentLanguage::Latex)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(
            analysis
                .semantic_evidence
                .formula_disposition(&formula.content_range),
            (
                crate::semantic_index::EvidencePolarity::Negative,
                crate::semantic_index::EvidenceModality::Asserted,
            )
        );
    }

    #[test]
    fn refuses_equation_flow_attachment_that_contradicts_an_existing_role() {
        let source = r"The rotameter reports volumetric flow $Q$. Define the mean axial speed by the flow relation
\[Q=Av.\]";
        let analysis = analyze(source);
        assert!(analysis.definitions.iter().any(|definition| {
            definition.symbol == "Q"
                && classify_role(&definition.description).as_deref()
                    == Some("quantities-units:volumetric-flow-rate")
        }));
        assert!(!analysis.definitions.iter().any(|definition| {
            definition.symbol == "Q"
                && classify_role(&definition.description).as_deref()
                    == Some("quantities-units:velocity")
        }));
    }

    #[test]
    fn equation_flow_role_conflicts_respect_source_order_and_scope() {
        for source in [
            r"Define the mean axial speed by the flow relation
\[Q=Av.\]
The rotameter later reports volumetric flow $Q$.",
            r"\section{Flow}
The rotameter reports volumetric flow $Q$.
\section{Velocity}
Define the mean axial speed by the flow relation
\[Q=Av.\]",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis.definitions.iter().any(|definition| {
                    definition.symbol == "Q"
                        && definition.evidence.rule_id == "english-equation-flow-definition"
                        && classify_role(&definition.description).as_deref()
                            == Some("quantities-units:velocity")
                }),
                "{source}: {:?}",
                analysis.definitions
            );
        }
    }

    #[test]
    fn attaches_definitions_through_an_explicit_equation_reference() {
        for reference in ["\\ref{eq:flux}", "\\eqref{eq:flux}"] {
            let source = format!(
                "$q=-k\\nabla T\\label{{eq:flux}}$ In Equation~{reference}, $q$ denotes heat flux, $k$ thermal conductivity, and $T$ temperature."
            );
            let analysis = analyze(&source);
            assert_eq!(analysis.definitions.len(), 3, "{:?}", analysis.definitions);
            assert!(analysis.definitions.iter().all(|definition| {
                definition.evidence.kind == "attached-prose"
                    && definition.evidence.source_ranges.len() == 2
            }));
        }
    }

    #[test]
    fn postposed_formula_reference_requires_a_typed_head_and_bounded_adjacency() {
        let attached = analyze("$P=Fv$. This relation is the mechanical power relation.");
        assert!(
            attached
                .semantic_evidence
                .law_activations
                .iter()
                .any(|activation| {
                    activation.law_id == "mechanical-power"
                        && !activation.attached_formula_ranges.is_empty()
                })
        );

        for source in [
            "$P=Fv$. According to this study, the mechanical power relation is useful.",
            "$P=Fv$. Background material separates the statements. This relation is the mechanical power relation.",
        ] {
            let analysis = analyze(source);
            assert!(
                analysis
                    .semantic_evidence
                    .law_activations
                    .iter()
                    .all(|activation| {
                        activation.law_id != "mechanical-power"
                            || activation.attached_formula_ranges.is_empty()
                    }),
                "{source}"
            );
        }
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
        assert!(descriptions.contains(&("p", "$d$")), "{descriptions:?}");
        assert!(descriptions.contains(&("q", "$e$")));
        assert!(descriptions.contains(&("r", "$f$")), "{descriptions:?}");
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
        assert!(definitions.contains(&("g", "gradient")), "{definitions:?}");
    }

    #[test]
    fn maps_elided_copulas_across_parallel_declarations() {
        let analysis = analyze(
            "Let $h$ be heat transfer, $m$ mass, $s$ specific heat, and $d$ temperature change. Then $h=msd$.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        for expected in [
            ("h", "heat transfer"),
            ("m", "mass"),
            ("s", "specific heat"),
            ("d", "temperature change"),
        ] {
            assert!(definitions.contains(&expected), "{definitions:?}");
        }
    }

    #[test]
    fn keeps_declarations_independent_from_nonsemantic_neighbor_lines() {
        let analysis = analyze(
            "For comparison, inspect the displayed expression numbered 1026. [given that]\nA presentation-only symbol follows: \\[q_{1226}\\]\n\nLet $p_{26}$ be an $n$-state, $c_{26}$ an $m$-input, $R_{26}$ an $n\\times n$ matrix, and $S_{26}$ an $n\\times m$ matrix.\nLet $p_{26}$ denote state vector, $R_{26}$ denote state matrix, $S_{26}$ denote input matrix, and $c_{26}$ denote control input vector.\n\\[\\dot{p_{26}}=\\bigl(R_{26}p_{26}\\bigr)+\\bigl(S_{26}c_{26}\\bigr)\\]",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        for expected in [
            ("p_26", "state vector"),
            ("R_26", "state matrix"),
            ("S_26", "input matrix"),
            ("c_26", "control input vector"),
        ] {
            assert!(definitions.contains(&expected), "{definitions:?}");
        }
        let state_shapes = analysis
            .shapes
            .iter()
            .filter(|claim| claim.symbol == "p_26")
            .map(|claim| &claim.shape)
            .collect::<Vec<_>>();
        assert!(
            state_shapes
                .iter()
                .all(|shape| matches!(shape, ProseShape::Vector(_))),
            "{state_shapes:?}",
        );
    }

    #[test]
    fn recognizes_compact_vector_dimensions_with_a_semantic_head() {
        let analysis = analyze("Let $K$ be a three-vector of energies.");
        assert!(
            analysis.shapes.iter().any(|claim| {
                claim.symbol == "K" && claim.shape == ProseShape::Vector("3".into())
            })
        );
    }

    #[test]
    fn maps_fronted_plural_types_without_domain_specific_grammar() {
        let analysis = analyze(
            "Events $C$ and $D$ belong to one probability space. Sets $S$ and $T$ are defined on a common universe. Vectors $u$ and $v$ share one coordinate frame. For two events $A$ and $B$, consider their joint occurrence. Given matrices $M$ and $N$, compare their spectra. Take $U$ and $V$ as sets.",
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
            ("A", "events"),
            ("B", "events"),
            ("M", "matrices"),
            ("N", "matrices"),
            ("U", "sets"),
            ("V", "sets"),
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
    fn composes_role_first_nominals_from_shared_description_events() {
        let source = concat!(
            "For a segment of mass $m$ moving at speed $v$, the review continues. ",
            "A charge packet with signed charge $q_b$ enters a region held at electric potential $V_b$. ",
            "The saline density $\\rho$ was measured. For each nozzle, the optical diameter ",
            "supplied the area $A$, while particle tracking supplied the area-mean exit speed $v_e$."
        );
        let analysis = analyze(source);
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        for expected in [
            ("m", "mass"),
            ("v", "speed"),
            ("q_b", "signed charge"),
            ("V_b", "electric potential"),
            ("rho", "saline density"),
            ("A", "area"),
            ("v_e", "area-mean exit speed"),
        ] {
            assert!(
                definitions.contains(&expected),
                "{expected:?}: {definitions:?}"
            );
        }
    }

    #[test]
    fn local_description_events_survive_soft_wrapped_coordination() {
        let analysis = analyze(
            "The ultrasonic meter reports the cross-section mean speed $v$, and the machined\nbore determines the wetted area $A$.",
        );
        let definitions = analysis
            .definitions
            .iter()
            .map(|definition| (definition.symbol.as_str(), definition.description.as_str()))
            .collect::<Vec<_>>();
        assert!(
            definitions.contains(&("v", "cross-section mean speed")),
            "{definitions:?}"
        );
        assert!(
            definitions.contains(&("A", "wetted area")),
            "{definitions:?}"
        );
    }

    #[test]
    fn precise_role_first_nominals_outrank_broad_trailing_clause_guesses() {
        let analysis = analyze(
            "For a velocity field that is uniform over a cross-section, the volume crossing the section in a time interval $\\Delta t$ is the area $A$ times the travelled distance $v\\Delta t$.",
        );
        let definition = analysis
            .definitions
            .iter()
            .find(|definition| definition.symbol == "A")
            .expect("area definition");
        assert_eq!(
            definition.description, "area",
            "{:#?}",
            analysis.definitions
        );
        assert_eq!(
            definition.evidence.rule_id,
            "english-role-first-nominal-definition"
        );
    }

    #[test]
    fn nominal_composition_refuses_untyped_incidental_neighbors() {
        let analysis = analyze(
            "Compare plain $y$ with $z$. The archived channel $r$ is mentioned without a scientific role.",
        );
        assert!(
            analysis.definitions.is_empty(),
            "{:?}",
            analysis.definitions
        );
    }

    #[test]
    fn role_first_nominals_refuse_a_clause_that_ends_in_a_relation_word() {
        let source = "At any settled observation write this depth simply as $h$.";
        let analysis = analyze(source);
        assert!(
            analysis.definitions.is_empty(),
            "{:#?}",
            analysis.definitions
        );
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
