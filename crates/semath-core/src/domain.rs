use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::domain_signature::{
    DomainSignature, compile_domain_signatures, contains_domain_term, normalize_domain_text,
};
use crate::pack::built_in_packs;
use crate::prose::ScientificSemanticEvidence;
use crate::scope::ScopeGraph;
use crate::{
    ConstraintStatus, DomainActivation, DomainRelevance, DomainSupportTier, Evidence,
    LawBindingProof, LawRecognition, LawRecognitionStatus, MathRootState, ProjectDocument,
    SourceRange,
};

const MAX_PRIOR_MATCHES: usize = 64;
const MAX_ACTIVATIONS: usize = 8;
const MAX_EVIDENCE_PER_ACTIVATION: usize = 8;

static DOMAIN_SIGNATURES: LazyLock<Vec<DomainSignature>> =
    LazyLock::new(|| compile_domain_signatures(built_in_packs()));

#[derive(Clone, Debug)]
struct ScopedHypothesis {
    pack_id: String,
    pack_version: String,
    title: String,
    scope_id: usize,
    scope_kind: &'static str,
    support: DomainSupportTier,
    specificity: usize,
    evidence: Evidence,
}

#[derive(Clone, Debug)]
struct EquationActivation {
    pack_id: String,
    pack_version: String,
    title: String,
    range: SourceRange,
    scope_id: usize,
    source_established: bool,
    evidence: Evidence,
}

#[derive(Clone, Debug)]
pub(crate) struct DomainObservations {
    hypotheses: Vec<ScopedHypothesis>,
    equations: Vec<EquationActivation>,
    scopes: ScopeGraph,
    include_current_equation: bool,
}

#[derive(Clone, Debug)]
struct ActivationAccumulator {
    pack_version: String,
    title: String,
    scope_kind: &'static str,
    scope_range: SourceRange,
    support: DomainSupportTier,
    specificity: usize,
    evidence: Vec<Evidence>,
}

struct ActivationInput<'a> {
    pack_id: &'a str,
    pack_version: &'a str,
    title: &'a str,
    scope_kind: &'static str,
    scope_range: SourceRange,
    support: DomainSupportTier,
    specificity: usize,
    evidence: Evidence,
}

impl DomainObservations {
    pub fn at(&self, offset: u32) -> (Vec<DomainActivation>, bool) {
        let mut active = self.all_at(offset);
        let truncated = active.len() > MAX_ACTIVATIONS;
        active.truncate(MAX_ACTIVATIONS);
        (active, truncated)
    }

    pub(crate) fn all_at(&self, offset: u32) -> Vec<DomainActivation> {
        self.accumulate(offset, None)
            .into_iter()
            .map(|(pack_id, activation)| DomainActivation {
                pack_id,
                pack_version: activation.pack_version,
                title: activation.title,
                support: activation.support,
                scope_kind: activation.scope_kind.into(),
                scope_range: activation.scope_range,
                evidence: activation.evidence,
            })
            .collect()
    }

    pub(crate) fn all_for_range_with_truncation(
        &self,
        range: &SourceRange,
    ) -> (Vec<DomainActivation>, bool) {
        let active = self.all_for_range(range);
        let truncated = active.len() > MAX_ACTIVATIONS;
        (active, truncated)
    }

    pub(crate) fn all_for_range(&self, range: &SourceRange) -> Vec<DomainActivation> {
        self.accumulate(range.start_offset, Some(range))
            .into_iter()
            .map(|(pack_id, activation)| DomainActivation {
                pack_id,
                pack_version: activation.pack_version,
                title: activation.title,
                support: activation.support,
                scope_kind: activation.scope_kind.into(),
                scope_range: activation.scope_range,
                evidence: activation.evidence,
            })
            .collect()
    }

