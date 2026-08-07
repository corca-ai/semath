use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::parser::ParsedMath;
use crate::shape::{KnownShape, Shape, ShapeAnalysis};
use crate::{
    Evidence, FormulaBinding, FormulaCompletion, FormulaConstraint, FormulaParameter,
    FormulaPattern, FormulaRecognition, FormulaSideCondition, ProjectDocument, SemanticEditFile,
    SemanticEditProposal, SemanticTextEdit, SourceIndex, SourceRange,
};

const PATTERN_SCHEMA_VERSION: u32 = 1;
const MAX_REGIONS: usize = 64;
const MAX_RECOGNITIONS: usize = 8;
const MAX_COMPLETIONS: usize = 8;

static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[A-Za-z]\s*=\s*(.+?)\s*$").unwrap());
static COMPLETION_TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z])\s*=\s*$").unwrap());
static BINARY_PRODUCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*(?:\\cdot\s*)?([A-Za-z])\s*$").unwrap());
static TRANSPOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*$").unwrap());
static INNER_PRODUCT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*([A-Za-z])\s*$").unwrap()
});
static QUADRATIC_FORM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*([A-Za-z])\s*([A-Za-z])\s*$")
        .unwrap()
});

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPack {
    schema_version: u32,
    pack_id: String,
    pack_version: String,
    patterns: Vec<RawPattern>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPattern {
    id: String,
    title: String,
    matcher: String,
    parameters: Vec<FormulaParameter>,
    result: FormulaConstraint,
    side_conditions: Vec<FormulaSideCondition>,
    generation_template: String,
}

static LINEAR_ALGEBRA_PATTERNS: LazyLock<Vec<FormulaPattern>> = LazyLock::new(|| {
    let pack: RawPack = serde_json::from_str(include_str!("../../../packs/linear-algebra/v1.json"))
        .expect("linear-algebra pack must be valid JSON");
    let patterns = pack
        .patterns
        .into_iter()
        .map(|pattern| FormulaPattern {
            schema_version: pack.schema_version,
            pack_id: pack.pack_id.clone(),
            pack_version: pack.pack_version.clone(),
            id: pattern.id,
            title: pattern.title,
            matcher: pattern.matcher,
            parameters: pattern.parameters,
            result: pattern.result,
            side_conditions: pattern.side_conditions,
            generation_template: pattern.generation_template,
        })
        .collect::<Vec<_>>();
    validate_patterns(&patterns).expect("linear-algebra pack must satisfy the pattern schema");
    patterns
});

#[derive(Clone, Debug, Default)]
pub(crate) struct FormulaAnalysis {
    recognitions: Vec<FormulaRecognition>,
}

impl FormulaAnalysis {
    pub fn all(&self) -> &[FormulaRecognition] {
        &self.recognitions
    }

    pub fn at(&self, offset: u32) -> Vec<FormulaRecognition> {
        self.recognitions
            .iter()
            .filter(|recognition| recognition.range.contains(offset))
            .take(MAX_RECOGNITIONS)
            .cloned()
            .collect()
    }
}

pub(crate) fn analyze_formulas(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    shapes: &ShapeAnalysis,
) -> FormulaAnalysis {
    let index = SourceIndex::new(&document.content);
    let mut recognitions = Vec::new();
    for math in parsed.iter().take(MAX_REGIONS) {
        if recognitions.len() >= MAX_RECOGNITIONS {
            break;
        }
        let Some((content, content_start)) = math_content(document, math, &index) else {
            continue;
        };
        let Some(assignment) = ASSIGNMENT.captures(content) else {
            continue;
        };
        let expression = assignment.get(1).unwrap();
        let expression_range = absolute_range(
            &index,
            content_start + expression.start(),
            content_start + expression.end(),
        );
        let known = known_by_symbol(shapes.known_shapes_at(expression_range.start_offset));
        if let Some(recognition) =
            recognize_expression(expression.as_str(), expression_range, &known)
        {
            recognitions.push(recognition);
        }
    }
    FormulaAnalysis { recognitions }
}

pub(crate) fn formula_completions(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    shapes: &ShapeAnalysis,
    offset: u32,
) -> Vec<FormulaCompletion> {
    let Some(math) = parsed.iter().find(|math| {
        math.region.full_range.start_offset <= offset && offset <= math.region.full_range.end_offset
    }) else {
        return Vec::new();
    };
    let index = SourceIndex::new(&document.content);
    let content_start = index.byte_for_utf16(math.region.content_range.start_offset);
    let cursor_byte = index.byte_for_utf16(offset);
    if cursor_byte < content_start || cursor_byte > document.content.len() {
        return Vec::new();
    }
    let prefix = &document.content[content_start..cursor_byte];
    let Some(target) = COMPLETION_TARGET
        .captures(prefix)
        .and_then(|captures| captures.get(1))
        .map(|found| found.as_str())
    else {
        return Vec::new();
    };
    let known = shapes.known_shapes_at(offset);
    let Some(target_shape) = known.iter().find(|fact| fact.symbol == target) else {
        return Vec::new();
    };
    let candidates = completion_candidates(target, &target_shape.shape, &known);
    candidates
        .into_iter()
        .take(MAX_COMPLETIONS)
        .enumerate()
        .map(|(rank, candidate)| completion(document, offset, rank, candidate))
        .collect()
}

struct CompletionCandidate<'a> {
    pattern: &'static FormulaPattern,
    latex: String,
    facts: Vec<&'a KnownShape>,
}

