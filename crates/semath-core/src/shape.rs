use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::parser::ParsedMath;
use crate::prose::{ProseShape, ProseShapeClaim};
use crate::scope::ScopeGraph;
use crate::{
    Evidence, FormulaConstraint, ProjectDocument, SemanticDiagnostic, ShapeInfo, SourceIndex,
    SourceRange,
};

static DECLARATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)([A-Za-z])\s*\\in\s*\\mathbb\s*\{\s*R\s*\}(?:\s*\^\s*(?:\{([^}]*)\}|([A-Za-z0-9]+)))?",
    )
    .unwrap()
});
static DIMENSION_PRODUCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:\\times|×)\s*").unwrap());
static INEQUALITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9]+)\s*(?:\\(?:ne|neq)|!=)\s*([A-Za-z0-9]+)").unwrap());
static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)([A-Za-z])\s*=\s*([^,\n]+)").unwrap());
static ALIAS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*$").unwrap());
static ADDITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*\+\s*([A-Za-z])\s*$").unwrap());
static PRODUCT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*(\^\s*(?:\{\s*\\top\s*\}|\\top))?\s*(?:\\cdot\s*)?([A-Za-z])\s*$")
        .unwrap()
});
static TRANSPOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*$").unwrap());
static QUADRATIC_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*([A-Za-z])\s*([A-Za-z])\s*$")
        .unwrap()
});
const MAX_SYMBOL_CLAIMS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Scalar,
    Vector(String),
    Matrix(String, String),
    Tensor(Vec<String>),
}

impl Shape {
    fn display(&self) -> String {
        match self {
            Self::Scalar => "Scalar".into(),
            Self::Vector(dimension) => format!("Vector[{dimension}]"),
            Self::Matrix(rows, columns) => format!("Matrix[{rows} × {columns}]"),
            Self::Tensor(dimensions) => format!("Tensor[{}]", dimensions.join(" × ")),
        }
    }

    fn info(&self, symbol: &str, evidence: Evidence, refinements: Vec<String>) -> ShapeInfo {
        let (kind, dimensions) = match self {
            Self::Scalar => ("scalar", Vec::new()),
            Self::Vector(dimension) => ("vector", vec![dimension.clone()]),
            Self::Matrix(rows, columns) => ("matrix", vec![rows.clone(), columns.clone()]),
            Self::Tensor(dimensions) => ("tensor", dimensions.clone()),
        };
        ShapeInfo {
            symbol: symbol.into(),
            kind: kind.into(),
            dimensions,
            refinements,
            display: self.display(),
            evidence,
        }
    }

    fn transpose(&self) -> Self {
        match self {
            Self::Matrix(rows, columns) => Self::Matrix(columns.clone(), rows.clone()),
            other => other.clone(),
        }
    }

