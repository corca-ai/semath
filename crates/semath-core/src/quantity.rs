use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use regex::Regex;

use crate::pack::{PackDimensionExponent, built_in_packs};
use crate::parser::ParsedMath;
use crate::scope::ScopeGraph;
use crate::{
    DefinitionInfo, DimensionExponentInfo, Evidence, PhysicalDimensionInfo, ProjectDocument,
    ProjectMacroExpansionStatus, ProjectMacroKind, QuantityInfo, SemanticDiagnostic, SourceIndex,
    SourceRange,
};

const MAX_QUANTITY_CLAIMS: usize = 8;

static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*=\s*(.+?)\s*$").expect("quantity assignment regex")
});
static ALIAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*$").expect("quantity alias regex"));
static BINARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*(\+|/|\*|\\cdot)?\s*([A-Za-z])\s*$")
        .expect("quantity binary regex")
});

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

    fn add(self, other: Self) -> Self {
        let numerator =
            self.numerator * other.denominator as i32 + other.numerator * self.denominator as i32;
        Self::new(numerator, self.denominator * other.denominator)
    }

    fn negate(self) -> Self {
        Self::new(-self.numerator, self.denominator)
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

    fn multiply(&self, other: &Self) -> Self {
        self.combine(other, false)
    }

    fn divide(&self, other: &Self) -> Self {
        self.combine(other, true)
    }

    fn combine(&self, other: &Self, subtract: bool) -> Self {
        let mut values = self.0.clone();
        for (base, exponent) in &other.0 {
            let right = if subtract {
                exponent.negate()
            } else {
                *exponent
            };
            let combined = values
                .get(base)
                .copied()
                .unwrap_or_else(|| Exponent::new(0, 1))
                .add(right);
            if combined.numerator == 0 {
                values.remove(base);
            } else {
                values.insert(base.clone(), combined);
            }
        }
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
    parsed: &[ParsedMath],
    definitions: &[DefinitionInfo],
) -> QuantityObservations {
    let scopes = ScopeGraph::new(document);
    let mut facts = explicit_facts(document, definitions, &scopes);
    let mut diagnostics = explicit_diagnostics(&facts);
    propagate_formula_dimensions(document, parsed, &scopes, &mut facts, &mut diagnostics);
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
            let available_from = definition
                .evidence
                .source_ranges
                .iter()
                .map(|range| range.end_offset)
                .max()
                .unwrap_or(definition.location.range.end_offset);
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
    diagnostics
}

fn propagate_formula_dimensions(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    scopes: &ScopeGraph,
    facts: &mut Vec<QuantityFact>,
    diagnostics: &mut Vec<SemanticDiagnostic>,
) {
    let index = SourceIndex::new(&document.content);
    let mut formulas = parsed
        .iter()
        .filter(|math| math.region.closed)
        .collect::<Vec<_>>();
    formulas.sort_by_key(|math| math.region.content_range.start_offset);
    for math in formulas {
        let start_byte = index.byte_for_utf16(math.region.content_range.start_offset);
        let end_byte = index.byte_for_utf16(math.region.content_range.end_offset);
        let Some(content) = document.content.get(start_byte..end_byte) else {
            continue;
        };
        let Some(captures) = ASSIGNMENT.captures(content) else {
            continue;
        };
        let left = captures.get(1).expect("assignment lhs").as_str();
        let right = captures.get(2).expect("assignment rhs").as_str();
        let scope_id = scopes.id_at(math.region.content_range.start_offset);
        let known = known_facts(facts, scopes, math.region.content_range.start_offset);
        let derived = if let Some(alias) = ALIAS.captures(right) {
            let source = alias.get(1).expect("alias source").as_str();
            known
                .get(source)
                .map(|fact| (fact.dimension.clone(), vec![source.into()]))
        } else if let Some(binary) = BINARY.captures(right) {
            let first = binary.get(1).expect("binary lhs").as_str();
            let second = binary.get(3).expect("binary rhs").as_str();
            let operator = binary.get(2).map_or("", |found| found.as_str());
            match (known.get(first), known.get(second), operator) {
                (Some(left_fact), Some(right_fact), "+")
                    if left_fact.dimension == right_fact.dimension =>
                {
                    Some((
                        left_fact.dimension.clone(),
                        vec![first.into(), second.into()],
                    ))
                }
                (Some(left_fact), Some(right_fact), "+") => {
                    diagnostics.push(dimension_diagnostic(
                        "quantity-addition-dimension-mismatch",
                        format!("Cannot add {first} and {second} with different dimensions."),
                        format!(
                            "Addition requires equal dimensions; {} has {}, while {} has {}.",
                            first,
                            left_fact.dimension.info().display,
                            second,
                            right_fact.dimension.info().display
                        ),
                        math.region.content_range.clone(),
                        vec![left_fact.evidence.clone(), right_fact.evidence.clone()],
                    ));
                    None
                }
                (Some(left_fact), Some(right_fact), "/") => Some((
                    left_fact.dimension.divide(&right_fact.dimension),
                    vec![first.into(), second.into()],
                )),
                (Some(left_fact), Some(right_fact), "" | "*" | "\\cdot") => Some((
                    left_fact.dimension.multiply(&right_fact.dimension),
                    vec![first.into(), second.into()],
                )),
                _ => None,
            }
        } else {
            None
        };
        let Some((dimension, derived_from)) = derived else {
            continue;
        };
        if let Some(existing) = known.get(left)
            && existing.dimension != dimension
        {
            diagnostics.push(dimension_diagnostic(
                "quantity-assignment-dimension-mismatch",
                format!("The expression assigned to {left} has an incompatible dimension."),
                format!(
                    "{left} is declared as {}, but the right-hand side derives {}.",
                    existing.dimension.info().display,
                    dimension.info().display
                ),
                math.region.content_range.clone(),
                vec![
                    existing.evidence.clone(),
                    derived_evidence(math.region.content_range.clone()),
                ],
            ));
            continue;
        }
        let symbol_range = math
            .symbols
            .iter()
            .find(|(symbol, _)| symbol == left)
            .map(|(_, range)| range.clone())
            .unwrap_or_else(|| math.region.content_range.clone());
        facts.push(QuantityFact {
            symbol: left.into(),
            symbol_range,
            available_from: math.region.content_range.end_offset,
            scope_id,
            quantity_kind_id: None,
            quantity_kind: None,
            unit_id: None,
            unit: None,
            dimension,
            evidence: derived_evidence(math.region.content_range.clone()),
            derived_from,
        });
    }
}

fn known_facts<'a>(
    facts: &'a [QuantityFact],
    scopes: &ScopeGraph,
    offset: u32,
) -> HashMap<&'a str, &'a QuantityFact> {
    let mut known = HashMap::new();
    for fact in facts {
        if fact.available_from <= offset
            && scopes.visible(fact.scope_id, offset)
            && known
                .get(fact.symbol.as_str())
                .is_none_or(|current: &&QuantityFact| {
                    (scopes.depth(current.scope_id), current.available_from)
                        < (scopes.depth(fact.scope_id), fact.available_from)
                })
        {
            known.insert(fact.symbol.as_str(), fact);
        }
    }
    known
}

