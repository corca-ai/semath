const MAX_DESCRIPTION_BYTES: usize = 120;

#[derive(Clone, Copy)]
struct ConstructionSpec {
    leads: &'static [&'static str],
    links: &'static [&'static str],
    rule_id: &'static str,
    strip_article: bool,
    stop_at_comma: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DefinitionConstruction<'a> {
    pub description: &'a str,
    pub rule_id: &'static str,
    pub prefix_start: usize,
    pub suffix_end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoordinationLead {
    Let,
    Direct,
    Write,
    DenoteBy,
}

const DEFINITION_CONSTRUCTIONS: &[ConstructionSpec] = &[
    ConstructionSpec {
        leads: &[
            "let",
            "where",
            "take",
            "given",
            "suppose",
            "assume",
            "subject to",
        ],
        links: &[
            "denotes",
            "denote",
            "stands for",
            "stand for",
            "to be",
            "be",
            "is",
            "are",
            "as",
            "represents",
            "represent",
        ],
        rule_id: "english-construction-definition",
        strip_article: false,
        stop_at_comma: false,
    },
    ConstructionSpec {
        leads: &["we write"],
        links: &["for"],
        rule_id: "english-write-for-definition",
        strip_article: false,
        stop_at_comma: false,
    },
    ConstructionSpec {
        leads: &["define", "set"],
        links: &["as", "equal to"],
        rule_id: "english-imperative-definition",
        strip_article: false,
        stop_at_comma: false,
    },
    ConstructionSpec {
        leads: &["denote by", "call"],
        links: &[""],
        rule_id: "english-imperative-definition",
        strip_article: true,
        stop_at_comma: false,
    },
    ConstructionSpec {
        leads: &["we use"],
        links: &["to represent", "to denote", "for"],
        rule_id: "english-use-definition",
        strip_article: false,
        stop_at_comma: false,
    },
    ConstructionSpec {
        leads: &[
            "here",
            "throughout",
            "with",
            "given",
            "suppose",
            "assume",
            "subject to",
        ],
        links: &[
            "denoting",
            "denotes",
            "denote",
            "designates",
            "designate",
            "represents",
            "representing",
            "represent",
            "is",
            "be",
        ],
        rule_id: "english-contextual-definition",
        strip_article: false,
        stop_at_comma: true,
    },
    ConstructionSpec {
        leads: &["", "the symbol", "the notation", "symbol", "notation"],
        links: &[
            "means",
            "mean",
            "stands for",
            "stand for",
            "refers to",
            "will denote",
            "shall be",
            "designates",
            "designate",
            "denotes",
            "denote",
            "represents",
            "represent",
            "is",
            "are",
        ],
        rule_id: "english-relational-definition",
        strip_article: false,
        stop_at_comma: false,
    },
];

pub(crate) fn match_definition<'a>(
    before: &str,
    after: &'a str,
    math_contains_assignment: bool,
) -> Option<DefinitionConstruction<'a>> {
    let clause_lead = current_clause(before);
    let trimmed_after = after.trim_start();
    if trimmed_after.starts_with(',') || trimmed_after.starts_with("and ") {
        return None;
    }
    for spec in DEFINITION_CONSTRUCTIONS {
        let Some(prefix_start) = match_lead(before, clause_lead, spec.leads) else {
            continue;
        };
        let Some((description, suffix_end)) =
            match_description(after, spec.links, spec.stop_at_comma)
        else {
            continue;
        };
        let description = if spec.strip_article {
            strip_article(description)
        } else {
            description
        };
        if valid_description(description) {
            return Some(DefinitionConstruction {
                description,
                rule_id: spec.rule_id,
                prefix_start,
                suffix_end,
            });
        }
    }
    if math_contains_assignment
        && clause_lead.trim().is_empty()
        && let Some((description, suffix_end)) =
            match_description(after, &["defines", "define"], false)
        && valid_description(description)
    {
        return Some(DefinitionConstruction {
            description: strip_article(description),
            rule_id: "english-math-assignment-definition",
            prefix_start: before.len(),
            suffix_end,
        });
    }
    None
}

pub(crate) fn is_declaration_lead(before: &str) -> bool {
    let lead = current_clause(before).trim();
    [
        "let",
        "where",
        "take",
        "given",
        "suppose",
        "assume",
        "subject to",
    ]
    .iter()
    .any(|candidate| lead.eq_ignore_ascii_case(candidate))
}