fn completion_candidates<'a>(
    target: &str,
    target_shape: &Shape,
    known: &'a [KnownShape],
) -> Vec<CompletionCandidate<'a>> {
    let inputs = known
        .iter()
        .filter(|fact| fact.symbol != target)
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();

    for left in &inputs {
        for right in &inputs {
            if left.symbol == right.symbol {
                continue;
            }
            match (&left.shape, &right.shape, target_shape) {
                (Shape::Matrix(rows, inner), Shape::Vector(length), Shape::Vector(result))
                    if inner == length && rows == result =>
                {
                    push_candidate(
                        &mut candidates,
                        "matrix-vector-product",
                        &[(&left.symbol, "matrix"), (&right.symbol, "vector")],
                        vec![*left, *right],
                    );
                }
                (
                    Shape::Matrix(rows, inner),
                    Shape::Matrix(right_rows, columns),
                    Shape::Matrix(result_rows, result_columns),
                ) if inner == right_rows && rows == result_rows && columns == result_columns => {
                    push_candidate(
                        &mut candidates,
                        "matrix-matrix-product",
                        &[(&left.symbol, "left"), (&right.symbol, "right")],
                        vec![*left, *right],
                    );
                }
                (Shape::Vector(left_length), Shape::Vector(right_length), Shape::Scalar)
                    if left_length == right_length =>
                {
                    push_candidate(
                        &mut candidates,
                        "vector-inner-product",
                        &[(&left.symbol, "left"), (&right.symbol, "right")],
                        vec![*left, *right],
                    );
                }
                _ => {}
            }
        }
    }

    for matrix in &inputs {
        if let (Shape::Matrix(rows, columns), Shape::Matrix(result_rows, result_columns)) =
            (&matrix.shape, target_shape)
            && rows == result_columns
            && columns == result_rows
        {
            push_candidate(
                &mut candidates,
                "matrix-transpose",
                &[(&matrix.symbol, "matrix")],
                vec![*matrix],
            );
        }
    }

    if matches!(target_shape, Shape::Scalar) {
        for vector in &inputs {
            let Shape::Vector(length) = &vector.shape else {
                continue;
            };
            for matrix in &inputs {
                let Shape::Matrix(rows, columns) = &matrix.shape else {
                    continue;
                };
                if rows == length && columns == length {
                    push_candidate(
                        &mut candidates,
                        "quadratic-form",
                        &[(&vector.symbol, "vector"), (&matrix.symbol, "matrix")],
                        vec![*vector, *matrix],
                    );
                }
            }
        }
    }

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.latex.clone()));
    candidates.sort_by(|left, right| {
        pattern_order(&left.pattern.id)
            .cmp(&pattern_order(&right.pattern.id))
            .then(left.latex.cmp(&right.latex))
    });
    candidates
}

fn push_candidate<'a>(
    candidates: &mut Vec<CompletionCandidate<'a>>,
    pattern_id: &str,
    bindings: &[(&str, &str)],
    facts: Vec<&'a KnownShape>,
) {
    let Some(pattern) = pattern(pattern_id) else {
        return;
    };
    let mut latex = pattern.generation_template.clone();
    for (symbol, parameter) in bindings {
        latex = latex.replace(&format!("{{{{{parameter}}}}}"), symbol);
    }
    candidates.push(CompletionCandidate {
        pattern,
        latex,
        facts,
    });
}