    pub(crate) fn relevance(&self, pack_id: &str, offset: u32) -> Option<DomainRelevance> {
        self.accumulate(offset, None)
            .into_iter()
            .find(|(candidate, _)| candidate == pack_id)
            .map(|(_, activation)| DomainRelevance {
                support: activation.support,
                evidence: activation.evidence,
            })
    }

    pub(crate) fn hypothesis_count(&self) -> u32 {
        self.hypotheses.len() as u32
    }

    pub(crate) fn evidence_count(&self) -> u32 {
        self.hypotheses
            .iter()
            .map(|hypothesis| hypothesis.evidence.source_ranges.len() as u32)
            .sum::<u32>()
            + self.equations.len() as u32
    }

    fn accumulate(
        &self,
        offset: u32,
        selected_range: Option<&SourceRange>,
    ) -> Vec<(String, ActivationAccumulator)> {
        let mut active = BTreeMap::<String, ActivationAccumulator>::new();
        for hypothesis in &self.hypotheses {
            if !self.scopes.visible(hypothesis.scope_id, offset) {
                continue;
            }
            merge_activation(
                &mut active,
                ActivationInput {
                    pack_id: &hypothesis.pack_id,
                    pack_version: &hypothesis.pack_version,
                    title: &hypothesis.title,
                    scope_kind: hypothesis.scope_kind,
                    scope_range: self.scopes.range_at(offset),
                    support: hypothesis.support,
                    specificity: hypothesis.specificity,
                    evidence: hypothesis.evidence.clone(),
                },
            );
        }
        for equation in &self.equations {
            let in_equation = self.include_current_equation
                && selected_range.map_or_else(
                    || equation.range.contains(offset),
                    |range| {
                        equation.range.start_offset < range.end_offset
                            && range.start_offset < equation.range.end_offset
                    },
                );
            let precedes_in_scope = equation.range.end_offset <= offset
                && equation.source_established
                && self.scopes.visible(equation.scope_id, offset);
            if in_equation || precedes_in_scope {
                merge_activation(
                    &mut active,
                    ActivationInput {
                        pack_id: &equation.pack_id,
                        pack_version: &equation.pack_version,
                        title: &equation.title,
                        scope_kind: if in_equation { "equation" } else { "section" },
                        scope_range: if in_equation {
                            equation.range.clone()
                        } else {
                            self.scopes.range_at(offset)
                        },
                        support: if in_equation {
                            DomainSupportTier::Explicit
                        } else {
                            DomainSupportTier::Supported
                        },
                        specificity: usize::MAX,
                        evidence: equation.evidence.clone(),
                    },
                );
            }
        }
        let mut active = active.into_iter().collect::<Vec<_>>();
        for (_, activation) in &mut active {
            activation.evidence.sort_by_key(|evidence| {
                (
                    evidence.rule_id.clone(),
                    evidence
                        .source_ranges
                        .first()
                        .map_or(0, |range| range.start_offset),
                )
            });
            activation.evidence.dedup();
            activation.evidence.truncate(MAX_EVIDENCE_PER_ACTIVATION);
        }
        active.sort_by(|(left_id, left), (right_id, right)| {
            support_rank(left.support)
                .cmp(&support_rank(right.support))
                .then(right.specificity.cmp(&left.specificity))
                .then(left_id.cmp(right_id))
        });
        active
    }

    pub(crate) fn has_forward_law_routing_target(&self, formula_ranges: &[SourceRange]) -> bool {
        self.equations
            .iter()
            .filter(|equation| equation.source_established)
            .any(|equation| {
                formula_ranges.iter().any(|formula| {
                    let target_offset = if equation.range.end_offset <= formula.start_offset {
                        formula.start_offset
                    } else if formula.start_offset <= equation.range.start_offset
                        && equation.range.end_offset < formula.end_offset
                    {
                        // One math root can contain multiple source-ordered
                        // relations. Its envelope begins before the established
                        // relation, so preserve the routed pass whenever content
                        // remains that may contain a later relation.
                        equation.range.end_offset
                    } else {
                        return false;
                    };
                    self.scopes.visible(equation.scope_id, target_offset)
                })
            })
    }

