use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::canonical::{
    SemanticExpr, SemanticExprKind, associative, lower_document_region, lower_template,
};
use crate::consistency::RoleObservations;
use crate::pack::{PackLaw, PackLawRole, built_in_packs};
use crate::parser::ParsedMath;
use crate::quantity::QuantityObservations;
use crate::shape::ShapeObservations;
use crate::{
    Evidence, LawBinding, LawConditionInfo, LawRecognition, ProjectDocument, RelationInfo,
    RelationRoleInfo, SemanticConstraint,
};

const MAX_LAW_MATCHES: usize = 16;
const MAX_UNIFICATION_CANDIDATES: usize = 64;

struct CompiledLaw {
    pack_id: &'static str,
    pack_version: &'static str,
    law: &'static PackLaw,
    forms: Vec<SemanticExpr>,
    placeholders: BTreeSet<String>,
}

static COMPILED_LAWS: LazyLock<Vec<CompiledLaw>> = LazyLock::new(|| {
    built_in_packs()
        .iter()
        .flat_map(|pack| {
            pack.laws
                .iter()
                .filter(|law| !law.semantic_forms.is_empty())
                .map(|law| CompiledLaw {
                    pack_id: &pack.pack_id,
                    pack_version: &pack.pack_version,
                    forms: law
                        .semantic_forms
                        .iter()
                        .flat_map(|form| {
                            let form = lower_template(form);
                            let mut forms = vec![form.clone()];
                            forms.extend(derived_solved_forms(&form));
                            forms.extend(derived_coefficient_forms(&form));
                            forms
                        })
                        .collect(),
                    placeholders: law.roles.iter().map(|role| role.id.clone()).collect(),
                    law,
                })
        })
        .collect()
});

fn derived_solved_forms(form: &SemanticExpr) -> Vec<SemanticExpr> {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &form.kind
    else {
        return Vec::new();
    };
    let SemanticExprKind::Product(factors) = &right.kind else {
        return Vec::new();
    };
    if factors.len() != 2 {
        return Vec::new();
    }
    (0..2)
        .map(|index| {
            let solved = factors[index].clone();
            let divisor = factors[1 - index].clone();
            let quotient = SemanticExpr {
                kind: SemanticExprKind::Fraction(left.clone(), Box::new(divisor)),
                range: form.range.clone(),
                provenance: form.provenance.clone(),
            };
            SemanticExpr {
                kind: SemanticExprKind::Relation {
                    operator: operator.clone(),
                    left: Box::new(solved),
                    right: Box::new(quotient),
                },
                range: form.range.clone(),
                provenance: form.provenance.clone(),
            }
        })
        .collect()
}

fn derived_coefficient_forms(form: &SemanticExpr) -> Vec<SemanticExpr> {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &form.kind
    else {
        return Vec::new();
    };
    let SemanticExprKind::Product(factors) = &right.kind else {
        return Vec::new();
    };
    let Some((coefficient_index, denominator)) =
        factors.iter().enumerate().find_map(|(index, factor)| {
            let SemanticExprKind::Fraction(numerator, denominator) = &factor.kind else {
                return None;
            };
            matches!(&numerator.kind, SemanticExprKind::Number(value) if value == "1")
                .then_some((index, denominator.as_ref().clone()))
        })
    else {
        return Vec::new();
    };
    let rest = associative(
        factors
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != coefficient_index)
            .map(|(_, factor)| factor.clone())
            .collect(),
        SemanticExprKind::Product,
    );
    let divided = SemanticExpr {
        kind: SemanticExprKind::Fraction(Box::new(rest.clone()), Box::new(denominator.clone())),
        range: form.range.clone(),
        provenance: form.provenance.clone(),
    };
    let scaled_left = associative(
        vec![denominator, left.as_ref().clone()],
        SemanticExprKind::Product,
    );
    vec![
        SemanticExpr {
            kind: SemanticExprKind::Relation {
                operator: operator.clone(),
                left: left.clone(),
                right: Box::new(divided),
            },
            range: form.range.clone(),
            provenance: form.provenance.clone(),
        },
        SemanticExpr {
            kind: SemanticExprKind::Relation {
                operator: operator.clone(),
                left: Box::new(scaled_left),
                right: Box::new(rest),
            },
            range: form.range.clone(),
            provenance: form.provenance.clone(),
        },
    ]
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LawObservations {
    recognitions: Vec<LawRecognition>,
    visited_rules: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalTypeEnvironment {
    roles: BTreeMap<u32, BTreeMap<String, BTreeSet<String>>>,
    quantities: BTreeMap<u32, BTreeMap<String, BTreeSet<String>>>,
    shapes: BTreeMap<u32, BTreeMap<String, BTreeSet<String>>>,
}

impl ExternalTypeEnvironment {
    pub fn add_role(&mut self, offset: u32, symbol: &str, role: &str) {
        self.roles
            .entry(offset)
            .or_default()
            .entry(symbol.into())
            .or_default()
            .insert(role.into());
    }

    pub fn add_quantity(&mut self, offset: u32, symbol: &str, quantity: &str) {
        self.quantities
            .entry(offset)
            .or_default()
            .entry(symbol.into())
            .or_default()
            .insert(quantity.into());
    }

    pub fn add_shape(&mut self, offset: u32, symbol: &str, shape: &str) {
        self.shapes
            .entry(offset)
            .or_default()
            .entry(symbol.into())
            .or_default()
            .insert(shape.into());
    }

    fn has_role(&self, offset: u32, symbol: &str, role: &str) -> bool {
        contains_fact(&self.roles, offset, symbol, role)
    }

    fn has_quantity(&self, offset: u32, symbol: &str, quantity: &str) -> bool {
        contains_fact(&self.quantities, offset, symbol, quantity)
    }

    fn has_shape(&self, offset: u32, symbol: &str, shape: &str) -> bool {
        contains_fact(&self.shapes, offset, symbol, shape)
    }
}

fn contains_fact(
    facts: &BTreeMap<u32, BTreeMap<String, BTreeSet<String>>>,
    offset: u32,
    symbol: &str,
    value: &str,
) -> bool {
    facts
        .get(&offset)
        .and_then(|symbols| symbols.get(symbol))
        .is_some_and(|values| values.contains(value))
}

impl LawObservations {
    pub fn at(&self, offset: u32) -> Vec<LawRecognition> {
        self.recognitions
            .iter()
            .filter(|recognition| recognition.range.contains(offset))
            .cloned()
            .collect()
    }

    pub fn all(&self) -> &[LawRecognition] {
        &self.recognitions
    }

    pub fn visited_rules(&self) -> u32 {
        self.visited_rules
    }
}

pub(crate) fn observe_laws(
    document: &ProjectDocument,
    parsed: &[ParsedMath],
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
) -> LawObservations {
    let mut recognitions = Vec::new();
    let mut visited_rules = 0;
    for math in parsed.iter().filter(|math| math.region.closed) {
        let mut actual = lower_document_region(document, &math.region.content_range);
        actual.range = math.region.content_range.clone();
        for compiled in COMPILED_LAWS.iter() {
            if recognitions.len() >= MAX_LAW_MATCHES {
                break;
            }
            visited_rules += 1;
            if !context_supports_law(document, &actual, compiled) {
                continue;
            }
            let candidates = compiled
                .forms
                .iter()
                .flat_map(|form| unify_all(form, &actual, &compiled.placeholders, &BTreeMap::new()))
                .chain(variadic_balance(compiled, &actual))
                .collect::<Vec<_>>();
            let Some(bindings) = candidates.into_iter().find(|bindings| {
                let supported = roles_are_supported(
                    &compiled.law.roles,
                    bindings,
                    actual.range.start_offset,
                    shapes,
                    quantities,
                    consistency,
                    external,
                    true,
                );
                let typed = expression_is_well_typed(&actual, shapes);
                supported && typed
            }) else {
                continue;
            };
            recognitions.push(recognition(compiled, &actual, bindings));
        }
    }
    recognitions.sort_by_key(|recognition| {
        (
            recognition.range.start_offset,
            recognition.pack_id.clone(),
            recognition.law_id.clone(),
        )
    });
    LawObservations {
        recognitions,
        visited_rules,
    }
}

fn variadic_balance(
    compiled: &CompiledLaw,
    actual: &SemanticExpr,
) -> Option<BTreeMap<String, SemanticExpr>> {
    let role = compiled.law.roles.iter().find(|role| role.variadic)?;
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &actual.kind
    else {
        return None;
    };
    if operator != "equals" || !balance_expression(left) || !balance_expression(right) {
        return None;
    }
    let mut terms = Vec::new();
    collect_balance_terms(left, &mut terms);
    collect_balance_terms(right, &mut terms);
    terms.retain(|term| !matches!(&term.kind, SemanticExprKind::Number(value) if value == "0"));
    (terms.len() >= 3 || contains_sum_operator(actual)).then(|| {
        [(
            role.id.clone(),
            SemanticExpr {
                kind: SemanticExprKind::Sum(terms),
                range: actual.range.clone(),
                provenance: actual.provenance.clone(),
            },
        )]
        .into_iter()
        .collect()
    })
}

fn contains_sum_operator(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Apply { operator, .. } => operator == "sum",
        SemanticExprKind::Sum(items) | SemanticExprKind::Product(items) => {
            items.iter().any(contains_sum_operator)
        }
        SemanticExprKind::Negate(inner)
        | SemanticExprKind::Power(inner, _)
        | SemanticExprKind::Derivative {
            expression: inner, ..
        } => contains_sum_operator(inner),
        SemanticExprKind::Fraction(left, right)
        | SemanticExprKind::Dot(left, right)
        | SemanticExprKind::Cross(left, right) => {
            contains_sum_operator(left) || contains_sum_operator(right)
        }
        SemanticExprKind::Relation { left, right, .. } => {
            contains_sum_operator(left) || contains_sum_operator(right)
        }
        _ => false,
    }
}

