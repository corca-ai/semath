use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use crate::consistency::ConsistencyAnalysis;
use crate::pack::{PackPattern, built_in_packs};
use crate::parser::ParsedMath;
use crate::shape::{KnownShape, Shape, ShapeAnalysis};
use crate::{
    Evidence, FormulaBinding, FormulaCompletion, FormulaConditionInfo, FormulaConstraint,
    FormulaRecognition, ProjectDocument, RoleInfo, SemanticEditFile, SemanticEditProposal,
    SemanticTextEdit, SourceIndex, SourceRange,
};
use regex::Regex;

const MAX_REGIONS: usize = 64;
const MAX_RECOGNITIONS: usize = 8;
const MAX_COMPLETIONS: usize = 8;

static ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[A-Za-z]\s*=\s*(.+?)\s*$").unwrap());
static TRIMMED_EXPRESSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*(\S(?:.*\S)?)\s*$").unwrap());
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
static CONDITIONAL_PROBABILITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*\\mathbb\s*\{\s*P\s*\}\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\mid|\\vert|\|)\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
    )
    .unwrap()
});
static EVENT_PROBABILITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*\\mathbb\s*\{\s*P\s*\}\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
    )
    .unwrap()
});
static EXPECTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*\\mathbb\s*\{\s*E\s*\}\s*(?:\\left\s*)?\[\s*([A-Za-z])\s*(?:\\right\s*)?\]\s*$",
    )
    .unwrap()
});
static VARIANCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*\\(?:operatorname|mathrm)\s*\{\s*Var\s*\}\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
    )
    .unwrap()
});

struct CompiledRegexPattern {
    pattern: &'static PackPattern,
    regex: Regex,
}

static REGEX_PATTERNS: LazyLock<Vec<CompiledRegexPattern>> = LazyLock::new(|| {
    built_in_packs()
        .iter()
        .flat_map(|pack| &pack.patterns)
        .filter(|pattern| pattern.matcher.primitive == "regex-captures")
        .map(|pattern| CompiledRegexPattern {
            pattern,
            regex: Regex::new(
                pattern
                    .matcher
                    .expression
                    .as_deref()
                    .expect("validated regex matcher has an expression"),
            )
            .expect("validated pack regex compiles"),
        })
        .collect()
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
    consistency: &ConsistencyAnalysis,
) -> FormulaAnalysis {
    let index = SourceIndex::new(&document.content);
    let mut recognitions = Vec::new();
    for math in parsed.iter().take(MAX_REGIONS) {
        if recognitions.len() >= MAX_RECOGNITIONS {
            break;
        }
        if !math.region.closed {
            continue;
        }
        let Some((content, content_start)) = math_content(document, math, &index) else {
            continue;
        };
        let full_expression = trimmed_match(content);
        let expression = ASSIGNMENT
            .captures(content)
            .and_then(|assignment| assignment.get(1))
            .or(full_expression);
        let Some(expression) = expression else {
            continue;
        };
        let expression_range = absolute_range(
            &index,
            content_start + expression.start(),
            content_start + expression.end(),
        );
        let known = known_by_symbol(shapes.known_shapes_at(expression_range.start_offset));
        let roles = roles_by_symbol(consistency.effective_roles_at(expression_range.start_offset));
        let mut found = recognize_legacy_expression(
            expression.as_str(),
            expression_range.clone(),
            &known,
            &roles,
        )
        .into_iter()
        .collect::<Vec<_>>();
        if let Some(full_expression) = full_expression {
            found.extend(recognize_regex_patterns(
                full_expression.as_str(),
                absolute_range(
                    &index,
                    content_start + full_expression.start(),
                    content_start + full_expression.end(),
                ),
                content_start + full_expression.start(),
                &index,
                &known,
                &roles,
            ));
        }
        if full_expression.is_some_and(|full| {
            full.start() != expression.start() || full.end() != expression.end()
        }) {
            found.extend(recognize_regex_patterns(
                expression.as_str(),
                expression_range,
                content_start + expression.start(),
                &index,
                &known,
                &roles,
            ));
        }
        let mut seen = HashSet::new();
        found.retain(|recognition| {
            seen.insert((
                recognition.pack_id.clone(),
                recognition.pattern_id.clone(),
                recognition.range.start_offset,
                recognition.range.end_offset,
            ))
        });
        recognitions.extend(
            found
                .into_iter()
                .take(MAX_RECOGNITIONS.saturating_sub(recognitions.len())),
        );
    }
    FormulaAnalysis { recognitions }
}

pub(crate) fn formula_completions(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    shapes: &ShapeAnalysis,
    consistency: &ConsistencyAnalysis,
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
    let roles = if matches!(target_shape.shape, Shape::Scalar)
        && matches!(
            target_shape.evidence.kind.as_str(),
            "explicit-math" | "explicit-prose"
        ) {
        consistency.effective_roles_at(offset)
    } else {
        Vec::new()
    };
    let candidates = completion_candidates(target, &target_shape.shape, &known, &roles);
    candidates
        .into_iter()
        .take(MAX_COMPLETIONS)
        .enumerate()
        .map(|(rank, candidate)| completion(document, offset, rank, candidate))
        .collect()
}

struct CompletionCandidate {
    pattern: &'static PackPattern,
    latex: String,
    evidence: Vec<Evidence>,
}

fn completion_candidates(
    target: &str,
    target_shape: &Shape,
    known: &[KnownShape],
    roles: &[RoleInfo],
) -> Vec<CompletionCandidate> {
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
                        vec![left.evidence.clone(), right.evidence.clone()],
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
                        vec![left.evidence.clone(), right.evidence.clone()],
                    );
                }
                (Shape::Vector(left_length), Shape::Vector(right_length), Shape::Scalar)
                    if left_length == right_length =>
                {
                    push_candidate(
                        &mut candidates,
                        "vector-inner-product",
                        &[(&left.symbol, "left"), (&right.symbol, "right")],
                        vec![left.evidence.clone(), right.evidence.clone()],
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
                vec![matrix.evidence.clone()],
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
                        vec![vector.evidence.clone(), matrix.evidence.clone()],
                    );
                }
            }
        }

        probability_completion_candidates(&mut candidates, roles);
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

