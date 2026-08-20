use std::collections::BTreeMap;

use crate::canonical::{SemanticExpr, SemanticExprKind, expression_name, render_canonical};
use crate::parser::ParsedMath;
use crate::prose::{ProseShape, ProseShapeClaim};
use crate::scope::ScopeGraph;
use crate::{Evidence, ProjectDocument, SemanticDiagnostic, ShapeInfo, SourceRange};
const MAX_SYMBOL_CLAIMS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Scalar,
    Vector(String),
    Matrix(String, String),
    Tensor(Vec<String>),
}

impl Shape {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Vector(_) => "vector",
            Self::Matrix(_, _) => "matrix",
            Self::Tensor(_) => "tensor",
        }
    }

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
pub(crate) struct ShapeObservations {
    facts: Vec<ShapeFact>,
    pub diagnostics: Vec<SemanticDiagnostic>,
    scopes: ScopeGraph,
    notation_families: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub(crate) struct ExplicitShapeClaim {
    pub symbol: String,
    pub kind: String,
    pub dimensions: Vec<String>,
    pub display: String,
    pub symbol_range: SourceRange,
    pub evidence: Evidence,
}

impl ShapeObservations {
    pub fn exported(&self) -> Vec<ShapeInfo> {
        self.facts
            .iter()
            .filter(|fact| self.scopes.depth(fact.scope_id) == 0)
            .map(|fact| {
                fact.shape.info(
                    &fact.symbol,
                    fact.evidence.clone(),
                    fact.refinements.clone(),
                )
            })
            .collect()
    }