fn completion(
    document: &ProjectDocument,
    offset: u32,
    rank: usize,
    candidate: CompletionCandidate<'_>,
) -> FormulaCompletion {
    let evidence = candidate
        .facts
        .iter()
        .map(|fact| fact.evidence.clone())
        .collect::<Vec<_>>();
    FormulaCompletion {
        pattern_id: candidate.pattern.id.clone(),
        title: candidate.latex.clone(),
        detail: format!(
            "{} · {} {}",
            candidate.pattern.title, candidate.pattern.pack_id, candidate.pattern.pack_version
        ),
        rank: rank as u32,
        proposal: SemanticEditProposal {
            title: format!("Insert {}: {}", candidate.pattern.title, candidate.latex),
            safety: "review-required".into(),
            evidence,
            files: vec![SemanticEditFile {
                file_id: document.file_id.clone(),
                path: document.path.clone(),
                document_version: document.document_version,
                edits: vec![SemanticTextEdit {
                    range: SourceRange {
                        start_offset: offset,
                        end_offset: offset,
                    },
                    expected_text: String::new(),
                    replacement_text: candidate.latex,
                }],
            }],
        },
    }
}

fn recognize_expression(
    expression: &str,
    range: SourceRange,
    known: &BTreeMap<String, KnownShape>,
) -> Option<FormulaRecognition> {
    if let Some(captures) = QUADRATIC_FORM.captures(expression) {
        let vector = captures.get(1).unwrap().as_str();
        let matrix = captures.get(2).unwrap().as_str();
        let trailing = captures.get(3).unwrap().as_str();
        if vector != trailing {
            return None;
        }
        let vector_fact = known.get(vector)?;
        let matrix_fact = known.get(matrix)?;
        if let (Shape::Vector(length), Shape::Matrix(rows, columns)) =
            (&vector_fact.shape, &matrix_fact.shape)
            && length == rows
            && length == columns
        {
            return recognition(
                "quadratic-form",
                range,
                vec![("vector", vector_fact), ("matrix", matrix_fact)],
                Shape::Scalar,
            );
        }
        return None;
    }

    if let Some(captures) = INNER_PRODUCT.captures(expression) {
        let left = known.get(captures.get(1).unwrap().as_str())?;
        let right = known.get(captures.get(2).unwrap().as_str())?;
        if let (Shape::Vector(left_length), Shape::Vector(right_length)) =
            (&left.shape, &right.shape)
            && left_length == right_length
        {
            return recognition(
                "vector-inner-product",
                range,
                vec![("left", left), ("right", right)],
                Shape::Scalar,
            );
        }
        return None;
    }

    if let Some(captures) = TRANSPOSE.captures(expression) {
        let matrix = known.get(captures.get(1).unwrap().as_str())?;
        if let Shape::Matrix(rows, columns) = &matrix.shape {
            return recognition(
                "matrix-transpose",
                range,
                vec![("matrix", matrix)],
                Shape::Matrix(columns.clone(), rows.clone()),
            );
        }
        return None;
    }

    if let Some(captures) = BINARY_PRODUCT.captures(expression) {
        let left = known.get(captures.get(1).unwrap().as_str())?;
        let right = known.get(captures.get(2).unwrap().as_str())?;
        match (&left.shape, &right.shape) {
            (Shape::Matrix(rows, inner), Shape::Vector(length)) if inner == length => recognition(
                "matrix-vector-product",
                range,
                vec![("matrix", left), ("vector", right)],
                Shape::Vector(rows.clone()),
            ),
            (Shape::Matrix(rows, inner), Shape::Matrix(right_rows, columns))
                if inner == right_rows =>
            {
                recognition(
                    "matrix-matrix-product",
                    range,
                    vec![("left", left), ("right", right)],
                    Shape::Matrix(rows.clone(), columns.clone()),
                )
            }
            _ => None,
        }
    } else {
        None
    }
}

fn recognition(
    pattern_id: &str,
    range: SourceRange,
    facts: Vec<(&str, &KnownShape)>,
    result: Shape,
) -> Option<FormulaRecognition> {
    let pattern = pattern(pattern_id)?;
    let bindings = facts
        .iter()
        .map(|(parameter, fact)| FormulaBinding {
            parameter: (*parameter).into(),
            symbol: fact.symbol.clone(),
            constraint: fact.constraint(),
            evidence: fact.evidence.clone(),
        })
        .collect::<Vec<_>>();
    let mut source_ranges = facts
        .iter()
        .flat_map(|(_, fact)| fact.evidence.source_ranges.iter().cloned())
        .collect::<Vec<_>>();
    source_ranges.push(range.clone());
    source_ranges.sort_by_key(|source| (source.start_offset, source.end_offset));
    source_ranges.dedup();
    let pattern_evidence = Evidence {
        rule_id: format!("{}/{}", pattern.pack_id, pattern.id),
        kind: "domain-pattern".into(),
        strength: "strong".into(),
        source_ranges,
    };
    let mut evidence = bindings
        .iter()
        .map(|binding| binding.evidence.clone())
        .collect::<Vec<_>>();
    evidence.push(pattern_evidence);
    Some(FormulaRecognition {
        pattern_id: pattern.id.clone(),
        title: pattern.title.clone(),
        pack_id: pattern.pack_id.clone(),
        pack_version: pattern.pack_version.clone(),
        range,
        bindings,
        result: result.constraint(),
        evidence,
        rank: 100,
    })
}