    pub(crate) fn constraint(&self) -> FormulaConstraint {
        let (kind, dimensions) = match self {
            Self::Scalar => ("scalar", Vec::new()),
            Self::Vector(dimension) => ("vector", vec![dimension.clone()]),
            Self::Matrix(rows, columns) => ("matrix", vec![rows.clone(), columns.clone()]),
            Self::Tensor(dimensions) => ("tensor", dimensions.clone()),
        };
        FormulaConstraint {
            kind: kind.into(),
            dimensions,
            refinements: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct ShapeFact {
    symbol: String,
    shape: Shape,
    symbol_range: SourceRange,
    available_from: u32,
    evidence: Evidence,
    refinements: Vec<String>,
    explicit: bool,
    scope_id: usize,
}

#[derive(Clone, Debug)]
struct Inequality {
    left: String,
    right: String,
    range: SourceRange,
    scope_id: usize,
}

#[derive(Clone, Debug)]
struct DimensionMismatch {
    left: String,
    right: String,
    evidence_range: Option<SourceRange>,
}

#[derive(Clone, Debug)]
pub(crate) struct ShapeAnalysis {
    facts: Vec<ShapeFact>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    scopes: ScopeGraph,
}

#[derive(Clone, Debug)]
pub(crate) struct KnownShape {
    pub symbol: String,
    pub shape: Shape,
    pub evidence: Evidence,
    pub refinements: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct ExplicitShapeClaim {
    pub symbol: String,
    pub kind: String,
    pub display: String,
    pub symbol_range: SourceRange,
    pub evidence: Evidence,
}

impl KnownShape {
    pub fn constraint(&self) -> FormulaConstraint {
        let mut constraint = self.shape.constraint();
        constraint.refinements = self.refinements.clone();
        constraint
    }
}

impl ShapeAnalysis {
    pub fn explicit_claims(&self) -> Vec<ExplicitShapeClaim> {
        self.facts
            .iter()
            .filter(|fact| fact.explicit)
            .map(|fact| ExplicitShapeClaim {
                symbol: fact.symbol.clone(),
                kind: fact.shape.constraint().kind,
                display: fact.shape.display(),
                symbol_range: fact.symbol_range.clone(),
                evidence: fact.evidence.clone(),
            })
            .collect()
    }

    pub fn shape_at(&self, symbol: &str, offset: u32) -> Option<ShapeInfo> {
        self.facts
            .iter()
            .filter(|fact| {
                fact.symbol == symbol
                    && (fact.available_from <= offset || fact.symbol_range.contains(offset))
                    && self.scopes.visible(fact.scope_id, offset)
            })
            .max_by_key(|fact| (self.scopes.depth(fact.scope_id), fact.available_from))
            .map(|fact| {
                fact.shape
                    .info(symbol, fact.evidence.clone(), fact.refinements.clone())
            })
    }

    pub fn diagnostic(&self, code: &str, offset: u32) -> Option<SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code && diagnostic.range.contains(offset))
            .cloned()
    }

    pub fn known_shapes_at(&self, offset: u32) -> Vec<KnownShape> {
        let mut latest = BTreeMap::<&str, &ShapeFact>::new();
        for fact in &self.facts {
            if fact.available_from <= offset
                && self.scopes.visible(fact.scope_id, offset)
                && latest.get(fact.symbol.as_str()).is_none_or(|current| {
                    (self.scopes.depth(current.scope_id), current.available_from)
                        < (self.scopes.depth(fact.scope_id), fact.available_from)
                })
            {
                latest.insert(fact.symbol.as_str(), fact);
            }
        }
        latest
            .into_values()
            .map(|fact| KnownShape {
                symbol: fact.symbol.clone(),
                shape: fact.shape.clone(),
                evidence: fact.evidence.clone(),
                refinements: fact.refinements.clone(),
            })
            .collect()
    }

    pub fn claims_at(&self, symbol: &str, offset: u32) -> (Vec<ShapeInfo>, bool) {
        let mut facts = self
            .facts
            .iter()
            .filter(|fact| {
                fact.symbol == symbol
                    && (fact.available_from <= offset || fact.symbol_range.contains(offset))
                    && self.scopes.visible(fact.scope_id, offset)
            })
            .collect::<Vec<_>>();
        facts.sort_by_key(|fact| {
            (
                std::cmp::Reverse(self.scopes.depth(fact.scope_id)),
                std::cmp::Reverse(fact.available_from),
            )
        });
        let truncated = facts.len() > MAX_SYMBOL_CLAIMS;
        let claims = facts
            .into_iter()
            .take(MAX_SYMBOL_CLAIMS)
            .map(|fact| {
                fact.shape
                    .info(symbol, fact.evidence.clone(), fact.refinements.clone())
            })
            .collect();
        (claims, truncated)
    }

    pub fn diagnostics_for(
        &self,
        offset: u32,
        claims: &[ShapeInfo],
    ) -> (Vec<SemanticDiagnostic>, bool) {
        let claim_ranges = claims
            .iter()
            .flat_map(|claim| claim.evidence.source_ranges.iter())
            .collect::<Vec<_>>();
        let related = self
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.range.start_offset <= offset
                    && (diagnostic.range.contains(offset)
                        || diagnostic.evidence.iter().any(|evidence| {
                            evidence
                                .source_ranges
                                .iter()
                                .any(|source| claim_ranges.contains(&source))
                        }))
            })
            .collect::<Vec<_>>();
        let truncated = related.len() > MAX_SYMBOL_CLAIMS;
        (
            related
                .into_iter()
                .take(MAX_SYMBOL_CLAIMS)
                .cloned()
                .collect(),
            truncated,
        )
    }
}