pub(crate) fn match_passive_definition<'a>(
    before: &'a str,
    after: &str,
) -> Option<DefinitionConstruction<'a>> {
    let punctuation_end = leading_punctuation_end(after)?;
    let clause = current_clause(before);
    let trimmed = clause.trim();
    for link in [
        " is denoted by",
        " are denoted by",
        " is represented by",
        " are represented by",
        " is written as",
        " are written as",
        " is written by",
        " are written by",
    ] {
        if trimmed
            .get(trimmed.len().saturating_sub(link.len())..)
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case(link))
        {
            let description = strip_article(trimmed[..trimmed.len() - link.len()].trim());
            if valid_description(description) {
                let description_start = before.len() - clause.len() + clause.find(description)?;
                return Some(DefinitionConstruction {
                    description,
                    rule_id: "english-passive-definition",
                    prefix_start: description_start,
                    suffix_end: punctuation_end,
                });
            }
        }
    }
    None
}

pub(crate) fn match_apposition(after: &str) -> Option<DefinitionConstruction<'_>> {
    let leading = after.len() - after.trim_start().len();
    let value = after[leading..].strip_prefix(',')?.trim_start();
    let end = value.find(',')?;
    let description = strip_article(value[..end].trim());
    valid_plain_description(description).then_some(DefinitionConstruction {
        description,
        rule_id: "english-apposition-definition",
        prefix_start: 0,
        suffix_end: after.len() - value.len() + end + 1,
    })
}

pub(crate) fn match_parenthetical<'a>(
    before: &'a str,
    after: &str,
) -> Option<DefinitionConstruction<'a>> {
    let suffix_end = after
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .and_then(|(offset, character)| (character == ')').then_some(offset + 1))?;
    let clause = current_clause(before).trim_end();
    let prefix = clause.strip_suffix('(')?.trim_end();
    let description = strip_article(prefix);
    valid_plain_description(description).then_some(DefinitionConstruction {
        description,
        rule_id: "english-parenthetical-definition",
        prefix_start: before.len() - clause.len(),
        suffix_end,
    })
}

pub(crate) fn match_quantified<'a>(
    before: &'a str,
    after: &str,
) -> Option<DefinitionConstruction<'a>> {
    let suffix_end = leading_punctuation_end(after)?;
    let clause = current_clause(before).trim();
    let lower = clause.to_ascii_lowercase();
    let body = ["for each ", "for every "]
        .iter()
        .find_map(|prefix| lower.starts_with(prefix).then(|| &clause[prefix.len()..]))?;
    let description = strip_article(body.trim());
    valid_plain_description(description).then_some(DefinitionConstruction {
        description,
        rule_id: "english-quantified-definition",
        prefix_start: before.len() - current_clause(before).len(),
        suffix_end,
    })
}

pub(crate) fn coordination_lead(before: &str) -> Option<(CoordinationLead, usize)> {
    let clause = current_clause(before);
    let whole = clause.trim();
    let trimmed = whole
        .rsplit_once(',')
        .map_or(whole, |(_, tail)| tail.trim());
    let lower = trimmed.to_ascii_lowercase();
    let lead = if ["let", "take", "given", "suppose", "assume"].contains(&lower.as_str()) {
        CoordinationLead::Let
    } else if lower == "we write" {
        CoordinationLead::Write
    } else if lower == "denote by" {
        CoordinationLead::DenoteBy
    } else if matches!(
        lower.as_str(),
        "" | "here" | "the symbols" | "the notations" | "here the symbols" | "here the notations"
    ) && !whole.contains(['$', '\\'])
    {
        CoordinationLead::Direct
    } else {
        return None;
    };
    Some((
        lead,
        before.len() - clause.len() + clause.rfind(trimmed).unwrap_or(0),
    ))
}

