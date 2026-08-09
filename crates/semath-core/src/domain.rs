use std::collections::BTreeMap;

use regex::Regex;

use crate::pack::built_in_packs;
use crate::scope::ScopeGraph;
use crate::{
    DomainActivation, Evidence, LawRecognition, ProjectDocument, SourceIndex, SourceRange,
};

const MAX_PRIOR_MATCHES: usize = 64;
const MAX_ACTIVATIONS: usize = 8;
const MAX_EVIDENCE_PER_ACTIVATION: usize = 8;

#[derive(Clone, Debug)]
struct ScopedPrior {
    pack_id: String,
    pack_version: String,
    title: String,
    scope_id: usize,
    evidence: Evidence,
}

#[derive(Clone, Debug)]
struct EquationActivation {
    pack_id: String,
    pack_version: String,
    title: String,
    range: SourceRange,
    evidence: Evidence,
}

#[derive(Clone, Debug)]
pub(crate) struct DomainObservations {
    priors: Vec<ScopedPrior>,
    equations: Vec<EquationActivation>,
    scopes: ScopeGraph,
}

#[derive(Clone, Debug)]
struct ActivationAccumulator {
    pack_version: String,
    title: String,
    equation_range: Option<SourceRange>,
    evidence: Vec<Evidence>,
}

impl DomainObservations {
    pub fn at(&self, offset: u32) -> (Vec<DomainActivation>, bool) {
        let mut active = BTreeMap::<String, ActivationAccumulator>::new();
        for prior in &self.priors {
            if !self.scopes.visible(prior.scope_id, offset) {
                continue;
            }
            let activation =
                active
                    .entry(prior.pack_id.clone())
                    .or_insert_with(|| ActivationAccumulator {
                        pack_version: prior.pack_version.clone(),
                        title: prior.title.clone(),
                        equation_range: None,
                        evidence: Vec::new(),
                    });
            activation.evidence.push(prior.evidence.clone());
        }
        for equation in &self.equations {
            if !equation.range.contains(offset) {
                continue;
            }
            let activation =
                active
                    .entry(equation.pack_id.clone())
                    .or_insert_with(|| ActivationAccumulator {
                        pack_version: equation.pack_version.clone(),
                        title: equation.title.clone(),
                        equation_range: None,
                        evidence: Vec::new(),
                    });
            activation.equation_range = Some(equation.range.clone());
            activation.evidence.push(equation.evidence.clone());
        }

        let truncated = active.len() > MAX_ACTIVATIONS;
        let current_scope = self.scopes.range_at(offset);
        let document_scope = self.scopes.is_document_scope_at(offset);
        let activations = active
            .into_iter()
            .take(MAX_ACTIVATIONS)
            .map(|(pack_id, mut activation)| {
                activation.evidence.sort_by_key(|evidence| {
                    (
                        evidence.strength != "strong",
                        evidence.rule_id.clone(),
                        evidence
                            .source_ranges
                            .first()
                            .map_or(0, |range| range.start_offset),
                    )
                });
                activation.evidence.dedup();
                activation.evidence.truncate(MAX_EVIDENCE_PER_ACTIVATION);
                let (scope_kind, scope_range) = activation.equation_range.map_or_else(
                    || {
                        (
                            if document_scope {
                                "document"
                            } else {
                                "section"
                            },
                            current_scope.clone(),
                        )
                    },
                    |range| ("equation", range),
                );
                let strength = if activation
                    .evidence
                    .iter()
                    .any(|evidence| evidence.strength == "strong")
                {
                    "strong"
                } else {
                    "weak"
                };
                DomainActivation {
                    pack_id,
                    pack_version: activation.pack_version,
                    title: activation.title,
                    strength: strength.into(),
                    scope_kind: scope_kind.into(),
                    scope_range,
                    evidence: activation.evidence,
                }
            })
            .collect();
        (activations, truncated)
    }
}

pub(crate) fn observe_domains(
    document: &ProjectDocument,
    formulas: &[LawRecognition],
) -> DomainObservations {
    let scopes = ScopeGraph::new(document);
    let priors = collect_priors(document, &scopes);
    let titles = built_in_packs()
        .iter()
        .map(|pack| (pack.pack_id.as_str(), pack.title.as_str()))
        .collect::<BTreeMap<_, _>>();
    let equations = formulas
        .iter()
        .filter_map(|formula| {
            let evidence = formula.evidence.first()?.clone();
            Some(EquationActivation {
                pack_id: formula.pack_id.clone(),
                pack_version: formula.pack_version.clone(),
                title: titles
                    .get(formula.pack_id.as_str())
                    .copied()
                    .unwrap_or(formula.pack_id.as_str())
                    .into(),
                range: formula.range.clone(),
                evidence,
            })
        })
        .collect();
    DomainObservations {
        priors,
        equations,
        scopes,
    }
}

