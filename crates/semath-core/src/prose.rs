use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use crate::canonical::{SemanticExpr, SemanticExprKind, declared_symbols};
use crate::construction::{
    coordinated_descriptions, coordination_lead, defines_by_formula, fronted_labeled_descriptions,
    fronted_shared_description, is_declaration_lead, match_apposition, match_definition,
    match_fronted_single, match_parenthetical, match_passive_definition, match_quantified,
};
use crate::pack::{PackActivationStructure, built_in_packs};
use crate::parser::ParsedMath;
use crate::scientific_prose::{
    DiscourseFrame, ScientificClause, ScientificMention, align_ordered_descriptions, clause_at,
    extract_assumptions, segment_scientific_clauses,
};
use crate::{
    AssumptionInfo, DefinitionInfo, Evidence, Location, ProjectDocument, SourceIndex, SourceRange,
};

static VECTOR_DIMENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z]|[0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)[ -]dimensional\s*(?:[a-z][a-z-]*\s+){0,4}(?:vectors?|states?|inputs?|controls?)(?:\s+of\s+[a-z -]+)?\s*(?:,?\s+and)?\s*$")
        .unwrap()
});
static MATRIX_DIMENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b([a-z0-9]+)(?:\s+by\s+|\s+x\s+|\s*×\s*|\s*\\times\s*)([a-z0-9]+)(?:\s+(?:[a-z][a-z-]*\s+){0,4}(?:matrix|matrices))?\s*(?:,?\s+and)?\s*$",
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
}