fn probability_completion_candidates(
    candidates: &mut Vec<CompletionCandidate>,
    roles: &[RoleInfo],
) {
    let events = effective_roles(roles, "event");
    let variables = effective_roles(roles, "random-variable");

    for event in &events {
        push_candidate(
            candidates,
            "event-probability",
            &[(&event.symbol, "event")],
            vec![event.evidence.clone()],
        );
    }

    for event in &events {
        for condition in &events {
            if event.symbol == condition.symbol || !has_positive_probability_evidence(condition) {
                continue;
            }
            push_candidate(
                candidates,
                "conditional-probability",
                &[(&event.symbol, "event"), (&condition.symbol, "condition")],
                vec![event.evidence.clone(), condition.evidence.clone()],
            );
        }
    }

    for variable in variables {
        let binding = &[(&variable.symbol[..], "variable")];
        let evidence = vec![variable.evidence.clone()];
        push_candidate(candidates, "expectation", binding, evidence.clone());
        push_candidate(candidates, "variance", binding, evidence);
    }
}

fn effective_roles<'a>(roles: &'a [RoleInfo], role: &str) -> Vec<&'a RoleInfo> {
    let mut symbols = HashSet::new();
    roles
        .iter()
        .filter(|claim| claim.role == role && symbols.insert(claim.symbol.as_str()))
        .collect()
}

fn has_positive_probability_evidence(role: &RoleInfo) -> bool {
    let description = role
        .description
        .to_ascii_lowercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    description.contains("positive probability")
        || description.contains("nonzero probability")
        || description.contains("probability greater than zero")
}

fn push_candidate(
    candidates: &mut Vec<CompletionCandidate>,
    pattern_id: &str,
    bindings: &[(&str, &str)],
    evidence: Vec<Evidence>,
) {
    let Some(pattern) = pattern(pattern_id) else {
        return;
    };
    let Some(mut latex) = pattern.generation_template.clone() else {
        return;
    };
    for (symbol, parameter) in bindings {
        latex = latex.replace(&format!("{{{{{parameter}}}}}"), symbol);
    }
    candidates.push(CompletionCandidate {
        pattern,
        latex,
        evidence,
    });
}