    pub(crate) fn for_forward_law_routing(mut self) -> Self {
        self.include_current_equation = false;
        self
    }
}

fn merge_activation(
    active: &mut BTreeMap<String, ActivationAccumulator>,
    input: ActivationInput<'_>,
) {
    let activation =
        active
            .entry(input.pack_id.to_owned())
            .or_insert_with(|| ActivationAccumulator {
                pack_version: input.pack_version.to_owned(),
                title: input.title.to_owned(),
                scope_kind: input.scope_kind,
                scope_range: input.scope_range.clone(),
                support: input.support,
                specificity: input.specificity,
                evidence: Vec::new(),
            });
    if support_rank(input.support) < support_rank(activation.support)
        || (input.support == activation.support && input.specificity > activation.specificity)
    {
        activation.scope_kind = input.scope_kind;
        activation.scope_range = input.scope_range;
        activation.support = input.support;
        activation.specificity = input.specificity;
    }
    activation.evidence.push(input.evidence);
}

pub(crate) fn support_rank(support: DomainSupportTier) -> u32 {
    match support {
        DomainSupportTier::Explicit => 0,
        DomainSupportTier::Supported => 10,
        DomainSupportTier::Tentative => 20,
    }
}

pub(crate) fn observe_domains(
    document: &ProjectDocument,
    scopes: ScopeGraph,
    semantic_evidence: &ScientificSemanticEvidence,
    formulas: &[LawRecognition],
) -> DomainObservations {
    let mut hypotheses = collect_priors(semantic_evidence, &scopes);
    hypotheses.extend(collect_document_fields(document, &scopes));
    hypotheses.extend(collect_section_headings(document, &scopes));
    hypotheses.sort_by(|left, right| {
        left.pack_id
            .cmp(&right.pack_id)
            .then(left.scope_id.cmp(&right.scope_id))
            .then(left.evidence.rule_id.cmp(&right.evidence.rule_id))
    });
    hypotheses.dedup_by(|left, right| {
        left.pack_id == right.pack_id
            && left.scope_id == right.scope_id
            && left.evidence == right.evidence
    });
    let titles = DOMAIN_SIGNATURES
        .iter()
        .map(|signature| (signature.pack_id.as_str(), signature.title.as_str()))
        .collect::<BTreeMap<_, _>>();
    let equations = formulas
        .iter()
        .filter(|formula| !formula.non_authoritative)
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
                scope_id: scopes.id_at(formula.range.start_offset),
                source_established: formula_is_source_established(formula),
                evidence,
            })
        })
        .collect();
    DomainObservations {
        hypotheses,
        equations,
        scopes,
        include_current_equation: true,
    }
}

fn formula_is_source_established(formula: &LawRecognition) -> bool {
    let independently_typed = formula_has_independent_typed_evidence(formula);
    matches!(
        formula.status,
        LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
    ) && independently_typed
        && formula.bindings.iter().all(|binding| {
            matches!(
                binding.proof,
                LawBindingProof::Typed | LawBindingProof::Derived
            ) && !binding.evidence.source_ranges.is_empty()
        })
        && formula
            .conditions
            .iter()
            .all(|condition| condition.status == ConstraintStatus::Verified)
}

pub(crate) fn formula_has_independent_typed_evidence(formula: &LawRecognition) -> bool {
    formula.bindings.iter().any(|binding| {
        binding.proof == LawBindingProof::Typed && !binding.evidence.source_ranges.is_empty()
    }) || formula.conditions.iter().any(|condition| {
        condition.status == ConstraintStatus::Verified
            && condition.evidence.iter().any(|evidence| {
                evidence.kind == "canonical-binding"
                    && evidence.rule_id.starts_with("typed-law-role/")
                    && !evidence.source_ranges.is_empty()
            })
    })
}