pub(crate) fn analyze_shapes(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    prose_claims: &[ProseShapeClaim],
) -> ShapeAnalysis {
    let index = SourceIndex::new(&document.content);
    let scopes = ScopeGraph::new(document);
    let inequalities = collect_inequalities(document, parsed, &index, &scopes);
    let mut analysis = ShapeAnalysis {
        facts: Vec::new(),
        diagnostics: Vec::new(),
        scopes,
    };

    for claim in prose_claims {
        analysis.facts.push(ShapeFact {
            symbol: claim.symbol.clone(),
            shape: prose_shape(&claim.shape),
            symbol_range: claim.symbol_range.clone(),
            available_from: claim.available_from,
            evidence: claim.evidence.clone(),
            refinements: claim.refinements.clone(),
            explicit: true,
            scope_id: analysis.scopes.id_at(claim.symbol_range.start_offset),
        });
    }

    for math in parsed {
        let Some((content, content_start)) = math_content(document, math, &index) else {
            continue;
        };
        for captures in DECLARATION.captures_iter(content) {
            let whole = captures.get(0).unwrap();
            let symbol_match = captures.get(1).unwrap();
            let dimensions = captures
                .get(2)
                .or_else(|| captures.get(3))
                .map(|found| found.as_str());
            let Some(shape) = declared_shape(dimensions) else {
                continue;
            };
            let declaration_range = absolute_range(
                &index,
                content_start + whole.start(),
                content_start + whole.end(),
            );
            let symbol_range = absolute_range(
                &index,
                content_start + symbol_match.start(),
                content_start + symbol_match.end(),
            );
            let symbol = symbol_match.as_str();
            let scope_id = analysis.scopes.id_at(symbol_range.start_offset);
            analysis.facts.push(ShapeFact {
                symbol: symbol.into(),
                shape,
                symbol_range,
                available_from: declaration_range.end_offset,
                evidence: Evidence {
                    rule_id: "explicit-real-shape-declaration".into(),
                    kind: "explicit-math".into(),
                    strength: "hard".into(),
                    source_ranges: vec![declaration_range],
                },
                refinements: Vec::new(),
                explicit: true,
                scope_id,
            });
        }
    }

    add_explicit_conflict_diagnostics(&mut analysis, &inequalities);

    for math in parsed {
        let Some((content, content_start)) = math_content(document, math, &index) else {
            continue;
        };
        for captures in ASSIGNMENT.captures_iter(content) {
            let whole = captures.get(0).unwrap();
            let left = captures.get(1).unwrap();
            let right = captures.get(2).unwrap();
            let expression_range = absolute_range(
                &index,
                content_start + right.start(),
                content_start + right.end(),
            );
            let left_range = absolute_range(
                &index,
                content_start + left.start(),
                content_start + left.end(),
            );
            let available_from = absolute_range(
                &index,
                content_start + whole.start(),
                content_start + whole.end(),
            )
            .end_offset;
            analyze_assignment(
                &mut analysis,
                &inequalities,
                right.as_str(),
                expression_range,
                left.as_str(),
                left_range,
                available_from,
            );
        }
    }

    analysis
        .diagnostics
        .sort_by_key(|diagnostic| diagnostic.range.start_offset);
    analysis
}

fn prose_shape(shape: &ProseShape) -> Shape {
    match shape {
        ProseShape::Scalar => Shape::Scalar,
        ProseShape::Vector(dimension) => Shape::Vector(dimension.clone()),
        ProseShape::Matrix(rows, columns) => Shape::Matrix(rows.clone(), columns.clone()),
        ProseShape::Tensor(dimensions) => Shape::Tensor(dimensions.clone()),
    }
}