fn known_by_symbol(known: Vec<KnownShape>) -> BTreeMap<String, KnownShape> {
    known
        .into_iter()
        .map(|fact| (fact.symbol.clone(), fact))
        .collect()
}

fn pattern(id: &str) -> Option<&'static FormulaPattern> {
    LINEAR_ALGEBRA_PATTERNS
        .iter()
        .find(|pattern| pattern.id == id)
}

fn pattern_order(id: &str) -> usize {
    LINEAR_ALGEBRA_PATTERNS
        .iter()
        .position(|pattern| pattern.id == id)
        .unwrap_or(usize::MAX)
}

fn validate_patterns(patterns: &[FormulaPattern]) -> Result<(), String> {
    let mut ids = HashSet::new();
    for pattern in patterns {
        if pattern.schema_version != PATTERN_SCHEMA_VERSION {
            return Err(format!("unsupported schema for {}", pattern.id));
        }
        if !ids.insert(&pattern.id) {
            return Err(format!("duplicate pattern {}", pattern.id));
        }
        if pattern.parameters.is_empty()
            || pattern.side_conditions.is_empty()
            || pattern.generation_template.is_empty()
        {
            return Err(format!("incomplete pattern {}", pattern.id));
        }
        for parameter in &pattern.parameters {
            let placeholder = format!("{{{{{}}}}}", parameter.id);
            if !pattern.generation_template.contains(&placeholder) {
                return Err(format!(
                    "missing placeholder {placeholder} in {}",
                    pattern.id
                ));
            }
        }
    }
    Ok(())
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
    use super::{analyze_formulas, formula_completions};
    use crate::parser::{math_regions, parse_regions};
    use crate::shape::analyze_shapes;
    use crate::{DocumentLanguage, ProjectDocument};

    fn analyze(
        source: &str,
    ) -> (
        ProjectDocument,
        Vec<crate::parser::ParsedMath>,
        crate::shape::ShapeAnalysis,
    ) {
        let regions = math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
        };
        let parsed = parse_regions(source, &regions);
        let shapes = analyze_shapes(&document, &parsed, &[]);
        (document, parsed, shapes)
    }

    #[test]
    fn recognizes_typed_linear_algebra_formulas() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}$\n$y = Ax$";
        let (document, parsed, shapes) = analyze(source);
        let formulas = analyze_formulas(&document, &parsed, &shapes);
        let offset = source.rfind("Ax").unwrap() as u32;
        let recognized = formulas.at(offset);
        assert_eq!(recognized[0].pattern_id, "matrix-vector-product");
        assert_eq!(recognized[0].result.kind, "vector");
    }

    #[test]
    fn completes_a_typed_formula_slot_from_known_symbols() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}, y \\in \\mathbb{R}^{m}$\n$y = $";
        let (document, parsed, shapes) = analyze(source);
        let offset = source.rfind(" = ").unwrap() as u32 + 3;
        let completions = formula_completions(&document, &parsed, &shapes, offset);
        assert!(
            completions
                .iter()
                .any(|completion| completion.title == "Ax")
        );
        let proposal = &completions
            .iter()
            .find(|completion| completion.title == "Ax")
            .unwrap()
            .proposal;
        assert_eq!(proposal.safety, "review-required");
        assert_eq!(proposal.files[0].edits[0].range.start_offset, offset);
    }

    #[test]
    fn recognizes_inner_products_and_quadratic_forms() {
        let source = "$A \\in \\mathbb{R}^{n \\times n}, x \\in \\mathbb{R}^{n}, y \\in \\mathbb{R}^{n}$\n$s = x^{\\top}y$\n$q = x^{\\top}Ax$";
        let (document, parsed, shapes) = analyze(source);
        let formulas = analyze_formulas(&document, &parsed, &shapes);
        assert_eq!(
            formulas.at(source.find("x^{\\top}y").unwrap() as u32)[0].pattern_id,
            "vector-inner-product"
        );
        assert_eq!(
            formulas.at(source.rfind("x^{\\top}Ax").unwrap() as u32)[0].pattern_id,
            "quadratic-form"
        );
    }
}