fn completion(
    document: &ProjectDocument,
    offset: u32,
    rank: usize,
    candidate: CompletionCandidate,
) -> FormulaCompletion {
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
            evidence: candidate.evidence,
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

fn recognize_legacy_expression(
    expression: &str,
    range: SourceRange,
    known: &BTreeMap<String, KnownShape>,
    roles: &BTreeMap<String, Vec<RoleInfo>>,
) -> Option<FormulaRecognition> {
    if let Some(captures) = CONDITIONAL_PROBABILITY.captures(expression) {
        let event = role(roles, captures.get(1).unwrap().as_str(), "event")?;
        let condition = role(roles, captures.get(2).unwrap().as_str(), "event")?;
        if event.symbol == condition.symbol || !has_positive_probability_evidence(condition) {
            return None;
        }
        return role_recognition(
            "conditional-probability",
            range,
            vec![("event", event), ("condition", condition)],
            FormulaConstraint {
                kind: "scalar".into(),
                dimensions: Vec::new(),
                refinements: vec!["probability".into()],
            },
        );
    }

    if let Some(captures) = EVENT_PROBABILITY.captures(expression) {
        let event = role(roles, captures.get(1).unwrap().as_str(), "event")?;
        return role_recognition(
            "event-probability",
            range,
            vec![("event", event)],
            FormulaConstraint {
                kind: "scalar".into(),
                dimensions: Vec::new(),
                refinements: vec!["probability".into()],
            },
        );
    }

    if let Some(captures) = EXPECTATION.captures(expression) {
        let variable = role(roles, captures.get(1).unwrap().as_str(), "random-variable")?;
        return role_recognition(
            "expectation",
            range,
            vec![("variable", variable)],
            FormulaConstraint {
                kind: "scalar".into(),
                dimensions: Vec::new(),
                refinements: Vec::new(),
            },
        );
    }

    if let Some(captures) = VARIANCE.captures(expression) {
        let variable = role(roles, captures.get(1).unwrap().as_str(), "random-variable")?;
        return role_recognition(
            "variance",
            range,
            vec![("variable", variable)],
            FormulaConstraint {
                kind: "scalar".into(),
                dimensions: Vec::new(),
                refinements: vec!["nonnegative".into()],
            },
        );
    }

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

fn recognize_regex_patterns(
    expression: &str,
    range: SourceRange,
    expression_byte_start: usize,
    index: &SourceIndex,
    known: &BTreeMap<String, KnownShape>,
    roles: &BTreeMap<String, Vec<RoleInfo>>,
) -> Vec<FormulaRecognition> {
    if !complete_expression_surface(expression) {
        return Vec::new();
    }
    REGEX_PATTERNS
        .iter()
        .filter_map(|compiled| {
            let captures = compiled.regex.captures(expression)?;
            let whole = captures.get(0)?;
            if whole.start() != 0 || whole.end() != expression.len() {
                return None;
            }
            let mut dimensions = BTreeMap::<String, String>::new();
            let bindings = compiled
                .pattern
                .parameters
                .iter()
                .enumerate()
                .map(|(index_in_pattern, parameter)| {
                    let capture = captures.get(index_in_pattern + 1)?;
                    regex_binding(
                        compiled.pattern,
                        parameter,
                        capture.as_str(),
                        absolute_range(
                            index,
                            expression_byte_start + capture.start(),
                            expression_byte_start + capture.end(),
                        ),
                        known,
                        roles,
                        &mut dimensions,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            if !generic_side_conditions_hold(compiled.pattern, &bindings) {
                return None;
            }
            let mut source_ranges = bindings
                .iter()
                .flat_map(|binding| binding.evidence.source_ranges.iter().cloned())
                .chain(std::iter::once(range.clone()))
                .collect::<Vec<_>>();
            source_ranges.sort_by_key(|source| (source.start_offset, source.end_offset));
            source_ranges.dedup();
            let pattern_evidence = Evidence {
                rule_id: format!("{}/{}", compiled.pattern.pack_id, compiled.pattern.id),
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
                pattern_id: compiled.pattern.id.clone(),
                title: compiled.pattern.title.clone(),
                description: compiled.pattern.description.clone(),
                description_key: compiled.pattern.description_key.clone(),
                maturity: compiled.pattern.maturity.as_str().into(),
                status: if compiled.pattern.side_conditions.is_empty() {
                    "recognized"
                } else {
                    "verified"
                }
                .into(),
                pack_id: compiled.pattern.pack_id.clone(),
                pack_version: compiled.pattern.pack_version.clone(),
                range: range.clone(),
                bindings,
                result: substitute_dimensions(&compiled.pattern.result, &dimensions),
                conditions: verified_conditions(compiled.pattern),
                evidence,
                rank: 50,
            })
        })
        .collect()
}

fn complete_expression_surface(expression: &str) -> bool {
    let trimmed = expression.trim_end();
    !trimmed.is_empty()
        && !trimmed.ends_with(['_', '^', '\\', '{', '(', '[', ',', '=', '+', '-'])
        && balanced_pair(trimmed, '{', '}')
        && balanced_pair(trimmed, '(', ')')
        && balanced_pair(trimmed, '[', ']')
}

fn balanced_pair(expression: &str, open: char, close: char) -> bool {
    let mut depth = 0_u32;
    for character in expression.chars() {
        if character == open {
            depth = depth.saturating_add(1);
        } else if character == close {
            let Some(next) = depth.checked_sub(1) else {
                return false;
            };
            depth = next;
        }
    }
    depth == 0
}

#[allow(clippy::too_many_arguments)]
fn regex_binding(
    pattern: &PackPattern,
    parameter: &crate::FormulaParameter,
    captured: &str,
    capture_range: SourceRange,
    known: &BTreeMap<String, KnownShape>,
    roles: &BTreeMap<String, Vec<RoleInfo>>,
    dimensions: &mut BTreeMap<String, String>,
) -> Option<FormulaBinding> {
    let symbol = captured.trim();
    if symbol.is_empty() {
        return None;
    }
    if parameter.constraint.kind == "expression" {
        return Some(FormulaBinding {
            parameter: parameter.id.clone(),
            symbol: symbol.into(),
            constraint: parameter.constraint.clone(),
            evidence: Evidence {
                rule_id: format!(
                    "{}/{}/capture/{}",
                    pattern.pack_id, pattern.id, parameter.id
                ),
                kind: "syntax".into(),
                strength: "strong".into(),
                source_ranges: vec![capture_range],
            },
        });
    }

    if matches!(
        parameter.constraint.kind.as_str(),
        "scalar" | "vector" | "matrix" | "tensor"
    ) {
        let fact = known.get(symbol)?;
        let actual = fact.constraint();
        if !constraint_matches(&parameter.constraint, &actual, dimensions) {
            return None;
        }
        return Some(FormulaBinding {
            parameter: parameter.id.clone(),
            symbol: symbol.into(),
            constraint: actual,
            evidence: fact.evidence.clone(),
        });
    }

    let role = role(roles, symbol, &parameter.constraint.kind)?;
    let actual = role_constraint(role);
    if !constraint_matches(&parameter.constraint, &actual, dimensions) {
        return None;
    }
    Some(FormulaBinding {
        parameter: parameter.id.clone(),
        symbol: symbol.into(),
        constraint: actual,
        evidence: role.evidence.clone(),
    })
}

fn constraint_matches(
    expected: &FormulaConstraint,
    actual: &FormulaConstraint,
    dimensions: &mut BTreeMap<String, String>,
) -> bool {
    if expected.kind != actual.kind
        || !expected
            .refinements
            .iter()
            .all(|refinement| actual.refinements.contains(refinement))
        || expected.dimensions.len() != actual.dimensions.len()
    {
        return false;
    }
    expected
        .dimensions
        .iter()
        .zip(&actual.dimensions)
        .all(|(expected, actual)| {
            if let Some(known) = dimensions.get(expected) {
                known == actual
            } else {
                dimensions.insert(expected.clone(), actual.clone());
                true
            }
        })
}

fn substitute_dimensions(
    constraint: &FormulaConstraint,
    dimensions: &BTreeMap<String, String>,
) -> FormulaConstraint {
    FormulaConstraint {
        kind: constraint.kind.clone(),
        dimensions: constraint
            .dimensions
            .iter()
            .map(|dimension| {
                dimensions
                    .get(dimension)
                    .cloned()
                    .unwrap_or_else(|| dimension.clone())
            })
            .collect(),
        refinements: constraint.refinements.clone(),
    }
}

fn generic_side_conditions_hold(pattern: &PackPattern, bindings: &[FormulaBinding]) -> bool {
    pattern.side_conditions.iter().all(|condition| {
        let binding = bindings
            .iter()
            .find(|binding| binding.parameter == condition.left);
        match condition.kind.as_str() {
            "explicit-role" => binding.is_some_and(|binding| {
                binding.constraint.kind == condition.right
                    && matches!(
                        binding.evidence.kind.as_str(),
                        "explicit-math" | "explicit-prose"
                    )
            }),
            "positive-probability" => binding.is_some_and(|binding| {
                binding
                    .constraint
                    .refinements
                    .iter()
                    .any(|refinement| refinement == "positive-probability")
            }),
            "dimension-equality" | "presentation-safe" => true,
            _ => false,
        }
    })
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
        description: pattern.description.clone(),
        description_key: pattern.description_key.clone(),
        maturity: pattern.maturity.as_str().into(),
        status: "verified".into(),
        pack_id: pattern.pack_id.clone(),
        pack_version: pattern.pack_version.clone(),
        range,
        bindings,
        result: result.constraint(),
        conditions: verified_conditions(pattern),
        evidence,
        rank: 100,
    })
}

fn role_recognition(
    pattern_id: &str,
    range: SourceRange,
    facts: Vec<(&str, &RoleInfo)>,
    result: FormulaConstraint,
) -> Option<FormulaRecognition> {
    let pattern = pattern(pattern_id)?;
    let bindings = facts
        .iter()
        .map(|(parameter, role)| FormulaBinding {
            parameter: (*parameter).into(),
            symbol: role.symbol.clone(),
            constraint: role_constraint(role),
            evidence: role.evidence.clone(),
        })
        .collect::<Vec<_>>();
    let mut source_ranges = facts
        .iter()
        .flat_map(|(_, role)| role.evidence.source_ranges.iter().cloned())
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
        description: pattern.description.clone(),
        description_key: pattern.description_key.clone(),
        maturity: pattern.maturity.as_str().into(),
        status: "verified".into(),
        pack_id: pattern.pack_id.clone(),
        pack_version: pattern.pack_version.clone(),
        range,
        bindings,
        result,
        conditions: verified_conditions(pattern),
        evidence,
        rank: 100,
    })
}