fn collect_priors(
    semantic_evidence: &ScientificSemanticEvidence,
    scopes: &ScopeGraph,
) -> Vec<ScopedHypothesis> {
    semantic_evidence
        .domain_priors
        .iter()
        .filter(|prior| prior.frame.establishes())
        .take(MAX_PRIOR_MATCHES)
        .map(|prior| {
            let offset = prior
                .evidence
                .source_ranges
                .first()
                .map_or(0, |range| range.start_offset);
            ScopedHypothesis {
                pack_id: prior.pack_id.clone(),
                pack_version: prior.pack_version.clone(),
                title: prior.title.clone(),
                scope_id: scopes.id_at(offset),
                scope_kind: if scopes.is_document_scope_at(offset) {
                    "document"
                } else {
                    "section"
                },
                support: DomainSupportTier::Tentative,
                specificity: 0,
                evidence: prior.evidence.clone(),
            }
        })
        .collect()
}

fn collect_document_fields(
    document: &ProjectDocument,
    scopes: &ScopeGraph,
) -> Vec<ScopedHypothesis> {
    document
        .prose_annotations
        .iter()
        .filter(|annotation| {
            annotation.kind == "document-field"
                && matches!(annotation.name.as_str(), "title" | "keywords")
                && annotation.state == MathRootState::Complete
        })
        .flat_map(|annotation| {
            let range = annotation.value_range.as_ref().unwrap_or(&annotation.range);
            let text = source_text(document, range);
            hypotheses_for_text(
                text,
                range.clone(),
                scopes.id_at(range.start_offset),
                "document",
                "document-field",
                DomainSupportTier::Supported,
            )
        })
        .collect()
}

fn collect_section_headings(
    document: &ProjectDocument,
    scopes: &ScopeGraph,
) -> Vec<ScopedHypothesis> {
    document
        .scopes
        .iter()
        .filter_map(|scope| {
            let name = scope.name.as_deref()?;
            let source = scope.source.as_ref()?;
            Some(hypotheses_for_text(
                name,
                source.range.clone(),
                scopes.id_at(source.range.start_offset),
                "section",
                "section-heading",
                DomainSupportTier::Supported,
            ))
        })
        .flatten()
        .collect()
}

fn hypotheses_for_text(
    text: &str,
    range: SourceRange,
    scope_id: usize,
    scope_kind: &'static str,
    source: &str,
    support: DomainSupportTier,
) -> Vec<ScopedHypothesis> {
    let normalized = normalize_domain_text(text);
    DOMAIN_SIGNATURES
        .iter()
        .filter_map(|signature| {
            let term = signature
                .terms
                .iter()
                .filter(|term| contains_domain_term(&normalized, &term.text))
                .max_by_key(|term| term.text.len())?;
            Some(ScopedHypothesis {
                pack_id: signature.pack_id.clone(),
                pack_version: signature.pack_version.clone(),
                title: signature.title.clone(),
                scope_id,
                scope_kind,
                support,
                specificity: term.text.len(),
                evidence: Evidence {
                    rule_id: format!("domain-signature/{source}/{}", term.source),
                    kind: "domain-context".into(),
                    strength: "contextual".into(),
                    source_ranges: vec![range.clone()],
                    source_anchors: Vec::new(),
                },
            })
        })
        .collect()
}

