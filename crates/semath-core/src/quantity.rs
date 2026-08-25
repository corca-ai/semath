use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::concept::{classify_role, classify_role_candidates, is_pack_quantity_concept};
use crate::pack::{PackDimensionExponent, built_in_packs};
use crate::parser::ParsedMath;
use crate::prose::definition_available_from;
use crate::scope::ScopeGraph;
use crate::{
    DefinitionInfo, DimensionExponentInfo, Evidence, PhysicalDimensionInfo, ProjectDocument,
    ProjectMacroExpansionStatus, ProjectMacroKind, QuantityInfo, SemanticDiagnostic, SourceRange,
};

const MAX_QUANTITY_CLAIMS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Exponent {
    numerator: i32,
    denominator: u32,
}

impl Exponent {
    fn new(numerator: i32, denominator: u32) -> Self {
        debug_assert!(denominator > 0);
        if numerator == 0 {
            return Self {
                numerator: 0,
                denominator: 1,
            };
        }
        let divisor = gcd(numerator.unsigned_abs(), denominator);
        Self {
            numerator: numerator / divisor as i32,
            denominator: denominator / divisor,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Dimension(BTreeMap<String, Exponent>);

impl Dimension {
    fn from_pack(exponents: &[PackDimensionExponent]) -> Self {
        let values = exponents
            .iter()
            .map(|exponent| {
                (
                    exponent.base.clone(),
                    Exponent::new(exponent.numerator, exponent.denominator),
                )
            })
            .collect();
        Self(values)
    }

    fn info(&self) -> PhysicalDimensionInfo {
        let exponents = self
            .0
            .iter()
            .map(|(base, exponent)| DimensionExponentInfo {
                base: base.clone(),
                numerator: exponent.numerator,
                denominator: exponent.denominator,
            })
            .collect::<Vec<_>>();
        let display = if exponents.is_empty() {
            "dimensionless".into()
        } else {
            exponents
                .iter()
                .map(
                    |exponent| match (exponent.numerator, exponent.denominator) {
                        (1, 1) => exponent.base.clone(),
                        (numerator, 1) => format!("{}^{numerator}", exponent.base),
                        (numerator, denominator) => {
                            format!("{}^({numerator}/{denominator})", exponent.base)
                        }
                    },
                )
                .collect::<Vec<_>>()
                .join(" · ")
        };
        PhysicalDimensionInfo { exponents, display }
    }
}

#[derive(Clone, Debug)]
struct QuantityKindSpec {
    id: String,
    title: String,
    aliases: Vec<String>,
    dimension: Dimension,
    default_unit: Option<String>,
}

#[derive(Clone, Debug)]
struct UnitSpec {
    id: String,
    symbol: String,
    aliases: Vec<String>,
    dimension: Dimension,
    affine: bool,
}

#[derive(Clone, Debug, Default)]
struct QuantityCatalog {
    kinds: Vec<QuantityKindSpec>,
    units: Vec<UnitSpec>,
}

static QUANTITY_CATALOG: LazyLock<QuantityCatalog> = LazyLock::new(|| {
    let mut catalog = QuantityCatalog::default();
    for pack in built_in_packs() {
        for kind in &pack.quantity_kinds {
            let qualified_id = format!("{}:{}", pack.namespace, kind.id);
            let mut aliases = vec![kind.id.replace('-', " "), kind.title.to_lowercase()];
            aliases.extend(kind.aliases.iter().map(|alias| alias.to_ascii_lowercase()));
            aliases.sort_by_key(|alias| std::cmp::Reverse(alias.len()));
            aliases.dedup();
            catalog.kinds.push(QuantityKindSpec {
                id: qualified_id,
                title: kind.title.clone(),
                aliases,
                dimension: Dimension::from_pack(&kind.dimension),
                default_unit: kind.default_unit.clone(),
            });
        }
        for unit in &pack.units {
            let mut aliases = unit.aliases.clone();
            aliases.sort_by_key(|alias| std::cmp::Reverse(alias.len()));
            aliases.dedup();
            catalog.units.push(UnitSpec {
                id: format!("{}:{}", pack.namespace, unit.id),
                symbol: unit.symbol.clone(),
                aliases,
                dimension: Dimension::from_pack(&unit.dimension),
                affine: unit.offset.is_some(),
            });
        }
    }
    catalog
        .kinds
        .sort_by_key(|kind| std::cmp::Reverse(kind.aliases[0].len()));
    catalog.units.sort_by_key(|unit| {
        std::cmp::Reverse(unit.aliases.first().map_or(unit.symbol.len(), String::len))
    });
    catalog
});

static CONCEPT_QUANTITY_KINDS: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    let mut mappings = BTreeMap::<String, String>::new();
    let mut ambiguous = std::collections::BTreeSet::new();
    for pack in built_in_packs() {
        for role in pack.laws.iter().flat_map(|law| &law.roles) {
            let Some(quantity) = &role.quantity else {
                continue;
            };
            if mappings
                .get(&role.concept)
                .is_some_and(|existing| existing != quantity)
            {
                ambiguous.insert(role.concept.clone());
            } else {
                mappings.insert(role.concept.clone(), quantity.clone());
            }
        }
    }
    mappings.retain(|concept, _| !ambiguous.contains(concept));
    mappings
});

pub(crate) fn unit_symbol_supports_quantity(symbol: &str, quantity_kind_id: &str) -> bool {
    let symbol = symbol.trim_start_matches('\\');
    let Some(kind) = QUANTITY_CATALOG
        .kinds
        .iter()
        .find(|kind| kind.id == quantity_kind_id)
    else {
        return false;
    };
    let Some(default_unit) = &kind.default_unit else {
        return false;
    };
    QUANTITY_CATALOG.units.iter().any(|unit| {
        &unit.id == default_unit
            && (unit.symbol == symbol
                || unit
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(symbol)))
    })
}

