use crate::concept::{classify_role_candidates, concepts_share_lineage};
use crate::prose::definition_available_from;
use crate::scope::ScopeGraph;
use crate::shape::{ExplicitShapeClaim, ShapeObservations};
use crate::{DefinitionInfo, Evidence, ProjectDocument, RoleInfo, SemanticDiagnostic, SourceRange};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROLE_CLAIMS: usize = 8;
const MAX_ROLE_DIAGNOSTICS: usize = 8;

#[derive(Clone, Debug)]
struct ScopedRoleClaim {
    info: RoleInfo,
    symbol_range: SourceRange,
    available_from: u32,
    scope_id: usize,
    occurrence_bound: bool,
}

#[derive(Clone, Debug)]
struct ScopedShapeClaim {
    claim: ExplicitShapeClaim,
    scope_id: usize,
}

#[derive(Clone, Debug)]
struct DiagnosticEntry {
    symbol: String,
    scope_id: usize,
    diagnostic: SemanticDiagnostic,
}

#[derive(Clone, Debug)]
pub(crate) struct RoleObservations {
    roles: Vec<ScopedRoleClaim>,
    entries: Vec<DiagnosticEntry>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    scopes: ScopeGraph,
    notation_families: BTreeMap<String, String>,
}

impl RoleObservations {
    pub fn all(&self) -> Vec<RoleInfo> {
        self.roles.iter().map(|claim| claim.info.clone()).collect()
    }

    pub fn exported(&self) -> Vec<RoleInfo> {
        self.roles
            .iter()
            .filter(|claim| self.scopes.depth(claim.scope_id) == 0 && !claim.occurrence_bound)
            .map(|claim| claim.info.clone())
            .collect()
    }

    pub fn roles_at(&self, symbol: &str, offset: u32) -> (Vec<RoleInfo>, bool) {
        let mut roles = self
            .roles
            .iter()
            .filter(|claim| {
                self.symbols_equivalent(&claim.info.symbol, symbol)
                    && (!claim.occurrence_bound || claim.symbol_range.contains(offset))
                    && (self.scopes.depth(claim.scope_id) == 0
                        || claim.available_from <= offset
                        || claim.symbol_range.contains(offset))
                    && self.scopes.visible(claim.scope_id, offset)
            })
            .collect::<Vec<_>>();
        roles.sort_by_key(|claim| {
            (
                std::cmp::Reverse(self.scopes.depth(claim.scope_id)),
                std::cmp::Reverse(claim.available_from),
            )
        });
        let truncated = roles.len() > MAX_ROLE_CLAIMS;
        (
            roles
                .into_iter()
                .take(MAX_ROLE_CLAIMS)
                .map(|claim| claim.info.clone())
                .collect(),
            truncated,
        )
    }

    pub(crate) fn has_occurrence_role(
        &self,
        symbol: &str,
        range: &SourceRange,
        concept: &str,
    ) -> bool {
        self.roles.iter().any(|claim| {
            claim.occurrence_bound
                && claim.symbol_range == *range
                && self.symbols_equivalent(&claim.info.symbol, symbol)
                && concepts_share_lineage(&claim.info.concept_id, concept)
        })
    }

    pub(crate) fn occurrence_role_evidence_ranges(
        &self,
        symbol: &str,
        range: &SourceRange,
        concept: &str,
    ) -> Vec<SourceRange> {
        self.roles
            .iter()
            .filter(|claim| {
                claim.occurrence_bound
                    && claim.symbol_range == *range
                    && self.symbols_equivalent(&claim.info.symbol, symbol)
                    && concepts_share_lineage(&claim.info.concept_id, concept)
            })
            .flat_map(|claim| claim.info.evidence.source_ranges.iter().cloned())
            .collect()
    }

    pub fn diagnostics_for(&self, symbol: &str, offset: u32) -> (Vec<SemanticDiagnostic>, bool) {
        let diagnostics = self
            .entries
            .iter()
            .filter(|entry| {
                self.symbols_equivalent(&entry.symbol, symbol)
                    && entry.diagnostic.range.start_offset <= offset
                    && self.scopes.visible(entry.scope_id, offset)
            })
            .collect::<Vec<_>>();
        let truncated = diagnostics.len() > MAX_ROLE_DIAGNOSTICS;
        (
            diagnostics
                .into_iter()
                .take(MAX_ROLE_DIAGNOSTICS)
                .map(|entry| entry.diagnostic.clone())
                .collect(),
            truncated,
        )
    }

