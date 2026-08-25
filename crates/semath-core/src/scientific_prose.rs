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
        if self.conditionality != Conditionality::Unconditional
            || self.act == CommunicativeAct::Alternative
        {
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
    pub math_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefinitionAction {
    Define,
    Denote,
    Represent,
    Mean,
    Write,
    Call,
    Compute,
    Produce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefinitionLink {
    As,
    By,
    Copula,
    For,
    ToBe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscourseConnective {
    Where,
    Here,
    Thus,
    Hence,
    Therefore,
    Respectively,
    InThatOrder,
    Alternative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnaphorKind {
    SingularDemonstrative,
    FormulaDemonstrative,
    PluralDemonstrative,
    Former,
    Latter,
    PluralPronoun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProseEventKind {
    ClauseStart,
    ClauseEnd,
    MathMention(usize),
    DefinitionAction(DefinitionAction),
    DefinitionLink(DefinitionLink),
    DescriptionSpan,
    Coordination,
    Connective(DiscourseConnective),
    Anaphor(AnaphorKind),
    DiscourseFeature(DiscourseFeatureKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProseEvent {
    pub clause_index: usize,
    pub start: usize,
    pub end: usize,
    pub kind: ProseEventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProseEventStream {
    pub events: Vec<ProseEvent>,
    pub clause_mentions: Vec<Vec<usize>>,
    clause_anaphors: Vec<Vec<AnaphorKind>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DefinitionConstruction {
    pub action: DefinitionAction,
    pub action_precedes_mention: bool,
    pub mention_index: usize,
    pub description_start: usize,
    pub description_end: usize,
    pub evidence_start: usize,
    pub evidence_end: usize,
    pub frame: DiscourseFrame,
    pub coordinated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttachmentCandidate {
    pub mention_indices: Vec<usize>,
    pub evidence_start: usize,
    pub evidence_end: usize,
    pub distance_bytes: usize,
}

const MAX_ATTACHMENT_MENTIONS: usize = 8;
const MAX_ANAPHORIC_DISTANCE_BYTES: usize = 160;
pub(crate) const MAX_ATTACHMENT_DISTANCE_BYTES: usize = 320;
const MAX_EQUATION_FLOW_CLAUSES: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscourseConstruction {
    Definition(DefinitionConstruction),
    Anaphoric {
        antecedent_clause_index: usize,
        description_clause_index: usize,
        candidate: AttachmentCandidate,
        frame: DiscourseFrame,
    },
    EquationFlow {
        mention_index: usize,
        prose_start: usize,
        prose_end: usize,
        precedes_formula: bool,
        candidate: AttachmentCandidate,
        frame: DiscourseFrame,
    },
    OutputDefinition {
        producer_mention_index: usize,
        result_mention_index: usize,
        description_start: usize,
        description_end: usize,
        candidate: AttachmentCandidate,
        frame: DiscourseFrame,
    },
    AlternativeSelection {
        alternatives_clause_index: usize,
        selection_clause_index: usize,
        target_mention_index: usize,
        selected: bool,
        evidence_start: usize,
        evidence_end: usize,
    },
}

impl ProseEventStream {
    pub(crate) fn mentions_in_clause(&self, clause_index: usize) -> &[usize] {
        self.clause_mentions
            .get(clause_index)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn has_anaphor(&self, clause_index: usize) -> bool {
        self.clause_anaphors
            .get(clause_index)
            .is_some_and(|items| !items.is_empty())
    }

    pub(crate) fn has_anaphor_kind(&self, clause_index: usize, kind: AnaphorKind) -> bool {
        self.clause_anaphors
            .get(clause_index)
            .is_some_and(|items| items.contains(&kind))
    }

    pub(crate) fn starts_with_anaphor_kind(
        &self,
        clause_index: usize,
        clause_start: usize,
        kind: AnaphorKind,
    ) -> bool {
        self.events.iter().any(|event| {
            event.clause_index == clause_index
                && event.start == clause_start
                && event.kind == ProseEventKind::Anaphor(kind)
        })
    }

    pub(crate) fn first_event_after_is_anaphor(
        &self,
        clause_index: usize,
        lower_bound: usize,
        kind: AnaphorKind,
    ) -> bool {
        let first_start = self
            .events
            .iter()
            .filter(|event| event.clause_index == clause_index && lower_bound <= event.start)
            .filter(|event| {
                !matches!(
                    event.kind,
                    ProseEventKind::ClauseEnd | ProseEventKind::DiscourseFeature(_)
                )
            })
            .map(|event| event.start)
            .min();
        first_start.is_some_and(|start| {
            self.events.iter().any(|event| {
                event.clause_index == clause_index
                    && event.start == start
                    && event.kind == ProseEventKind::Anaphor(kind)
            })
        })
    }

    pub(crate) fn has_connective(
        &self,
        clause_index: usize,
        accepted: &[DiscourseConnective],
    ) -> bool {
        self.events.iter().any(|event| {
            event.clause_index == clause_index
                && matches!(event.kind, ProseEventKind::Connective(kind) if accepted.contains(&kind))
        })
    }

    pub(crate) fn first_connective(
        &self,
        clause_index: usize,
        accepted: &[DiscourseConnective],
    ) -> Option<&ProseEvent> {
        self.events.iter().find(|event| {
            event.clause_index == clause_index
                && matches!(event.kind, ProseEventKind::Connective(kind) if accepted.contains(&kind))
        })
    }

    pub(crate) fn last_definition_action(&self, start: usize, end: usize) -> Option<&ProseEvent> {
        self.events
            .iter()
            .filter(|event| {
                start <= event.start
                    && event.end <= end
                    && matches!(event.kind, ProseEventKind::DefinitionAction(_))
            })
            .max_by_key(|event| (event.start, event.end))
    }

    pub(crate) fn description_before(
        &self,
        source: &str,
        clause_index: usize,
        mention_start: usize,
    ) -> Option<(usize, usize)> {
        self.events
            .iter()
            .filter(|event| {
                (event.clause_index == clause_index
                    || event.clause_index.checked_add(1) == Some(clause_index))
                    && event.end <= mention_start
                    && mention_start - event.end <= 2
                    && source[event.end..mention_start]
                        .chars()
                        .all(char::is_whitespace)
                    && event.kind == ProseEventKind::DescriptionSpan
            })
            .max_by_key(|event| event.end)
            .map(|event| {
                let value = &source[event.start..event.end];
                let trailing = value.len() - value.trim_end().len();
                let end = if trailing > 0 && value[value.len() - trailing..].contains(['\n', '\r'])
                {
                    event.end - trailing
                } else {
                    event.end
                };
                (event.start, end)
            })
    }

    pub(crate) fn definition_constructions(
        &self,
        source: &str,
        mentions: &[ScientificMention],
        clauses: &[ScientificClause<'_>],
    ) -> Vec<DefinitionConstruction> {
        mentions
            .iter()
            .enumerate()
            .filter_map(|(mention_index, mention)| {
                let mention_event = self
                    .events
                    .iter()
                    .find(|event| event.kind == ProseEventKind::MathMention(mention_index))?;
                let clause = clauses.get(mention_event.clause_index)?;
                let action = self.nearest_definition_action(mention_event, mention)?;
                let action_precedes = action.end <= mention.start;
                let gap = if action_precedes {
                    &source[action.end..mention.start]
                } else {
                    &source[mention.end..action.start]
                };
                if !gap.chars().all(char::is_whitespace) {
                    return None;
                }
                let boundary = if action_precedes {
                    mention.end
                } else {
                    action.end
                };
                let description = self.events.iter().find(|event| {
                    event.clause_index == mention_event.clause_index
                        && event.kind == ProseEventKind::DescriptionSpan
                        && event.start <= boundary
                        && boundary <= event.end
                })?;
                let description_limit = self
                    .events
                    .iter()
                    .filter(|event| {
                        event.clause_index == mention_event.clause_index
                            && boundary <= event.start
                            && event.end <= description.end
                            && matches!(event.kind, ProseEventKind::DefinitionAction(_))
                    })
                    .min_by_key(|event| event.start)
                    .map_or(description.end, |following_action| {
                        self.events
                            .iter()
                            .filter(|event| {
                                event.clause_index == mention_event.clause_index
                                    && boundary <= event.start
                                    && event.end <= following_action.start
                                    && event.kind == ProseEventKind::Coordination
                                    && source[event.end..following_action.start]
                                        .chars()
                                        .all(char::is_whitespace)
                            })
                            .max_by_key(|event| event.start)
                            .map_or(following_action.start, |coordination| coordination.start)
                    });
                let description_start = self
                    .events
                    .iter()
                    .filter(|event| {
                        event.clause_index == mention_event.clause_index
                            && boundary <= event.start
                            && event.end <= description_limit
                            && matches!(event.kind, ProseEventKind::DefinitionLink(_))
                    })
                    .min_by_key(|event| event.start)
                    .map_or(boundary, |link| link.end);
                let description_end = self
                    .events
                    .iter()
                    .filter(|event| {
                        event.clause_index == mention_event.clause_index
                            && description_start < event.start
                            && event.end <= description_limit
                            && event.kind == ProseEventKind::Coordination
                    })
                    .max_by_key(|event| event.start)
                    .filter(|event| {
                        source[event.end..description_limit]
                            .chars()
                            .all(|character| character.is_whitespace() || character == ',')
                    })
                    .map_or(description_limit, |event| event.start);
                let (description_start, description_end) =
                    trim_range(source, description_start, description_end);
                let description_end =
                    trim_terminal_punctuation(source, description_start, description_end);
                let coordinated = self.events.iter().any(|event| {
                    event.clause_index == mention_event.clause_index
                        && event.start < mention.start
                        && event.kind == ProseEventKind::Coordination
                });
                (description_start < description_end).then_some(DefinitionConstruction {
                    action: match action.kind {
                        ProseEventKind::DefinitionAction(action) => action,
                        _ => unreachable!(),
                    },
                    action_precedes_mention: action_precedes,
                    mention_index,
                    description_start,
                    description_end,
                    evidence_start: action.start.min(mention.start),
                    evidence_end: description_limit,
                    frame: clause.frame.clone(),
                    coordinated,
                })
            })
            .collect()
    }

    pub(crate) fn discourse_constructions(
        &self,
        source: &str,
        mentions: &[ScientificMention],
        clauses: &[ScientificClause<'_>],
    ) -> Vec<DiscourseConstruction> {
        let mut constructions = self
            .definition_constructions(source, mentions, clauses)
            .into_iter()
            .map(DiscourseConstruction::Definition)
            .collect::<Vec<_>>();
        constructions.extend(self.anaphoric_constructions(clauses));
        let equation_flows = self.equation_flow_constructions(mentions, clauses);
        constructions.extend(self.output_definition_constructions(
            source,
            mentions,
            clauses,
            &equation_flows,
        ));
        constructions.extend(equation_flows);
        constructions.extend(self.alternative_selection_constructions(mentions, clauses));
        constructions
    }

    fn alternative_selection_constructions(
        &self,
        mentions: &[ScientificMention],
        clauses: &[ScientificClause<'_>],
    ) -> Vec<DiscourseConstruction> {
        clauses
            .iter()
            .enumerate()
            .filter_map(|(selection_clause_index, clause)| {
                let words = clause
                    .text
                    .to_ascii_lowercase()
                    .split(|character: char| !character.is_ascii_alphabetic())
                    .filter(|word| !word.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let selected = if words
                    .iter()
                    .any(|word| matches!(word.as_str(), "neither" | "none"))
                    && words.iter().any(|word| {
                        word.starts_with("select")
                            || word.starts_with("choos")
                            || word.starts_with("adopt")
                    }) {
                    false
                } else {
                    return None;
                };
                let alternatives_clause_index = selection_clause_index.checked_sub(1)?;
                let alternative_events = self
                    .events
                    .iter()
                    .filter(|event| {
                        event.clause_index == alternatives_clause_index
                            && event.kind
                                == ProseEventKind::Connective(DiscourseConnective::Alternative)
                    })
                    .count();
                if alternative_events < 2 {
                    return None;
                }
                let (target_mention_index, target) = mentions
                    .iter()
                    .enumerate()
                    .filter(|(_, mention)| {
                        clause.end <= mention.start
                            && mention.start - clause.end <= MAX_ATTACHMENT_DISTANCE_BYTES
                    })
                    .min_by_key(|(_, mention)| mention.start)?;
                Some(DiscourseConstruction::AlternativeSelection {
                    alternatives_clause_index,
                    selection_clause_index,
                    target_mention_index,
                    selected,
                    evidence_start: clause.start,
                    evidence_end: target.start,
                })
            })
            .collect()
    }

    fn output_definition_constructions(
        &self,
        source: &str,
        mentions: &[ScientificMention],
        clauses: &[ScientificClause<'_>],
        equation_flows: &[DiscourseConstruction],
    ) -> Vec<DiscourseConstruction> {
        self.events
            .iter()
            .filter(|event| {
                event.kind == ProseEventKind::DefinitionAction(DefinitionAction::Produce)
            })
            .filter_map(|action| {
                let clause = clauses.get(action.clause_index)?;
                let producer_mention_index = self
                    .mentions_in_clause(action.clause_index)
                    .iter()
                    .copied()
                    .filter(|mention_index| mentions[*mention_index].end <= action.start)
                    .max_by_key(|mention_index| mentions[*mention_index].end)?;
                let producer = &mentions[producer_mention_index];
                if !source[producer.end..action.start]
                    .chars()
                    .all(|character| character.is_whitespace() || matches!(character, ','))
                {
                    return None;
                }
                let flow = equation_flows
                    .iter()
                    .filter_map(|construction| {
                        let DiscourseConstruction::EquationFlow {
                            mention_index,
                            prose_start,
                            prose_end,
                            precedes_formula: true,
                            candidate,
                            ..
                        } = construction
                        else {
                            return None;
                        };
                        (*prose_start <= action.start
                            && action.end <= *prose_end
                            && producer_mention_index < *mention_index)
                            .then_some((*mention_index, candidate))
                    })
                    .min_by_key(|(_, candidate)| candidate.distance_bytes)?;
                if self
                    .mentions_in_clause(action.clause_index)
                    .iter()
                    .any(|mention_index| {
                        *mention_index != flow.0 && action.end <= mentions[*mention_index].start
                    })
                {
                    return None;
                }
                let (description_start, description_end) =
                    trim_range(source, action.end, clause.end.min(mentions[flow.0].start));
                let description_end =
                    trim_terminal_punctuation(source, description_start, description_end);
                if description_start >= description_end {
                    return None;
                }
                if self.events.iter().any(|event| {
                    event.clause_index == action.clause_index
                        && description_start <= event.start
                        && event.end <= description_end
                        && event.kind
                            == ProseEventKind::Connective(DiscourseConnective::Alternative)
                }) {
                    return None;
                }
                let mut candidate = flow.1.clone();
                candidate.evidence_start = producer.start;
                Some(DiscourseConstruction::OutputDefinition {
                    producer_mention_index,
                    result_mention_index: flow.0,
                    description_start,
                    description_end,
                    candidate,
                    frame: clause.frame.clone(),
                })
            })
            .collect()
    }

    fn anaphoric_constructions(
        &self,
        clauses: &[ScientificClause<'_>],
    ) -> Vec<DiscourseConstruction> {
        clauses
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(description_clause_index, _)| self.has_anaphor(*description_clause_index))
            .filter_map(|(description_clause_index, description_clause)| {
                let antecedent_clause_index = description_clause_index - 1;
                let antecedent_clause = &clauses[antecedent_clause_index];
                let distance_bytes = description_clause
                    .start
                    .saturating_sub(antecedent_clause.end);
                if distance_bytes > MAX_ANAPHORIC_DISTANCE_BYTES {
                    return None;
                }
                let mention_indices = self.mentions_in_clause(antecedent_clause_index).to_vec();
                if mention_indices.is_empty() || mention_indices.len() > MAX_ATTACHMENT_MENTIONS {
                    return None;
                }
                Some(DiscourseConstruction::Anaphoric {
                    antecedent_clause_index,
                    description_clause_index,
                    candidate: AttachmentCandidate {
                        mention_indices,
                        evidence_start: antecedent_clause.start,
                        evidence_end: description_clause.end,
                        distance_bytes,
                    },
                    frame: description_clause.frame.clone(),
                })
            })
            .collect()
    }

    fn equation_flow_constructions(
        &self,
        mentions: &[ScientificMention],
        clauses: &[ScientificClause<'_>],
    ) -> Vec<DiscourseConstruction> {
        mentions
            .iter()
            .enumerate()
            .flat_map(|(mention_index, mention)| {
                equation_flow_windows(clauses, mentions, mention_index, mention)
                    .into_iter()
                    .filter_map(move |(prose_start, prose_end, precedes_formula)| {
                        let clause = clause_at(clauses, prose_start).or_else(|| {
                            clauses.iter().find(|clause| {
                                prose_start <= clause.start && clause.start < prose_end
                            })
                        })?;
                        Some(DiscourseConstruction::EquationFlow {
                            mention_index,
                            prose_start,
                            prose_end,
                            precedes_formula,
                            candidate: AttachmentCandidate {
                                mention_indices: vec![mention_index],
                                evidence_start: prose_start.min(mention.start),
                                evidence_end: prose_end.max(mention.end),
                                distance_bytes: if precedes_formula {
                                    mention.start.saturating_sub(prose_end)
                                } else {
                                    prose_start.saturating_sub(mention.end)
                                },
                            },
                            frame: clause.frame.clone(),
                        })
                    })
            })
            .collect()
    }

    fn nearest_definition_action<'a>(
        &'a self,
        mention_event: &ProseEvent,
        mention: &ScientificMention,
    ) -> Option<&'a ProseEvent> {
        let accepted = |event: &&ProseEvent| {
            event.clause_index == mention_event.clause_index
                && matches!(
                    event.kind,
                    ProseEventKind::DefinitionAction(
                        DefinitionAction::Define
                            | DefinitionAction::Denote
                            | DefinitionAction::Represent
                            | DefinitionAction::Mean
                            | DefinitionAction::Call
                    )
                )
        };
        let before = self
            .events
            .iter()
            .filter(accepted)
            .filter(|event| event.end <= mention.start)
            .max_by_key(|event| event.end);
        let after = self
            .events
            .iter()
            .filter(accepted)
            .filter(|event| mention.end <= event.start)
            .min_by_key(|event| event.start);
        [before, after].into_iter().flatten().min_by_key(|event| {
            if event.end <= mention.start {
                mention.start - event.end
            } else {
                event.start - mention.end
            }
        })
    }
}

fn equation_flow_windows(
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
    target_index: usize,
    target: &ScientificMention,
) -> Vec<(usize, usize, bool)> {
    let mut windows = Vec::new();
    let preceding_math_end = mentions[..target_index]
        .iter()
        .map(|mention| mention.end)
        .max()
        .unwrap_or_default();
    windows.extend(
        clauses
            .iter()
            .rev()
            .filter_map(|clause| {
                let start = clause.start.max(preceding_math_end);
                (start < target.start && target.start - start <= MAX_ATTACHMENT_DISTANCE_BYTES)
                    .then_some((start, target.start, true))
            })
            .take(MAX_EQUATION_FLOW_CLAUSES + 1),
    );
    if let Some(clause) = clauses
        .iter()
        .find(|clause| clause.start < target.end && target.end <= clause.end)
        && target.end < clause.end
    {
        windows.push((target.end, clause.end, false));
    }
    if let Some(clause) = clauses.iter().find(|clause| target.end <= clause.start)
        && clause.end - target.end <= MAX_ATTACHMENT_DISTANCE_BYTES
    {
        windows.push((target.end, clause.end, false));
    }
    windows.retain(|(start, end, _)| {
        start < end
            && !mentions.iter().enumerate().any(|(index, mention)| {
                index != target_index && *start < mention.end && mention.start < *end
            })
    });
    windows.sort_by_key(|(start, end, _)| end - start);
    windows.dedup();
    windows
}

fn trim_terminal_punctuation(source: &str, start: usize, mut end: usize) -> usize {
    while start < end
        && source[..end].chars().next_back().is_some_and(|character| {
            character.is_whitespace() || matches!(character, '.' | ',' | ':' | ';')
        })
    {
        end -= source[..end].chars().next_back().unwrap().len_utf8();
    }
    end
}

pub(crate) fn normalize_prose_events(
    source: &str,
    clauses: &[ScientificClause<'_>],
    mentions: &[ScientificMention],
) -> ProseEventStream {
    let mut events = Vec::new();
    let mut clause_mentions = vec![Vec::new(); clauses.len()];
    for (clause_index, clause) in clauses.iter().enumerate() {
        events.push(ProseEvent {
            clause_index,
            start: clause.start,
            end: clause.start,
            kind: ProseEventKind::ClauseStart,
        });
        for feature in &clause.frame.evidence {
            events.push(ProseEvent {
                clause_index,
                start: feature.start,
                end: feature.end,
                kind: ProseEventKind::DiscourseFeature(feature.kind),
            });
        }
        for (mention_index, mention) in mentions
            .iter()
            .enumerate()
            .filter(|(_, mention)| clause.start <= mention.start && mention.start < clause.end)
        {
            clause_mentions[clause_index].push(mention_index);
            events.push(ProseEvent {
                clause_index,
                start: mention.start,
                end: mention.end,
                kind: ProseEventKind::MathMention(mention_index),
            });
        }
        emit_lexical_events(clause_index, clause, &mut events);
        emit_description_spans(source, clause_index, clause, mentions, &mut events);
        events.push(ProseEvent {
            clause_index,
            start: clause.end,
            end: clause.end,
            kind: ProseEventKind::ClauseEnd,
        });
    }
    events.sort_by_key(|event| (event.start, event.end, event_kind_order(event.kind)));
    let mut clause_anaphors = vec![Vec::new(); clauses.len()];
    for event in &events {
        if let ProseEventKind::Anaphor(kind) = event.kind {
            clause_anaphors[event.clause_index].push(kind);
        }
    }
    ProseEventStream {
        events,
        clause_mentions,
        clause_anaphors,
    }
}

fn emit_lexical_events(
    clause_index: usize,
    clause: &ScientificClause<'_>,
    output: &mut Vec<ProseEvent>,
) {
    const ACTIONS: &[(&str, DefinitionAction)] = &[
        ("defines", DefinitionAction::Define),
        ("defined", DefinitionAction::Define),
        ("define", DefinitionAction::Define),
        ("denotes", DefinitionAction::Denote),
        ("denote", DefinitionAction::Denote),
        ("represents", DefinitionAction::Represent),
        ("represent", DefinitionAction::Represent),
        ("means", DefinitionAction::Mean),
        ("mean", DefinitionAction::Mean),
        ("computed", DefinitionAction::Compute),
        ("compute", DefinitionAction::Compute),
        ("calculated", DefinitionAction::Compute),
        ("calculate", DefinitionAction::Compute),
        ("evaluated", DefinitionAction::Compute),
        ("evaluate", DefinitionAction::Compute),
        ("derived", DefinitionAction::Compute),
        ("derive", DefinitionAction::Compute),
        ("obtained", DefinitionAction::Compute),
        ("obtain", DefinitionAction::Compute),
        ("gives", DefinitionAction::Compute),
        ("give", DefinitionAction::Compute),
        ("returns", DefinitionAction::Produce),
        ("return", DefinitionAction::Produce),
        ("produces", DefinitionAction::Produce),
        ("produce", DefinitionAction::Produce),
        ("yields", DefinitionAction::Produce),
        ("yield", DefinitionAction::Produce),
        ("expressed", DefinitionAction::Write),
        ("written", DefinitionAction::Write),
        ("write", DefinitionAction::Write),
        ("calls", DefinitionAction::Call),
        ("called", DefinitionAction::Call),
        ("call", DefinitionAction::Call),
    ];
    const CONNECTIVES: &[(&str, DiscourseConnective)] = &[
        ("where", DiscourseConnective::Where),
        ("here", DiscourseConnective::Here),
        ("thus", DiscourseConnective::Thus),
        ("hence", DiscourseConnective::Hence),
        ("therefore", DiscourseConnective::Therefore),
        ("respectively", DiscourseConnective::Respectively),
        ("in that order", DiscourseConnective::InThatOrder),
        ("either", DiscourseConnective::Alternative),
        ("or", DiscourseConnective::Alternative),
    ];
    const LINKS: &[(&str, DefinitionLink)] = &[
        ("to be", DefinitionLink::ToBe),
        ("as", DefinitionLink::As),
        ("by", DefinitionLink::By),
        ("for", DefinitionLink::For),
        ("is", DefinitionLink::Copula),
        ("are", DefinitionLink::Copula),
        ("be", DefinitionLink::Copula),
    ];
    const ANAPHORS: &[(&str, AnaphorKind)] = &[
        ("these quantities", AnaphorKind::PluralDemonstrative),
        ("these symbols", AnaphorKind::PluralDemonstrative),
        ("those quantities", AnaphorKind::PluralDemonstrative),
        ("those symbols", AnaphorKind::PluralDemonstrative),
        ("this quantity", AnaphorKind::SingularDemonstrative),
        ("this vector field", AnaphorKind::SingularDemonstrative),
        ("this symbol", AnaphorKind::SingularDemonstrative),
        ("this variable", AnaphorKind::SingularDemonstrative),
        ("this equation", AnaphorKind::FormulaDemonstrative),
        ("this calculation", AnaphorKind::FormulaDemonstrative),
        ("this conversion", AnaphorKind::FormulaDemonstrative),
        ("this derivation", AnaphorKind::FormulaDemonstrative),
        ("this equality", AnaphorKind::FormulaDemonstrative),
        ("this expression", AnaphorKind::FormulaDemonstrative),
        ("this identity", AnaphorKind::FormulaDemonstrative),
        ("this relation", AnaphorKind::FormulaDemonstrative),
        ("this result", AnaphorKind::FormulaDemonstrative),
        ("this formula", AnaphorKind::FormulaDemonstrative),
        ("the former", AnaphorKind::Former),
        ("the latter", AnaphorKind::Latter),
        ("they", AnaphorKind::PluralPronoun),
    ];
    for (phrase, kind) in ACTIONS {
        emit_phrase_events(
            clause_index,
            clause,
            phrase,
            ProseEventKind::DefinitionAction(*kind),
            output,
        );
    }
    for (phrase, kind) in CONNECTIVES {
        emit_phrase_events(
            clause_index,
            clause,
            phrase,
            ProseEventKind::Connective(*kind),
            output,
        );
    }
    for (phrase, kind) in LINKS {
        emit_phrase_events(
            clause_index,
            clause,
            phrase,
            ProseEventKind::DefinitionLink(*kind),
            output,
        );
    }
    for (phrase, kind) in ANAPHORS {
        emit_phrase_events(
            clause_index,
            clause,
            phrase,
            ProseEventKind::Anaphor(*kind),
            output,
        );
    }
    let lower = clause.text.to_ascii_lowercase();
    let copular_start = lower
        .char_indices()
        .find_map(|(offset, character)| character.is_ascii_alphabetic().then_some(offset))
        .unwrap_or_default();
    let copular_text = &lower[copular_start..];
    let copular_formula_reference = ["this is ", "this was "]
        .iter()
        .any(|prefix| copular_text.starts_with(prefix))
        && copular_text
            .split(|character: char| !character.is_ascii_alphabetic())
            .any(|word| {
                matches!(
                    word,
                    "equation" | "expression" | "formula" | "identity" | "law" | "relation"
                )
            });
    if copular_formula_reference {
        output.push(ProseEvent {
            clause_index,
            start: clause.start + copular_start,
            end: clause.start + copular_start + "this".len(),
            kind: ProseEventKind::Anaphor(AnaphorKind::FormulaDemonstrative),
        });
    }
    for phrase in ["and", "while", "whereas"] {
        emit_phrase_events(
            clause_index,
            clause,
            phrase,
            ProseEventKind::Coordination,
            output,
        );
    }
}

fn emit_phrase_events(
    clause_index: usize,
    clause: &ScientificClause<'_>,
    phrase: &str,
    kind: ProseEventKind,
    output: &mut Vec<ProseEvent>,
) {
    let lower = clause.text.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(relative) = lower[search_from..].find(phrase) {
        let local_start = search_from + relative;
        let local_end = local_start + phrase.len();
        let bounded = (local_start == 0
            || !lower.as_bytes()[local_start - 1].is_ascii_alphanumeric())
            && (local_end == lower.len() || !lower.as_bytes()[local_end].is_ascii_alphanumeric())
            && (!matches!(kind, ProseEventKind::DefinitionAction(_))
                || ((local_start == 0 || lower.as_bytes()[local_start - 1] != b'-')
                    && (local_end == lower.len() || lower.as_bytes()[local_end] != b'-')));
        if bounded {
            let start = clause.start + local_start;
            output.push(ProseEvent {
                clause_index,
                start,
                end: start + phrase.len(),
                kind,
            });
        }
        search_from = local_end;
    }
}

fn emit_description_spans(
    source: &str,
    clause_index: usize,
    clause: &ScientificClause<'_>,
    mentions: &[ScientificMention],
    output: &mut Vec<ProseEvent>,
) {
    let mut boundaries = vec![clause.start, clause.end];
    for mention in mentions
        .iter()
        .filter(|mention| clause.start <= mention.start && mention.end <= clause.end)
    {
        boundaries.extend([mention.start, mention.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    for pair in boundaries.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let is_math = mentions
            .iter()
            .any(|mention| mention.start <= start && end <= mention.end);
        if !is_math && source[start..end].chars().any(char::is_alphabetic) {
            output.push(ProseEvent {
                clause_index,
                start,
                end,
                kind: ProseEventKind::DescriptionSpan,
            });
        }
    }
}

fn event_kind_order(kind: ProseEventKind) -> u8 {
    match kind {
        ProseEventKind::ClauseStart => 0,
        ProseEventKind::DiscourseFeature(_) => 1,
        ProseEventKind::Connective(_) => 2,
        ProseEventKind::Anaphor(_) => 3,
        ProseEventKind::DefinitionAction(_) => 4,
        ProseEventKind::DefinitionLink(_) => 5,
        ProseEventKind::MathMention(_) => 6,
        ProseEventKind::Coordination => 7,
        ProseEventKind::DescriptionSpan => 8,
        ProseEventKind::ClauseEnd => 9,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssumptionCandidate {
    pub kind: String,
    pub value: String,
    pub target_relation_id: Option<String>,
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
    merge_soft_wrapped_ranges(source, language, ranges)
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

fn merge_soft_wrapped_ranges(
    source: &str,
    language: DocumentLanguage,
    ranges: Vec<(usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut hard_boundary = false;
    for range in ranges {
        let trimmed = trim_range(source, range.0, range.1);
        if trimmed.0 >= trimmed.1 {
            hard_boundary = true;
            continue;
        }
        if let Some(previous) = merged.last_mut()
            && !hard_boundary
            && soft_wrap_connects(source, language, *previous, trimmed)
        {
            previous.1 = trimmed.1;
        } else {
            merged.push(trimmed);
        }
        hard_boundary = false;
    }
    merged
}

fn soft_wrap_connects(
    source: &str,
    language: DocumentLanguage,
    previous: (usize, usize),
    next: (usize, usize),
) -> bool {
    if next.1.saturating_sub(previous.0) > MAX_CLAUSE_BYTES {
        return false;
    }
    let separator = &source[previous.1..next.0];
    if separator
        .chars()
        .filter(|character| *character == '\n')
        .count()
        != 1
        || separator
            .chars()
            .any(|character| !character.is_whitespace())
    {
        return false;
    }
    if source[..previous.1]
        .chars()
        .next_back()
        .is_some_and(|character| matches!(character, '.' | '!' | '?' | ';'))
    {
        return false;
    }
    let previous_text = source[previous.0..previous.1].trim_start();
    let next_text = source[next.0..next.1].trim_start();
    match language {
        DocumentLanguage::Latex => !previous_text.starts_with('\\') && !next_text.starts_with('\\'),
        DocumentLanguage::Markdown => {
            !markdown_structure_line(previous_text) && !markdown_structure_line(next_text)
        }
        DocumentLanguage::Bibtex => false,
    }
}

fn markdown_structure_line(text: &str) -> bool {
    text.starts_with(['#', '>', '|'])
        || text.starts_with("```")
        || text.starts_with("~~~")
        || text.starts_with("- ")
        || text.starts_with("* ")
        || text.starts_with("+ ")
        || text
            .split_once('.')
            .is_some_and(|(number, _)| number.chars().all(|character| character.is_ascii_digit()))
}

#[cfg(test)]
pub(crate) fn extract_assumptions(
    clause: &ScientificClause<'_>,
    mentions: &[ScientificMention],
) -> Vec<AssumptionCandidate> {
    extract_assumptions_with_phrases(clause, mentions, &[])
}

#[cfg(test)]
pub(crate) fn extract_assumptions_with_phrases(
    clause: &ScientificClause<'_>,
    mentions: &[ScientificMention],
    additional_phrases: &[(&str, &str, &str)],
) -> Vec<AssumptionCandidate> {
    extract_assumptions_with_formula_descriptors(clause, mentions, additional_phrases, &[])
}

pub(crate) fn extract_assumptions_with_formula_descriptors(
    clause: &ScientificClause<'_>,
    mentions: &[ScientificMention],
    additional_phrases: &[(&str, &str, &str)],
    formula_descriptors: &[(String, String)],
) -> Vec<AssumptionCandidate> {
    if !clause.frame.establishes() {
        return Vec::new();
    }
    let lower = clause.text.to_ascii_lowercase().replace('-', " ");
    let mut matches = assumption_phrases()
        .iter()
        .copied()
        .chain(additional_phrases.iter().copied())
        .filter_map(|(phrase, kind, value)| {
            let normalized_phrase = phrase.to_ascii_lowercase().replace('-', " ");
            let offset = lower.find(&normalized_phrase)?;
            let prefix = &lower[..offset];
            let suffix = &lower[offset + normalized_phrase.len()..];
            let phrase_start = clause.start + offset;
            let phrase_end = phrase_start + normalized_phrase.len();
            let has_direct_math_subject = mentions
                .iter()
                .filter(|mention| clause.start <= mention.start && mention.end <= phrase_start)
                .max_by_key(|mention| mention.end)
                .is_some_and(|mention| {
                    let subject_start = mention.start.saturating_sub(clause.start);
                    let bridge_start = mention.end.saturating_sub(clause.start);
                    lower
                        .get(..subject_start)
                        .is_some_and(math_subject_context_is_current)
                        && lower
                            .get(bridge_start..offset)
                            .is_some_and(math_subject_directly_affirms_phrase)
                });
            let has_immediate_following_math_target = mentions
                .iter()
                .filter(|mention| phrase_end <= mention.start && mention.end <= clause.end)
                .min_by_key(|mention| mention.start)
                .is_some_and(|mention| {
                    let bridge_end = mention.start.saturating_sub(clause.start);
                    lower
                        .get(offset + normalized_phrase.len()..bridge_end)
                        .is_some_and(|bridge| {
                            bridge.chars().all(|character| {
                                character.is_whitespace()
                                    || matches!(
                                        character,
                                        ',' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '$' | '\\'
                                    )
                            })
                        })
                });
            let introduced_relation_id = mentions
                .iter()
                .filter(|mention| phrase_end <= mention.start && mention.end <= clause.end)
                .min_by_key(|mention| mention.start)
                .and_then(|mention| {
                    let bridge_end = mention.start.saturating_sub(clause.start);
                    lower
                        .get(offset + normalized_phrase.len()..bridge_end)
                        .and_then(|bridge| {
                            formula_introduction_relation_id(bridge, formula_descriptors)
                        })
                })
                .or_else(|| {
                    let words = suffix
                        .split(|character: char| !character.is_ascii_alphabetic())
                        .filter(|word| !word.is_empty())
                        .collect::<Vec<_>>();
                    formula_introduction_relation_id_words(&words, formula_descriptors)
                });
            let formula_context = AssumptionFormulaContext {
                has_direct_math_subject,
                has_immediate_following_math_target,
                introduced_relation_id,
            };
            let prefix_negation = match assumption_phrase_polarity(
                prefix,
                suffix,
                kind,
                &normalized_phrase,
                formula_context,
            ) {
                AssumptionPhrasePolarity::Refuted { start, end } => {
                    Some((clause.start + start, clause.start + end))
                }
                AssumptionPhrasePolarity::Ignored => return None,
                AssumptionPhrasePolarity::Supported => None,
            };
            let value = if prefix_negation.is_some() {
                format!("not-{value}")
            } else {
                value.into()
            };
            let phrase_start =
                prefix_negation.map_or(phrase_start, |(start, _)| start.min(clause.start + offset));
            let phrase_end = prefix_negation.map_or(
                clause.start + offset + normalized_phrase.len(),
                |(_, end)| end.max(clause.start + offset + normalized_phrase.len()),
            );
            let subjects = nearest_subjects(mentions, clause, phrase_start);
            Some((
                offset,
                normalized_phrase.len(),
                AssumptionCandidate {
                    kind: kind.into(),
                    value,
                    target_relation_id: (kind == "sign-convention")
                        .then(|| introduced_relation_id.map(str::to_owned))
                        .flatten(),
                    subjects,
                    phrase_start,
                    phrase_end,
                },
            ))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)));
    let mut accepted = Vec::<(usize, usize, String, String, Option<String>)>::new();
    matches
        .into_iter()
        .filter_map(|(start, length, candidate)| {
            let end = start + length;
            let duplicate =
                accepted
                    .iter()
                    .any(|(used_start, used_end, kind, value, target_relation_id)| {
                        start == *used_start
                            && end == *used_end
                            && candidate.kind == *kind
                            && candidate.value == *value
                            && candidate.target_relation_id == *target_relation_id
                    });
            let conflicting_overlap = accepted.iter().any(|(used_start, used_end, _, _, _)| {
                start < *used_end && *used_start < end && (start != *used_start || end != *used_end)
            });
            (!duplicate && !conflicting_overlap).then(|| {
                accepted.push((
                    start,
                    end,
                    candidate.kind.clone(),
                    candidate.value.clone(),
                    candidate.target_relation_id.clone(),
                ));
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
        .or_else(|| first_marker(&lower, &["seems to", "appears to", "is likely to"]))
        .or_else(|| {
            words
                .contains(&"calculation")
                .then(|| first_marker(&lower, &["draft"]))
                .flatten()
        });
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

    let explicit_refusal_marker = first_marker(
        &lower,
        &[
            "forbids ",
            " cannot use ",
            " cannot apply ",
            " cannot be used",
            " cannot be applied",
            " should not use ",
            " should not apply ",
            " should not be used",
            " should not be applied",
            " is forbidden",
            " are forbidden",
            " is invalid",
            " are invalid",
            " is unavailable",
            " are unavailable",
            " is unusable",
            " are unusable",
            " must be discarded",
            " must be ignored",
            " must be rejected",
            " must be withdrawn",
            " is discarded",
            " are discarded",
            " is excluded",
            " are excluded",
            " is rejected",
            " are rejected",
            " is withdrawn",
            " are withdrawn",
            " was discarded",
            " were discarded",
            " was excluded",
            " were excluded",
            " was rejected",
            " were rejected",
            " was withdrawn",
            " were withdrawn",
            " has been discarded",
            " have been discarded",
            " had been discarded",
            " has been excluded",
            " have been excluded",
            " had been excluded",
            " has been rejected",
            " have been rejected",
            " had been rejected",
            " has been withdrawn",
            " have been withdrawn",
            " had been withdrawn",
        ],
    )
    .or_else(|| active_demonstrative_formula_refusal_marker(&lower));
    let explicit_negative_action_marker = first_marker(
        &lower,
        &[
            " does not use ",
            " did not use ",
            " is never used",
            " are never used",
            " was never used",
            " were never used",
            " has not been used",
            " have not been used",
            " had not been used",
        ],
    );
    let bounded_negative_action_marker = negative_formula_action_marker(&lower);
    let negative_marker = if starts_with_any(
        &lower,
        &[
            "not ",
            "do not ",
            "does not ",
            "we do not ",
            "cannot ",
            "never ",
            "no longer ",
        ],
    ) || lower.contains(" is not ")
        || lower.contains(" are not ")
        || lower.contains(" no longer ")
        || lower.contains(" must not ")
        || lower.contains(" does not apply")
        || lower.contains(" do not apply")
        || lower.contains(" did not apply")
        || lower.contains(" does not assume")
        || lower.contains(" do not assume")
        || lower.contains(" did not adopt")
        || lower.contains(" does not adopt")
        || lower.contains(" do not adopt")
        || lower.contains(" is not adopted")
        || lower.contains(" are not adopted")
        || lower.contains(" was not adopted")
        || lower.contains(" were not adopted")
        || explicit_refusal_marker.is_some()
        || explicit_negative_action_marker.is_some()
        || bounded_negative_action_marker.is_some()
        || ((lower.starts_with("drops ") || lower.contains(" drops "))
            && lower.contains(" without assuming"))
        || lower.contains(" inapplicable")
        || [" not define", " not denote", " not represent", " not mean"]
            .iter()
            .any(|marker| lower.contains(marker))
    {
        first_bounded_marker(
            &lower,
            &[
                "not",
                "never",
                "cannot",
                "without",
                "no longer",
                "inapplicable",
            ],
        )
        .into_iter()
        .chain(explicit_refusal_marker)
        .chain(explicit_negative_action_marker)
        .chain(bounded_negative_action_marker)
        .min_by_key(|(start, _)| *start)
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
    // Sentence-initial “According to …” is attribution.  A predicate such as
    // “x evolves according to <formula>” instead introduces the author's own
    // model and must remain assertive.
    let according_to = lower
        .trim_start()
        .starts_with("according to")
        .then(|| first_marker(&lower, &["according to"]))
        .flatten();
    let attributed = formula_attribution_marker(&lower);
    let lexical_attribution = first_marker(&lower, &["as reported"])
        .or(according_to)
        .or(attributed)
        .or_else(|| {
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

fn formula_attribution_marker(lower: &str) -> Option<(usize, usize)> {
    let words = lower
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let is_formula_metanoun = |word: &str| {
        matches!(
            word,
            "formula"
                | "equation"
                | "identity"
                | "law"
                | "model"
                | "relation"
                | "balance"
                | "comparison"
        )
    };
    let attributed_descriptor = words.iter().enumerate().any(|(index, word)| {
        *word == "attributed"
            && words[index + 1..]
                .iter()
                .take(3)
                .any(|word| is_formula_metanoun(word))
    });
    let attributed_metanoun = words.windows(3).any(|window| {
        is_formula_metanoun(window[0])
            && window[1] == "attributed"
            && matches!(window[2], "to" | "by")
    });
    let archived_descriptor = words.iter().enumerate().any(|(index, word)| {
        *word == "archived"
            && words[index + 1..]
                .iter()
                .take(3)
                .any(|word| is_formula_metanoun(word))
    });
    (attributed_descriptor || attributed_metanoun || archived_descriptor)
        .then(|| first_marker(lower, &["attributed", "archived"]))
        .flatten()
}

fn classify_act(lower: &str) -> (CommunicativeAct, Option<(usize, usize)>) {
    let trimmed = lower.trim_start();
    if starts_with_any(trimmed, &["alternatively", "otherwise", "either"])
        && let Some(marker) = first_marker(lower, &["alternatively", "otherwise", "either"])
    {
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

fn active_demonstrative_formula_refusal_marker(value: &str) -> Option<(usize, usize)> {
    let mut words = Vec::new();
    let mut start = None;
    for (offset, character) in value.char_indices() {
        if character.is_ascii_alphabetic() {
            start.get_or_insert(offset);
        } else if let Some(start) = start.take() {
            words.push((start, offset, &value[start..offset]));
        }
    }
    if let Some(start) = start {
        words.push((start, value.len(), &value[start..]));
    }
    words.windows(3).find_map(|window| {
        matches!(
            window[0].2,
            "reject"
                | "rejects"
                | "rejected"
                | "withdraw"
                | "withdraws"
                | "withdrew"
                | "discard"
                | "discards"
                | "discarded"
        )
        .then_some(())?;
        matches!(window[1].2, "this" | "that").then_some(())?;
        matches!(
            window[2].2,
            "formula"
                | "equation"
                | "identity"
                | "law"
                | "model"
                | "relation"
                | "balance"
                | "estimate"
                | "calculation"
                | "proposal"
        )
        .then_some((window[0].0, window[0].1))
    })
}

fn first_bounded_marker(value: &str, markers: &[&str]) -> Option<(usize, usize)> {
    markers
        .iter()
        .flat_map(|marker| value.match_indices(marker))
        .filter(|(start, marker)| {
            let end = start + marker.len();
            value[..*start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_ascii_alphabetic())
                && value[end..]
                    .chars()
                    .next()
                    .is_none_or(|character| !character.is_ascii_alphabetic())
        })
        .map(|(start, marker)| (start, start + marker.len()))
        .min_by_key(|(start, _)| *start)
}

fn negative_formula_action_marker(value: &str) -> Option<(usize, usize)> {
    let words = value
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let has_negative_action = words.iter().enumerate().any(|(index, word)| {
        if !matches!(*word, "no" | "not" | "never" | "cannot") {
            return false;
        }
        if index > 0 && words[index - 1] == "without" {
            return false;
        }
        let tail = &words[index + 1..];
        let modifiers = tail
            .iter()
            .take_while(|word| {
                matches!(
                    **word,
                    "be" | "been"
                        | "being"
                        | "ever"
                        | "longer"
                        | "still"
                        | "currently"
                        | "directly"
                        | "explicitly"
                        | "necessarily"
                )
            })
            .count();
        tail.get(modifiers).is_some_and(|action| {
            matches!(
                *action,
                "publish"
                    | "publishes"
                    | "published"
                    | "publishing"
                    | "assert"
                    | "asserts"
                    | "asserted"
                    | "asserting"
                    | "assume"
                    | "assumes"
                    | "assumed"
                    | "assuming"
                    | "use"
                    | "uses"
                    | "used"
                    | "using"
                    | "apply"
                    | "applies"
                    | "applied"
                    | "applying"
                    | "adopt"
                    | "adopts"
                    | "adopted"
                    | "adopting"
                    | "accept"
                    | "accepts"
                    | "accepted"
                    | "accepting"
                    | "select"
                    | "selects"
                    | "selected"
                    | "selecting"
            )
        })
    });
    has_negative_action
        .then(|| first_bounded_marker(value, &["no", "not", "never", "cannot"]))
        .flatten()
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
        ("effectively constant", "uniformity", "uniform"),
        ("held constant", "uniformity", "uniform"),
        ("constant over", "uniformity", "uniform"),
        ("uniform section", "uniformity", "uniform"),
        (
            "common probability space",
            "context",
            "common-probability-space",
        ),
        (
            "same probability space",
            "context",
            "common-probability-space",
        ),
        (
            "one probability space",
            "context",
            "common-probability-space",
        ),
        ("different experiments", "context", "different-context"),
        ("different experiment", "context", "different-context"),
        (
            "different probability spaces",
            "context",
            "different-context",
        ),
        (
            "different probability space",
            "context",
            "different-context",
        ),
        (
            "distinct probability spaces",
            "context",
            "different-context",
        ),
        ("distinct probability space", "context", "different-context"),
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
        ("an include", "project-reachability", "included"),
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
    } else if let Some(nearest) = candidates
        .iter()
        .filter(|mention| phrase_start <= mention.start)
        .map(|mention| mention.start)
        .min()
    {
        candidates.retain(|mention| phrase_start <= mention.start && mention.start - nearest <= 96);
    }
    candidates.truncate(4);
    candidates
}

enum AssumptionPhrasePolarity {
    Supported,
    Refuted { start: usize, end: usize },
    Ignored,
}

#[derive(Clone, Copy)]
struct AssumptionFormulaContext<'a> {
    has_direct_math_subject: bool,
    has_immediate_following_math_target: bool,
    introduced_relation_id: Option<&'a str>,
}

fn assumption_phrase_polarity(
    prefix: &str,
    suffix: &str,
    kind: &str,
    phrase: &str,
    formula_context: AssumptionFormulaContext<'_>,
) -> AssumptionPhrasePolarity {
    let mut words = Vec::new();
    let mut word_start = None;
    for (offset, character) in prefix.char_indices() {
        if character.is_ascii_alphabetic() {
            word_start.get_or_insert(offset);
        } else if let Some(start) = word_start.take() {
            words.push((start, offset, &prefix[start..offset]));
        }
    }
    if let Some(start) = word_start {
        words.push((start, prefix.len(), &prefix[start..]));
    }

    let negative_cue = |word: &str| {
        [
            "reject",
            "abandon",
            "oppos",
            "refus",
            "declin",
            "avoid",
            "exclud",
            "omit",
            "waiv",
            "eschew",
            "forbid",
            "forgo",
            "forego",
            "fail",
            "lack",
            "deny",
            "disavow",
            "renounc",
            "shun",
            "repudiat",
            "neglect",
            "withdraw",
            "refrain",
            "stop",
            "cease",
            "discontinu",
            "prohibit",
        ]
        .iter()
        .any(|prefix| word.starts_with(prefix))
    };
    let suffix_words = suffix
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .take(8)
        .collect::<Vec<_>>();
    let direct_modifier = |word: &str| {
        word.ends_with("ly")
            || matches!(
                word,
                "a" | "an" | "the" | "this" | "that" | "only" | "its" | "their" | "our" | "any"
            )
    };
    let withholding_word =
        |word: &str| word.starts_with("withhold") || word.starts_with("withheld");
    let suffix_withholds = |head: usize| {
        suffix_words.get(head..).is_some_and(|tail| {
            let modifiers = tail.iter().take_while(|word| direct_modifier(word)).count();
            tail.get(modifiers)
                .is_some_and(|word| withholding_word(word))
        })
    };
    let direct_withholding = words
        .iter()
        .rposition(|(_, _, word)| withholding_word(word))
        .is_some_and(|cue| {
            let bridge = &words[cue + 1..];
            let leading_modifiers = bridge
                .iter()
                .take_while(|(_, _, word)| direct_modifier(word))
                .count();
            bridge.iter().all(|(_, _, word)| direct_modifier(word))
                || (matches!(bridge, [(_, _, "of"), ..])
                    && bridge[1..].iter().all(|(_, _, word)| direct_modifier(word)))
                || (matches!(
                    &bridge[leading_modifiers..],
                    [(_, _, "adoption" | "use" | "application"), (_, _, "of"), ..]
                ) && bridge[leading_modifiers + 2..]
                    .iter()
                    .all(|(_, _, word)| direct_modifier(word)))
        })
        || (suffix_words.first().is_some_and(|word| {
            matches!(
                *word,
                "is" | "are" | "was" | "were" | "remain" | "remains" | "remained"
            )
        }) && suffix_withholds(1))
        || (matches!(
            suffix_words.as_slice(),
            ["has" | "have" | "had", "been", ..]
        ) && suffix_withholds(2))
        || (matches!(
            suffix_words.as_slice(),
            ["must" | "should" | "will", "be", ..]
        ) && suffix_withholds(2))
        || (matches!(suffix_words.as_slice(), ["cannot", "be", ..]) && suffix_withholds(2));
    let headed_withholding = suffix_words.first().is_some_and(|word| {
        matches!(
            *word,
            "assumption" | "condition" | "convention" | "hypothesis" | "requirement"
        )
    }) && ((suffix_words.get(1).is_some_and(|word| {
        matches!(
            *word,
            "is" | "are" | "was" | "were" | "remain" | "remains" | "remained"
        )
    }) && suffix_withholds(2))
        || (matches!(
            suffix_words.get(1..3),
            Some(["has" | "have" | "had", "been"])
        ) && suffix_withholds(3))
        || (matches!(
            suffix_words.get(1..3),
            Some(["must" | "should" | "will", "be"])
        ) && suffix_withholds(3)));
    let contrastive_prefix = words.windows(2).any(|pair| {
        matches!(
            (pair[0].2, pair[1].2),
            ("rather", "than")
                | ("instead", "of")
                | ("prior", "to")
                | ("in", "lieu")
                | ("in", "place")
        )
    }) || words
        .iter()
        .any(|(_, _, word)| matches!(*word, "alternative" | "before" | "until"));
    if direct_withholding
        || headed_withholding
        || contrastive_prefix
        || words.iter().any(|(_, _, word)| negative_cue(word))
        || suffix_words.iter().any(|word| negative_cue(word))
        || suffix_words
            .iter()
            .any(|word| matches!(*word, "no" | "not" | "never" | "neither" | "without"))
    {
        return AssumptionPhrasePolarity::Ignored;
    }
    let negations = words
        .iter()
        .filter(|(_, _, word)| matches!(*word, "no" | "not" | "never" | "neither" | "without"))
        .collect::<Vec<_>>();
    if negations.is_empty() {
        if kind == "sign-convention"
            && phrase.contains("sign convention")
            && !sign_convention_is_affirmed(&words, &suffix_words, formula_context)
        {
            return AssumptionPhrasePolarity::Ignored;
        }
        return AssumptionPhrasePolarity::Supported;
    }
    if negations.len() != 1 {
        return AssumptionPhrasePolarity::Ignored;
    }
    let &(start, end, _) = negations[0];
    if words.first().map(|(start, _, _)| *start) != Some(start) {
        return AssumptionPhrasePolarity::Ignored;
    }
    if !negation_precedes_positive_phrase(&prefix[end..]) {
        return AssumptionPhrasePolarity::Ignored;
    }
    AssumptionPhrasePolarity::Refuted { start, end }
}

fn sign_convention_is_affirmed(
    words: &[(usize, usize, &str)],
    suffix_words: &[&str],
    formula_context: AssumptionFormulaContext<'_>,
) -> bool {
    let prefix_cue = |word: &str| {
        word == "under"
            || matches!(
                word,
                "adopt"
                    | "adopts"
                    | "adopted"
                    | "adopting"
                    | "assume"
                    | "assumes"
                    | "assumed"
                    | "assuming"
                    | "use"
                    | "uses"
                    | "used"
                    | "using"
                    | "follow"
                    | "follows"
                    | "followed"
                    | "following"
                    | "apply"
                    | "applies"
                    | "applied"
                    | "applying"
                    | "establish"
                    | "establishes"
                    | "established"
                    | "establishing"
            )
    };
    let phrase_modifier = |word: &str| {
        matches!(
            word,
            "a" | "an"
                | "any"
                | "the"
                | "this"
                | "that"
                | "its"
                | "one"
                | "same"
                | "stated"
                | "consistent"
                | "explicit"
                | "explicitly"
                | "reviewed"
                | "local"
        )
    };
    let cue = words.iter().rposition(|(_, _, word)| prefix_cue(word));
    let direct_under_prefix = cue.is_some_and(|cue| {
        words[cue].2 == "under"
            && words[..cue].is_empty()
            && words[cue + 1..]
                .iter()
                .all(|(_, _, word)| phrase_modifier(word))
    });
    let direct_prefix = cue.is_some_and(|cue| {
        let before = &words[..cue];
        let after = &words[cue + 1..];
        let cue_word = words[cue].2;
        let direct_subject = formula_context.has_direct_math_subject;
        let before_words = before.iter().map(|(_, _, word)| *word).collect::<Vec<_>>();
        let direct_author = matches!(
            before_words.as_slice(),
            ["we"]
                | ["here", "we"]
                | ["we", "explicitly" | "currently"]
                | [
                    "in",
                    "this",
                    "derivation" | "calculation" | "model" | "analysis" | "case",
                    "we"
                ]
                | [
                    "for",
                    "this",
                    "derivation" | "calculation" | "model" | "analysis" | "case",
                    "we"
                ]
                | [
                    "the" | "this",
                    "derivation" | "calculation" | "model" | "analysis"
                ]
        );
        let direct_imperative = before.is_empty() && cue_word != "under";
        let direct_under = before.is_empty() && cue_word == "under";
        after.iter().all(|(_, _, word)| phrase_modifier(word))
            && (direct_subject || direct_author || direct_imperative || direct_under)
    });
    let contextual_suffix = suffix_words.iter().any(|word| {
        matches!(
            *word,
            "alternative"
                | "another"
                | "but"
                | "cited"
                | "elsewhere"
                | "example"
                | "for"
                | "if"
                | "later"
                | "only"
                | "option"
                | "optional"
                | "other"
                | "provided"
                | "separate"
                | "unless"
                | "when"
                | "whereas"
        )
    });
    let direct_suffix = !contextual_suffix && direct_sign_convention_suffix(suffix_words);
    !contextual_suffix
        && (direct_prefix
            && (direct_formula_continuation(suffix_words)
                || formula_context.introduced_relation_id.is_some()
                || direct_under_prefix
                    && (formula_context.has_immediate_following_math_target
                        || formula_context.introduced_relation_id.is_some()))
            || direct_suffix)
}

fn formula_introduction_relation_id<'a>(
    bridge: &str,
    formula_descriptors: &'a [(String, String)],
) -> Option<&'a str> {
    let words = bridge
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    formula_introduction_relation_id_words(&words, formula_descriptors)
}

fn formula_introduction_relation_id_words<'a>(
    words: &[&str],
    formula_descriptors: &'a [(String, String)],
) -> Option<&'a str> {
    let [
        "the",
        descriptor,
        "equation" | "formula" | "law" | "model" | "reference" | "relation",
        "is" | "reads",
    ] = words
    else {
        return None;
    };
    let mut matches = formula_descriptors
        .iter()
        .filter(|(allowed, _)| allowed == descriptor)
        .map(|(_, relation_id)| relation_id.as_str());
    let relation_id = matches.next()?;
    matches.next().is_none().then_some(relation_id)
}

fn math_subject_directly_affirms_phrase(bridge: &str) -> bool {
    let words = bridge
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words.first().is_some_and(|word| {
        matches!(
            *word,
            "adopt"
                | "adopts"
                | "adopted"
                | "assume"
                | "assumes"
                | "assumed"
                | "follow"
                | "follows"
                | "followed"
                | "use"
                | "uses"
                | "used"
        )
    }) && words[1..].iter().all(|word| {
        matches!(
            *word,
            "a" | "an" | "the" | "this" | "that" | "same" | "stated" | "reviewed"
        )
    })
}

fn math_subject_context_is_current(context: &str) -> bool {
    !context
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .any(|word| {
            matches!(
                word,
                "according"
                    | "another"
                    | "cited"
                    | "elsewhere"
                    | "example"
                    | "instructions"
                    | "note"
                    | "other"
                    | "previous"
                    | "previously"
                    | "prior"
                    | "reported"
                    | "separate"
            )
        })
}

fn direct_sign_convention_suffix(words: &[&str]) -> bool {
    match words {
        ["applies" | "holds", tail @ ..] => direct_formula_continuation(tail),
        ["is" | "remains", middle @ ..] => {
            let action = middle
                .iter()
                .position(|word| !matches!(*word, "explicitly" | "fully" | "currently" | "here"));
            action.is_some_and(|action| {
                matches!(
                    middle[action],
                    "adopted" | "applied" | "established" | "followed" | "used"
                ) && direct_formula_continuation(&middle[action + 1..])
            })
        }
        _ => false,
    }
}

fn direct_formula_continuation(words: &[&str]) -> bool {
    words.is_empty()
        || matches!(
            words,
            ["consider" | "take" | "use" | "write", ..]
                | ["and" | "then", "consider" | "take" | "use" | "write", ..]
                | [
                    "and" | "then",
                    "we" | "here",
                    "consider" | "take" | "use" | "write",
                    ..
                ]
                | [
                    "the",
                    "accepted" | "following" | "stated",
                    "equation" | "formula" | "law" | "model" | "reference" | "relation",
                    "is" | "reads",
                    ..
                ]
        )
}

fn negation_precedes_positive_phrase(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .all(|word| {
            matches!(
                word,
                "a" | "an"
                    | "any"
                    | "the"
                    | "this"
                    | "that"
                    | "named"
                    | "stated"
                    | "explicitly"
                    | "fully"
                    | "currently"
                    | "here"
                    | "adopt"
                    | "adopted"
                    | "adopting"
                    | "assume"
                    | "assumed"
                    | "assuming"
                    | "use"
                    | "used"
                    | "using"
                    | "follow"
                    | "followed"
                    | "following"
                    | "apply"
                    | "applied"
                    | "applying"
                    | "under"
            )
        })
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
        let source = "Assume $A$ is symmetric. If $B$ were invertible, continue. $C$ might denote a matrix. Without adopting the convention, inspect $D$. Alternatively, use $E$. % Assume steady state\n```\nAssume idealized operation.\n```";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Markdown, &[]);
        assert_eq!(clauses.len(), 6);
        assert!(clauses[0].frame.establishes());
        assert_eq!(
            clauses[1].frame.conditionality,
            Conditionality::Counterfactual
        );
        assert_eq!(clauses[2].frame.modality, EvidenceModality::Hedged);
        assert_eq!(clauses[3].frame.polarity, EvidencePolarity::Positive);
        assert_eq!(
            clauses[4].frame.evidence_modality(),
            EvidenceModality::Hypothetical
        );

        let descriptive = segment_scientific_clauses(
            "The otherwise undeclared symbols appear in the product $h=msd$.",
            DocumentLanguage::Latex,
            &[],
        );
        assert!(descriptive[0].frame.establishes());
    }

    #[test]
    fn limits_draft_modality_to_proposed_calculations() {
        let proposed = segment_scientific_clauses(
            "The draft go/no-go calculation added $P(A\\cup B)=P(A)+P(B)$.",
            DocumentLanguage::Latex,
            &[],
        );
        let retained = segment_scientific_clauses(
            "The draft still contains $u_h\\approx u$.",
            DocumentLanguage::Latex,
            &[],
        );

        assert_eq!(proposed[0].frame.modality, EvidenceModality::Hedged);
        assert_eq!(retained[0].frame.modality, EvidenceModality::Asserted);
    }

    #[test]
    fn joins_soft_wrapped_sentences_without_crossing_document_structure() {
        let latex = "Let $A$ be the set of samples, and let $B$ be the\nset of accepted samples.\n\\[A\\cap B\\]\nNext paragraph\n\nstarts here.";
        let clauses = segment_scientific_clauses(latex, DocumentLanguage::Latex, &[]);
        assert_eq!(
            clauses.iter().map(|clause| clause.text).collect::<Vec<_>>(),
            [
                "Let $A$ be the set of samples, and let $B$ be the\nset of accepted samples.",
                "\\[A\\cap B\\]",
                "Next paragraph",
                "starts here.",
            ]
        );

        let markdown = "A wrapped scientific\nsentence continues.\n\n# Heading\n- list item";
        let clauses = segment_scientific_clauses(markdown, DocumentLanguage::Markdown, &[]);
        assert_eq!(clauses[0].text, "A wrapped scientific\nsentence continues.");
        assert_eq!(clauses[1].text, "# Heading");
        assert_eq!(clauses[2].text, "- list item");
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
    fn normalizes_mentions_actions_anaphora_and_boundaries_once() {
        let source = "$x$ and $y$. The former denotes input and the latter output.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let mentions = vec![
            ScientificMention {
                symbol: "x".into(),
                start: 0,
                end: 3,
                math_index: 0,
            },
            ScientificMention {
                symbol: "y".into(),
                start: 8,
                end: 11,
                math_index: 1,
            },
        ];
        let stream = normalize_prose_events(source, &clauses, &mentions);
        assert_eq!(stream.mentions_in_clause(0), &[0, 1]);
        assert!(
            stream
                .events
                .iter()
                .any(|event| matches!(event.kind, ProseEventKind::Anaphor(AnaphorKind::Former)))
        );
        assert!(stream.events.iter().any(|event| matches!(
            event.kind,
            ProseEventKind::DefinitionAction(DefinitionAction::Denote)
        )));
        assert_eq!(
            stream
                .events
                .iter()
                .filter(|event| event.kind == ProseEventKind::ClauseStart)
                .count(),
            2
        );
    }

    #[test]
    fn composes_an_active_definition_from_typed_events_without_rescanning_grammar() {
        let source = "The notation calls $K$ both kinetic energy and stiffness.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let math_start = source.find("$K$").unwrap();
        let mentions = vec![ScientificMention {
            symbol: "K".into(),
            start: math_start,
            end: math_start + 3,
            math_index: 0,
        }];
        let stream = normalize_prose_events(source, &clauses, &mentions);
        let construction = stream
            .definition_constructions(source, &mentions, &clauses)
            .into_iter()
            .next()
            .expect("typed definition construction");

        assert_eq!(construction.action, DefinitionAction::Call);
        assert_eq!(
            &source[construction.description_start..construction.description_end],
            "both kinetic energy and stiffness"
        );
        assert_eq!(construction.evidence_start, source.find("calls").unwrap());
    }

    #[test]
    fn definition_constructions_are_stable_across_verb_and_whitespace_variants() {
        for source in [
            "We define $r$ as residual.",
            "We denote $r$ by residual.",
            "We represent $r$ as residual.",
            "We call $r$ residual.",
            "We define\n$r$ as residual.",
            "$r$ means residual.",
        ] {
            let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
            let start = source.find("$r$").unwrap();
            let mentions = [ScientificMention {
                symbol: "r".into(),
                start,
                end: start + 3,
                math_index: 0,
            }];
            let stream = normalize_prose_events(source, &clauses, &mentions);
            let construction = stream
                .definition_constructions(source, &mentions, &clauses)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("missing construction for {source:?}"));

            assert_eq!(
                &source[construction.description_start..construction.description_end],
                "residual",
                "unexpected description for {source:?}",
            );
        }
    }

    #[test]
    fn definition_descriptions_stop_before_a_following_formula_action() {
        for (language, separator) in [
            (DocumentLanguage::Latex, "\n"),
            (DocumentLanguage::Markdown, "\n\n"),
        ] {
            let source =
                format!("The draft calls $x$ the unique estimate and defines it by{separator}$\n$");
            let clauses = segment_scientific_clauses(&source, language, &[]);
            let start = source.find("$x$").unwrap();
            let mentions = [ScientificMention {
                symbol: "x".into(),
                start,
                end: start + 3,
                math_index: 0,
            }];
            let stream = normalize_prose_events(&source, &clauses, &mentions);
            let construction = stream
                .definition_constructions(&source, &mentions, &clauses)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("missing construction for {language:?}"));

            assert_eq!(construction.action, DefinitionAction::Call);
            assert_eq!(
                &source[construction.description_start..construction.description_end],
                "the unique estimate"
            );
            assert_eq!(
                construction.evidence_end,
                source.find("and defines").unwrap()
            );
        }
    }

    #[test]
    fn definition_constructions_preserve_nonasserted_frames_for_one_safety_gate() {
        for source in [
            "We might define $r$ as residual.",
            "We do not define $r$ as residual.",
            "According to Smith, we define $r$ as residual.",
        ] {
            let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
            let start = source.find("$r$").unwrap();
            let mentions = [ScientificMention {
                symbol: "r".into(),
                start,
                end: start + 3,
                math_index: 0,
            }];
            let stream = normalize_prose_events(source, &clauses, &mentions);
            let construction = stream
                .definition_constructions(source, &mentions, &clauses)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("missing construction for {source:?}"));

            assert!(
                !construction.frame.establishes(),
                "unsafe frame for {source:?}"
            );
        }
    }

    #[test]
    fn exposes_typed_attachment_connectives_without_downstream_text_scans() {
        let source = "Where $x$ is positive, the variables are input and output, in that order.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let start = source.find("$x$").unwrap();
        let mentions = [ScientificMention {
            symbol: "x".into(),
            start,
            end: start + 3,
            math_index: 0,
        }];
        let stream = normalize_prose_events(source, &clauses, &mentions);

        assert!(stream.has_connective(0, &[DiscourseConnective::Where]));
        assert!(stream.has_connective(0, &[DiscourseConnective::InThatOrder]));
    }

    #[test]
    fn equation_flow_candidates_start_after_preceding_math_and_remain_bounded() {
        let clauses = [ScientificClause {
            start: 0,
            end: 150,
            text: "",
            frame: asserted_author_frame(),
        }];
        let mentions = [
            ScientificMention {
                symbol: "Q".into(),
                start: 40,
                end: 80,
                math_index: 0,
            },
            ScientificMention {
                symbol: "m".into(),
                start: 120,
                end: 140,
                math_index: 1,
            },
        ];

        assert_eq!(
            equation_flow_windows(&clauses, &mentions, 1, &mentions[1]),
            vec![(140, 150, false), (80, 120, true)],
        );
    }

    #[test]
    fn composes_postposed_formula_anaphor_flow() {
        let source = "We advance the pulse by matching\n$x=y$. This is the scalar wave equation used by the solver.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let start = source.find("$x=y$").unwrap();
        let mentions = [ScientificMention {
            symbol: "x".into(),
            start,
            end: start + "$x=y$".len(),
            math_index: 0,
        }];
        let stream = normalize_prose_events(source, &clauses, &mentions);

        assert!(
            stream
                .discourse_constructions(source, &mentions, &clauses)
                .iter()
                .any(|construction| matches!(
                    construction,
                    DiscourseConstruction::EquationFlow {
                        precedes_formula: false,
                        ..
                    }
                )),
            "clauses={clauses:?}; events={:?}",
            stream.events
        );
    }

    #[test]
    fn composes_bounded_anaphoric_attachment_candidates_from_typed_events() {
        let source = "$x$ and $y$ are introduced. They denote input and output, respectively.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let mentions = ["$x$", "$y$"]
            .into_iter()
            .enumerate()
            .map(|(math_index, needle)| {
                let start = source.find(needle).unwrap();
                ScientificMention {
                    symbol: needle[1..2].into(),
                    start,
                    end: start + needle.len(),
                    math_index,
                }
            })
            .collect::<Vec<_>>();
        let stream = normalize_prose_events(source, &clauses, &mentions);

        let candidate = stream
            .discourse_constructions(source, &mentions, &clauses)
            .into_iter()
            .find_map(|construction| match construction {
                DiscourseConstruction::Anaphoric { candidate, .. } => Some(candidate),
                _ => None,
            })
            .expect("anaphoric attachment candidate");
        assert_eq!(candidate.mention_indices, [0, 1]);
        assert!(candidate.distance_bytes <= MAX_ANAPHORIC_DISTANCE_BYTES);
    }

    #[test]
    fn distinguishes_formula_anaphors_from_symbol_anaphors() {
        let source = "$x=y$. This equation defines the balance. This symbol denotes input.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let mentions = [ScientificMention {
            symbol: "x".into(),
            start: 0,
            end: 5,
            math_index: 0,
        }];
        let stream = normalize_prose_events(source, &clauses, &mentions);

        assert!(stream.starts_with_anaphor_kind(
            1,
            clauses[1].start,
            AnaphorKind::FormulaDemonstrative
        ));
        assert!(!stream.starts_with_anaphor_kind(
            1,
            clauses[1].start,
            AnaphorKind::SingularDemonstrative
        ));
        assert!(stream.starts_with_anaphor_kind(
            2,
            clauses[2].start,
            AnaphorKind::SingularDemonstrative
        ));

        for head in [
            "This calculation",
            "This conversion",
            "This derivation",
            "This equality",
            "This expression",
            "This result",
        ] {
            let source = format!("$x=y$. {head} establishes the claimed value.");
            let clauses = segment_scientific_clauses(&source, DocumentLanguage::Latex, &[]);
            let mentions = [ScientificMention {
                symbol: "x".into(),
                start: 0,
                end: 5,
                math_index: 0,
            }];
            let stream = normalize_prose_events(&source, &clauses, &mentions);
            assert!(stream.starts_with_anaphor_kind(
                1,
                clauses[1].start,
                AnaphorKind::FormulaDemonstrative
            ));
        }

        let source = "$x=y$. This is the wave equation used by the solver.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let stream = normalize_prose_events(source, &clauses, &mentions);
        assert!(stream.starts_with_anaphor_kind(
            1,
            clauses[1].start,
            AnaphorKind::FormulaDemonstrative
        ));

        let source = "$x=y$. This is the input symbol used by the solver.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let stream = normalize_prose_events(source, &clauses, &mentions);
        assert!(!stream.starts_with_anaphor_kind(
            1,
            clauses[1].start,
            AnaphorKind::FormulaDemonstrative
        ));
    }

    #[test]
    fn refuses_unbounded_anaphoric_attachment_candidates() {
        let padding = "x".repeat(MAX_ANAPHORIC_DISTANCE_BYTES + 1);
        let source = format!("$x$ is introduced.\n\n{padding}\n\nThis symbol denotes input.");
        let clauses = segment_scientific_clauses(&source, DocumentLanguage::Latex, &[]);
        let mentions = [ScientificMention {
            symbol: "x".into(),
            start: 0,
            end: 3,
            math_index: 0,
        }];
        let stream = normalize_prose_events(&source, &clauses, &mentions);

        assert!(
            !stream
                .discourse_constructions(&source, &mentions, &clauses)
                .iter()
                .any(|construction| matches!(
                    construction,
                    DiscourseConstruction::Anaphoric { .. }
                ))
        );
    }

    #[test]
    fn exposes_the_source_backed_description_adjacent_to_each_mention() {
        let source = "The saline density $\\rho$ was measured.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let mention = ScientificMention {
            symbol: "rho".into(),
            start: 19,
            end: 25,
            math_index: 0,
        };
        let stream = normalize_prose_events(source, &clauses, std::slice::from_ref(&mention));
        let range = stream.description_before(source, 0, mention.start).unwrap();
        assert_eq!(&source[range.0..range.1], "The saline density ");

        let source = "The optical diameter supplied the area\n$A$ was recorded.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let start = source.find("$A$").unwrap();
        let mention = ScientificMention {
            symbol: "A".into(),
            start,
            end: start + 3,
            math_index: 0,
        };
        let stream = normalize_prose_events(source, &clauses, std::slice::from_ref(&mention));
        let clause_index = clauses
            .iter()
            .position(|clause| clause.start <= start && start < clause.end)
            .unwrap();
        let range = stream
            .description_before(source, clause_index, mention.start)
            .unwrap();
        assert_eq!(
            &source[range.0..range.1],
            "The optical diameter supplied the area"
        );

        let source = "The volume in interval $\\Delta t$ is the area $A$ times a distance.";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let delta_start = source.find("$\\Delta t$").unwrap();
        let area_start = source.find("$A$").unwrap();
        let mentions = [
            ScientificMention {
                symbol: "Deltat".into(),
                start: delta_start,
                end: delta_start + "$\\Delta t$".len(),
                math_index: 0,
            },
            ScientificMention {
                symbol: "A".into(),
                start: area_start,
                end: area_start + "$A$".len(),
                math_index: 1,
            },
        ];
        let stream = normalize_prose_events(source, &clauses, &mentions);
        let clause_index = clauses
            .iter()
            .position(|clause| clause.start <= area_start && area_start < clause.end)
            .unwrap();
        let range = stream
            .description_before(source, clause_index, area_start)
            .unwrap();
        assert_eq!(&source[range.0..range.1], " is the area ");
    }

    #[test]
    fn assigns_a_multiline_display_mention_to_its_opening_clause() {
        let source = "We compute the mass rate as\n\\begin{equation}\nQ=Av.\n\\end{equation}\n";
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let start = source.find("\\begin{equation}").unwrap();
        let mention = ScientificMention {
            symbol: "Q".into(),
            start,
            end: source.find("\\end{equation}").unwrap() + "\\end{equation}".len(),
            math_index: 0,
        };
        let stream = normalize_prose_events(source, &clauses, &[mention]);
        let opening_clause = clauses
            .iter()
            .position(|clause| clause.start <= start && start < clause.end)
            .unwrap();
        assert_eq!(stream.mentions_in_clause(opening_clause), &[0]);
        assert_eq!(
            stream
                .clause_mentions
                .iter()
                .filter(|mentions| !mentions.is_empty())
                .count(),
            1
        );
    }

    #[test]
    fn keeps_semicolon_anaphora_composable() {
        let source = "Let $x$ and $u$ be introduced. The former is the state vector; the latter is the control input.";
        let mentions = vec![
            ScientificMention {
                symbol: "x".into(),
                start: 4,
                end: 7,
                math_index: 0,
            },
            ScientificMention {
                symbol: "u".into(),
                start: 12,
                end: 15,
                math_index: 1,
            },
        ];
        let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
        let events = normalize_prose_events(source, &clauses, &mentions);
        assert_eq!(
            clauses.iter().map(|clause| clause.text).collect::<Vec<_>>(),
            vec![
                "Let $x$ and $u$ be introduced.",
                "The former is the state vector;",
                "the latter is the control input.",
            ]
        );
        assert!(events.has_anaphor(1));
        assert!(events.has_anaphor(2));
        assert_eq!(events.mentions_in_clause(0), &[0, 1]);
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
            math_index: 0,
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
    fn attaches_a_negated_project_link_to_the_following_symbol() {
        let source = "Without an include, inspect $r$ here.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let start = source.find("$r$").unwrap();
        let mentions = [ScientificMention {
            symbol: "r".into(),
            start,
            end: start + 3,
            math_index: 0,
        }];

        let assumptions = extract_assumptions(&clause, &mentions);

        assert!(assumptions.iter().any(|assumption| {
            assumption.kind == "project-reachability"
                && assumption.value == "not-included"
                && assumption.subjects[0].symbol == "r"
        }));
    }

    #[test]
    fn grounds_a_negated_condition_in_the_negation_and_phrase() {
        let source = "Without adopting the passive sign convention, consider $i=Cv$.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let start = source.find("$i=Cv$").unwrap();
        let mentions = [ScientificMention {
            symbol: "i".into(),
            start,
            end: start + "$i=Cv$".len(),
            math_index: 0,
        }];

        let assumptions = extract_assumptions_with_phrases(
            &clause,
            &mentions,
            &[(
                "passive sign convention",
                "sign-convention",
                "passive-sign-convention",
            )],
        );
        let assumption = assumptions
            .iter()
            .find(|assumption| assumption.value == "not-passive-sign-convention")
            .expect("negated reviewed condition phrase");

        assert_eq!(
            &source[assumption.phrase_start..assumption.phrase_end],
            "Without adopting the passive sign convention"
        );

        for source in [
            "Without rejecting the passive sign convention, continue.",
            "Without ever rejecting the passive sign convention, continue.",
            "Without not adopting the passive sign convention, continue.",
            "Without ever repeatedly explicitly continuing to firmly and deliberately reject the passive sign convention, continue.",
            "Without adopting, even provisionally, the passive sign convention, continue.",
            "Without adopting the research and development passive sign convention, continue.",
            "Without adopting the ideal source assumption and the passive sign convention, continue.",
            "Without rejecting the active convention and the passive sign convention, continue.",
            "With no adoption of the passive sign convention, continue.",
            "We refuse to adopt the passive sign convention and continue.",
            "Declining to adopt the passive sign convention, continue.",
            "Avoiding adoption of the passive sign convention, continue.",
            "The passive sign convention is declined; continue.",
            "Rather than use the passive sign convention, continue.",
            "Instead of adopting the passive sign convention, continue.",
            "Prior to adopting the passive sign convention, continue.",
            "The passive sign convention ceased to be used; continue.",
            "The passive sign convention is never used; continue.",
            "We use an auxiliary meter to describe the passive sign convention, then continue.",
            "The passive sign convention is described while we use an auxiliary meter, then continue.",
            "We use an alternative to the passive sign convention and continue.",
            "We adopt an alternative to the passive sign convention and continue.",
            "An alternative to the passive sign convention is used; continue.",
            "In lieu of using the passive sign convention, continue.",
            "In place of adopting the passive sign convention, continue.",
            "The passive sign convention is used only in a separate example, whereas here we continue.",
            "In a separate example, we use the passive sign convention, and here we continue.",
            "The cited note uses the passive sign convention, but here we continue.",
            "Smith uses the passive sign convention, but here we continue.",
            "For the inductor, we use the passive sign convention, and for the capacitor we continue.",
            "The passive sign convention applies to another circuit, but here we continue.",
            "The passive sign convention is used for the inductor, but for the capacitor we continue.",
            "The instructions say use the passive sign convention, but here we continue.",
            "We intend to use the passive sign convention later, but currently continue.",
            "The passive sign convention applies if the reference terminal is positive, then continue.",
            "We use the passive sign convention if needed, then continue.",
            "One option is to use the passive sign convention, then continue.",
            "The measured current $i$ is recorded, and Smith uses the passive sign convention, then continue.",
            "The cited note reports current $i$ and uses the passive sign convention, then continue.",
            "For the other circuit, current $i$ is recorded and Smith uses the passive sign convention, then continue.",
            "The passive sign convention was used previously, and currently we continue.",
            "The passive sign convention applies whenever the reference terminal is positive, and we continue.",
            "The passive sign convention applies assuming the terminal is positive, and we continue.",
            "We explicitly use the passive sign convention conditionally, then continue.",
            "In the cited example, $i$ uses the passive sign convention, then continue.",
            "In Smiths cited model, $i$ uses the passive sign convention, then continue.",
            "For the other circuit, current $i$ uses the passive sign convention, then continue.",
            "For a separate circuit, $i$ uses the passive sign convention, then continue.",
            "In another model, $i$ uses the passive sign convention, then continue.",
            "For this inductor we use the passive sign convention, and continue.",
            "For this appendix we use the passive sign convention, and continue.",
            "For this example we use the passive sign convention, and in the current derivation continue.",
            "Under the passive sign convention whenever the reference terminal is positive, continue.",
            "Under the passive sign convention assuming the terminal is positive, continue.",
            "Under the passive sign convention while analyzing the inductor, continue.",
            "Under the passive sign convention subject to a positive terminal, continue.",
            "Under the passive sign convention during the auxiliary analysis, continue.",
            "Under the passive sign convention 조건부로, continue.",
            "Under the passive sign convention 만약 단자가 양수라면, continue.",
            "Under the passive sign convention εάν ισχύει, continue.",
            "Under the passive sign convention 若条件成立, continue.",
            "For this inductor we use the passive sign convention, and we continue.",
            "For this appendix we use the passive sign convention, and we continue.",
            "For this example we use the passive sign convention, and in the current derivation we continue.",
        ] {
            let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
                .into_iter()
                .find(|clause| clause.text.contains("passive sign convention"))
                .unwrap();
            let assumptions = extract_assumptions_with_phrases(
                &clause,
                &[],
                &[(
                    "passive sign convention",
                    "sign-convention",
                    "passive-sign-convention",
                )],
            );
            assert!(
                assumptions.iter().all(|assumption| {
                    !matches!(
                        assumption.value.as_str(),
                        "passive-sign-convention" | "not-passive-sign-convention"
                    )
                }),
                "{source}: {assumptions:#?}"
            );
        }

        for source in [
            "We explicitly use the passive sign convention.",
            "We currently use the passive sign convention.",
            "In this derivation we use the passive sign convention.",
            "For this calculation we use the passive sign convention.",
            "The calculation uses the passive sign convention.",
        ] {
            let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
                .into_iter()
                .find(|clause| clause.text.contains("passive sign convention"))
                .unwrap();
            let assumptions = extract_assumptions_with_phrases(
                &clause,
                &[],
                &[(
                    "passive sign convention",
                    "sign-convention",
                    "passive-sign-convention",
                )],
            );
            assert!(
                assumptions
                    .iter()
                    .any(|assumption| assumption.value == "passive-sign-convention"),
                "{source}: {assumptions:#?}"
            );
        }

        let formula_descriptors = vec![(
            "capacitor".to_owned(),
            "circuits:capacitor-current-law".to_owned(),
        )];
        let source = "Under this passive sign convention, the capacitor law is\n\\[\ni=Cv.\n\\]";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .find(|clause| clause.text.contains("passive sign convention"))
            .unwrap();
        let start = source.find("i=Cv").unwrap();
        let mentions = [ScientificMention {
            symbol: "i".into(),
            start,
            end: start + "i=Cv".len(),
            math_index: 0,
        }];
        let phrase_end =
            source.find("passive sign convention").unwrap() + "passive sign convention".len();
        assert_eq!(
            formula_introduction_relation_id(&source[phrase_end..start], &formula_descriptors),
            Some("circuits:capacitor-current-law")
        );
        let descriptor_assumptions = extract_assumptions_with_formula_descriptors(
            &clause,
            &mentions,
            &[(
                "passive sign convention",
                "sign-convention",
                "passive-sign-convention",
            )],
            &formula_descriptors,
        );
        assert!(
            descriptor_assumptions
                .iter()
                .any(|assumption| assumption.value == "passive-sign-convention"),
            "{descriptor_assumptions:#?}"
        );

        for source in [
            "Under this passive sign convention, the hypothetical law is $i=Cv$.",
            "Under this passive sign convention, the rejected law is $i=Cv$.",
            "Under this passive sign convention, the proposed law is $i=Cv$.",
            "Under this passive sign convention, the cited law is $i=Cv$.",
            "Under this passive sign convention, the other law is $i=Cv$.",
            "Under this passive sign convention, the untrusted law is $i=Cv$.",
            "Under this passive sign convention, the fictional law is $i=Cv$.",
            "Under this passive sign convention, the obsolete law is $i=Cv$.",
            "Under this passive sign convention, the incorrect law is $i=Cv$.",
            "Under this passive sign convention, the capacitor law is rejected before $i=Cv$.",
        ] {
            let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
                .into_iter()
                .find(|clause| clause.text.contains("passive sign convention"))
                .unwrap();
            let start = source.find("i=Cv").unwrap();
            let mentions = [ScientificMention {
                symbol: "i".into(),
                start,
                end: start + "i=Cv".len(),
                math_index: 0,
            }];
            assert!(
                extract_assumptions_with_formula_descriptors(
                    &clause,
                    &mentions,
                    &[(
                        "passive sign convention",
                        "sign-convention",
                        "passive-sign-convention",
                    )],
                    &formula_descriptors,
                )
                .iter()
                .all(|assumption| assumption.value != "passive-sign-convention"),
                "{source}"
            );
        }

        let source = "The input is not singular, and after routine calibration under the passive sign convention, continue.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .find(|clause| clause.text.contains("passive sign convention"))
            .unwrap();
        assert!(
            extract_assumptions_with_phrases(
                &clause,
                &[],
                &[(
                    "passive sign convention",
                    "sign-convention",
                    "passive-sign-convention",
                )],
            )
            .iter()
            .all(|assumption| {
                !matches!(
                    assumption.value.as_str(),
                    "passive-sign-convention" | "not-passive-sign-convention"
                )
            }),
            "{clause:#?}"
        );

        let source = "The auxiliary meter is never fully used under the passive sign convention, while continuing.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .find(|clause| clause.text.contains("passive sign convention"))
            .unwrap();
        let assumptions = extract_assumptions_with_phrases(
            &clause,
            &[],
            &[(
                "passive sign convention",
                "sign-convention",
                "passive-sign-convention",
            )],
        );
        assert!(
            assumptions.iter().all(|assumption| {
                !matches!(
                    assumption.value.as_str(),
                    "passive-sign-convention" | "not-passive-sign-convention"
                )
            }),
            "{clause:#?}: {assumptions:#?}"
        );
    }

    #[test]
    fn withheld_condition_phrases_do_not_become_supporting_assumptions() {
        let source =
            "The analysis withholds the uniform section condition before evaluating $r=abc$.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let start = source.find("$r=abc$").unwrap();
        let mentions = [ScientificMention {
            symbol: "r".into(),
            start,
            end: start + "$r=abc$".len(),
            math_index: 0,
        }];

        let assumptions = extract_assumptions_with_phrases(
            &clause,
            &mentions,
            &[("uniform section", "uniformity", "uniform")],
        );

        assert!(assumptions.is_empty(), "{assumptions:#?}");
    }

    #[test]
    fn unrelated_withholding_does_not_suppress_a_positive_condition() {
        let context = AssumptionFormulaContext {
            has_direct_math_subject: false,
            has_immediate_following_math_target: true,
            introduced_relation_id: None,
        };

        assert!(matches!(
            assumption_phrase_polarity(
                "the recorder withholds metadata under the ",
                " before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Supported
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "the analysis withholds the ",
                " before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "the analysis withholds only the ",
                " before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "under the ",
                " condition is explicitly withheld before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "under the ",
                " condition has been explicitly withheld before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "under the ",
                " condition is intentionally withheld before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "under the ",
                " condition is purposefully withheld before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "under the ",
                " condition is temporarily withheld before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "the analysis withholds adoption of the ",
                " before evaluation",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "the analysis withholds its adoption of the ",
                " condition while evaluating",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
        assert!(matches!(
            assumption_phrase_polarity(
                "the analysis records the withholding of the ",
                " condition while evaluating",
                "uniformity",
                "uniform section",
                context,
            ),
            AssumptionPhrasePolarity::Ignored
        ));
    }

    #[test]
    fn attaches_scientific_assumptions_to_following_subjects_or_the_local_context() {
        let source = "The cross-section mean speed $v$ is recorded.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let start = source.find("$v$").unwrap();
        let mentions = [ScientificMention {
            symbol: "v".into(),
            start,
            end: start + 3,
            math_index: 0,
        }];
        let assumptions = extract_assumptions_with_phrases(
            &clause,
            &mentions,
            &[("cross section mean", "averaging", "mean-normal-velocity")],
        );
        assert!(assumptions.iter().any(|assumption| {
            assumption.value == "mean-normal-velocity"
                && assumption
                    .subjects
                    .iter()
                    .any(|subject| subject.symbol == "v")
        }));

        let source = "Density and bore area are effectively constant over each window.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let assumptions = extract_assumptions(&clause, &[]);
        assert!(assumptions.iter().any(|assumption| {
            assumption.kind == "uniformity"
                && assumption.value == "uniform"
                && assumption.subjects.is_empty()
        }));

        let source = "The stages share one estimate.";
        let clause = segment_scientific_clauses(source, DocumentLanguage::Latex, &[])
            .into_iter()
            .next()
            .unwrap();
        let assumptions = extract_assumptions_with_phrases(
            &clause,
            &[],
            &[
                ("share one estimate", "context", "same-input"),
                ("share one estimate", "context", "same-output"),
                ("share one estimate", "context", "same-output"),
            ],
        );
        assert_eq!(
            assumptions
                .iter()
                .map(|assumption| assumption.value.as_str())
                .collect::<Vec<_>>(),
            ["same-input", "same-output"]
        );
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

    #[test]
    fn treats_explicit_non_applicability_as_negative_evidence() {
        for source in [
            "Let $A$ be an event, but this law does not apply: $A \\cap B$.",
            "The cited gradient assumption is inapplicable to these data.",
            "The document drops the product derivative and claims $i=C(t)\\frac{dv}{dt}$ without assuming $\\dot C=0$.",
            "For comparison only, the report mentions $K=\\frac12mv^2$ but does not adopt the kinetic-energy model.",
            "The model forbids the electric-power formula $P=VI$.",
            "The archived relation $PV=nRT$ is invalid for this analysis.",
            "The balance $\\Delta U=Q-W$ is unavailable in the open-system model.",
            "The update $x_{k+1}=x_k-\\eta g_k$ must be discarded.",
            "The equation $F=ma$ is excluded from this model.",
            "The archived relation $PV=nRT$ cannot be used in this analysis.",
            "The power formula $P=VI$ should not be applied to this device.",
            "The balance $\\Delta U=Q-W$ is rejected for the open system.",
            "The update $x_{k+1}=x_k-\\eta g_k$ is withdrawn.",
            "The equation $F=ma$ is unusable for this model.",
        ] {
            let clauses = segment_scientific_clauses(source, DocumentLanguage::Latex, &[]);
            assert_eq!(
                clauses[0].frame.polarity,
                EvidencePolarity::Negative,
                "{source}: {clauses:#?}"
            );
            assert!(!clauses[0].frame.establishes(), "{source}: {clauses:#?}");
        }
    }
}