fn collect_priors(document: &ProjectDocument, scopes: &ScopeGraph) -> Vec<ScopedPrior> {
    let index = SourceIndex::new(&document.content);
    let mut priors = Vec::new();
    for pack in built_in_packs() {
        for rule in &pack.activation_rules {
            let matcher = literal_matcher(&rule.patterns);
            let mut ranges_by_scope = BTreeMap::<usize, Vec<SourceRange>>::new();
            for found in matcher.find_iter(&document.content) {
                if ranges_by_scope.values().map(Vec::len).sum::<usize>() >= MAX_PRIOR_MATCHES {
                    break;
                }
                if ignored_source(document, found.start()) {
                    continue;
                }
                let range = SourceRange {
                    start_offset: index.utf16_for_byte(found.start()),
                    end_offset: index.utf16_for_byte(found.end()),
                };
                ranges_by_scope
                    .entry(scopes.id_at(range.start_offset))
                    .or_default()
                    .push(range);
            }
            for (scope_id, mut source_ranges) in ranges_by_scope {
                source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
                source_ranges.dedup();
                priors.push(ScopedPrior {
                    pack_id: pack.pack_id.clone(),
                    pack_version: pack.pack_version.clone(),
                    title: pack.title.clone(),
                    scope_id,
                    evidence: Evidence {
                        rule_id: format!("{}/activation/{}", pack.pack_id, rule.id),
                        kind: "domain-prior".into(),
                        strength: "weak".into(),
                        source_ranges,
                    },
                });
            }
        }
    }
    priors
}

fn literal_matcher(patterns: &[String]) -> Regex {
    let mut patterns = patterns
        .iter()
        .map(|pattern| regex::escape(pattern))
        .collect::<Vec<_>>();
    patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.len()));
    Regex::new(&format!("(?i)(?:{})", patterns.join("|")))
        .expect("escaped literals are valid regex")
}

fn ignored_source(document: &ProjectDocument, byte_offset: usize) -> bool {
    match document.language {
        crate::DocumentLanguage::Latex => in_latex_comment(&document.content, byte_offset),
        crate::DocumentLanguage::Markdown => in_markdown_code(&document.content, byte_offset),
        crate::DocumentLanguage::Bibtex => false,
    }
}

fn in_latex_comment(source: &str, byte_offset: usize) -> bool {
    let line_start = source[..byte_offset]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    let line = &source[line_start..byte_offset];
    line.char_indices().any(|(position, character)| {
        character == '%'
            && line[..position]
                .chars()
                .rev()
                .take_while(|character| *character == '\\')
                .count()
                % 2
                == 0
    })
}

fn in_markdown_code(source: &str, byte_offset: usize) -> bool {
    let before = &source[..byte_offset];
    let mut fenced = false;
    let mut line_start = 0;
    for line in before.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        line_start += line.len();
    }
    if fenced {
        return true;
    }
    let current_line = &before[before[..line_start.min(before.len())]
        .rfind('\n')
        .map_or(0, |position| position + 1)..];
    current_line.matches('`').count() % 2 == 1
        || before
            .rfind("<!--")
            .is_some_and(|open| before.rfind("-->").is_none_or(|closed| closed < open))
}

#[cfg(test)]
mod tests {
    use super::observe_domains;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str, language: DocumentLanguage) -> super::DomainObservations {
        let regions = test_math_regions(source, language);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.md".into(),
            language,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let _ = parsed;
        observe_domains(&document, &[])
    }

    #[test]
    fn activates_multiple_weak_packs_in_one_section() {
        let source = "# Model\nA probability distribution over a random vector.\n$x$";
        let domains = analyze(source, DocumentLanguage::Markdown);
        let (active, truncated) = domains.at(source.rfind('x').unwrap() as u32);
        assert!(!truncated);
        assert_eq!(
            active
                .iter()
                .map(|domain| domain.pack_id.as_str())
                .collect::<Vec<_>>(),
            ["linear-algebra", "probability"]
        );
        assert!(active.iter().all(|domain| domain.strength == "weak"));
        assert!(active.iter().all(|domain| domain.scope_kind == "section"));
    }

    #[test]
    fn keeps_prior_evidence_inside_its_section() {
        let source = "# Probability\nrandom variable\n$x$\n# Algebra\nmatrix\n$A$";
        let domains = analyze(source, DocumentLanguage::Markdown);
        let (active, _) = domains.at(source.rfind('A').unwrap() as u32);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].pack_id, "linear-algebra");
    }

    #[test]
    fn ignores_commented_and_code_fenced_priors() {
        let latex = "% probability matrix\n$x$";
        assert!(
            analyze(latex, DocumentLanguage::Latex)
                .at(latex.rfind('x').unwrap() as u32)
                .0
                .is_empty()
        );
        let markdown = "```\nprobability matrix\n```\n$x$";
        assert!(
            analyze(markdown, DocumentLanguage::Markdown)
                .at(markdown.rfind('x').unwrap() as u32)
                .0
                .is_empty()
        );
    }
}