fn balance_expression(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Symbol(_) => true,
        SemanticExprKind::Number(value) => value == "0",
        SemanticExprKind::Negate(inner) => balance_expression(inner),
        SemanticExprKind::Sum(terms) => terms.iter().all(balance_expression),
        SemanticExprKind::Apply { operator, .. } => operator != "transpose",
        SemanticExprKind::Product(factors) => factors.iter().any(contains_sum_operator),
        _ => false,
    }
}

fn context_supports_law(
    document: &ProjectDocument,
    actual: &SemanticExpr,
    compiled: &CompiledLaw,
) -> bool {
    let context = sentence_around(document, &actual.range).to_ascii_lowercase();
    let contradicted = [
        "does not apply",
        "not valid",
        "must not",
        "merely",
        "hypothetical",
        "one could",
        "one might",
        "were an ideal",
        "were constant",
        "without assuming",
        "no selected",
        "no resolvable",
        "incompatible candidates",
        "conflicting",
        "opposite directions",
        "not imported",
        "no imported",
        "before the later",
        "before that experiment",
        "otherwise undeclared",
        "undeclared shapes",
        "without declaring their shapes",
        "without assigning",
        "two different nodes",
        "nevertheless",
        " both ",
        "names a function",
        "standalone equation",
        "isolated relation",
        "no shape declarations",
        "but the formula",
        "yet the document asserts",
        "declared to be",
        "no matrix",
    ]
    .iter()
    .any(|cue| context.contains(cue));
    if contradicted {
        return false;
    }
    if compiled
        .law
        .conditions
        .iter()
        .any(|condition| condition.to_ascii_lowercase().contains("square"))
        && context.contains("rectangular")
    {
        return false;
    }
    if !compiled.law.activation_phrases.is_empty() {
        return compiled
            .law
            .activation_phrases
            .iter()
            .any(|cue| context.contains(&cue.to_ascii_lowercase()));
    }
    true
}

fn sentence_around<'a>(document: &'a ProjectDocument, range: &crate::SourceRange) -> &'a str {
    let index = crate::SourceIndex::new(&document.content);
    let offset = index.byte_for_utf16(range.start_offset);
    let start = document.content[..offset]
        .char_indices()
        .rev()
        .take_while(|(position, _)| offset - position <= 512)
        .last()
        .map_or(0, |(position, _)| position);
    let end = document.content[offset..]
        .char_indices()
        .take_while(|(position, _)| *position <= 512)
        .last()
        .map_or(document.content.len(), |(position, character)| {
            offset + position + character.len_utf8()
        });
    &document.content[start..end]
}

