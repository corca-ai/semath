use std::sync::LazyLock;

use regex::Regex;

use crate::parser::ParsedMath;
use crate::{Evidence, ProjectDocument, SemanticDiagnostic, ShapeInfo, SourceIndex, SourceRange};

static DECLARATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)([A-Za-z])\s*\\in\s*\\mathbb\s*\{\s*R\s*\}\s*\^\s*(?:\{([^}]*)\}|([A-Za-z0-9]+))",
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    Vector(String),
    Matrix(String, String),
}

impl Shape {
    fn display(&self) -> String {
        match self {
            Self::Vector(dimension) => format!("Vector[{dimension}]"),
            Self::Matrix(rows, columns) => format!("Matrix[{rows} × {columns}]"),
        }
    }

    fn info(&self, symbol: &str, evidence: Evidence) -> ShapeInfo {
        let (kind, dimensions) = match self {
            Self::Vector(dimension) => ("vector", vec![dimension.clone()]),
            Self::Matrix(rows, columns) => ("matrix", vec![rows.clone(), columns.clone()]),
        };
        ShapeInfo {
            symbol: symbol.into(),
            kind: kind.into(),
            dimensions,
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
}

#[derive(Clone, Debug)]
struct ShapeFact {
    symbol: String,
    shape: Shape,
    symbol_range: SourceRange,
    available_from: u32,
    evidence: Evidence,
    explicit: bool,
}

#[derive(Clone, Debug)]
struct Inequality {
    left: String,
    right: String,
    range: SourceRange,
}

#[derive(Clone, Debug)]
struct DimensionMismatch {
    left: String,
    right: String,
    evidence_range: Option<SourceRange>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ShapeAnalysis {
    facts: Vec<ShapeFact>,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

impl ShapeAnalysis {
    pub fn shape_at(&self, symbol: &str, offset: u32) -> Option<ShapeInfo> {
        self.facts
            .iter()
            .filter(|fact| {
                fact.symbol == symbol
                    && (fact.available_from <= offset || fact.symbol_range.contains(offset))
            })
            .max_by_key(|fact| fact.available_from)
            .map(|fact| fact.shape.info(symbol, fact.evidence.clone()))
    }

    pub fn diagnostic(&self, code: &str, offset: u32) -> Option<SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code && diagnostic.range.contains(offset))
            .cloned()
    }
}

pub(crate) fn analyze_shapes(document: &ProjectDocument, parsed: &[ParsedMath]) -> ShapeAnalysis {
    let index = SourceIndex::new(&document.content);
    let inequalities = collect_inequalities(document, parsed, &index);
    let mut analysis = ShapeAnalysis::default();

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
                .unwrap()
                .as_str();
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
            if let Some(previous) =
                latest_explicit_fact(&analysis.facts, symbol, symbol_range.start_offset)
                && shapes_conflict(
                    &previous.shape,
                    &shape,
                    &inequalities,
                    symbol_range.start_offset,
                )
            {
                analysis.diagnostics.push(SemanticDiagnostic {
                    code: "notation-shape-conflict".into(),
                    severity: "warning".into(),
                    message: format!(
                        "Notation `{symbol}` is redeclared as {} after {}.",
                        shape.display(),
                        previous.shape.display()
                    ),
                    explanation: format!(
                        "Both declarations are explicit and assign incompatible shapes to `{symbol}` in the same document scope."
                    ),
                    range: symbol_range.clone(),
                    evidence: vec![previous.evidence.clone(), Evidence {
                        rule_id: "explicit-real-shape-declaration".into(),
                        kind: "explicit-math".into(),
                        strength: "hard".into(),
                        source_ranges: vec![declaration_range.clone()],
                    }],
                });
            }
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
                explicit: true,
            });
        }
    }

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
    if let Some(captures) = ADDITION.captures(expression) {
        let left_symbol = captures.get(1).unwrap().as_str();
        let right_symbol = captures.get(2).unwrap().as_str();
        let Some(left) = latest_fact(&analysis.facts, left_symbol, at).cloned() else {
            return;
        };
        let Some(right) = latest_fact(&analysis.facts, right_symbol, at).cloned() else {
            return;
        };
        if shapes_conflict(&left.shape, &right.shape, inequalities, at) {
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
            left.shape,
            "derived-shape-addition",
            vec![left.symbol_range, right.symbol_range],
        );
        return;
    }

    if let Some(captures) = PRODUCT.captures(expression) {
        let left_symbol = captures.get(1).unwrap().as_str();
        let right_symbol = captures.get(3).unwrap().as_str();
        let Some(mut left) = latest_fact(&analysis.facts, left_symbol, at).cloned() else {
            return;
        };
        let Some(right) = latest_fact(&analysis.facts, right_symbol, at).cloned() else {
            return;
        };
        if captures.get(2).is_some() {
            left.shape = left.shape.transpose();
        }
        let (result, mismatch) = match (&left.shape, &right.shape) {
            (Shape::Matrix(rows, inner), Shape::Vector(dimension)) => (
                Some(Shape::Vector(rows.clone())),
                dimension_mismatch(inner, dimension, inequalities, at),
            ),
            (Shape::Matrix(rows, inner), Shape::Matrix(right_rows, columns)) => (
                Some(Shape::Matrix(rows.clone(), columns.clone())),
                dimension_mismatch(inner, right_rows, inequalities, at),
            ),
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
            vec![left.symbol_range, right.symbol_range],
        );
        return;
    }

    if let Some(captures) = ALIAS.captures(expression) {
        let source_symbol = captures.get(1).unwrap().as_str();
        if let Some(source) = latest_fact(&analysis.facts, source_symbol, at).cloned() {
            push_derived_fact(
                analysis,
                target,
                target_range,
                available_from,
                source.shape,
                "derived-shape-alias",
                vec![source.symbol_range],
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
        explicit: false,
    });
}