impl ScientificSemanticEvidence {
    pub fn formula_is_asserted(&self, range: &SourceRange) -> bool {
        self.clause_for(range)
            .is_none_or(|clause| clause.frame.establishes())
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
            .find(|activation| ranges_overlap(&activation.clause_range, range))
        {
            return Some(activation);
        }

        // Scientific prose commonly names a law or model, declares its roles,
        // and puts the asserted equation in the immediately following clause.
        // Keep this attachment local: do not let a law name leak across another
        // clause, section, or unrelated formula later in the document.
        let formula_clause = self.clause_for(range)?;
        let previous_clause = self
            .clauses
            .iter()
            .filter(|clause| clause.range.end_offset <= formula_clause.range.start_offset)
            .max_by_key(|clause| clause.range.end_offset)?;
        self.law_activations
            .iter()
            .filter(matching)
            .find(|activation| {
                activation.clause_range == previous_clause.range
                    && previous_clause.frame.establishes()
            })
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
    analysis.semantic_evidence =
        collect_semantic_evidence(document, source, &index, &clauses, canonical_expressions);
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
    collect_cross_clause_ordered_definitions(document, parsed, &index, &clauses, &mut analysis);
    collect_clause_definitions(document, source, parsed, &index, &clauses, &mut analysis);
    for math in parsed {
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
        } else if let Some(fronted) = match_fronted_single(before, after) {
            push_claim(
                &mut analysis,
                document,
                &index,
                &symbol,
                &symbol_range,
                fronted.description,
                fronted.rule_id,
                before_start + fronted.prefix_start,
                end_byte + fronted.suffix_end,
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
        let reference = format!("\\ref{{{key}}}");
        let Some(reference_relative) = source[search_from..].find(&reference) else {
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

fn collect_cross_clause_ordered_definitions(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    output: &mut ProseObservations,
) {
    for pair in clauses.windows(2) {
        let [symbols_clause, description_clause] = pair else {
            continue;
        };
        if !symbols_clause.frame.establishes() || !description_clause.frame.establishes() {
            continue;
        }
        let text = description_clause.text.trim();
        let lower = text.to_ascii_lowercase();
        let Some(lead) = ["they denote ", "these denote ", "they represent "]
            .into_iter()
            .find(|lead| lower.starts_with(lead))
        else {
            continue;
        };
        let Some(marker) = lower.find("respectively") else {
            continue;
        };
        let body = text[lead.len()..marker].trim().trim_end_matches(',').trim();
        let symbols = parsed
            .iter()
            .filter(|math| {
                let start = index.byte_for_utf16(math.region.full_range.start_offset);
                symbols_clause.start <= start
                    && start < symbols_clause.end
                    && is_definition_slot_math(math)
            })
            .collect::<Vec<_>>();
        let Some(descriptions) = align_ordered_descriptions(body, symbols.len()) else {
            continue;
        };
        for (math, description) in symbols.into_iter().zip(descriptions) {
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
                "english-cross-clause-respectively-definition",
                symbols_clause.start,
                description_clause.end,
            );
        }
    }
}

fn collect_semantic_evidence(
    document: &ProjectDocument,
    source: &str,
    index: &SourceIndex,
    clauses: &[ScientificClause<'_>],
    canonical_expressions: &[SemanticExpr],
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
                for phrase in &law.activation_phrases {
                    for range in phrase_ranges(source, index, clause, phrase) {
                        law_activations.push(LawActivationEvidence {
                            pack_id: pack.pack_id.clone(),
                            law_id: law.id.clone(),
                            clause_range: SourceRange {
                                start_offset: index.utf16_for_byte(clause.start),
                                end_offset: index.utf16_for_byte(clause.end),
                            },
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
    }
}

fn evidence_range_key(evidence: &Evidence) -> (u32, u32) {
    evidence
        .source_ranges
        .first()
        .map_or((0, 0), |range| (range.start_offset, range.end_offset))
}

fn phrase_ranges(
    source: &str,
    index: &SourceIndex,
    clause: &ScientificClause<'_>,
    phrase: &str,
) -> Vec<SourceRange> {
    let lower = clause.text.to_ascii_lowercase();
    let phrase = phrase.to_ascii_lowercase();
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
        || expression_children(expression).any(|child| expression_has_structure(child, structure))
}

fn expression_children(expression: &SemanticExpr) -> impl Iterator<Item = &SemanticExpr> {
    let children: Vec<&SemanticExpr> = match &expression.kind {
        SemanticExprKind::Sum(items) | SemanticExprKind::Product(items) => items.iter().collect(),
        SemanticExprKind::Dot(left, right)
        | SemanticExprKind::Cross(left, right)
        | SemanticExprKind::Fraction(left, right) => vec![left, right],
        SemanticExprKind::Power(base, exponent) => vec![base, exponent],
        SemanticExprKind::Negate(inner)
        | SemanticExprKind::Derivative {
            expression: inner, ..
        } => {
            vec![inner]
        }
        SemanticExprKind::Relation { left, right, .. } => vec![left, right],
        SemanticExprKind::Apply { arguments, .. } => arguments.iter().collect(),
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Unknown(_) => Vec::new(),
    };
    children.into_iter()
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
    output: &mut ProseObservations,
) {
    for clause in clauses {
        output.match_stats.matcher_work += 1;
        if !clause.frame.establishes() {
            continue;
        }
        let sentence_start = clause.start;
        let sentence_end = clause.end;
        let sentence = clause.text;
        let sentence_lower = sentence.to_ascii_lowercase();
        if sentence_lower.contains("respectively") || sentence_lower.contains("in that order") {
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
                sentence_start <= start
                    && start < sentence_end
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
        // A leading copular/definition verb scopes over a coordinated list:
        // “q denotes heat flux, k conductivity, and T temperature”.  Requiring
        // the elided verb to be repeated for every item rejects ordinary
        // scientific English and also breaks definitions attached by an
        // equation reference.
        if !contextual && explicit < 2 && !first_is_explicit {
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
    if rule_id.contains("definition") {
        analysis.match_stats.construction_candidates += 1;
    }
    let evidence_range = SourceRange {
        start_offset: index.utf16_for_byte(evidence_start),
        end_offset: index.utf16_for_byte(evidence_end),
    };
    let attached = matches!(
        rule_id,
        "english-clause-definition" | "english-clause-ordered-definition"
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
    use crate::canonical::lower_document_region;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::ProseObservations {
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 7,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        observe_prose(&document, &parsed, &canonical)
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
    fn attaches_definitions_through_an_explicit_equation_reference() {
        let source = "$q=-k\\nabla T\\label{eq:flux}$ In Equation~\\ref{eq:flux}, $q$ denotes heat flux, $k$ thermal conductivity, and $T$ temperature.";
        let analysis = analyze(source);
        assert_eq!(analysis.definitions.len(), 3, "{:?}", analysis.definitions);
        assert!(analysis.definitions.iter().all(|definition| {
            definition.evidence.kind == "attached-prose"
                && definition.evidence.source_ranges.len() == 2
        }));
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
