use std::collections::BTreeSet;

use crate::canonical::{SemanticExpr, SemanticExprKind, associative};

const MAX_EQUIVALENT_FORMS: usize = 64;
const MAX_COMMUTATIVE_FACTORS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EquivalenceStep {
    ScalarPermutation,
    FactorIsolation,
    ReciprocalNormalization,
}

impl EquivalenceStep {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::ScalarPermutation => "scalar-permutation",
            Self::FactorIsolation => "factor-isolation",
            Self::ReciprocalNormalization => "reciprocal-normalization",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EquivalenceGuard {
    Nonzero(SemanticExpr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GuardedForm {
    pub(crate) expression: SemanticExpr,
    pub(crate) guards: Vec<EquivalenceGuard>,
    pub(crate) steps: Vec<EquivalenceStep>,
}

pub(crate) fn compile_guarded_forms(
    canonical: SemanticExpr,
    scalar_placeholders: &BTreeSet<String>,
) -> Vec<GuardedForm> {
    let mut forms = vec![GuardedForm {
        expression: canonical.clone(),
        guards: Vec::new(),
        steps: Vec::new(),
    }];
    add_scalar_permutations(&canonical, scalar_placeholders, &mut forms);
    add_factor_isolations(&canonical, scalar_placeholders, &mut forms);
    add_reciprocal_forms(&canonical, scalar_placeholders, &mut forms);
    deduplicate(forms)
}

pub(crate) fn instantiate_guard(
    guard: &EquivalenceGuard,
    bindings: &std::collections::BTreeMap<String, SemanticExpr>,
) -> EquivalenceGuard {
    match guard {
        EquivalenceGuard::Nonzero(subject) => {
            EquivalenceGuard::Nonzero(substitute(subject, bindings))
        }
    }
}

fn add_scalar_permutations(
    relation: &SemanticExpr,
    scalar_placeholders: &BTreeSet<String>,
    output: &mut Vec<GuardedForm>,
) {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &relation.kind
    else {
        return;
    };
    for (product_on_left, factors) in [
        (true, product_factors(left)),
        (false, product_factors(right)),
    ] {
        let Some(factors) = factors else { continue };
        if factors.len() < 2
            || factors.len() > MAX_COMMUTATIVE_FACTORS
            || !factors
                .iter()
                .all(|factor| is_declared_scalar(factor, scalar_placeholders))
        {
            continue;
        }
        for permutation in permutations(factors) {
            let product = expression_like(relation, SemanticExprKind::Product(permutation));
            output.push(GuardedForm {
                expression: relation_like(
                    relation,
                    operator,
                    if product_on_left {
                        product.clone()
                    } else {
                        (**left).clone()
                    },
                    if product_on_left {
                        (**right).clone()
                    } else {
                        product
                    },
                ),
                guards: Vec::new(),
                steps: vec![EquivalenceStep::ScalarPermutation],
            });
        }
    }
}

fn add_factor_isolations(
    relation: &SemanticExpr,
    scalar_placeholders: &BTreeSet<String>,
    output: &mut Vec<GuardedForm>,
) {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &relation.kind
    else {
        return;
    };
    if operator != "equals" {
        return;
    }
    for (product, result) in [
        (right.as_ref(), left.as_ref()),
        (left.as_ref(), right.as_ref()),
    ] {
        let Some(factors) = product_factors(product) else {
            continue;
        };
        if factors.len() < 2
            || factors.len() > MAX_COMMUTATIVE_FACTORS
            || !factors
                .iter()
                .all(|factor| is_declared_scalar(factor, scalar_placeholders))
        {
            continue;
        }
        for index in 0..factors.len() {
            let divisor_factors = factors
                .iter()
                .enumerate()
                .filter(|(candidate, _)| *candidate != index)
                .map(|(_, factor)| factor.clone())
                .collect::<Vec<_>>();
            let divisor = associative(divisor_factors.clone(), SemanticExprKind::Product);
            let quotient = expression_like(
                relation,
                SemanticExprKind::Fraction(Box::new(result.clone()), Box::new(divisor)),
            );
            output.push(GuardedForm {
                expression: relation_like(relation, operator, factors[index].clone(), quotient),
                guards: divisor_factors
                    .into_iter()
                    .map(EquivalenceGuard::Nonzero)
                    .collect(),
                steps: vec![EquivalenceStep::FactorIsolation],
            });
        }
    }
}

fn add_reciprocal_forms(
    relation: &SemanticExpr,
    scalar_placeholders: &BTreeSet<String>,
    output: &mut Vec<GuardedForm>,
) {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &relation.kind
    else {
        return;
    };
    if operator == "equals" {
        for (reciprocal, result) in [
            (right.as_ref(), left.as_ref()),
            (left.as_ref(), right.as_ref()),
        ] {
            if let SemanticExprKind::Fraction(numerator, denominator) = &reciprocal.kind
                && matches!(&numerator.kind, SemanticExprKind::Number(value) if value == "1")
                && is_declared_scalar(denominator, scalar_placeholders)
                && is_declared_scalar(result, scalar_placeholders)
            {
                let inverted = expression_like(
                    relation,
                    SemanticExprKind::Fraction(
                        Box::new(number_one(relation)),
                        Box::new(result.clone()),
                    ),
                );
                output.push(GuardedForm {
                    expression: relation_like(
                        relation,
                        operator,
                        denominator.as_ref().clone(),
                        inverted,
                    ),
                    guards: vec![EquivalenceGuard::Nonzero(result.clone())],
                    steps: vec![EquivalenceStep::ReciprocalNormalization],
                });
            }
        }
    }
    for (product_on_left, product) in [(true, left.as_ref()), (false, right.as_ref())] {
        let Some(factors) = product_factors(product) else {
            continue;
        };
        let Some((reciprocal_index, denominator)) = factors.iter().enumerate().find_map(
            |(index, factor)| match &factor.kind {
                SemanticExprKind::Fraction(numerator, denominator)
                    if matches!(&numerator.kind, SemanticExprKind::Number(value) if value == "1")
                        && is_declared_scalar(denominator, scalar_placeholders) =>
                {
                    Some((index, denominator.as_ref().clone()))
                }
                _ => None,
            },
        ) else {
            continue;
        };
        let numerator = associative(
            factors
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != reciprocal_index)
                .map(|(_, factor)| factor.clone())
                .collect(),
            SemanticExprKind::Product,
        );
        let fraction = expression_like(
            relation,
            SemanticExprKind::Fraction(Box::new(numerator.clone()), Box::new(denominator.clone())),
        );
        output.push(GuardedForm {
            expression: relation_like(
                relation,
                operator,
                if product_on_left {
                    fraction.clone()
                } else {
                    (**left).clone()
                },
                if product_on_left {
                    (**right).clone()
                } else {
                    fraction
                },
            ),
            guards: vec![EquivalenceGuard::Nonzero(denominator.clone())],
            steps: vec![EquivalenceStep::ReciprocalNormalization],
        });
        if operator == "equals" {
            let opposite = if product_on_left {
                right.as_ref().clone()
            } else {
                left.as_ref().clone()
            };
            let scaled_opposite = expression_like(
                relation,
                SemanticExprKind::Product(vec![denominator.clone(), opposite]),
            );
            output.push(GuardedForm {
                expression: relation_like(relation, operator, scaled_opposite, numerator),
                guards: vec![EquivalenceGuard::Nonzero(denominator)],
                steps: vec![EquivalenceStep::ReciprocalNormalization],
            });
        }
    }
}

fn number_one(source: &SemanticExpr) -> SemanticExpr {
    expression_like(source, SemanticExprKind::Number("1".into()))
}

fn relation_like(
    source: &SemanticExpr,
    operator: &str,
    left: SemanticExpr,
    right: SemanticExpr,
) -> SemanticExpr {
    expression_like(
        source,
        SemanticExprKind::Relation {
            operator: operator.to_owned(),
            left: Box::new(left),
            right: Box::new(right),
        },
    )
}

fn expression_like(source: &SemanticExpr, kind: SemanticExprKind) -> SemanticExpr {
    SemanticExpr {
        kind,
        range: source.range.clone(),
        provenance: source.provenance.clone(),
    }
}

fn product_factors(expression: &SemanticExpr) -> Option<&[SemanticExpr]> {
    match &expression.kind {
        SemanticExprKind::Product(factors) => Some(factors),
        _ => None,
    }
}

fn is_declared_scalar(expression: &SemanticExpr, scalars: &BTreeSet<String>) -> bool {
    match &expression.kind {
        SemanticExprKind::Number(_) => true,
        SemanticExprKind::Symbol(symbol) => scalars.contains(symbol),
        SemanticExprKind::Negate(inner) => is_declared_scalar(inner, scalars),
        SemanticExprKind::Fraction(left, right) | SemanticExprKind::Power(left, right) => {
            is_declared_scalar(left, scalars) && is_declared_scalar(right, scalars)
        }
        SemanticExprKind::Derivative {
            expression,
            variable,
            ..
        } => is_declared_scalar(expression, scalars) && scalars.contains(variable),
        SemanticExprKind::Product(items) | SemanticExprKind::Sum(items) => {
            items.iter().all(|item| is_declared_scalar(item, scalars))
        }
        _ => false,
    }
}

fn permutations(items: &[SemanticExpr]) -> Vec<Vec<SemanticExpr>> {
    fn visit(
        items: &[SemanticExpr],
        used: &mut [bool],
        current: &mut Vec<SemanticExpr>,
        output: &mut Vec<Vec<SemanticExpr>>,
    ) {
        if current.len() == items.len() {
            output.push(current.clone());
            return;
        }
        for index in 0..items.len() {
            if used[index] {
                continue;
            }
            used[index] = true;
            current.push(items[index].clone());
            visit(items, used, current, output);
            current.pop();
            used[index] = false;
        }
    }
    let mut output = Vec::new();
    visit(
        items,
        &mut vec![false; items.len()],
        &mut Vec::new(),
        &mut output,
    );
    output
}

fn substitute(
    expression: &SemanticExpr,
    bindings: &std::collections::BTreeMap<String, SemanticExpr>,
) -> SemanticExpr {
    if let SemanticExprKind::Symbol(symbol) = &expression.kind
        && let Some(bound) = bindings.get(symbol)
    {
        return bound.clone();
    }
    let map = |value: &SemanticExpr| Box::new(substitute(value, bindings));
    let kind = match &expression.kind {
        SemanticExprKind::Symbol(value) => SemanticExprKind::Symbol(value.clone()),
        SemanticExprKind::Number(value) => SemanticExprKind::Number(value.clone()),
        SemanticExprKind::Sum(items) => SemanticExprKind::Sum(
            items
                .iter()
                .map(|item| substitute(item, bindings))
                .collect(),
        ),
        SemanticExprKind::Product(items) => SemanticExprKind::Product(
            items
                .iter()
                .map(|item| substitute(item, bindings))
                .collect(),
        ),
        SemanticExprKind::Dot(left, right) => SemanticExprKind::Dot(map(left), map(right)),
        SemanticExprKind::Cross(left, right) => SemanticExprKind::Cross(map(left), map(right)),
        SemanticExprKind::Fraction(left, right) => {
            SemanticExprKind::Fraction(map(left), map(right))
        }
        SemanticExprKind::Power(left, right) => SemanticExprKind::Power(map(left), map(right)),
        SemanticExprKind::Negate(inner) => SemanticExprKind::Negate(map(inner)),
        SemanticExprKind::Derivative {
            expression,
            variable,
            order,
        } => SemanticExprKind::Derivative {
            expression: map(expression),
            variable: variable.clone(),
            order: *order,
        },
        SemanticExprKind::Relation {
            operator,
            left,
            right,
        } => SemanticExprKind::Relation {
            operator: operator.clone(),
            left: map(left),
            right: map(right),
        },
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => SemanticExprKind::Apply {
            operator: operator.clone(),
            arguments: arguments
                .iter()
                .map(|item| substitute(item, bindings))
                .collect(),
        },
        SemanticExprKind::Unknown(value) => SemanticExprKind::Unknown(value.clone()),
    };
    expression_like(expression, kind)
}

fn deduplicate(forms: Vec<GuardedForm>) -> Vec<GuardedForm> {
    let mut output = Vec::new();
    for form in forms.into_iter().take(MAX_EQUIVALENT_FORMS) {
        if !output.iter().any(|existing: &GuardedForm| {
            existing.expression == form.expression && existing.guards == form.guards
        }) {
            output.push(form);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{EquivalenceGuard, EquivalenceStep, compile_guarded_forms};
    use crate::canonical::{SemanticExprKind, lower_template};
    use std::collections::BTreeSet;

    #[test]
    fn isolates_every_scalar_factor_with_explicit_nonzero_guards() {
        let scalars = ["result", "density", "area", "velocity"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let forms =
            compile_guarded_forms(lower_template("result = density area velocity"), &scalars);
        let isolated = forms
            .iter()
            .filter(|form| form.steps == [EquivalenceStep::FactorIsolation])
            .collect::<Vec<_>>();
        assert_eq!(isolated.len(), 3);
        assert!(isolated.iter().all(|form| form.guards.len() == 2));
    }

    #[test]
    fn treats_a_derivative_of_declared_scalars_as_an_isolatable_scalar_factor() {
        let scalars = ["current", "capacitance", "voltage", "time"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let forms = compile_guarded_forms(
            lower_template("current = capacitance \\frac{d voltage}{d time}"),
            &scalars,
        );
        let isolated = forms
            .iter()
            .filter(|form| form.steps == [EquivalenceStep::FactorIsolation])
            .collect::<Vec<_>>();
        assert_eq!(isolated.len(), 2);
        assert!(isolated.iter().all(|form| form.guards.len() == 1));
    }

    #[test]
    fn never_reorders_or_isolates_an_undeclared_matrix_product() {
        let canonical = lower_template("output = operator input");
        let forms = compile_guarded_forms(canonical.clone(), &BTreeSet::new());
        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].expression, canonical);
        assert!(forms[0].guards.is_empty());
    }

    #[test]
    fn normalizes_a_reciprocal_coefficient_with_a_denominator_guard() {
        let scalars = ["energy", "mass", "velocity"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let forms =
            compile_guarded_forms(lower_template("energy = 1 mass velocity^2 / 2"), &scalars);
        assert!(forms.iter().all(|form| {
            form.guards
                .iter()
                .all(|guard| matches!(guard, EquivalenceGuard::Nonzero(_)))
        }));
        assert!(
            forms
                .iter()
                .any(|form| { matches!(form.expression.kind, SemanticExprKind::Relation { .. }) })
        );
    }

    #[test]
    fn inverts_a_scalar_reciprocal_with_a_guard_on_the_new_denominator() {
        let canonical = lower_template("frequency = 1 / period");
        let scalars = ["frequency", "period"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let forms = compile_guarded_forms(canonical, &scalars);
        let inverse = forms.iter().find(|form| {
            matches!(
                &form.expression.kind,
                SemanticExprKind::Relation { left, right, .. }
                    if matches!(&left.kind, SemanticExprKind::Symbol(value) if value == "period")
                        && matches!(&right.kind, SemanticExprKind::Fraction(one, denominator)
                            if matches!(&one.kind, SemanticExprKind::Number(value) if value == "1")
                                && matches!(&denominator.kind, SemanticExprKind::Symbol(value) if value == "frequency"))
            )
        });
        assert!(inverse.is_some());
        assert_eq!(inverse.unwrap().guards.len(), 1);
    }
}