pub(crate) fn coordinated_descriptions(
    lead: CoordinationLead,
    after: &str,
    arity: usize,
) -> Option<(Vec<&str>, &'static str, usize)> {
    let trimmed = after.trim_start();
    let body = match lead {
        CoordinationLead::Let | CoordinationLead::Direct => consume_any(
            trimmed,
            &[
                "denote",
                "denotes",
                "represent",
                "represents",
                "mean",
                "means",
                "stand for",
                "stands for",
            ],
        ),
        CoordinationLead::Write => consume_any(trimmed, &["for"]),
        CoordinationLead::DenoteBy => Some(trimmed),
    };
    if let Some(body) = body
        && let Some((descriptions, consumed)) = ordered_body(body, arity)
    {
        return Some((
            descriptions,
            "english-respectively-definition",
            after.len() - body.len() + consumed,
        ));
    }
    if lead == CoordinationLead::Let {
        let shared = consume_any(
            trimmed,
            &[
                "as",
                "denote",
                "denotes",
                "represent",
                "represents",
                "stand for",
                "stands for",
                "to be",
                "be",
                "are",
            ],
        )?;
        let end = shared.find([',', ';', '.', '\n']).unwrap_or(shared.len());
        let description = shared[..end].trim();
        if valid_plain_description(description)
            && !description.to_ascii_lowercase().contains(" and ")
        {
            return Some((
                vec![description; arity],
                "english-coordinated-definition",
                after.len() - shared.len() + end,
            ));
        }
    }
    None
}

pub(crate) fn fronted_shared_description<'a>(
    before: &'a str,
    after: &str,
) -> Option<(&'a str, usize, usize)> {
    let clause = current_clause(before);
    let phrase = clause.trim();
    let description = strip_fronted_modifiers(phrase);
    let words = description.split_whitespace().collect::<Vec<_>>();
    if words.is_empty()
        || words.len() > 4
        || !words.last()?.trim_end_matches('-').ends_with('s')
        || !valid_plain_description(description)
    {
        return None;
    }
    let trimmed_suffix = after.trim_start();
    let suffix = trimmed_suffix.to_ascii_lowercase();
    let relational_suffix = [
        "belong to",
        "belongs to",
        "are in",
        "are on",
        "are defined in",
        "are defined on",
        "are drawn from",
        "are measured in",
        "share",
    ]
    .iter()
    .any(|prefix| suffix.starts_with(prefix));
    let nominal_suffix = trimmed_suffix.starts_with(',') && !suffix.starts_with(", and");
    if !relational_suffix && !nominal_suffix {
        return None;
    }
    let suffix_end = if nominal_suffix {
        after.find(',').map_or(0, |offset| offset + 1)
    } else {
        after
            .find(['.', ';', '\n'])
            .map_or(after.len(), |offset| offset + 1)
    };
    Some((
        description,
        before.len() - clause.len() + clause.find(description)?,
        suffix_end,
    ))
}

fn strip_fronted_modifiers(value: &str) -> &str {
    let mut remaining = value;
    if let Some((first, rest)) = split_first_word(remaining)
        && ["for", "given"].contains(&first.to_ascii_lowercase().as_str())
    {
        remaining = rest;
    }
    if let Some((first, rest)) = split_first_word(remaining)
        && [
            "a", "an", "the", "both", "two", "three", "four", "five", "six", "seven", "eight",
        ]
        .contains(&first.to_ascii_lowercase().as_str())
    {
        remaining = rest;
    }
    remaining.trim()
}

fn split_first_word(value: &str) -> Option<(&str, &str)> {
    let split = value.find(char::is_whitespace)?;
    Some((&value[..split], value[split..].trim_start()))
}

fn current_clause(value: &str) -> &str {
    let start = value
        .rfind(['.', '!', '?', '\n'])
        .map_or(0, |offset| offset + 1);
    &value[start..]
}

fn consume_any<'a>(value: &'a str, phrases: &[&str]) -> Option<&'a str> {
    phrases
        .iter()
        .filter_map(|phrase| consume_phrase(value, phrase).map(|rest| (*phrase, rest)))
        .max_by_key(|(phrase, _)| phrase.len())
        .map(|(_, rest)| rest)
}

fn ordered_body(value: &str, arity: usize) -> Option<(Vec<&str>, usize)> {
    for marker in ["respectively", "in that order"] {
        let lower = value.to_ascii_lowercase();
        let Some(marker_start) = lower.find(marker) else {
            continue;
        };
        let descriptions = value[..marker_start]
            .trim_end_matches(|character: char| character.is_whitespace() || character == ',');
        let aligned = crate::scientific_prose::align_ordered_descriptions(descriptions, arity)?;
        let consumed = marker_start + marker.len();
        return Some((aligned, consumed));
    }
    None
}