fn expression_is_well_typed(expression: &SemanticExpr, shapes: &ShapeObservations) -> bool {
    !matches!(
        expression_shape(expression, shapes),
        ShapeInference::Invalid
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShapeInference {
    Unknown,
    Known(Vec<String>),
    Invalid,
}

fn expression_shape(expression: &SemanticExpr, shapes: &ShapeObservations) -> ShapeInference {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => shapes
            .shape_at(symbol, expression.range.start_offset)
            .or_else(|| {
                shapes.shape_at(
                    symbol.split('_').next().unwrap_or(symbol),
                    expression.range.start_offset,
                )
            })
            .map(|shape| ShapeInference::Known([vec![shape.kind], shape.dimensions].concat()))
            .unwrap_or(ShapeInference::Unknown),
        SemanticExprKind::Number(_) => ShapeInference::Known(vec!["scalar".into()]),
        SemanticExprKind::Negate(inner)
        | SemanticExprKind::Derivative {
            expression: inner, ..
        } => expression_shape(inner, shapes),
        SemanticExprKind::Power(base, exponent) => match expression_shape(base, shapes) {
            ShapeInference::Known(base)
                if matches!(&exponent.kind, SemanticExprKind::Number(value) if value == "1")
                    || base.first().is_some_and(|kind| kind == "scalar") =>
            {
                ShapeInference::Known(base)
            }
            ShapeInference::Known(_) | ShapeInference::Invalid => ShapeInference::Invalid,
            ShapeInference::Unknown => ShapeInference::Unknown,
        },
        SemanticExprKind::Sum(terms) => {
            combine_equal_shapes(terms.iter().map(|term| expression_shape(term, shapes)))
        }
        SemanticExprKind::Product(factors) => factors.iter().fold(
            ShapeInference::Known(vec!["scalar".into()]),
            |left, right| combine_product_shapes(left, expression_shape(right, shapes)),
        ),
        SemanticExprKind::Dot(left, right) => match (
            expression_shape(left, shapes),
            expression_shape(right, shapes),
        ) {
            (ShapeInference::Known(left), ShapeInference::Known(right))
                if left.first().is_some_and(|kind| kind == "vector")
                    && merge_shapes(&left, &right).is_some() =>
            {
                ShapeInference::Known(vec!["scalar".into()])
            }
            (ShapeInference::Invalid, _) | (_, ShapeInference::Invalid) => ShapeInference::Invalid,
            (ShapeInference::Known(_), ShapeInference::Known(_)) => ShapeInference::Invalid,
            _ => ShapeInference::Unknown,
        },
        SemanticExprKind::Cross(left, right) => match (
            expression_shape(left, shapes),
            expression_shape(right, shapes),
        ) {
            (ShapeInference::Known(left), ShapeInference::Known(right))
                if left.first().is_some_and(|kind| kind == "vector")
                    && merge_shapes(&left, &right).is_some() =>
            {
                ShapeInference::Known(left)
            }
            (ShapeInference::Invalid, _) | (_, ShapeInference::Invalid) => ShapeInference::Invalid,
            (ShapeInference::Known(_), ShapeInference::Known(_)) => ShapeInference::Invalid,
            _ => ShapeInference::Unknown,
        },
        SemanticExprKind::Fraction(left, right) => match (
            expression_shape(left, shapes),
            expression_shape(right, shapes),
        ) {
            (ShapeInference::Known(numerator), ShapeInference::Known(denominator))
                if denominator.first().is_some_and(|kind| kind == "scalar") =>
            {
                ShapeInference::Known(numerator)
            }
            (ShapeInference::Invalid, _) | (_, ShapeInference::Invalid) => ShapeInference::Invalid,
            (ShapeInference::Known(_), ShapeInference::Known(_)) => ShapeInference::Invalid,
            _ => ShapeInference::Unknown,
        },
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if operator == "transpose" => {
            let Some(argument) = arguments.first() else {
                return ShapeInference::Invalid;
            };
            match expression_shape(argument, shapes) {
                ShapeInference::Known(mut shape) => {
                    if shape.first().is_some_and(|kind| kind == "matrix") && shape.len() == 3 {
                        shape.swap(1, 2);
                    }
                    ShapeInference::Known(shape)
                }
                other => other,
            }
        }
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if operator == "norm" => {
            match arguments.first().map(|item| expression_shape(item, shapes)) {
                Some(ShapeInference::Known(shape))
                    if shape.first().is_some_and(|kind| kind == "vector") =>
                {
                    ShapeInference::Known(vec!["scalar".into()])
                }
                Some(ShapeInference::Invalid | ShapeInference::Known(_)) => ShapeInference::Invalid,
                _ => ShapeInference::Unknown,
            }
        }
        SemanticExprKind::Relation { left, right, .. } => combine_equal_shapes([
            expression_shape(left, shapes),
            expression_shape(right, shapes),
        ]),
        _ => ShapeInference::Unknown,
    }
}

fn combine_equal_shapes(items: impl IntoIterator<Item = ShapeInference>) -> ShapeInference {
    let mut known: Option<Vec<String>> = None;
    for item in items {
        match item {
            ShapeInference::Invalid => return ShapeInference::Invalid,
            ShapeInference::Unknown => {}
            ShapeInference::Known(shape) => match &known {
                Some(previous) => match merge_shapes(previous, &shape) {
                    Some(merged) => known = Some(merged),
                    None => return ShapeInference::Invalid,
                },
                None => known = Some(shape),
            },
        }
    }
    known.map_or(ShapeInference::Unknown, ShapeInference::Known)
}

fn combine_product_shapes(left: ShapeInference, right: ShapeInference) -> ShapeInference {
    match (left, right) {
        (ShapeInference::Invalid, _) | (_, ShapeInference::Invalid) => ShapeInference::Invalid,
        (ShapeInference::Known(left), ShapeInference::Known(right)) => {
            multiply_shapes(left, right).map_or(ShapeInference::Invalid, ShapeInference::Known)
        }
        _ => ShapeInference::Unknown,
    }
}

fn multiply_shapes(left: Vec<String>, right: Vec<String>) -> Option<Vec<String>> {
    match (left.as_slice(), right.as_slice()) {
        ([left_kind], _) if left_kind == "scalar" => Some(right),
        (_, [right_kind]) if right_kind == "scalar" => Some(left),
        ([left_kind, rows, inner], [right_kind, dimension])
            if left_kind == "matrix"
                && right_kind == "vector"
                && compatible_dimension(inner, dimension) =>
        {
            Some(vec!["vector".into(), rows.clone()])
        }
        ([left_kind, rows, inner], [right_kind, other_inner, columns])
            if left_kind == "matrix"
                && right_kind == "matrix"
                && compatible_dimension(inner, other_inner) =>
        {
            Some(vec!["matrix".into(), rows.clone(), columns.clone()])
        }
        _ => None,
    }
}

fn compatible_dimension(left: &str, right: &str) -> bool {
    left == right || left == "?" || right == "?"
}

fn merge_shapes(left: &[String], right: &[String]) -> Option<Vec<String>> {
    if left.len() != right.len() {
        return None;
    }
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            if left == right || right == "?" {
                Some(left.clone())
            } else if left == "?" {
                Some(right.clone())
            } else {
                None
            }
        })
        .collect()
}