fn find_quantity_kind(description: &str) -> Option<&'static QuantityKindSpec> {
    let mut semantic_description = description;
    for separator in [" along ", " through ", " across "] {
        if let Some((quantity, _)) = semantic_description.split_once(separator) {
            semantic_description = quantity;
        }
    }
    let description = semantic_description.to_lowercase();
    QUANTITY_CATALOG.kinds.iter().find(|kind| {
        kind.aliases
            .iter()
            .any(|alias| contains_term(&description, alias))
    })
}

fn find_unit(description: &str) -> Option<&'static UnitSpec> {
    let lower = description.to_lowercase();
    QUANTITY_CATALOG.units.iter().find(|unit| {
        unit.aliases
            .iter()
            .any(|alias| contains_term(&lower, &alias.to_lowercase()))
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

fn derived_evidence(range: SourceRange) -> Evidence {
    Evidence {
        rule_id: "semath/dimensional-propagation".into(),
        kind: "derived-constraint".into(),
        strength: "strong".into(),
        source_ranges: vec![range],
    }
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

    use super::{Dimension, Exponent, observe_quantities};
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{
        DocumentLanguage, ProjectDocument, ProjectMacro, ProjectMacroExpansion,
        ProjectMacroExpansionStatus, ProjectMacroKind, ProjectSourceRef, SourceRange,
    };

    #[test]
    fn dimension_algebra_is_exact_and_canonical() {
        let velocity = Dimension(BTreeMap::from([
            ("length".into(), Exponent::new(1, 1)),
            ("time".into(), Exponent::new(-1, 1)),
        ]));
        let duration = Dimension(BTreeMap::from([("time".into(), Exponent::new(1, 1))]));

        assert_eq!(velocity.multiply(&duration).info().display, "length");
        assert_eq!(Exponent::new(2, 4), Exponent::new(1, 2));
    }

    #[test]
    fn explicit_prose_drives_product_propagation_and_mismatch_diagnostics() {
        let content = "Let $m$ be mass in kilograms. Let $a$ be acceleration in metres per second squared. Let $F$ be force in newtons.\n$F = m a$\n$F = m + a$";
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: content.into(),
            document_version: 1,
            math_regions: test_math_regions(content, DocumentLanguage::Latex),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(&document.content, &document.math_regions);
        let prose = crate::prose::observe_prose(&document, &parsed);
        let analysis = observe_quantities(&document, &parsed, &prose.definitions);
        let force_offset = content.find("$F = m a$").unwrap() as u32 + 1;
        let claims = analysis.at("F", force_offset).0;

        assert!(
            claims.iter().any(|claim| {
                claim.quantity_kind_id.as_deref() == Some("quantities-units:force")
            })
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "quantity-addition-dimension-mismatch")
        );
    }

    #[test]
    fn transparent_prose_macros_contribute_meaning_but_opaque_macros_do_not() {
        let content = "Let $v$ be \\velocity.";
        let call_start = content.find("\\velocity").unwrap() as u32;
        let call_end = call_start + "\\velocity".len() as u32;
        let mut document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: content.into(),
            document_version: 1,
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
                },
            }],
            includes: Vec::new(),
        };
        let parsed = parse_regions(&document.content, &document.math_regions);
        let prose = crate::prose::observe_prose(&document, &parsed);
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