fn verified_conditions(pattern: &PackPattern) -> Vec<FormulaConditionInfo> {
    pattern
        .side_conditions
        .iter()
        .zip(&pattern.condition_descriptions)
        .map(|(condition, label)| FormulaConditionInfo {
            kind: condition.kind.clone(),
            label: label.clone(),
            status: "verified".into(),
        })
        .collect()
}

fn role_constraint(role: &RoleInfo) -> FormulaConstraint {
    FormulaConstraint {
        kind: role.role.clone(),
        dimensions: Vec::new(),
        refinements: if role.role == "event" && has_positive_probability_evidence(role) {
            vec!["positive-probability".into()]
        } else {
            Vec::new()
        },
    }
}

fn known_by_symbol(known: Vec<KnownShape>) -> BTreeMap<String, KnownShape> {
    known
        .into_iter()
        .map(|fact| (fact.symbol.clone(), fact))
        .collect()
}

fn roles_by_symbol(roles: Vec<RoleInfo>) -> BTreeMap<String, Vec<RoleInfo>> {
    let mut by_symbol = BTreeMap::<String, Vec<RoleInfo>>::new();
    for role in roles {
        by_symbol.entry(role.symbol.clone()).or_default().push(role);
    }
    by_symbol
}

fn role<'a>(
    roles: &'a BTreeMap<String, Vec<RoleInfo>>,
    symbol: &str,
    expected: &str,
) -> Option<&'a RoleInfo> {
    roles.get(symbol)?.iter().find(|role| role.role == expected)
}