#[derive(Clone, Debug)]
struct QuantityFact {
    symbol: String,
    symbol_range: SourceRange,
    available_from: u32,
    scope_id: usize,
    quantity_kind_id: Option<String>,
    quantity_kind: Option<String>,
    unit_id: Option<String>,
    unit: Option<String>,
    dimension: Dimension,
    evidence: Evidence,
    derived_from: Vec<String>,
}

impl QuantityFact {
    fn info(&self) -> QuantityInfo {
        let dimension = self.dimension.info();
        let mut parts = Vec::new();
        if let Some(kind) = &self.quantity_kind {
            parts.push(kind.clone());
        }
        if let Some(unit) = &self.unit {
            parts.push(unit.clone());
        }
        parts.push(dimension.display.clone());
        QuantityInfo {
            symbol: self.symbol.clone(),
            quantity_kind_id: self.quantity_kind_id.clone(),
            quantity_kind: self.quantity_kind.clone(),
            unit_id: self.unit_id.clone(),
            unit: self.unit.clone(),
            dimension,
            display: parts.join(" · "),
            evidence: self.evidence.clone(),
            derived_from: self.derived_from.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QuantityObservations {
    facts: Vec<QuantityFact>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    scopes: ScopeGraph,
}

impl QuantityObservations {
    pub fn explicit(&self) -> Vec<QuantityInfo> {
        self.facts
            .iter()
            .filter(|fact| fact.derived_from.is_empty())
            .map(QuantityFact::info)
            .collect()
    }

    pub fn exported(&self) -> Vec<QuantityInfo> {
        self.facts
            .iter()
            .filter(|fact| self.scopes.depth(fact.scope_id) == 0)
            .map(QuantityFact::info)
            .collect()
    }

    pub fn at(&self, symbol: &str, offset: u32) -> (Vec<QuantityInfo>, bool) {
        let mut facts = self
            .facts
            .iter()
            .filter(|fact| {
                fact.symbol == symbol
                    && (self.scopes.depth(fact.scope_id) == 0
                        || fact.available_from <= offset
                        || fact.symbol_range.contains(offset))
                    && self.scopes.visible(fact.scope_id, offset)
            })
            .collect::<Vec<_>>();
        facts.sort_by_key(|fact| {
            (
                std::cmp::Reverse(self.scopes.depth(fact.scope_id)),
                std::cmp::Reverse(fact.derived_from.is_empty()),
                std::cmp::Reverse(fact.available_from),
            )
        });
        let truncated = facts.len() > MAX_QUANTITY_CLAIMS;
        (
            facts
                .into_iter()
                .take(MAX_QUANTITY_CLAIMS)
                .map(QuantityFact::info)
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
}

pub(crate) fn observe_quantities(
    document: &ProjectDocument,
    _parsed: &[ParsedMath],
    definitions: &[DefinitionInfo],
) -> QuantityObservations {
    let scopes = ScopeGraph::new(document);
    let mut facts = explicit_facts(document, definitions, &scopes);
    let mut diagnostics = explicit_diagnostics(&facts);
    facts.sort_by_key(|fact| fact.available_from);
    diagnostics.sort_by_key(|diagnostic| diagnostic.range.start_offset);
    QuantityObservations {
        facts,
        diagnostics,
        scopes,
    }
}

fn explicit_facts(
    document: &ProjectDocument,
    definitions: &[DefinitionInfo],
    scopes: &ScopeGraph,
) -> Vec<QuantityFact> {
    definitions
        .iter()
        .filter_map(|definition| {
            let description = semantic_description(document, definition);
            let declared_kind = find_quantity_kind(&description);
            let unit = find_unit(&description);
            let kind = declared_kind.or_else(|| {
                unit.and_then(|unit| {
                    QUANTITY_CATALOG
                        .kinds
                        .iter()
                        .find(|kind| kind.default_unit.as_deref() == Some(unit.id.as_str()))
                })
            });
            let dimension = unit
                .map(|unit| unit.dimension.clone())
                .or_else(|| kind.map(|kind| kind.dimension.clone()))?;
            let available_from = definition_available_from(definition);
            Some(QuantityFact {
                symbol: definition.symbol.clone(),
                symbol_range: definition.location.range.clone(),
                available_from,
                scope_id: scopes.id_at(definition.location.range.start_offset),
                quantity_kind_id: kind.map(|kind| kind.id.clone()),
                quantity_kind: kind.map(|kind| kind.title.clone()),
                unit_id: unit.map(|unit| unit.id.clone()),
                unit: unit.map(|unit| unit.symbol.clone()),
                dimension,
                evidence: Evidence {
                    rule_id: format!("{}/quantity-declaration", definition.evidence.rule_id),
                    kind: "explicit-prose".into(),
                    strength: "strong".into(),
                    source_ranges: definition.evidence.source_ranges.clone(),
                    source_anchors: definition.evidence.source_anchors.clone(),
                },
                derived_from: Vec::new(),
            })
        })
        .collect()
}

fn semantic_description(document: &ProjectDocument, definition: &DefinitionInfo) -> String {
    let mut description = definition.description.clone();
    let mut expanded_surfaces = Vec::new();
    for project_macro in &document.macros {
        if project_macro.kind != ProjectMacroKind::Call
            || project_macro.source.file_id != document.file_id
        {
            continue;
        }
        let input = project_macro
            .expansion
            .input_range
            .as_ref()
            .unwrap_or(&project_macro.source.range);
        let belongs_to_definition = definition.evidence.source_ranges.iter().any(|range| {
            range.start_offset <= input.start_offset && input.end_offset <= range.end_offset
        });
        if !belongs_to_definition {
            continue;
        }
        description = description.replace(&format!("\\{}", project_macro.name), "");
        if project_macro.expansion.status == ProjectMacroExpansionStatus::Expanded
            && let Some(surface) = &project_macro.expansion.surface
        {
            expanded_surfaces.push(surface.as_str());
        }
    }
    std::iter::once(description.as_str())
        .chain(expanded_surfaces)
        .collect::<Vec<_>>()
        .join(" ")
}

fn explicit_diagnostics(facts: &[QuantityFact]) -> Vec<SemanticDiagnostic> {
    let mut diagnostics = Vec::new();
    for fact in facts {
        let (Some(kind_id), Some(unit_id)) = (&fact.quantity_kind_id, &fact.unit_id) else {
            continue;
        };
        let Some(kind) = QUANTITY_CATALOG
            .kinds
            .iter()
            .find(|kind| &kind.id == kind_id)
        else {
            continue;
        };
        let Some(unit) = QUANTITY_CATALOG
            .units
            .iter()
            .find(|unit| &unit.id == unit_id)
        else {
            continue;
        };
        if kind.dimension != unit.dimension {
            diagnostics.push(dimension_diagnostic(
                "quantity-unit-dimension-mismatch",
                format!("{} cannot be declared in {}.", kind.title, unit.symbol),
                format!(
                    "The explicit quantity kind has dimension {}, but the declared unit has dimension {}.",
                    kind.dimension.info().display,
                    unit.dimension.info().display
                ),
                fact.symbol_range.clone(),
                vec![fact.evidence.clone()],
            ));
        }
        if unit.affine {
            continue;
        }
    }
    for (index, left) in facts.iter().enumerate() {
        for right in &facts[index + 1..] {
            if left.symbol != right.symbol
                || left.scope_id != right.scope_id
                || left.dimension == right.dimension
            {
                continue;
            }
            diagnostics.push(dimension_diagnostic(
                "quantity-declaration-dimension-conflict",
                format!("`{}` has incompatible quantity declarations.", right.symbol),
                format!(
                    "The same semantic lifetime declares dimensions {} and {}.",
                    left.dimension.info().display,
                    right.dimension.info().display
                ),
                right.symbol_range.clone(),
                vec![left.evidence.clone(), right.evidence.clone()],
            ));
        }
    }
    diagnostics
}

fn find_quantity_kind(description: &str) -> Option<&'static QuantityKindSpec> {
    if classify_role_candidates(description).len() > 1 {
        return None;
    }
    if let Some(concept_id) = classify_role(description) {
        // A complete semantic role phrase outranks incidental quantity words
        // inside it. For example, pressure head and velocity head are both
        // length-valued head components; treating the adjectives as standalone
        // pressure and velocity declarations invents a dimensional conflict.
        if let Some(kind) = QUANTITY_CATALOG
            .kinds
            .iter()
            .find(|kind| kind.id == concept_id)
        {
            return Some(kind);
        }
        if let Some(quantity_id) = CONCEPT_QUANTITY_KINDS.get(&concept_id)
            && let Some(kind) = QUANTITY_CATALOG
                .kinds
                .iter()
                .find(|kind| kind.id == *quantity_id)
        {
            return Some(kind);
        }
        if is_pack_quantity_concept(&concept_id) {
            return None;
        }
    }
    let mut semantic_description = description;
    for separator in [" along ", " through ", " across ", " normal to "] {
        if let Some((quantity, _)) = semantic_description.split_once(separator) {
            semantic_description = quantity;
        }
    }
    let description = semantic_description.to_lowercase().replace('-', " ");
    let mut best = None;
    let mut best_length = 0;
    let mut ambiguous = false;
    for kind in &QUANTITY_CATALOG.kinds {
        let matched_length = kind
            .aliases
            .iter()
            .filter(|alias| contains_term(&description, alias))
            .map(String::len)
            .max()
            .unwrap_or_default();
        if matched_length > best_length {
            best = Some(kind);
            best_length = matched_length;
            ambiguous = false;
        } else if matched_length != 0
            && matched_length == best_length
            && best.is_some_and(|selected: &QuantityKindSpec| selected.id != kind.id)
        {
            ambiguous = true;
        }
    }
    (!ambiguous).then_some(best).flatten()
}

fn find_unit(description: &str) -> Option<&'static UnitSpec> {
    let lower = description.to_lowercase();
    QUANTITY_CATALOG.units.iter().find(|unit| {
        unit.aliases
            .iter()
            .any(|alias| explicit_symbol(&lower, &alias.to_lowercase()))
            || explicit_symbol(description, &unit.symbol)
    })
}

fn contains_term(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, matched)| {
        let end = start + matched.len();
        let before_ok = start == 0
            || text[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric());
        let after_ok = end == text.len()
            || text[end..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric());
        before_ok && after_ok
    })
}

fn explicit_symbol(description: &str, symbol: &str) -> bool {
    [" in ", " measured in ", " unit ", " units of "]
        .iter()
        .any(|prefix| description.contains(&format!("{prefix}{symbol}")))
}

fn dimension_diagnostic(
    code: &str,
    message: String,
    explanation: String,
    range: SourceRange,
    evidence: Vec<Evidence>,
) -> SemanticDiagnostic {
    SemanticDiagnostic {
        code: code.into(),
        severity: "warning".into(),
        message,
        explanation,
        range,
        evidence,
    }
}

const fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        Dimension, Exponent, find_quantity_kind, find_unit, observe_quantities,
        unit_symbol_supports_quantity,
    };
    use crate::canonical::lower_document_region;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{
        DocumentLanguage, ProjectDocument, ProjectMacro, ProjectMacroExpansion,
        ProjectMacroExpansionStatus, ProjectMacroKind, ProjectSourceRef, SourceRange,
    };

    #[test]
    fn dimension_values_are_reduced_and_displayed_canonically() {
        let velocity = Dimension(BTreeMap::from([
            ("length".into(), Exponent::new(1, 1)),
            ("time".into(), Exponent::new(-1, 1)),
        ]));

        assert_eq!(velocity.info().display, "length · time^-1");
        assert_eq!(Exponent::new(2, 4), Exponent::new(1, 2));
    }

    #[test]
    fn selects_the_longest_matching_quantity_term_across_packs() {
        assert_eq!(
            find_quantity_kind("specific heat scalar").map(|kind| kind.id.as_str()),
            Some("quantities-units:specific-heat-capacity")
        );
        assert_eq!(
            find_quantity_kind("heat transfer scalar").map(|kind| kind.id.as_str()),
            Some("quantities-units:energy")
        );
        assert_eq!(
            find_quantity_kind("wave propagation speed").map(|kind| kind.id.as_str()),
            Some("quantities-units:velocity")
        );
        assert_eq!(
            find_quantity_kind("cyclic frequency scalar").map(|kind| kind.id.as_str()),
            Some("quantities-units:frequency")
        );
        assert_eq!(
            find_quantity_kind("wavelength scalar").map(|kind| kind.id.as_str()),
            Some("quantities-units:length")
        );
        assert_eq!(
            find_quantity_kind("time variable").map(|kind| kind.id.as_str()),
            Some("quantities-units:duration")
        );
        assert_eq!(
            find_quantity_kind("electric force on the charge").map(|kind| kind.id.as_str()),
            Some("quantities-units:force")
        );
        assert!(find_quantity_kind("pressure head scalar").is_none());
        assert!(find_quantity_kind("velocity head scalar").is_none());
    }

    #[test]
    fn default_unit_symbols_support_only_their_declared_quantity_kind() {
        assert!(unit_symbol_supports_quantity(
            "Hz",
            "quantities-units:frequency"
        ));
        assert!(unit_symbol_supports_quantity(
            "hertz",
            "quantities-units:frequency"
        ));
        assert!(!unit_symbol_supports_quantity(
            "Hz",
            "quantities-units:velocity"
        ));
        assert!(!unit_symbol_supports_quantity(
            "m",
            "quantities-units:frequency"
        ));
        assert!(find_unit("second section area scalar").is_none());
        assert!(find_unit("duration measured in seconds").is_some());
    }

    #[test]
    fn explicit_quantity_extraction_does_not_reparse_formula_text() {
        let content = "Let $m$ be mass in kilograms. Let $a$ be acceleration in metres per second squared. Let $F$ be force in newtons.\n$F = m a$\n$F = m + a$";
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: content.into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: test_math_regions(content, DocumentLanguage::Latex),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(&document.content, &document.math_regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let prose = crate::prose::observe_prose(&document, &parsed, &canonical);
        let analysis = observe_quantities(&document, &parsed, &prose.definitions);
        let force_offset = content.find("$F = m a$").unwrap() as u32 + 1;
        let claims = analysis.at("F", force_offset).0;

        assert!(claims.iter().any(|claim| {
            claim.quantity_kind_id.as_deref() == Some("quantities-units:force")
                && claim.derived_from.is_empty()
        }));
        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn transparent_prose_macros_contribute_meaning_but_opaque_macros_do_not() {
        let content = "Let $v$ be \\velocity.";
        let call_start = content.find("\\velocity").unwrap() as u32;
        let call_end = call_start + "\\velocity".len() as u32;
        let mut document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: content.into(),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: test_math_regions(content, DocumentLanguage::Latex),
            macros: vec![ProjectMacro {
                kind: ProjectMacroKind::Call,
                name: "velocity".into(),
                source: ProjectSourceRef {
                    file_id: "main".into(),
                    path: "main.tex".into(),
                    range: SourceRange {
                        start_offset: call_start,
                        end_offset: call_end,
                    },
                },
                definitions: Vec::new(),
                expansion: ProjectMacroExpansion {
                    status: ProjectMacroExpansionStatus::Expanded,
                    depth: 0,
                    editable: false,
                    surface: Some("velocity".into()),
                    input_range: Some(SourceRange {
                        start_offset: call_start,
                        end_offset: call_end,
                    }),
                    notation: None,
                },
            }],
            includes: Vec::new(),
        };
        let parsed = parse_regions(&document.content, &document.math_regions);
        let canonical = parsed
            .iter()
            .map(|math| lower_document_region(&document, &math.region.content_range))
            .collect::<Vec<_>>();
        let prose = crate::prose::observe_prose(&document, &parsed, &canonical);
        let transparent = observe_quantities(&document, &parsed, &prose.definitions);
        assert_eq!(
            transparent.exported()[0].quantity_kind_id.as_deref(),
            Some("quantities-units:velocity")
        );

        document.macros[0].expansion.status = ProjectMacroExpansionStatus::Unresolved;
        let opaque = observe_quantities(&document, &parsed, &prose.definitions);
        assert!(opaque.exported().is_empty());
    }
}