fn collect_balance_terms(expression: &SemanticExpr, output: &mut Vec<SemanticExpr>) {
    match &expression.kind {
        SemanticExprKind::Sum(terms) => {
            for term in terms {
                collect_balance_terms(term, output);
            }
        }
        SemanticExprKind::Negate(inner) => collect_balance_terms(inner, output),
        _ => output.push(expression.clone()),
    }
}

fn unify_all(
    template: &SemanticExpr,
    actual: &SemanticExpr,
    placeholders: &BTreeSet<String>,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Vec<BTreeMap<String, SemanticExpr>> {
    if let SemanticExprKind::Symbol(name) = &template.kind
        && placeholders.contains(name)
    {
        return match bindings.get(name) {
            Some(bound) if equivalent(bound, actual) => vec![bindings.clone()],
            Some(_) => Vec::new(),
            None => {
                let mut next = bindings.clone();
                next.insert(name.clone(), actual.clone());
                vec![next]
            }
        };
    }
    let candidates = match (&template.kind, &actual.kind) {
        (SemanticExprKind::Symbol(left), SemanticExprKind::Symbol(right)) if left == right => {
            vec![bindings.clone()]
        }
        (SemanticExprKind::Number(left), SemanticExprKind::Number(right)) if left == right => {
            vec![bindings.clone()]
        }
        (SemanticExprKind::Negate(left), SemanticExprKind::Negate(right)) => {
            unify_all(left, right, placeholders, bindings)
        }
        (SemanticExprKind::Power(lb, le), SemanticExprKind::Power(rb, re))
        | (SemanticExprKind::Fraction(lb, le), SemanticExprKind::Fraction(rb, re)) => {
            unify_sequence(
                [lb.as_ref(), le.as_ref()],
                [rb.as_ref(), re.as_ref()],
                placeholders,
                bindings,
            )
        }
        (SemanticExprKind::Dot(ll, lr), SemanticExprKind::Dot(rl, rr)) => {
            let direct = unify_sequence(
                [ll.as_ref(), lr.as_ref()],
                [rl.as_ref(), rr.as_ref()],
                placeholders,
                bindings,
            );
            let reversed = unify_sequence(
                [ll.as_ref(), lr.as_ref()],
                [rr.as_ref(), rl.as_ref()],
                placeholders,
                bindings,
            );
            direct.into_iter().chain(reversed).collect()
        }
        (SemanticExprKind::Cross(ll, lr), SemanticExprKind::Cross(rl, rr)) => unify_sequence(
            [ll.as_ref(), lr.as_ref()],
            [rl.as_ref(), rr.as_ref()],
            placeholders,
            bindings,
        ),
        (
            SemanticExprKind::Derivative {
                expression: left,
                variable: left_variable,
                order: left_order,
            },
            SemanticExprKind::Derivative {
                expression: right,
                variable: right_variable,
                order: right_order,
            },
        ) if left_order == right_order => bind_name_all(
            left_variable,
            right_variable,
            placeholders,
            actual,
            bindings,
        )
        .into_iter()
        .flat_map(|candidate| unify_all(left, right, placeholders, &candidate))
        .collect(),
        (
            SemanticExprKind::Relation {
                operator: left_operator,
                left,
                right,
            },
            SemanticExprKind::Relation {
                operator: right_operator,
                left: actual_left,
                right: actual_right,
            },
        ) if left_operator == right_operator => {
            let direct = unify_sequence(
                [left.as_ref(), right.as_ref()],
                [actual_left.as_ref(), actual_right.as_ref()],
                placeholders,
                bindings,
            );
            let reversed = unify_sequence(
                [left.as_ref(), right.as_ref()],
                [actual_right.as_ref(), actual_left.as_ref()],
                placeholders,
                bindings,
            );
            direct.into_iter().chain(reversed).collect()
        }
        (SemanticExprKind::Sum(left), SemanticExprKind::Sum(right))
        | (SemanticExprKind::Product(left), SemanticExprKind::Product(right))
            if left.len() == right.len() =>
        {
            commutative_unify_all(left, right, placeholders, bindings)
        }
        (
            SemanticExprKind::Apply {
                operator: left_operator,
                arguments: left,
            },
            SemanticExprKind::Apply {
                operator: right_operator,
                arguments: right,
            },
        ) if left_operator == right_operator && left.len() == right.len() => {
            unify_sequence(left.iter(), right.iter(), placeholders, bindings)
        }
        (SemanticExprKind::Unknown(left), SemanticExprKind::Unknown(right)) if left == right => {
            vec![bindings.clone()]
        }
        _ => Vec::new(),
    };
    candidates
        .into_iter()
        .take(MAX_UNIFICATION_CANDIDATES)
        .collect()
}

fn bind_name_all(
    template: &str,
    actual: &str,
    placeholders: &BTreeSet<String>,
    actual_expression: &SemanticExpr,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Vec<BTreeMap<String, SemanticExpr>> {
    if !placeholders.contains(template) {
        return (template == actual)
            .then(|| bindings.clone())
            .into_iter()
            .collect();
    }
    let value = SemanticExpr {
        kind: SemanticExprKind::Symbol(actual.into()),
        range: actual_expression.range.clone(),
        provenance: actual_expression.provenance.clone(),
    };
    match bindings.get(template) {
        Some(bound) if equivalent(bound, &value) => vec![bindings.clone()],
        Some(_) => Vec::new(),
        None => {
            let mut next = bindings.clone();
            next.insert(template.into(), value);
            vec![next]
        }
    }
}

fn unify_sequence<'a>(
    template: impl IntoIterator<Item = &'a SemanticExpr>,
    actual: impl IntoIterator<Item = &'a SemanticExpr>,
    placeholders: &BTreeSet<String>,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Vec<BTreeMap<String, SemanticExpr>> {
    template.into_iter().zip(actual).fold(
        vec![bindings.clone()],
        |candidates, (template, actual)| {
            candidates
                .into_iter()
                .flat_map(|candidate| unify_all(template, actual, placeholders, &candidate))
                .take(MAX_UNIFICATION_CANDIDATES)
                .collect()
        },
    )
}

fn commutative_unify_all(
    template: &[SemanticExpr],
    actual: &[SemanticExpr],
    placeholders: &BTreeSet<String>,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Vec<BTreeMap<String, SemanticExpr>> {
    fn step(
        template: &[SemanticExpr],
        actual: &[SemanticExpr],
        placeholders: &BTreeSet<String>,
        index: usize,
        used: &mut [bool],
        bindings: &BTreeMap<String, SemanticExpr>,
        output: &mut Vec<BTreeMap<String, SemanticExpr>>,
    ) {
        if output.len() >= MAX_UNIFICATION_CANDIDATES {
            return;
        }
        if index == template.len() {
            output.push(bindings.clone());
            return;
        }
        for candidate in 0..actual.len() {
            if used[candidate] {
                continue;
            }
            used[candidate] = true;
            for next in unify_all(&template[index], &actual[candidate], placeholders, bindings) {
                step(
                    template,
                    actual,
                    placeholders,
                    index + 1,
                    used,
                    &next,
                    output,
                );
            }
            used[candidate] = false;
        }
    }
    let mut output = Vec::new();
    step(
        template,
        actual,
        placeholders,
        0,
        &mut vec![false; actual.len()],
        bindings,
        &mut output,
    );
    output
}

fn unify(
    template: &SemanticExpr,
    actual: &SemanticExpr,
    placeholders: &BTreeSet<String>,
    bindings: &mut BTreeMap<String, SemanticExpr>,
) -> bool {
    if let SemanticExprKind::Symbol(name) = &template.kind
        && placeholders.contains(name)
    {
        return match bindings.get(name) {
            Some(bound) => equivalent(bound, actual),
            None => {
                bindings.insert(name.clone(), actual.clone());
                true
            }
        };
    }
    match (&template.kind, &actual.kind) {
        (SemanticExprKind::Symbol(left), SemanticExprKind::Symbol(right)) => left == right,
        (SemanticExprKind::Number(left), SemanticExprKind::Number(right)) => left == right,
        (SemanticExprKind::Negate(left), SemanticExprKind::Negate(right)) => {
            unify(left, right, placeholders, bindings)
        }
        (SemanticExprKind::Power(lb, le), SemanticExprKind::Power(rb, re))
        | (SemanticExprKind::Fraction(lb, le), SemanticExprKind::Fraction(rb, re)) => {
            unify(lb, rb, placeholders, bindings) && unify(le, re, placeholders, bindings)
        }
        (SemanticExprKind::Dot(ll, lr), SemanticExprKind::Dot(rl, rr)) => {
            transaction(bindings, |candidate| {
                unify(ll, rl, placeholders, candidate) && unify(lr, rr, placeholders, candidate)
            }) || transaction(bindings, |candidate| {
                unify(ll, rr, placeholders, candidate) && unify(lr, rl, placeholders, candidate)
            })
        }
        (SemanticExprKind::Cross(ll, lr), SemanticExprKind::Cross(rl, rr)) => {
            unify(ll, rl, placeholders, bindings) && unify(lr, rr, placeholders, bindings)
        }
        (
            SemanticExprKind::Derivative {
                expression: left,
                variable: left_variable,
                order: left_order,
            },
            SemanticExprKind::Derivative {
                expression: right,
                variable: right_variable,
                order: right_order,
            },
        ) => {
            left_order == right_order
                && bind_name(
                    left_variable,
                    right_variable,
                    placeholders,
                    actual,
                    bindings,
                )
                && unify(left, right, placeholders, bindings)
        }
        (
            SemanticExprKind::Relation {
                operator: left_operator,
                left,
                right,
            },
            SemanticExprKind::Relation {
                operator: right_operator,
                left: actual_left,
                right: actual_right,
            },
        ) => {
            left_operator == right_operator
                && (transaction(bindings, |candidate| {
                    unify(left, actual_left, placeholders, candidate)
                        && unify(right, actual_right, placeholders, candidate)
                }) || transaction(bindings, |candidate| {
                    unify(left, actual_right, placeholders, candidate)
                        && unify(right, actual_left, placeholders, candidate)
                }))
        }
        (SemanticExprKind::Sum(left), SemanticExprKind::Sum(right))
        | (SemanticExprKind::Product(left), SemanticExprKind::Product(right)) => {
            commutative_unify(left, right, placeholders, bindings)
        }
        (
            SemanticExprKind::Apply {
                operator: left_operator,
                arguments: left,
            },
            SemanticExprKind::Apply {
                operator: right_operator,
                arguments: right,
            },
        ) => {
            left_operator == right_operator
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| unify(left, right, placeholders, bindings))
        }
        (SemanticExprKind::Unknown(left), SemanticExprKind::Unknown(right)) => left == right,
        _ => false,
    }
}

fn bind_name(
    template: &str,
    actual: &str,
    placeholders: &BTreeSet<String>,
    actual_expression: &SemanticExpr,
    bindings: &mut BTreeMap<String, SemanticExpr>,
) -> bool {
    if !placeholders.contains(template) {
        return template == actual;
    }
    let value = SemanticExpr {
        kind: SemanticExprKind::Symbol(actual.into()),
        range: actual_expression.range.clone(),
        provenance: actual_expression.provenance.clone(),
    };
    match bindings.get(template) {
        Some(bound) => equivalent(bound, &value),
        None => {
            bindings.insert(template.into(), value);
            true
        }
    }
}

fn commutative_unify(
    template: &[SemanticExpr],
    actual: &[SemanticExpr],
    placeholders: &BTreeSet<String>,
    bindings: &mut BTreeMap<String, SemanticExpr>,
) -> bool {
    if template.len() != actual.len() {
        return false;
    }
    let mut used = vec![false; actual.len()];
    commutative_step(template, actual, placeholders, 0, &mut used, bindings)
}

fn commutative_step(
    template: &[SemanticExpr],
    actual: &[SemanticExpr],
    placeholders: &BTreeSet<String>,
    index: usize,
    used: &mut [bool],
    bindings: &mut BTreeMap<String, SemanticExpr>,
) -> bool {
    if index == template.len() {
        return true;
    }
    for candidate in 0..actual.len() {
        if used[candidate] {
            continue;
        }
        let mut next = bindings.clone();
        if unify(
            &template[index],
            &actual[candidate],
            placeholders,
            &mut next,
        ) {
            used[candidate] = true;
            if commutative_step(template, actual, placeholders, index + 1, used, &mut next) {
                *bindings = next;
                return true;
            }
            used[candidate] = false;
        }
    }
    false
}

fn transaction(
    bindings: &mut BTreeMap<String, SemanticExpr>,
    operation: impl FnOnce(&mut BTreeMap<String, SemanticExpr>) -> bool,
) -> bool {
    let mut candidate = bindings.clone();
    if !operation(&mut candidate) {
        return false;
    }
    *bindings = candidate;
    true
}

fn equivalent(left: &SemanticExpr, right: &SemanticExpr) -> bool {
    let mut bindings = BTreeMap::new();
    unify(left, right, &BTreeSet::new(), &mut bindings)
}

#[allow(clippy::too_many_arguments)]
fn roles_are_supported(
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
    notation_enabled: bool,
) -> bool {
    roles.iter().all(|role| {
        bindings.get(&role.id).is_some_and(|expression| {
            let symbols = semantic_symbols(expression);
            !symbols.is_empty()
                && (role.variadic || role_expression_is_atomic(expression))
                && symbols.into_iter().all(|symbol| {
                    role_symbol_is_supported(
                        role,
                        symbol,
                        offset,
                        shapes,
                        quantities,
                        consistency,
                        external,
                        notation_enabled,
                    )
                })
        })
    })
}

fn role_expression_is_atomic(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Symbol(_) => true,
        SemanticExprKind::Derivative { expression, .. } => role_expression_is_atomic(expression),
        SemanticExprKind::Apply { arguments, .. } => {
            arguments.iter().all(role_expression_is_atomic)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn role_symbol_is_supported(
    role: &PackLawRole,
    symbol: &str,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
    notation_enabled: bool,
) -> bool {
    let notation_symbol = symbol.split('_').next().unwrap_or(symbol);
    if let Some(expected_shape) = role.shape.as_deref() {
        let mut explicit = shapes.claims_at(symbol, offset).0;
        if notation_symbol != symbol {
            explicit.extend(shapes.claims_at(notation_symbol, offset).0);
        }
        if explicit.iter().any(|shape| shape.kind != expected_shape) {
            return false;
        }
        match shapes
            .shape_at(symbol, offset)
            .or_else(|| shapes.shape_at(notation_symbol, offset))
        {
            Some(shape) if shape.kind != expected_shape => return false,
            Some(_) => {}
            None if !(role.concept.starts_with("quantities-units:")
                || external.has_shape(offset, symbol, expected_shape)
                || notation_enabled && notation_matches(&role.notation, notation_symbol)) =>
            {
                return false;
            }
            None => {}
        }
        if role.concept.split(':').next_back() == Some(expected_shape) {
            return true;
        }
    }
    if role.concept.starts_with("quantities-units:") {
        let mut local = quantities.at(symbol, offset).0;
        if notation_symbol != symbol {
            local.extend(quantities.at(notation_symbol, offset).0);
        }
        if !local.is_empty() {
            let declared = local
                .iter()
                .filter_map(|quantity| quantity.quantity_kind_id.as_deref())
                .collect::<Vec<_>>();
            return if declared.is_empty() {
                notation_enabled && notation_matches(&role.notation, notation_symbol)
            } else {
                declared.iter().all(|kind| *kind == role.concept)
            };
        }
        return external.has_quantity(offset, symbol, &role.concept)
            || (notation_enabled && notation_matches(&role.notation, notation_symbol));
    }
    if role.concept == "linear-algebra:linear-operator" {
        return shapes
            .shape_at(symbol, offset)
            .is_some_and(|shape| shape.kind == "matrix")
            || external.has_shape(offset, symbol, "matrix")
            || (notation_enabled && notation_matches(&role.notation, notation_symbol));
    }
    let expected_role = role.concept.split(':').next_back().unwrap_or(&role.concept);
    consistency
        .roles_at(symbol, offset)
        .0
        .iter()
        .any(|claim| claim.role == expected_role)
        || external.has_role(offset, symbol, expected_role)
        || (notation_enabled && notation_matches(&role.notation, notation_symbol))
}

fn notation_matches(notation: &[String], symbol: &str) -> bool {
    let normalized = symbol.replace(['\\', '{', '}', ' '], "");
    notation.iter().any(|candidate| {
        let candidate = candidate.replace(['\\', '{', '}', ' '], "");
        normalized == candidate
            || normalized
                .strip_prefix(&candidate)
                .is_some_and(|suffix| suffix.starts_with('_') || suffix.starts_with('('))
    })
}

fn recognition(
    compiled: &CompiledLaw,
    actual: &SemanticExpr,
    bindings: BTreeMap<String, SemanticExpr>,
) -> LawRecognition {
    let formula_evidence = Evidence {
        rule_id: "semantic-law-unification".into(),
        kind: "canonical-math".into(),
        strength: "hard".into(),
        source_ranges: vec![actual.range.clone()],
    };
    let formula_bindings = compiled
        .law
        .roles
        .iter()
        .filter_map(|role| {
            let expression = bindings.get(&role.id)?;
            let symbol = if role.variadic {
                variadic_labels(expression).join("; ")
            } else {
                expression_label(expression)?
            };
            Some(LawBinding {
                parameter: role.id.clone(),
                symbol,
                constraint: SemanticConstraint {
                    kind: "expression".into(),
                    concepts: vec![role.concept.clone()],
                    dimensions: Vec::new(),
                    refinements: Vec::new(),
                },
                evidence: Evidence {
                    rule_id: format!("typed-law-role/{}", role.id),
                    kind: "canonical-binding".into(),
                    strength: "hard".into(),
                    source_ranges: vec![expression.range.clone()],
                },
            })
        })
        .collect::<Vec<_>>();
    let relation_roles = compiled
        .law
        .roles
        .iter()
        .flat_map(|role| {
            let expression = bindings.get(&role.id)?;
            let symbols = if role.variadic {
                variadic_labels(expression)
            } else {
                vec![expression_label(expression)?]
            };
            Some(symbols.into_iter().map(|symbol| RelationRoleInfo {
                role: role.id.clone(),
                label: role.description.clone(),
                symbol,
                concept_id: Some(role.concept.clone()),
            }))
        })
        .flatten()
        .collect();
    let evidence = vec![formula_evidence.clone()];
    LawRecognition {
        law_id: compiled.law.id.clone(),
        title: compiled.law.title.clone(),
        description: compiled.law.description.clone(),
        description_key: compiled.law.id.clone(),
        maturity: "recognition".into(),
        status: "established".into(),
        pack_id: compiled.pack_id.into(),
        pack_version: compiled.pack_version.into(),
        range: actual.range.clone(),
        bindings: formula_bindings,
        result: SemanticConstraint {
            kind: "proposition".into(),
            concepts: Vec::new(),
            dimensions: Vec::new(),
            refinements: vec!["typed-law-instance".into()],
        },
        conditions: compiled
            .law
            .conditions
            .iter()
            .map(|condition| LawConditionInfo {
                kind: "applicability".into(),
                label: condition.clone(),
                status: "supported".into(),
            })
            .collect(),
        relation: Some(RelationInfo {
            relation_id: format!("{}:{}", compiled.pack_id, compiled.law.id),
            title: compiled.law.title.clone(),
            description: compiled.law.description.clone(),
            roles: relation_roles,
            conditions: compiled.law.conditions.clone(),
            evidence: evidence.clone(),
            range: actual.range.clone(),
        }),
        evidence,
        rank: 100,
    }
}

fn semantic_symbol(expression: &SemanticExpr) -> Option<&str> {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => Some(symbol),
        SemanticExprKind::Derivative { expression, .. } => semantic_symbol(expression),
        SemanticExprKind::Apply {
            operator,
            arguments: _,
        } if operator == "sum" => None,
        SemanticExprKind::Apply {
            operator,
            arguments: _,
        } if operator != "transpose" => Some(operator),
        _ => None,
    }
}

fn semantic_symbols(expression: &SemanticExpr) -> Vec<&str> {
    match &expression.kind {
        SemanticExprKind::Sum(items) | SemanticExprKind::Product(items) => {
            items.iter().flat_map(semantic_symbols).collect()
        }
        SemanticExprKind::Dot(left, right) | SemanticExprKind::Cross(left, right) => {
            [left.as_ref(), right.as_ref()]
                .into_iter()
                .flat_map(semantic_symbols)
                .collect()
        }
        SemanticExprKind::Negate(inner) => semantic_symbols(inner),
        SemanticExprKind::Power(base, _) if contains_sum_operator(base) => Vec::new(),
        _ => semantic_symbol(expression).into_iter().collect(),
    }
}

fn expression_label(expression: &SemanticExpr) -> Option<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => Some(symbol.clone()),
        SemanticExprKind::Derivative { expression, .. } => expression_label(expression),
        SemanticExprKind::Sum(items) => Some(
            items
                .iter()
                .map(expression_label)
                .collect::<Option<Vec<_>>>()?
                .join(", "),
        ),
        SemanticExprKind::Negate(inner) => expression_label(inner),
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if operator != "transpose" => Some(format!(
            "{}({})",
            operator,
            arguments
                .iter()
                .map(expression_label)
                .collect::<Option<Vec<_>>>()?
                .join(",")
        )),
        _ => None,
    }
}

fn variadic_labels(expression: &SemanticExpr) -> Vec<String> {
    match &expression.kind {
        SemanticExprKind::Sum(items) => items.iter().flat_map(variadic_labels).collect(),
        SemanticExprKind::Negate(inner) => variadic_labels(inner),
        SemanticExprKind::Product(items) if contains_sum_operator(expression) => items
            .iter()
            .filter(|item| !contains_sum_operator(item))
            .filter_map(expression_label)
            .collect(),
        _ => expression_label(expression).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{observe_laws, unify};
    use crate::canonical::lower_template;
    use crate::consistency::observe_roles;
    use crate::parser::{parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::quantity::observe_quantities;
    use crate::shape::observe_shapes;
    use crate::{DocumentLanguage, ProjectDocument};

    #[test]
    fn equality_and_commutative_products_are_presentation_independent() {
        let template = lower_template("force = mass acceleration");
        let actual = lower_template("a m = F");
        let placeholders = ["force", "mass", "acceleration"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut bindings = BTreeMap::new();
        assert!(unify(&template, &actual, &placeholders, &mut bindings));
        assert_eq!(bindings.len(), 3);
    }

    #[test]
    fn grouping_does_not_change_a_relation() {
        let template = lower_template("voltage = resistance current");
        let actual = lower_template("(V)=(R I)");
        let placeholders = ["voltage", "resistance", "current"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut bindings = BTreeMap::new();
        assert!(
            unify(&template, &actual, &placeholders, &mut bindings),
            "{actual:?}"
        );
    }

    #[test]
    fn conventional_circuit_notation_is_typed_by_the_pack() {
        assert_eq!(
            recognized_laws("The asserted device law is \\[(V)=(R\\,I)\\]."),
            ["ohm-law"]
        );
    }

    #[test]
    fn a_capacitor_refusal_can_still_be_a_valid_resistor_law() {
        let source =
            "The equation $i=V/R$ is a resistor current law, not a capacitor derivative law.";
        assert_eq!(recognized_laws(source), ["ohm-law"]);
    }

    #[test]
    fn recognizes_typed_mechanical_power_without_a_law_specific_matcher() {
        let source = "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\mathbf{F}\\cdot\\mathbf{v}$";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = observe_prose(&document, &parsed);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let laws = observe_laws(
            &document,
            &parsed,
            &shapes,
            &quantities,
            &roles,
            &Default::default(),
        );
        assert_eq!(laws.all()[0].law_id, "mechanical-power");
    }

    #[test]
    fn recognizes_a_typed_continuous_state_equation() {
        let source = "Let $x$ be an n-dimensional vector. Let $u$ be an m-dimensional vector. Let $A$ be an n by n matrix. Let $B$ be an n by m matrix. $\\dot{x}=Ax+Bu$";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = observe_prose(&document, &parsed);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let laws = observe_laws(
            &document,
            &parsed,
            &shapes,
            &quantities,
            &roles,
            &Default::default(),
        );
        assert_eq!(laws.all()[0].law_id, "continuous-state-equation");
    }

    #[test]
    fn recognizes_coordinated_state_space_declarations() {
        let source = "Let $x$ be an $n$-dimensional state vector, $u$ an $m$-dimensional control vector, $A$ an $n$ by $n$ matrix, and $B$ an $n$ by $m$ matrix. $\\dot{x}=Ax+Bu$";
        assert_eq!(recognized_laws(source), ["continuous-state-equation"]);
    }

    #[test]
    fn recognizes_symbolic_state_space_declarations() {
        let source = "In a continuous state-space model, $z\\in\\mathbb R^p$, $v\\in\\mathbb R^r$, $F\\in\\mathbb R^{p\\times p}$, and $G\\in\\mathbb R^{p\\times r}$. $\\dot{z} = Fz + Gv$.";
        assert_eq!(recognized_laws(source), ["continuous-state-equation"]);
    }

    #[test]
    fn recognizes_reordered_kinetic_energy() {
        let source = "Here $K$ denotes kinetic energy, $m$ denotes mass, and $v$ denotes speed. $\\frac{1}{2}mv^2=K$";
        assert_eq!(recognized_laws(source), ["kinetic-energy-definition"]);
    }

    #[test]
    fn recognizes_remaining_canonical_variants() {
        for (source, expected) in [
            (
                "During the launch segment, let $F_{n09}$ stand for net force, $m_{n09}$ for mass, and $a_{n09}$ for acceleration. The same balance is presented as $$(m_{n09}a_{n09})=F_{n09}$$",
                "newton-second-law",
            ),
            (
                "During the coasting interval, $K_{e07}$ is measured in joules, $m_{e07}$ in kilograms, and $v_{e07}$ in metres per second. The definition gives $$K_{e07}=\\frac12m_{e07}v_{e07}^{2}$$",
                "kinetic-energy-definition",
            ),
            (
                "For the pulling rope, let $P_{p08}$ be scalar power, $\\mathbf{F}_{p08}$ the force vector, and $\\mathbf{v}_{p08}$ the velocity vector. Then $P_{p08}=\\left(\\mathbf{F}_{p08}\\right)\\cdot\\left(\\mathbf{v}_{p08}\\right)$.",
                "mechanical-power",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    #[test]
    fn recognizes_grouped_subscripted_state_equation() {
        let source = "Let $s_1\\in\\mathbb R^d$, $v_1\\in\\mathbb R^c$, $K_1\\in\\mathbb R^{d\\times d}$, and $L_1\\in\\mathbb R^{d\\times c}$.\n\\[\\dot{s_1}=\\left(K_1s_1\\right)+\\left(L_1v_1\\right)\\]";
        assert_eq!(recognized_laws(source), ["continuous-state-equation"]);
    }

    #[test]
    fn explicit_shape_and_role_conflicts_refuse_laws() {
        for source in [
            "Let $\\mathbf{F}_{x12}$ and $\\mathbf{a}_{x12}$ be vectors, and let $\\mathbf{m}_{x12}$ be a three-component mass vector. The model claims $\\mathbf{F}_{x12}=\\mathbf{m}_{x12}\\mathbf{a}_{x12}$.",
            "Let $\\mathbf{K}_{x12}$ be a three-vector of energies, $m_{x12}$ scalar mass, and $v_{x12}$ scalar speed. The model claims $\\mathbf{K}_{x12}=\\frac12m_{x12}v_{x12}^{2}$.",
            "Let $\\mathbf{P}_{x12}$ be a three-vector, and let $\\mathbf{F}_{x12}$ and $\\mathbf{v}_{x12}$ be force and velocity vectors. The model claims $\\mathbf{P}_{x12}=\\mathbf{F}_{x12}\\cdot\\mathbf{v}_{x12}$.",
            "Let $P_{x13}$ be scalar power, while $F_{x13}$ and $v_{x13}$ are scalars for a one-dimensional model. The draft nevertheless writes $P_{x13}=F_{x13}\\cdot v_{x13}$ as a vector dot product.",
        ] {
            assert!(recognized_laws(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn blind_extension_adds_matrix_vector_product_with_pack_data_only() {
        let source = "Let $A$ be an m by n matrix. Let $x$ be an n-dimensional vector. Let $y$ be an m-dimensional vector. $y=Ax$";
        assert_eq!(recognized_laws(source), ["matrix-vector-product"]);
    }

    #[test]
    fn blind_extension_adds_event_intersection_with_pack_data_only() {
        let source = "Let $A$ be an event. Let $B$ be an event. $A \\cap B$";
        assert_eq!(recognized_laws(source), ["event-intersection"]);
    }

    #[test]
    fn promotion_laws_use_only_the_generic_compiled_runtime() {
        for (source, expected) in [
            (
                "For this matrix product, let $A$ be an m by n matrix. Let $B$ be an n by p matrix. Let $C$ be an m by p matrix. $C=AB$",
                "matrix-matrix-product",
            ),
            (
                "Let $A$ be an m by n matrix and $B$ an n by m matrix. The matrix transpose is $B=A^T$",
                "matrix-transpose-definition",
            ),
            (
                "This example states an event union. Let $A$ and $B$ denote events. $A\\cup B$",
                "event-union",
            ),
            (
                "This example states a first derivative. Let $f$ be a function of $x$, and let $g$ denote its first derivative. $g=\\frac{d f}{d x}$",
                "first-derivative-relation",
            ),
            (
                "This example states a set intersection. Let $S$ and $T$ be sets. $S\\cap T$",
                "set-intersection",
            ),
            (
                "This example states a set union. Let $S$ and $T$ be sets. $S\\cup T$",
                "set-union",
            ),
            (
                "This example states a gradient descent update. In a gradient descent step, let $x$ and $y$ be n-dimensional iterates, $g$ an n-dimensional gradient vector, and $\\alpha$ a scalar step size. $y=x-\\alpha g$",
                "gradient-descent-update",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    fn recognized_laws(source: &str) -> Vec<String> {
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let prose = observe_prose(&document, &parsed);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        observe_laws(
            &document,
            &parsed,
            &shapes,
            &quantities,
            &roles,
            &Default::default(),
        )
        .all()
        .iter()
        .map(|law| law.law_id.clone())
        .collect()
    }
}