fn add_explicit_conflict_diagnostics(analysis: &mut ShapeAnalysis, inequalities: &[Inequality]) {
    let mut facts = analysis
        .facts
        .iter()
        .filter(|fact| fact.explicit)
        .cloned()
        .collect::<Vec<_>>();
    facts.sort_by_key(|fact| fact.symbol_range.start_offset);
    let mut earlier = Vec::<ShapeFact>::new();
    for fact in facts {
        if let Some(previous) = earlier
            .iter()
            .filter(|previous| previous.symbol == fact.symbol && previous.scope_id == fact.scope_id)
            .max_by_key(|previous| previous.available_from)
            && shapes_conflict(
                &previous.shape,
                &fact.shape,
                inequalities,
                &analysis.scopes,
                fact.symbol_range.start_offset,
            )
        {
            analysis.diagnostics.push(SemanticDiagnostic {
                code: "notation-shape-conflict".into(),
                severity: "warning".into(),
                message: format!(
                    "Notation `{}` is redeclared as {} after {}.",
                    fact.symbol,
                    fact.shape.display(),
                    previous.shape.display()
                ),
                explanation: format!(
                    "Both declarations are explicit and assign incompatible shapes to `{}` in the same document scope.",
                    fact.symbol
                ),
                range: fact.symbol_range.clone(),
                evidence: vec![previous.evidence.clone(), fact.evidence.clone()],
            });
        }
        earlier.push(fact);
    }
}