fn pattern(id: &str) -> Option<&'static PackPattern> {
    built_in_packs()
        .iter()
        .flat_map(|pack| &pack.patterns)
        .find(|pattern| pattern.id == id)
}

fn pattern_order(id: &str) -> usize {
    built_in_packs()
        .iter()
        .flat_map(|pack| &pack.patterns)
        .position(|pattern| pattern.id == id)
        .unwrap_or(usize::MAX)
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

fn trimmed_match(content: &str) -> Option<regex::Match<'_>> {
    TRIMMED_EXPRESSION
        .captures(content)
        .and_then(|captures| captures.get(1))
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
    use crate::consistency::{ConsistencyAnalysis, analyze_consistency};
    use crate::parser::{math_regions, parse_regions};
    use crate::prose::analyze_prose;
    use crate::shape::analyze_shapes;
    use crate::{DocumentLanguage, ProjectDocument};
    use serde::Deserialize;

    fn analyze(
        source: &str,
    ) -> (
        ProjectDocument,
        Vec<crate::parser::ParsedMath>,
        crate::shape::ShapeAnalysis,
        ConsistencyAnalysis,
    ) {
        analyze_language(source, DocumentLanguage::Latex)
    }

    fn analyze_language(
        source: &str,
        language: DocumentLanguage,
    ) -> (
        ProjectDocument,
        Vec<crate::parser::ParsedMath>,
        crate::shape::ShapeAnalysis,
        ConsistencyAnalysis,
    ) {
        let regions = math_regions(source, language);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: if language == DocumentLanguage::Markdown {
                "main.md".into()
            } else {
                "main.tex".into()
            },
            language,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = analyze_prose(&document, &parsed);
        let shapes = analyze_shapes(&document, &parsed, &prose.shapes);
        let consistency = analyze_consistency(&document, &prose.definitions, &shapes);
        (document, parsed, shapes, consistency)
    }

    #[test]
    fn recognizes_typed_linear_algebra_formulas() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}$\n$y = Ax$";
        let (document, parsed, shapes, consistency) = analyze(source);
        let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
        let offset = source.rfind("Ax").unwrap() as u32;
        let recognized = formulas.at(offset);
        assert_eq!(recognized[0].pattern_id, "matrix-vector-product");
        assert_eq!(recognized[0].result.kind, "vector");
    }

    #[test]
    fn completes_a_typed_formula_slot_from_known_symbols() {
        let source = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{n}, y \\in \\mathbb{R}^{m}$\n$y = $";
        let (document, parsed, shapes, consistency) = analyze(source);
        let offset = source.rfind(" = ").unwrap() as u32 + 3;
        let completions = formula_completions(&document, &parsed, &shapes, &consistency, offset);
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
        let (document, parsed, shapes, consistency) = analyze(source);
        let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
        assert_eq!(
            formulas.at(source.find("x^{\\top}y").unwrap() as u32)[0].pattern_id,
            "vector-inner-product"
        );
        assert_eq!(
            formulas.at(source.rfind("x^{\\top}Ax").unwrap() as u32)[0].pattern_id,
            "quadratic-form"
        );
    }

    #[test]
    fn recognizes_calculus_optimization_and_discrete_catalog_entries() {
        for (source, expected_pack, expected_pattern) in [
            (
                "$\\frac{d f}{d x}$",
                "calculus-analysis",
                "ordinary-derivative",
            ),
            ("$\\argmin_{x} f(x)$", "optimization-ml", "argmin-objective"),
            ("$x \\in S$", "discrete-math", "set-membership"),
        ] {
            let (document, parsed, shapes, consistency) =
                analyze_language(source, DocumentLanguage::Markdown);
            let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
            let recognition = formulas.all().first().unwrap_or_else(|| {
                panic!("expected {expected_pack}/{expected_pattern} for {source}")
            });
            assert_eq!(recognition.pack_id, expected_pack);
            assert_eq!(recognition.pattern_id, expected_pattern);
            assert_eq!(recognition.rank, 50);
        }
    }

    #[test]
    fn refuses_unfinished_recognition_only_catalog_entries() {
        for source in ["$\\frac{d f}{d $", "$\\argmin_{x$", "$x \\in $"] {
            let (document, parsed, shapes, consistency) =
                analyze_language(source, DocumentLanguage::Markdown);
            assert!(
                analyze_formulas(&document, &parsed, &shapes, &consistency)
                    .all()
                    .is_empty(),
                "unexpected recognition for {source}"
            );
        }
    }

    #[test]
    fn completes_probability_formulas_from_explicit_visible_roles() {
        let source = "Let $A$ denote an event.\nLet $B$ denote an event of positive probability.\nLet $X$ denote a random variable.\n$p \\in \\mathbb{R}$\n$p = $";
        let (document, parsed, shapes, consistency) =
            analyze_language(source, DocumentLanguage::Markdown);
        let offset = source.rfind(" = ").unwrap() as u32 + 3;
        let completions = formula_completions(&document, &parsed, &shapes, &consistency, offset);
        let titles = completions
            .iter()
            .map(|completion| completion.title.as_str())
            .collect::<Vec<_>>();

        assert!(titles.contains(&"\\mathbb{P}(A)"));
        assert!(titles.contains(&"\\mathbb{P}(A \\mid B)"));
        assert!(titles.contains(&"\\mathbb{E}[X]"));
        assert!(titles.contains(&"\\operatorname{Var}(X)"));
        assert!(!titles.contains(&"\\mathbb{P}(B \\mid A)"));
        assert!(
            completions
                .iter()
                .all(|completion| completion.proposal.safety == "review-required")
        );
    }

    #[test]
    fn does_not_offer_probability_formulas_for_a_derived_scalar_target() {
        let source = "Let $X$ denote a random variable.\n$x \\in \\mathbb{R}^{n}, y \\in \\mathbb{R}^{n}$\n$s = x^{\\top}y$\n$s = $";
        let (document, parsed, shapes, consistency) =
            analyze_language(source, DocumentLanguage::Markdown);
        let offset = source.rfind(" = ").unwrap() as u32 + 3;
        let completions = formula_completions(&document, &parsed, &shapes, &consistency, offset);

        assert!(completions.iter().all(|completion| {
            !matches!(
                completion.pattern_id.as_str(),
                "event-probability" | "conditional-probability" | "expectation" | "variance"
            )
        }));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        false_positive_budget: usize,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        id: String,
        content: String,
        expected_patterns: Vec<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PackCorpus {
        false_positive_budget: usize,
        cases: Vec<PackCase>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PackCase {
        id: String,
        content: String,
        expected_pattern: String,
    }

    #[test]
    fn matches_the_labeled_probability_formula_corpus() {
        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../fixtures/v0.6/probability-formula-corpus.json"
        ))
        .unwrap();
        assert_eq!(corpus.false_positive_budget, 0);

        for case in corpus.cases {
            let (document, parsed, shapes, consistency) =
                analyze_language(&case.content, DocumentLanguage::Markdown);
            let formulas = analyze_formulas(&document, &parsed, &shapes, &consistency);
            let actual = formulas
                .all()
                .iter()
                .map(|formula| formula.pattern_id.clone())
                .collect::<Vec<_>>();
            assert_eq!(actual, case.expected_patterns, "corpus case {}", case.id);
        }
    }

    #[test]
    fn every_recognition_only_pack_entry_has_positive_and_unfinished_coverage() {
        use std::collections::BTreeSet;

        use crate::{PackMaturity, built_in_packs};

        let corpus: PackCorpus = serde_json::from_str(include_str!(
            "../../../fixtures/v0.11/domain-pack-recognition-corpus.json"
        ))
        .unwrap();
        assert_eq!(corpus.false_positive_budget, 0);
        let expected = built_in_packs()
            .iter()
            .flat_map(|pack| &pack.patterns)
            .filter(|pattern| pattern.maturity == PackMaturity::Recognition)
            .map(|pattern| pattern.id.as_str())
            .collect::<BTreeSet<_>>();
        let covered = corpus
            .cases
            .iter()
            .map(|case| case.expected_pattern.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(covered, expected, "corpus must account for every entry");

        for case in corpus.cases {
            let (document, parsed, shapes, consistency) =
                analyze_language(&case.content, DocumentLanguage::Markdown);
            let actual = analyze_formulas(&document, &parsed, &shapes, &consistency)
                .all()
                .iter()
                .map(|recognition| recognition.pattern_id.clone())
                .collect::<Vec<_>>();
            assert!(
                actual.contains(&case.expected_pattern),
                "positive corpus case {}: expected {}, got {actual:?}",
                case.id,
                case.expected_pattern
            );

            let expression = case
                .content
                .strip_prefix('$')
                .and_then(|value| value.strip_suffix('$'))
                .expect("pack corpus uses inline math");
            let unfinished = format!("${expression}{{$");
            let (document, parsed, shapes, consistency) =
                analyze_language(&unfinished, DocumentLanguage::Markdown);
            let adversarial = analyze_formulas(&document, &parsed, &shapes, &consistency);
            assert!(
                adversarial
                    .all()
                    .iter()
                    .all(|recognition| recognition.pattern_id != case.expected_pattern),
                "unfinished corpus case {} still recognized {}",
                case.id,
                case.expected_pattern
            );
        }
    }
}
