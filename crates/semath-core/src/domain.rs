use std::collections::BTreeMap;

use crate::pack::built_in_packs;
use crate::prose::ScientificSemanticEvidence;
use crate::scope::ScopeGraph;
use crate::{DomainActivation, Evidence, LawRecognition, SourceRange};

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
    scopes: ScopeGraph,
    semantic_evidence: &ScientificSemanticEvidence,
    formulas: &[LawRecognition],
) -> DomainObservations {
    let priors = collect_priors(semantic_evidence, &scopes);
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

fn collect_priors(
    semantic_evidence: &ScientificSemanticEvidence,
    scopes: &ScopeGraph,
) -> Vec<ScopedPrior> {
    semantic_evidence
        .domain_priors
        .iter()
        .filter(|prior| prior.frame.establishes())
        .take(MAX_PRIOR_MATCHES)
        .map(|prior| ScopedPrior {
            pack_id: prior.pack_id.clone(),
            pack_version: prior.pack_version.clone(),
            title: prior.title.clone(),
            scope_id: scopes.id_at(
                prior
                    .evidence
                    .source_ranges
                    .first()
                    .map_or(0, |range| range.start_offset),
            ),
            evidence: prior.evidence.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::observe_domains;
    use crate::canonical::lower_document_region;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::scope::ScopeGraph;
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str, language: DocumentLanguage) -> super::DomainObservations {
        let regions = test_math_regions(source, language);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.md".into(),
            language,
            content: source.into(),
            document_version: 1,
            schema_version: 6,
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
        let prose = observe_prose(&document, &parsed, &canonical);
        observe_domains(ScopeGraph::new(&document), &prose.semantic_evidence, &[])
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