fn collect_inequalities(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    index: &SourceIndex,
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
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn declared_shape(dimensions: &str) -> Option<Shape> {
    let dimensions = dimensions.trim();
    let parts: Vec<_> = DIMENSION_PRODUCT
        .split(dimensions)
        .map(normalize_dimension)
        .filter(|part| !part.is_empty())
        .collect();
    match parts.as_slice() {
        [dimension] => Some(Shape::Vector(dimension.clone())),
        [rows, columns] => Some(Shape::Matrix(rows.clone(), columns.clone())),
        _ => None,
    }
}

fn normalize_dimension(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '{' | '}'))
        .collect()
}

fn latest_fact<'a>(facts: &'a [ShapeFact], symbol: &str, at: u32) -> Option<&'a ShapeFact> {
    facts
        .iter()
        .filter(|fact| fact.symbol == symbol && fact.available_from <= at)
        .max_by_key(|fact| fact.available_from)
}

fn latest_explicit_fact<'a>(
    facts: &'a [ShapeFact],
    symbol: &str,
    at: u32,
) -> Option<&'a ShapeFact> {
    facts
        .iter()
        .filter(|fact| fact.explicit && fact.symbol == symbol && fact.available_from <= at)
        .max_by_key(|fact| fact.available_from)
}

fn shapes_conflict(left: &Shape, right: &Shape, inequalities: &[Inequality], at: u32) -> bool {
    match (left, right) {
        (Shape::Vector(left), Shape::Vector(right)) => {
            dimension_mismatch(left, right, inequalities, at).is_some()
        }
        (Shape::Matrix(left_rows, left_columns), Shape::Matrix(right_rows, right_columns)) => {
            dimension_mismatch(left_rows, right_rows, inequalities, at).is_some()
                || dimension_mismatch(left_columns, right_columns, inequalities, at).is_some()
        }
        _ => true,
    }
}

fn dimension_mismatch(
    left: &str,
    right: &str,
    inequalities: &[Inequality],
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
        .filter(|inequality| inequality.range.end_offset <= at)
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
        };
        analyze_shapes(&document, &parse_regions(source, &regions))
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
}