fn leading_punctuation_end(value: &str) -> Option<usize> {
    value
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .and_then(|(offset, character)| {
            matches!(character, ',' | '.' | ';' | ':').then_some(offset + 1)
        })
}

fn match_lead(before: &str, clause: &str, leads: &[&str]) -> Option<usize> {
    let whole = clause.trim();
    let tail = whole
        .rsplit_once(',')
        .map(|(_, value)| value.trim())
        .unwrap_or(whole);
    [whole, tail].into_iter().find_map(|candidate| {
        leads
            .iter()
            .any(|lead| {
                candidate.eq_ignore_ascii_case(lead) && (!lead.is_empty() || whole.is_empty())
            })
            .then(|| before.len() - clause.len() + clause.rfind(candidate).unwrap_or(0))
    })
}

fn match_description<'a>(
    after: &'a str,
    links: &[&str],
    stop_at_comma: bool,
) -> Option<(&'a str, usize)> {
    let leading = after.len() - after.trim_start().len();
    let value = &after[leading..];
    let (_, body) = links
        .iter()
        .filter_map(|link| consume_phrase(value, link).map(|body| (*link, body)))
        .max_by_key(|(link, _)| link.len())?;
    let body_offset = after.len() - body.len();
    let end = body
        .char_indices()
        .find(|(_, character)| {
            matches!(character, '.' | ';' | '\n') || (stop_at_comma && *character == ',')
        })
        .map_or(body.len(), |(offset, _)| offset);
    let description = body[..end].trim().trim_end_matches([',', ':']).trim_end();
    let description_offset = body[..end].find(description)?;
    Some((
        description,
        body_offset + description_offset + description.len(),
    ))
}

fn consume_phrase<'a>(value: &'a str, phrase: &str) -> Option<&'a str> {
    if phrase.is_empty() {
        return Some(value);
    }
    let prefix = value.get(..phrase.len())?;
    if !prefix.eq_ignore_ascii_case(phrase) {
        return None;
    }
    let rest = &value[phrase.len()..];
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim_start())
}

fn strip_article(value: &str) -> &str {
    for article in ["a ", "an ", "the "] {
        if value
            .get(..article.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(article))
        {
            return value[article.len()..].trim_start();
        }
    }
    value
}

fn valid_description(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DESCRIPTION_BYTES
        && !value.contains('=')
        && !value.contains("\\[")
        && !value.contains("$$")
}

fn valid_plain_description(value: &str) -> bool {
    valid_description(value)
        && !value.contains('$')
        && !value.contains("\\(")
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '\'')
        })
}

#[cfg(test)]
mod tests {
    use super::{CoordinationLead, coordinated_descriptions, match_definition};

    #[test]
    fn composes_lemmas_without_recognizer_branches() {
        for (before, after, expected) in [
            ("Let ", " denote a state vector.", "a state vector"),
            ("Where ", " represents elapsed time.", "elapsed time"),
            ("We write ", " for the objective.", "the objective"),
            ("Denote by ", " the distance.", "distance"),
            ("The symbol ", " stands for the graph.", "the graph"),
        ] {
            assert_eq!(
                match_definition(before, after, false).map(|item| item.description),
                Some(expected),
                "{before}_MATH_{after}",
            );
        }
    }

    #[test]
    fn aligns_coordinated_constructions() {
        assert_eq!(
            coordinated_descriptions(
                CoordinationLead::Let,
                " represent the input, state, and output, in that order.",
                3,
            )
            .map(|(items, _, _)| items),
            Some(vec!["input", "state", "output"]),
        );
        assert_eq!(
            coordinated_descriptions(
                CoordinationLead::Let,
                " denote gain, bias, scale, and offset, respectively.",
                4,
            )
            .map(|(items, _, _)| items),
            Some(vec!["gain", "bias", "scale", "offset"]),
        );
    }

    #[test]
    fn empty_lead_does_not_match_after_an_unrelated_comma() {
        assert_eq!(
            match_definition("For completeness, ", " is the state.", false),
            None,
        );
        assert_eq!(
            match_definition("", " is the state.", false).map(|item| item.description),
            Some("the state"),
        );
    }
}