    pub fn explicit_claims(&self) -> Vec<ExplicitShapeClaim> {
        self.facts
            .iter()
            .filter(|fact| fact.explicit)
            .map(|fact| ExplicitShapeClaim {
                symbol: fact.symbol.clone(),
                kind: fact.shape.kind_name().into(),
                dimensions: fact
                    .shape
                    .info("", fact.evidence.clone(), Vec::new())
                    .dimensions,
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
                self.symbols_equivalent(&fact.symbol, symbol)
                    && (self.scopes.depth(fact.scope_id) == 0
                        || fact.available_from <= offset
                        || fact.symbol_range.contains(offset))
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

    pub fn claims_at(&self, symbol: &str, offset: u32) -> (Vec<ShapeInfo>, bool) {
        let mut facts = self
            .facts
            .iter()
            .filter(|fact| {
                self.symbols_equivalent(&fact.symbol, symbol)
                    && (self.scopes.depth(fact.scope_id) == 0
                        || fact.available_from <= offset
                        || fact.symbol_range.contains(offset))
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

    pub(crate) fn symbols_equivalent(&self, left: &str, right: &str) -> bool {
        let left = left.trim_start_matches('\\');
        let right = right.trim_start_matches('\\');
        left == right
            || self
                .notation_families
                .get(left)
                .zip(self.notation_families.get(right))
                .is_some_and(|(left_family, right_family)| left_family == right_family)
    }

    pub(crate) fn notation_families(&self) -> BTreeMap<String, String> {
        self.notation_families.clone()
    }
}

pub(crate) fn observe_shapes(
    document: &ProjectDocument,
    _parsed: &[ParsedMath],
    canonical_expressions: &[SemanticExpr],
    prose_claims: &[ProseShapeClaim],
) -> ShapeObservations {
    let scopes = ScopeGraph::new(document);
    let mut analysis = ShapeObservations {
        facts: Vec::new(),
        diagnostics: Vec::new(),
        scopes,
        notation_families: notation_families(canonical_expressions),
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

    for expression in canonical_expressions {
        collect_shape_declarations(expression, &mut analysis);
    }

    add_explicit_conflict_diagnostics(&mut analysis);

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

fn notation_families(expressions: &[SemanticExpr]) -> BTreeMap<String, String> {
    fn collect(expression: &SemanticExpr, output: &mut BTreeMap<String, String>) {
        match &expression.kind {
            SemanticExprKind::Index { base, indices } => {
                if let Some(name) = expression_name(expression) {
                    output.insert(name, render_canonical(base));
                }
                collect(base, output);
                for index in indices {
                    collect(index, output);
                }
            }
            SemanticExprKind::Sum(items)
            | SemanticExprKind::Product(items)
            | SemanticExprKind::System(items) => {
                for item in items {
                    collect(item, output);
                }
            }
            SemanticExprKind::Dot(left, right)
            | SemanticExprKind::Cross(left, right)
            | SemanticExprKind::Fraction(left, right)
            | SemanticExprKind::Power(left, right)
            | SemanticExprKind::Relation { left, right, .. } => {
                collect(left, output);
                collect(right, output);
            }
            SemanticExprKind::Negate(value) => collect(value, output),
            SemanticExprKind::Derivative { expression, .. } => collect(expression, output),
            SemanticExprKind::Apply { arguments, .. } => {
                for argument in arguments {
                    collect(argument, output);
                }
            }
            SemanticExprKind::Condition { value, predicate } => {
                collect(value, output);
                collect(predicate, output);
            }
            SemanticExprKind::Binder {
                variables,
                lower,
                upper,
                body,
                ..
            } => {
                for variable in variables {
                    collect(variable, output);
                }
                if let Some(lower) = lower {
                    collect(lower, output);
                }
                if let Some(upper) = upper {
                    collect(upper, output);
                }
                collect(body, output);
            }
            SemanticExprKind::Piecewise(branches) => {
                for branch in branches {
                    collect(&branch.value, output);
                    if let Some(condition) = &branch.condition {
                        collect(condition, output);
                    }
                }
            }
            SemanticExprKind::Symbol(_)
            | SemanticExprKind::Number(_)
            | SemanticExprKind::Unknown(_) => {}
        }
    }

    let mut output = BTreeMap::new();
    for expression in expressions {
        collect(expression, &mut output);
    }
    output
}

fn add_explicit_conflict_diagnostics(analysis: &mut ShapeObservations) {
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
            && shapes_conflict(&previous.shape, &fact.shape)
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

fn collect_shape_declarations(expression: &SemanticExpr, analysis: &mut ShapeObservations) {
    match &expression.kind {
        SemanticExprKind::System(expressions) => {
            for expression in expressions {
                collect_shape_declarations(expression, analysis);
            }
        }
        SemanticExprKind::Relation {
            operator,
            left,
            right,
        } if operator.as_str() == "member-of" => {
            let Some(shape) = real_coordinate_shape(right) else {
                return;
            };
            for (symbol, symbol_range) in declaration_symbols(left) {
                let scope_id = analysis.scopes.id_at(symbol_range.start_offset);
                analysis.facts.push(ShapeFact {
                    symbol,
                    shape: shape.clone(),
                    symbol_range,
                    available_from: expression.range.end_offset,
                    evidence: Evidence {
                        rule_id: "explicit-real-space-declaration".into(),
                        kind: "explicit-math".into(),
                        strength: "hard".into(),
                        source_ranges: vec![expression.range.clone()],
                        source_anchors: Vec::new(),
                    },
                    refinements: Vec::new(),
                    explicit: true,
                    scope_id,
                });
            }
        }
        _ => {}
    }
}

fn declaration_symbols(expression: &SemanticExpr) -> Vec<(String, SourceRange)> {
    if let Some(symbol) = expression_name(expression) {
        return vec![(symbol, expression.range.clone())];
    }
    match &expression.kind {
        SemanticExprKind::Product(items) => items
            .iter()
            .filter(|item| !matches!(&item.kind, SemanticExprKind::Symbol(value) if value == ","))
            .flat_map(declaration_symbols)
            .collect(),
        _ => Vec::new(),
    }
}

fn real_coordinate_shape(expression: &SemanticExpr) -> Option<Shape> {
    if matches!(&expression.kind, SemanticExprKind::Symbol(symbol) if symbol == "R") {
        return Some(Shape::Scalar);
    }
    let SemanticExprKind::Power(base, dimensions) = &expression.kind else {
        return None;
    };
    if !matches!(&base.kind, SemanticExprKind::Symbol(symbol) if symbol == "R") {
        return None;
    }
    let dimensions = dimension_terms(dimensions);
    match dimensions.as_slice() {
        [dimension] => Some(Shape::Vector(dimension.clone())),
        [rows, columns] => Some(Shape::Matrix(rows.clone(), columns.clone())),
        dimensions if dimensions.len() >= 3 => Some(Shape::Tensor(dimensions.to_vec())),
        _ => None,
    }
}

fn dimension_terms(expression: &SemanticExpr) -> Vec<String> {
    match &expression.kind {
        SemanticExprKind::Cross(left, right) => {
            let mut dimensions = dimension_terms(left);
            dimensions.extend(dimension_terms(right));
            dimensions
        }
        _ => vec![dimension_label(expression)],
    }
}

fn dimension_label(expression: &SemanticExpr) -> String {
    if let Some(name) = expression_name(expression) {
        return name;
    }
    match &expression.kind {
        SemanticExprKind::Number(value) => value.clone(),
        SemanticExprKind::Sum(items) => items
            .iter()
            .map(dimension_label)
            .collect::<Vec<_>>()
            .join(" + "),
        SemanticExprKind::Product(items) => items
            .iter()
            .map(dimension_label)
            .collect::<Vec<_>>()
            .join("·"),
        SemanticExprKind::Power(base, exponent) => {
            format!("{}^{}", dimension_label(base), dimension_label(exponent))
        }
        _ => render_canonical(expression),
    }
}

fn shapes_conflict(left: &Shape, right: &Shape) -> bool {
    match (left, right) {
        (Shape::Vector(left), Shape::Vector(right)) => dimensions_conflict(left, right),
        (Shape::Matrix(left_rows, left_columns), Shape::Matrix(right_rows, right_columns)) => {
            dimensions_conflict(left_rows, right_rows)
                || dimensions_conflict(left_columns, right_columns)
        }
        (Shape::Scalar, Shape::Scalar) => false,
        (Shape::Tensor(left), Shape::Tensor(right)) if left.len() == right.len() => left
            .iter()
            .zip(right)
            .any(|(left, right)| dimensions_conflict(left, right)),
        _ => true,
    }
}

fn dimensions_conflict(left: &str, right: &str) -> bool {
    if left == right {
        return false;
    }
    if left.parse::<u64>().is_ok() && right.parse::<u64>().is_ok() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::observe_shapes;
    use crate::canonical::lower_document_region;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(source: &str) -> super::ShapeObservations {
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
        observe_shapes(&document, &parsed, &canonical, &[])
    }

    #[test]
    fn does_not_reparse_formula_text_for_derived_shapes_or_diagnostics() {
        let source =
            "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}, k \\ne n$\n$y = Ax$";
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty());
        assert!(analysis.shape_at("y", source.len() as u32).is_none());
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
        let regions = test_math_regions(source, DocumentLanguage::Markdown);
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
        let analysis = observe_shapes(&document, &parsed, &canonical, &[]);
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