    pub fn diagnostic(&self, code: &str, offset: u32) -> Option<SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code && diagnostic.range.contains(offset))
            .cloned()
    }

    fn symbols_equivalent(&self, left: &str, right: &str) -> bool {
        let left = left.trim_start_matches('\\');
        let right = right.trim_start_matches('\\');
        left == right
            || self
                .notation_families
                .get(left)
                .zip(self.notation_families.get(right))
                .is_some_and(|(left_family, right_family)| left_family == right_family)
    }
}

pub(crate) fn observe_roles(
    document: &ProjectDocument,
    definitions: &[DefinitionInfo],
    shapes: &ShapeObservations,
) -> RoleObservations {
    let scopes = ScopeGraph::new(document);
    let roles = definitions
        .iter()
        .flat_map(|definition| role_claims(definition, &scopes))
        .collect::<Vec<_>>();
    let shape_claims = shapes
        .explicit_claims()
        .into_iter()
        .map(|claim| ScopedShapeClaim {
            scope_id: scopes.id_at(claim.symbol_range.start_offset),
            claim,
        })
        .collect::<Vec<_>>();

    let mut groups = BTreeMap::<(String, usize), (Vec<usize>, Vec<usize>)>::new();
    for (index, role) in roles
        .iter()
        .enumerate()
        .filter(|(_, role)| !role.occurrence_bound)
    {
        groups
            .entry((role.info.symbol.clone(), role.scope_id))
            .or_default()
            .0
            .push(index);
    }
    for (index, shape) in shape_claims.iter().enumerate() {
        groups
            .entry((shape.claim.symbol.clone(), shape.scope_id))
            .or_default()
            .1
            .push(index);
    }

    let mut entries = Vec::new();
    for ((symbol, scope_id), (role_indexes, shape_indexes)) in groups {
        if let Some(diagnostic) = role_conflict_diagnostic(&symbol, &roles, &role_indexes) {
            entries.push(DiagnosticEntry {
                symbol: symbol.clone(),
                scope_id,
                diagnostic,
            });
        }
        if let Some(diagnostic) = role_type_conflict_diagnostic(
            &symbol,
            &roles,
            &role_indexes,
            &shape_claims,
            &shape_indexes,
        ) {
            entries.push(DiagnosticEntry {
                symbol,
                scope_id,
                diagnostic,
            });
        }
    }
    entries.sort_by_key(|entry| entry.diagnostic.range.start_offset);
    let diagnostics = entries
        .iter()
        .map(|entry| entry.diagnostic.clone())
        .collect();
    RoleObservations {
        roles,
        entries,
        diagnostics,
        scopes,
        notation_families: shapes.notation_families(),
    }
}

fn role_claims(definition: &DefinitionInfo, scopes: &ScopeGraph) -> Vec<ScopedRoleClaim> {
    let symbol_range = definition.location.range.clone();
    let available_from = definition_available_from(definition);
    classify_role_candidates(&definition.description)
        .into_iter()
        .map(|role| ScopedRoleClaim {
            info: RoleInfo {
                symbol: definition.symbol.clone(),
                concept_id: role.clone(),
                description: definition.description.clone(),
                evidence: Evidence {
                    rule_id: format!("{}/concept-{role}", definition.evidence.rule_id),
                    kind: "explicit-prose".into(),
                    strength: "strong".into(),
                    source_ranges: definition.evidence.source_ranges.clone(),
                },
            },
            scope_id: scopes.id_at(symbol_range.start_offset),
            symbol_range: symbol_range.clone(),
            available_from,
            occurrence_bound: definition
                .evidence
                .rule_id
                .starts_with("formula-occurrence-role/"),
        })
        .collect()
}