fn analyze_assignment(
    analysis: &mut ShapeAnalysis,
    inequalities: &[Inequality],
    expression: &str,
    expression_range: SourceRange,
    target: &str,
    target_range: SourceRange,
    available_from: u32,
) {
    let at = expression_range.start_offset;
    if let Some(captures) = QUADRATIC_FORM.captures(expression) {
        let vector_symbol = captures.get(1).unwrap().as_str();
        let matrix_symbol = captures.get(2).unwrap().as_str();
        let trailing_symbol = captures.get(3).unwrap().as_str();
        if vector_symbol != trailing_symbol {
            return;
        }
        let Some(vector) = latest_fact(analysis, vector_symbol, at).cloned() else {
            return;
        };
        let Some(matrix) = latest_fact(analysis, matrix_symbol, at).cloned() else {
            return;
        };
        let (Shape::Vector(length), Shape::Matrix(rows, columns)) = (&vector.shape, &matrix.shape)
        else {
            return;
        };
        if dimension_mismatch(length, rows, inequalities, &analysis.scopes, at).is_some()
            || dimension_mismatch(length, columns, inequalities, &analysis.scopes, at).is_some()
        {
            return;
        }
        push_derived_fact(
            analysis,
            target,
            target_range,
            available_from,
            Shape::Scalar,
            "linear-algebra/quadratic-form",
            evidence_ranges(&[&vector, &matrix]),
        );
        return;
    }

    if let Some(captures) = TRANSPOSE.captures(expression) {
        let source_symbol = captures.get(1).unwrap().as_str();
        if let Some(source) = latest_fact(analysis, source_symbol, at).cloned() {
            push_derived_fact(
                analysis,
                target,
                target_range,
                available_from,
                source.shape.transpose(),
                "linear-algebra/matrix-transpose",
                evidence_ranges(&[&source]),
            );
        }
        return;
    }
    if let Some(captures) = ADDITION.captures(expression) {
        let left_symbol = captures.get(1).unwrap().as_str();
        let right_symbol = captures.get(2).unwrap().as_str();
        let Some(left) = latest_fact(analysis, left_symbol, at).cloned() else {
            return;
        };
        let Some(right) = latest_fact(analysis, right_symbol, at).cloned() else {
            return;
        };
        if shapes_conflict(
            &left.shape,
            &right.shape,
            inequalities,
            &analysis.scopes,
            at,
        ) {
            analysis.diagnostics.push(SemanticDiagnostic {
                code: "shape-incompatible-addition".into(),
                severity: "warning".into(),
                message: format!(
                    "Cannot add {} and {} with the declared dimensions.",
                    left.shape.display(),
                    right.shape.display()
                ),
                explanation: "Addition requires both operands to have the same shape, but their explicit declarations prove that they differ.".into(),
                range: expression_range,
                evidence: vec![left.evidence, right.evidence],
            });
            return;
        }
        push_derived_fact(
            analysis,
            target,
            target_range,
            available_from,
            left.shape.clone(),
            "derived-shape-addition",
            evidence_ranges(&[&left, &right]),
        );
        return;
    }

    if let Some(captures) = PRODUCT.captures(expression) {
        let left_symbol = captures.get(1).unwrap().as_str();
        let right_symbol = captures.get(3).unwrap().as_str();
        let Some(mut left) = latest_fact(analysis, left_symbol, at).cloned() else {
            return;
        };
        let Some(right) = latest_fact(analysis, right_symbol, at).cloned() else {
            return;
        };
        if captures.get(2).is_some() {
            left.shape = left.shape.transpose();
        }
        let (result, mismatch) = match (&left.shape, &right.shape) {
            (Shape::Matrix(rows, inner), Shape::Vector(dimension)) => (
                Some(Shape::Vector(rows.clone())),
                dimension_mismatch(inner, dimension, inequalities, &analysis.scopes, at),
            ),
            (Shape::Matrix(rows, inner), Shape::Matrix(right_rows, columns)) => (
                Some(Shape::Matrix(rows.clone(), columns.clone())),
                dimension_mismatch(inner, right_rows, inequalities, &analysis.scopes, at),
            ),
            (Shape::Vector(left_dimension), Shape::Vector(right_dimension))
                if captures.get(2).is_some() =>
            {
                (
                    Some(Shape::Scalar),
                    dimension_mismatch(
                        left_dimension,
                        right_dimension,
                        inequalities,
                        &analysis.scopes,
                        at,
                    ),
                )
            }
            _ => (None, None),
        };
        let Some(result) = result else {
            return;
        };
        if let Some(mismatch) = mismatch {
            let mut evidence = vec![left.evidence, right.evidence];
            if let Some(range) = mismatch.evidence_range {
                evidence.push(Evidence {
                    rule_id: "explicit-dimension-inequality".into(),
                    kind: "explicit-math".into(),
                    strength: "hard".into(),
                    source_ranges: vec![range],
                });
            }
            analysis.diagnostics.push(SemanticDiagnostic {
                code: "shape-incompatible-product".into(),
                severity: "warning".into(),
                message: format!(
                    "Cannot multiply {} by {}: inner dimensions {} and {} are explicitly unequal.",
                    left.shape.display(),
                    right.shape.display(),
                    mismatch.left,
                    mismatch.right
                ),
                explanation: "Matrix multiplication requires the left inner dimension to equal the right leading dimension; the declarations and dimension constraints prove that this requirement is false.".into(),
                range: expression_range,
                evidence,
            });
            return;
        }
        push_derived_fact(
            analysis,
            target,
            target_range,
            available_from,
            result,
            "derived-shape-product",
            evidence_ranges(&[&left, &right]),
        );
        return;
    }

    if let Some(captures) = ALIAS.captures(expression) {
        let source_symbol = captures.get(1).unwrap().as_str();
        if let Some(source) = latest_fact(analysis, source_symbol, at).cloned() {
            push_derived_fact(
                analysis,
                target,
                target_range,
                available_from,
                source.shape.clone(),
                "derived-shape-alias",
                evidence_ranges(&[&source]),
            );
        }
    }
}

fn push_derived_fact(
    analysis: &mut ShapeAnalysis,
    symbol: &str,
    symbol_range: SourceRange,
    available_from: u32,
    shape: Shape,
    rule_id: &str,
    source_ranges: Vec<SourceRange>,
) {
    let scope_id = analysis.scopes.id_at(symbol_range.start_offset);
    analysis.facts.push(ShapeFact {
        symbol: symbol.into(),
        shape,
        symbol_range,
        available_from,
        evidence: Evidence {
            rule_id: rule_id.into(),
            kind: "derived-constraint".into(),
            strength: "strong".into(),
            source_ranges,
        },
        refinements: Vec::new(),
        explicit: false,
        scope_id,
    });
}