fn source_text<'a>(document: &'a ProjectDocument, range: &SourceRange) -> &'a str {
    document
        .content
        .get(range.start_offset as usize..range.end_offset as usize)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{EquationActivation, observe_domains};
    use crate::canonical::lower_document_region;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::scope::ScopeGraph;
    use crate::{
        DocumentLanguage, DomainSupportTier, Evidence, MathRootState, ProjectDocument,
        ProseAnnotation, SourceIndex, SourceRange,
    };

    fn analyze(source: &str, language: DocumentLanguage) -> super::DomainObservations {
        let regions = test_math_regions(source, language);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.md".into(),
            language,
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
        let parsed = parse_regions(source, &regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let prose = observe_prose(&document, &parsed, &canonical);
        observe_domains(
            &document,
            ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        )
    }

    fn add_equation(
        domains: &mut super::DomainObservations,
        source: &str,
        needle: &str,
        source_established: bool,
    ) -> SourceRange {
        let index = SourceIndex::new(source);
        let start_byte = source.find(needle).unwrap();
        let end_byte = start_byte + needle.len();
        let range = SourceRange {
            start_offset: index.utf16_for_byte(start_byte),
            end_offset: index.utf16_for_byte(end_byte),
        };
        domains.equations.push(EquationActivation {
            pack_id: "test-domain".into(),
            pack_version: "1".into(),
            title: "Test domain".into(),
            range: range.clone(),
            scope_id: domains.scopes.id_at(range.start_offset),
            source_established,
            evidence: Evidence {
                rule_id: "test-equation".into(),
                kind: "canonical-math".into(),
                strength: "exact".into(),
                source_ranges: vec![range.clone()],
                source_anchors: Vec::new(),
            },
        });
        range
    }

    fn formula_range(source: &str, needle: &str) -> SourceRange {
        let index = SourceIndex::new(source);
        let start_byte = source.find(needle).unwrap();
        SourceRange {
            start_offset: index.utf16_for_byte(start_byte),
            end_offset: index.utf16_for_byte(start_byte + needle.len()),
        }
    }

    #[test]
    fn forward_law_routing_requires_a_later_formula() {
        let source = "$first$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        let equation = add_equation(&mut domains, source, "first", true);

        assert!(!domains.has_forward_law_routing_target(&[]));
        assert!(!domains.has_forward_law_routing_target(&[equation]));
    }

    #[test]
    fn forward_law_routing_accepts_a_later_formula_in_visible_scope() {
        let source = "# Model\n$first$ and then $later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        add_equation(&mut domains, source, "first", true);

        assert!(domains.has_forward_law_routing_target(&[formula_range(source, "later")]));
    }

    #[test]
    fn forward_law_routing_uses_utf16_offsets_after_unicode_text() {
        let source = "🧪 model $first$ and then $later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        let equation = add_equation(&mut domains, source, "first", true);
        let later = formula_range(source, "later");

        assert_eq!(
            equation.start_offset,
            source[..source.find("first").unwrap()]
                .encode_utf16()
                .count() as u32
        );
        assert!(domains.has_forward_law_routing_target(&[later]));
    }

    #[test]
    fn forward_law_routing_accepts_a_child_formula_visible_from_its_parent_scope() {
        let source = "# Parent\n$first$\n## Child\n$later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        add_equation(&mut domains, source, "first", true);

        assert!(domains.has_forward_law_routing_target(&[formula_range(source, "later")]));
    }

    #[test]
    fn forward_law_routing_accepts_an_exactly_adjacent_range_boundary() {
        let source = "$first$$later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        let equation = add_equation(&mut domains, source, "first", true);
        let adjacent = SourceRange {
            start_offset: equation.end_offset,
            end_offset: equation.end_offset + 1,
        };

        assert!(domains.has_forward_law_routing_target(&[adjacent]));
    }

    #[test]
    fn forward_law_routing_preserves_later_relations_inside_one_formula_envelope() {
        let source = "$first, later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        let equation = add_equation(&mut domains, source, "first", true);
        let formula = SourceRange {
            start_offset: equation.start_offset,
            end_offset: formula_range(source, "later").end_offset,
        };

        assert!(domains.has_forward_law_routing_target(&[formula]));
    }

    #[test]
    fn forward_law_routing_rejects_a_later_formula_in_a_sibling_scope() {
        let source = "# First\n$first$\n# Second\n$later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        add_equation(&mut domains, source, "first", true);

        assert!(!domains.has_forward_law_routing_target(&[formula_range(source, "later")]));
    }

    #[test]
    fn forward_law_routing_does_not_escape_a_nested_scope() {
        let source = "# Parent\n## Child\n$first$\n# Sibling\n$later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        add_equation(&mut domains, source, "first", true);

        assert!(!domains.has_forward_law_routing_target(&[formula_range(source, "later")]));
    }

    #[test]
    fn forward_law_routing_requires_source_establishment() {
        let source = "$first$ and then $later$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        add_equation(&mut domains, source, "first", false);

        assert!(!domains.has_forward_law_routing_target(&[formula_range(source, "later")]));
    }

    #[test]
    fn a_current_recognized_equation_reports_explicit_domain_relevance() {
        let source = "$first$";
        let mut domains = analyze(source, DocumentLanguage::Markdown);
        let equation = add_equation(&mut domains, source, "first", false);

        let active = domains.at(equation.start_offset).0;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].support, DomainSupportTier::Explicit);
    }

    #[test]
    fn keeps_multiple_body_hypotheses_tentative() {
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
        assert!(
            active
                .iter()
                .all(|domain| domain.support == DomainSupportTier::Tentative)
        );
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
    fn syntax_section_names_upgrade_matching_body_priors_to_supported() {
        let source = "# Probability\nA probability model.\n$x$";
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.md".into(),
            language: DocumentLanguage::Markdown,
            content: source.into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: vec![
                crate::SyntaxScope {
                    kind: "document".into(),
                    parent: None,
                    range: SourceRange {
                        start_offset: 0,
                        end_offset: source.len() as u32,
                    },
                    state: MathRootState::Complete,
                    name: None,
                    level: None,
                    source: None,
                },
                crate::SyntaxScope {
                    kind: "section".into(),
                    parent: Some(0),
                    range: SourceRange {
                        start_offset: 0,
                        end_offset: source.len() as u32,
                    },
                    state: MathRootState::Complete,
                    name: Some("Probability".into()),
                    level: None,
                    source: Some(crate::ProjectSourceRef {
                        file_id: "main".into(),
                        path: "main.md".into(),
                        range: SourceRange {
                            start_offset: 0,
                            end_offset: 13,
                        },
                    }),
                },
            ],
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: test_math_regions(source, DocumentLanguage::Markdown),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &document.math_regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let prose = observe_prose(&document, &parsed, &canonical);
        let domains = observe_domains(
            &document,
            ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        );
        let active = domains.at(source.rfind('x').unwrap() as u32).0;
        assert_eq!(active[0].support, DomainSupportTier::Supported);
    }

    #[test]
    fn complete_title_and_keywords_are_supported_but_authors_are_not_domain_evidence() {
        let source = "Probability and stochastic models\n$x$";
        let mut document = ProjectDocument {
            prose_annotations: vec![ProseAnnotation {
                kind: "document-field".into(),
                name: "title".into(),
                range: SourceRange {
                    start_offset: 0,
                    end_offset: 33,
                },
                value_range: Some(SourceRange {
                    start_offset: 0,
                    end_offset: 33,
                }),
                state: MathRootState::Complete,
            }],
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
        let prose = observe_prose(&document, &[], &[]);
        let domains = observe_domains(
            &document,
            ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        );
        let active = domains.at(source.rfind('x').unwrap() as u32).0;
        assert!(active.iter().any(|domain| {
            domain.pack_id == "probability" && domain.support == DomainSupportTier::Supported
        }));

        document.prose_annotations[0].name = "author".into();
        let prose = observe_prose(&document, &[], &[]);
        assert!(
            observe_domains(
                &document,
                ScopeGraph::new(&document),
                &prose.semantic_evidence,
                &[],
            )
            .at(source.rfind('x').unwrap() as u32)
            .0
            .iter()
            .all(|domain| domain.support != DomainSupportTier::Supported)
        );
    }
}