fn role_conflict_diagnostic(
    symbol: &str,
    roles: &[ScopedRoleClaim],
    indexes: &[usize],
) -> Option<SemanticDiagnostic> {
    let mut conflicting = BTreeSet::new();
    for (position, left) in indexes.iter().enumerate() {
        for right in &indexes[position + 1..] {
            if roles_conflict(
                &roles[*left].info.concept_id,
                &roles[*right].info.concept_id,
            ) {
                conflicting.insert(*left);
                conflicting.insert(*right);
            }
        }
    }
    if conflicting.is_empty() {
        return None;
    }
    let claims = conflicting
        .into_iter()
        .map(|index| &roles[index])
        .collect::<Vec<_>>();
    let role_names = claims
        .iter()
        .map(|claim| concept_leaf(&claim.info.concept_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    Some(SemanticDiagnostic {
        code: "notation-role-conflict".into(),
        severity: "warning".into(),
        message: format!("Notation `{symbol}` has incompatible explicit roles: {role_names}."),
        explanation: format!(
            "The same notation is explicitly defined with mutually exclusive semantic roles in one document scope: {}.",
            descriptions(claims.iter().map(|claim| claim.info.description.as_str()))
        ),
        range: latest_role_range(&claims),
        evidence: evidence(claims.iter().map(|claim| &claim.info.evidence)),
    })
}

fn role_type_conflict_diagnostic(
    symbol: &str,
    roles: &[ScopedRoleClaim],
    role_indexes: &[usize],
    shapes: &[ScopedShapeClaim],
    shape_indexes: &[usize],
) -> Option<SemanticDiagnostic> {
    let mut conflicting_roles = BTreeSet::new();
    let mut conflicting_shapes = BTreeSet::new();
    for role in role_indexes {
        for shape in shape_indexes {
            if role_shape_conflict(&roles[*role].info.concept_id, &shapes[*shape].claim.kind) {
                conflicting_roles.insert(*role);
                conflicting_shapes.insert(*shape);
            }
        }
    }
    if conflicting_roles.is_empty() {
        return None;
    }
    let role_claims = conflicting_roles
        .into_iter()
        .map(|index| &roles[index])
        .collect::<Vec<_>>();
    let shape_claims = conflicting_shapes
        .into_iter()
        .map(|index| &shapes[index].claim)
        .collect::<Vec<_>>();
    let role_names = role_claims
        .iter()
        .map(|claim| concept_leaf(&claim.info.concept_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let shape_names = shape_claims
        .iter()
        .map(|claim| claim.display.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let latest_role = latest_role_range(&role_claims);
    let latest_shape = shape_claims
        .iter()
        .max_by_key(|claim| claim.symbol_range.start_offset)
        .map(|claim| claim.symbol_range.clone())
        .unwrap();
    let range = if latest_role.start_offset >= latest_shape.start_offset {
        latest_role
    } else {
        latest_shape
    };
    let mut all_evidence = role_claims
        .iter()
        .map(|claim| &claim.info.evidence)
        .chain(shape_claims.iter().map(|claim| &claim.evidence))
        .cloned()
        .collect::<Vec<_>>();
    all_evidence.sort_by_key(first_source_offset);
    all_evidence.dedup();
    Some(SemanticDiagnostic {
        code: "notation-role-type-conflict".into(),
        severity: "warning".into(),
        message: format!(
            "Notation `{symbol}` is explicitly declared as both {role_names} and {shape_names}."
        ),
        explanation: "These explicit semantic role and mathematical shape declarations are incompatible in the same document scope.".into(),
        range,
        evidence: all_evidence,
    })
}

pub(crate) fn roles_conflict(left: &str, right: &str) -> bool {
    if concepts_share_lineage(left, right) {
        return false;
    }
    let left = concept_leaf(left);
    let right = concept_leaf(right);
    if left == right {
        return false;
    }
    !matches!(
        (left, right),
        ("function", "operator")
            | ("operator", "function")
            | ("function", "linear-operator")
            | ("linear-operator", "function")
            | ("function", "random-variable")
            | ("random-variable", "function")
            | ("function", "distribution")
            | ("distribution", "function")
            | ("event", "set")
            | ("set", "event")
    )
}

pub(crate) fn role_shape_conflict(role: &str, shape: &str) -> bool {
    let role = concept_leaf(role);
    match role {
        "event" | "set" => true,
        "distribution" => false,
        "function" | "operator" => shape != "matrix",
        "index" => shape != "scalar",
        "random-variable" => false,
        _ => false,
    }
}

fn concept_leaf(concept_id: &str) -> &str {
    concept_id.split(':').next_back().unwrap_or(concept_id)
}

fn descriptions<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(|description| format!("“{description}”"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn latest_role_range(claims: &[&ScopedRoleClaim]) -> SourceRange {
    claims
        .iter()
        .max_by_key(|claim| claim.symbol_range.start_offset)
        .unwrap()
        .symbol_range
        .clone()
}

fn evidence<'a>(values: impl Iterator<Item = &'a Evidence>) -> Vec<Evidence> {
    let mut evidence = values.cloned().collect::<Vec<_>>();
    evidence.sort_by_key(first_source_offset);
    evidence.dedup();
    evidence
}

fn first_source_offset(evidence: &Evidence) -> u32 {
    evidence
        .source_ranges
        .first()
        .map_or(0, |range| range.start_offset)
}

#[cfg(test)]
mod tests {
    use super::observe_roles;
    use crate::canonical::lower_document_region;
    use crate::concept::classify_role;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::shape::observe_shapes;
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::RoleObservations {
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
        let parsed = parse_regions(source, &regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &canonical, &prose.shapes);
        observe_roles(&document, &prose.definitions, &shapes)
    }

    #[test]
    fn reports_incompatible_explicit_roles_with_every_source() {
        let source = "Let $p$ denote a probability distribution.\n$p$ is a random variable.";
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, "notation-role-conflict");
        assert_eq!(analysis.diagnostics[0].evidence.len(), 2);
    }

    #[test]
    fn reports_only_incompatible_role_shape_pairs() {
        let source = "Let $S$ denote a set.\n$S \\in \\mathbb{R}^{n}$\nLet $A$ denote a linear operator.\n$A \\in \\mathbb{R}^{m \\times n}$\nLet $X$ denote a random variable.\n$X \\in \\mathbb{R}^{n}$";
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, "notation-role-type-conflict");
        assert!(analysis.diagnostics[0].message.contains("`S`"));
        assert_eq!(analysis.diagnostics[0].evidence.len(), 2);
    }

    #[test]
    fn keeps_shadowed_roles_in_sibling_sections_separate() {
        let source = "\\section{One}\nLet $q$ denote a probability distribution.\n\\section{Two}\n$q$ is a random variable.";
        assert!(analyze(source).diagnostics.is_empty());
    }

    #[test]
    fn does_not_attach_a_sibling_sections_diagnostic_to_symbol_info() {
        let source = "\\section{One}\nLet $q$ denote a set.\n$q \\in \\mathbb{R}^{n}$\n\\section{Two}\nLet $q$ denote a set.\n$q$";
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 1);
        let final_q = source.rfind("$q$").unwrap() as u32 + 1;
        assert!(analysis.diagnostics_for("q", final_q).0.is_empty());
    }

    #[test]
    fn ignores_unclassified_and_compatible_definitions() {
        let source = "Let $A$ denote a linear operator.\n$A$ is a function.\nLet $p$ denote a probability distribution.\n$p \\in \\mathbb{R}^{n}$\nLet $x$ denote an input.\n$x$ is an output.";
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty());
        assert_eq!(analysis.roles.len(), 4);
    }

    #[test]
    fn classifies_law_roles_from_pack_concepts_in_singular_and_plural_prose() {
        let source = "Let $x$ and $y$ be iterates. Let $g$ be a gradient. Let $a$ be a step size.";
        let roles = analyze(source).exported();
        let classified = roles
            .iter()
            .map(|role| (role.symbol.as_str(), role.concept_id.as_str()))
            .collect::<Vec<_>>();
        assert!(classified.contains(&("x", "optimization-ml:iterate")));
        assert!(classified.contains(&("y", "optimization-ml:iterate")));
        assert!(classified.contains(&("g", "optimization-ml:gradient")));
        assert!(classified.contains(&("a", "optimization-ml:step-size")));
    }

    #[test]
    fn explicit_pack_concepts_outrank_generated_descriptive_phrases() {
        for (description, expected) in [
            ("control input vector", "control-systems:control-input"),
            ("momentum vector", "quantities-units:momentum"),
            ("iterate vector", "optimization-ml:iterate"),
            ("force vector", "quantities-units:force"),
            ("output vector", "control-systems:output"),
            ("output matrix", "control-systems:output-matrix"),
            ("state vector", "control-systems:state"),
            ("feedthrough matrix", "control-systems:feedthrough-matrix"),
        ] {
            assert_eq!(
                classify_role(description).as_deref(),
                Some(expected),
                "{description}"
            );
        }
    }

    #[test]
    fn refuses_to_collapse_coordinated_role_descriptions() {
        assert_eq!(classify_role("state and input"), None);
        assert_eq!(classify_role("state or input"), None);
    }
}