fn collect_inequalities(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    index: &SourceIndex,
    scopes: &ScopeGraph,
) -> Vec<Inequality> {
    parsed
        .iter()
        .flat_map(|math| {
            let Some((content, content_start)) = math_content(document, math, index) else {
                return Vec::new();
            };
            INEQUALITY
                .captures_iter(content)
                .map(|captures| {
                    let whole = captures.get(0).unwrap();
                    Inequality {
                        left: normalize_dimension(captures.get(1).unwrap().as_str()),
                        right: normalize_dimension(captures.get(2).unwrap().as_str()),
                        range: absolute_range(
                            index,
                            content_start + whole.start(),
                            content_start + whole.end(),
                        ),
                        scope_id: scopes.id_at(index.utf16_for_byte(content_start + whole.start())),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn declared_shape(dimensions: Option<&str>) -> Option<Shape> {
    let Some(dimensions) = dimensions else {
        return Some(Shape::Scalar);
    };
    let dimensions = dimensions.trim();
    let parts: Vec<_> = DIMENSION_PRODUCT
        .split(dimensions)
        .map(normalize_dimension)
        .filter(|part| !part.is_empty())
        .collect();
    match parts.as_slice() {
        [dimension] => Some(Shape::Vector(dimension.clone())),
        [rows, columns] => Some(Shape::Matrix(rows.clone(), columns.clone())),
        dimensions if dimensions.len() >= 3 => Some(Shape::Tensor(dimensions.to_vec())),
        _ => None,
    }
}

fn normalize_dimension(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '{' | '}'))
        .collect()
}

fn latest_fact<'a>(analysis: &'a ShapeAnalysis, symbol: &str, at: u32) -> Option<&'a ShapeFact> {
    analysis
        .facts
        .iter()
        .filter(|fact| {
            fact.symbol == symbol
                && fact.available_from <= at
                && analysis.scopes.visible(fact.scope_id, at)
        })
        .max_by_key(|fact| (analysis.scopes.depth(fact.scope_id), fact.available_from))
}

fn shapes_conflict(
    left: &Shape,
    right: &Shape,
    inequalities: &[Inequality],
    scopes: &ScopeGraph,
    at: u32,
) -> bool {
    match (left, right) {
        (Shape::Vector(left), Shape::Vector(right)) => {
            dimension_mismatch(left, right, inequalities, scopes, at).is_some()
        }
        (Shape::Matrix(left_rows, left_columns), Shape::Matrix(right_rows, right_columns)) => {
            dimension_mismatch(left_rows, right_rows, inequalities, scopes, at).is_some()
                || dimension_mismatch(left_columns, right_columns, inequalities, scopes, at)
                    .is_some()
        }
        (Shape::Scalar, Shape::Scalar) => false,
        (Shape::Tensor(left), Shape::Tensor(right)) if left.len() == right.len() => {
            left.iter().zip(right).any(|(left, right)| {
                dimension_mismatch(left, right, inequalities, scopes, at).is_some()
            })
        }
        _ => true,
    }
}

fn evidence_ranges(facts: &[&ShapeFact]) -> Vec<SourceRange> {
    let mut ranges = Vec::new();
    for fact in facts {
        ranges.extend(fact.evidence.source_ranges.iter().cloned());
        ranges.push(fact.symbol_range.clone());
    }
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    ranges
}

fn dimension_mismatch(
    left: &str,
    right: &str,
    inequalities: &[Inequality],
    scopes: &ScopeGraph,
    at: u32,
) -> Option<DimensionMismatch> {
    if left == right {
        return None;
    }
    if left.parse::<u64>().is_ok() && right.parse::<u64>().is_ok() {
        return Some(DimensionMismatch {
            left: left.into(),
            right: right.into(),
            evidence_range: None,
        });
    }
    inequalities
        .iter()
        .filter(|inequality| {
            inequality.range.end_offset <= at && scopes.visible(inequality.scope_id, at)
        })
        .find(|inequality| {
            (inequality.left == left && inequality.right == right)
                || (inequality.left == right && inequality.right == left)
        })
        .map(|inequality| DimensionMismatch {
            left: left.into(),
            right: right.into(),
            evidence_range: Some(inequality.range.clone()),
        })
}

fn math_content<'a>(
    document: &'a ProjectDocument,
    math: &ParsedMath,
    index: &SourceIndex,
) -> Option<(&'a str, usize)> {
    let start = index.byte_for_utf16(math.region.content_range.start_offset);
    let end = index.byte_for_utf16(math.region.content_range.end_offset);
    document
        .content
        .get(start..end)
        .map(|content| (content, start))
}

fn absolute_range(index: &SourceIndex, start: usize, end: usize) -> SourceRange {
    SourceRange {
        start_offset: index.utf16_for_byte(start),
        end_offset: index.utf16_for_byte(end),
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_shapes;
    use crate::parser::{math_regions, parse_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::ShapeAnalysis {
        let regions = math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            includes: Vec::new(),
        };
        analyze_shapes(&document, &parse_regions(source, &regions), &[])
    }

    #[test]
    fn reports_only_a_proven_matrix_vector_mismatch() {
        let source =
            "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}, k \\ne n$\n$y = Ax$";
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, "shape-incompatible-product");
        assert!(analysis.diagnostics[0].message.contains("n and k"));

        let uncertain =
            analyze("$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}$\n$y = Ax$");
        assert!(uncertain.diagnostics.is_empty());
    }

    #[test]
    fn propagates_a_product_shape_to_hover() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}$\n$y = Ax$\n$y$";
        let analysis = analyze(source);
        let offset = source.rfind("$y$").unwrap() as u32 + 1;
        let shape = analysis.shape_at("y", offset).unwrap();
        assert_eq!(shape.display, "Vector[m]");
        assert_eq!(shape.evidence.rule_id, "derived-shape-product");
    }

    #[test]
    fn reports_an_incompatible_explicit_redeclaration() {
        let source = "$x \\in \\mathbb{R}^{n}$\n$x \\in \\mathbb{R}^{m \\times n}$";
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 1);
        assert_eq!(analysis.diagnostics[0].code, "notation-shape-conflict");
    }

    #[test]
    fn keeps_explicit_shadowing_in_separate_sections() {
        let source = "# First\n$x \\in \\mathbb{R}^{n}$\n$x$\n# Second\n$x \\in \\mathbb{R}^{m \\times n}$\n$x$";
        let regions = math_regions(source, DocumentLanguage::Markdown);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.md".into(),
            language: DocumentLanguage::Markdown,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            includes: Vec::new(),
        };
        let analysis = analyze_shapes(&document, &parse_regions(source, &regions), &[]);
        assert!(analysis.diagnostics.is_empty());
        let first_use = source.find("$x$\n#").unwrap() as u32 + 1;
        let second_use = source.rfind("$x$").unwrap() as u32 + 1;
        assert_eq!(
            analysis.shape_at("x", first_use).unwrap().display,
            "Vector[n]"
        );
        assert_eq!(
            analysis.shape_at("x", second_use).unwrap().display,
            "Matrix[m × n]"
        );
    }

    #[test]
    fn represents_scalar_and_tensor_declarations() {
        let source = "$s \\in \\mathbb{R}, T \\in \\mathbb{R}^{a \\times b \\times c}$\n$s$ $T$";
        let analysis = analyze(source);
        let scalar = analysis
            .shape_at("s", source.rfind("$s$").unwrap() as u32 + 1)
            .unwrap();
        let tensor = analysis
            .shape_at("T", source.rfind("$T$").unwrap() as u32 + 1)
            .unwrap();
        assert_eq!(scalar.display, "Scalar");
        assert_eq!(tensor.display, "Tensor[a × b × c]");
    }
}
