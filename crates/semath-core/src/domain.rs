use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::pattern::FormulaAnalysis;
use crate::scope::ScopeGraph;
use crate::{DomainActivation, Evidence, ProjectDocument, SourceIndex, SourceRange};

const PACK_SCHEMA_VERSION: u32 = 1;
const MAX_PRIOR_MATCHES: usize = 64;
const MAX_ACTIVATIONS: usize = 8;
const MAX_EVIDENCE_PER_ACTIVATION: usize = 8;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainPack {
    schema_version: u32,
    pack_id: String,
    pack_version: String,
    title: String,
    activation_rules: Vec<ActivationRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivationRule {
    id: String,
    patterns: Vec<String>,
}

static DOMAIN_PACKS: LazyLock<Vec<DomainPack>> = LazyLock::new(|| {
    let packs = [
        include_str!("../../../packs/linear-algebra/v1.json"),
        include_str!("../../../packs/probability/v1.json"),
    ]
    .into_iter()
    .map(|source| {
        serde_json::from_str::<DomainPack>(source).expect("domain pack must be valid JSON")
    })
    .collect::<Vec<_>>();
    validate_packs(&packs).expect("domain packs must satisfy the activation schema");
    packs
});

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
pub(crate) struct DomainAnalysis {
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

impl DomainAnalysis {
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

pub(crate) fn analyze_domains(
    document: &ProjectDocument,
    formulas: &FormulaAnalysis,
) -> DomainAnalysis {
    let scopes = ScopeGraph::new(document);
    let priors = collect_priors(document, &scopes);
    let titles = DOMAIN_PACKS
        .iter()
        .map(|pack| (pack.pack_id.as_str(), pack.title.as_str()))
        .collect::<BTreeMap<_, _>>();
    let equations = formulas
        .all()
        .iter()
        .filter_map(|formula| {
            let evidence = formula
                .evidence
                .iter()
                .find(|evidence| evidence.kind == "domain-pattern")?
                .clone();
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
    DomainAnalysis {
        priors,
        equations,
        scopes,
    }
}

fn collect_priors(document: &ProjectDocument, scopes: &ScopeGraph) -> Vec<ScopedPrior> {
    let index = SourceIndex::new(&document.content);
    let mut priors = Vec::new();
    for pack in DOMAIN_PACKS.iter() {
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

fn validate_packs(packs: &[DomainPack]) -> Result<(), String> {
    let mut pack_ids = HashSet::new();
    for pack in packs {
        if pack.schema_version != PACK_SCHEMA_VERSION {
            return Err(format!("unsupported schema for {}", pack.pack_id));
        }
        if !pack_ids.insert(&pack.pack_id) {
            return Err(format!("duplicate domain pack {}", pack.pack_id));
        }
        if pack.title.is_empty() || pack.activation_rules.is_empty() {
            return Err(format!("incomplete domain pack {}", pack.pack_id));
        }
        let mut rule_ids = HashSet::new();
        for rule in &pack.activation_rules {
            if !rule_ids.insert(&rule.id) || rule.patterns.is_empty() {
                return Err(format!(
                    "invalid activation rule {}/{}",
                    pack.pack_id, rule.id
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::analyze_domains;
    use crate::consistency::analyze_consistency;
    use crate::parser::{math_regions, parse_regions};
    use crate::pattern::analyze_formulas;
    use crate::prose::analyze_prose;
    use crate::shape::analyze_shapes;
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str, language: DocumentLanguage) -> super::DomainAnalysis {
        let regions = math_regions(source, language);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.md".into(),
            language,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = analyze_prose(&document, &parsed);
        let shapes = analyze_shapes(&document, &parsed, &prose.shapes);
        let consistency = analyze_consistency(&document, &prose.definitions, &shapes);
        let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
        analyze_domains(&document, &formulas)
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
    fn promotes_a_typed_formula_pattern_only_in_its_equation() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}$\n$y = Ax$\n$z$";
        let domains = analyze(source, DocumentLanguage::Latex);
        let (formula_domains, _) = domains.at(source.rfind("Ax").unwrap() as u32);
        let algebra = formula_domains
            .iter()
            .find(|domain| domain.pack_id == "linear-algebra")
            .unwrap();
        assert_eq!(algebra.strength, "strong");
        assert_eq!(algebra.scope_kind, "equation");
        assert!(
            algebra
                .evidence
                .iter()
                .any(|evidence| evidence.kind == "domain-pattern")
        );

        let (later_domains, _) = domains.at(source.rfind('z').unwrap() as u32);
        assert!(later_domains.iter().all(|domain| domain.strength == "weak"));
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
