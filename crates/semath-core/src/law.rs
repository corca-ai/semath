use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::canonical::{SemanticExpr, SemanticExprKind, lower_template};
use crate::consistency::{RoleObservations, roles_conflict};
use crate::equivalence::{EquivalenceGuard, GuardedForm, compile_guarded_forms, instantiate_guard};
use crate::pack::{PackConditionKind, PackLaw, PackLawRole, built_in_packs};
use crate::prose::{FormulaOperationKind, ScientificSemanticEvidence};
use crate::quantity::QuantityObservations;
use crate::shape::ShapeObservations;
use crate::{
    AssumptionInfo, ConstraintStatus, Evidence, LawBinding, LawConditionInfo, LawRecognition,
    LawRecognitionStatus, QuantityInfo, RelationInfo, RelationRoleInfo, RoleInfo,
    ScientificConstraintKind, SemanticConstraint, SemanticConstraintKind, ShapeInfo,
};

const MAX_LAW_MATCHES: usize = 16;
const MAX_UNIFICATION_CANDIDATES: usize = 64;

struct CompiledLaw {
    pack_id: &'static str,
    pack_version: &'static str,
    law: &'static PackLaw,
    forms: Vec<GuardedForm>,
    placeholders: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DispatchRoot {
    Relation(String),
    Apply(String),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DispatchFeature {
    Apply(String),
    Cross,
    Derivative,
    Dot,
    Fraction,
    Power,
    Product(usize),
    Sum(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DispatchKey {
    root: DispatchRoot,
    feature: Option<DispatchFeature>,
    operands: Option<(DispatchOperand, DispatchOperand)>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DispatchOperand {
    Any,
    Apply(String),
    Atom,
    Cross,
    Derivative,
    Dot,
    Fraction,
    Negate,
    Power,
    Product(usize),
    Sum(usize),
}

#[derive(Default)]
struct LawDispatch {
    candidates: BTreeMap<DispatchKey, Vec<usize>>,
}

impl LawDispatch {
    fn compile(laws: &[CompiledLaw]) -> Self {
        let mut dispatch = Self::default();
        for (index, compiled) in laws.iter().enumerate() {
            dispatch.insert(
                index,
                &compiled.forms,
                &compiled.placeholders,
                compiled.law.roles.iter().any(|role| role.variadic),
            );
        }
        dispatch
    }

    fn insert(
        &mut self,
        index: usize,
        forms: &[GuardedForm],
        placeholders: &BTreeSet<String>,
        variadic: bool,
    ) {
        let mut keys = forms
            .iter()
            .map(|form| DispatchKey {
                root: dispatch_root(&form.expression),
                feature: strongest_dispatch_feature(&form.expression, placeholders),
                operands: dispatch_template_operands(&form.expression, placeholders),
            })
            .collect::<BTreeSet<_>>();
        if variadic {
            keys.insert(DispatchKey {
                root: DispatchRoot::Relation("equals".into()),
                feature: None,
                operands: None,
            });
        }
        for key in keys {
            self.candidates.entry(key).or_default().push(index);
        }
    }

    fn candidate_indices(&self, expression: &SemanticExpr) -> Vec<usize> {
        let root = dispatch_root(expression);
        let features = expression_dispatch_features(expression)
            .into_iter()
            .map(Some)
            .chain(std::iter::once(None));
        let operands = dispatch_actual_operands(expression);
        let mut keys = Vec::new();
        for feature in features {
            for operands in &operands {
                keys.push(DispatchKey {
                    root: root.clone(),
                    feature: feature.clone(),
                    operands: operands.clone(),
                });
            }
        }
        keys.into_iter()
            .filter_map(|key| self.candidates.get(&key))
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn candidates(&self, expression: &SemanticExpr) -> Vec<&'static CompiledLaw> {
        self.candidate_indices(expression)
            .into_iter()
            .map(|index| &COMPILED_LAWS[index])
            .collect()
    }
}

fn dispatch_template_operands(
    expression: &SemanticExpr,
    placeholders: &BTreeSet<String>,
) -> Option<(DispatchOperand, DispatchOperand)> {
    let SemanticExprKind::Relation { left, right, .. } = &expression.kind else {
        return None;
    };
    Some((
        dispatch_operand(left, placeholders),
        dispatch_operand(right, placeholders),
    ))
}

fn dispatch_actual_operands(
    expression: &SemanticExpr,
) -> Vec<Option<(DispatchOperand, DispatchOperand)>> {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &expression.kind
    else {
        return vec![None];
    };
    let mut pairs = Vec::new();
    for left in dispatch_operand_variants(left) {
        for right in dispatch_operand_variants(right) {
            for pair in [
                (left.clone(), right.clone()),
                (DispatchOperand::Any, right.clone()),
                (left.clone(), DispatchOperand::Any),
                (DispatchOperand::Any, DispatchOperand::Any),
            ] {
                pairs.push(Some(pair.clone()));
                if operator == "equals" {
                    pairs.push(Some((pair.1, pair.0)));
                }
            }
        }
    }
    pairs.push(None);
    pairs.sort();
    pairs.dedup();
    pairs
}

fn dispatch_operand(expression: &SemanticExpr, placeholders: &BTreeSet<String>) -> DispatchOperand {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) if placeholders.contains(symbol) => DispatchOperand::Any,
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Unknown(_) => DispatchOperand::Atom,
        SemanticExprKind::Apply { operator, .. } => DispatchOperand::Apply(operator.clone()),
        SemanticExprKind::Cross(_, _) => DispatchOperand::Cross,
        SemanticExprKind::Derivative { .. } => DispatchOperand::Derivative,
        SemanticExprKind::Dot(_, _) => DispatchOperand::Dot,
        SemanticExprKind::Fraction(_, _) => DispatchOperand::Fraction,
        SemanticExprKind::Negate(_) => DispatchOperand::Negate,
        SemanticExprKind::Power(_, _) => DispatchOperand::Power,
        SemanticExprKind::Product(items) => DispatchOperand::Product(items.len()),
        SemanticExprKind::Sum(items) => DispatchOperand::Sum(items.len()),
        SemanticExprKind::Relation { .. } => DispatchOperand::Atom,
    }
}

fn dispatch_operand_variants(expression: &SemanticExpr) -> Vec<DispatchOperand> {
    let mut variants = vec![dispatch_operand(expression, &BTreeSet::new())];
    if let Some(expanded) = expand_ambiguous_juxtaposition(expression) {
        variants.push(dispatch_operand(&expanded, &BTreeSet::new()));
    }
    variants.sort();
    variants.dedup();
    variants
}

static COMPILED_LAWS: LazyLock<Vec<CompiledLaw>> = LazyLock::new(|| {
    built_in_packs()
        .iter()
        .flat_map(|pack| {
            pack.laws
                .iter()
                .filter(|law| !law.canonical_relation.is_empty())
                .map(|law| {
                    let scalar_placeholders = law
                        .roles
                        .iter()
                        .filter(|role| role.shape.as_deref() == Some("scalar"))
                        .map(|role| role.id.clone())
                        .collect::<BTreeSet<_>>();
                    CompiledLaw {
                        pack_id: &pack.pack_id,
                        pack_version: &pack.pack_version,
                        forms: law
                            .relations()
                            .flat_map(|form| {
                                compile_guarded_forms(lower_template(form), &scalar_placeholders)
                            })
                            .collect(),
                        placeholders: law.roles.iter().map(|role| role.id.clone()).collect(),
                        law,
                    }
                })
        })
        .collect()
});

static LAW_DISPATCH: LazyLock<LawDispatch> =
    LazyLock::new(|| LawDispatch::compile(COMPILED_LAWS.as_slice()));

fn dispatch_root(expression: &SemanticExpr) -> DispatchRoot {
    match &expression.kind {
        SemanticExprKind::Relation { operator, .. } => DispatchRoot::Relation(operator.clone()),
        SemanticExprKind::Apply { operator, .. } => DispatchRoot::Apply(operator.clone()),
        _ => DispatchRoot::Other,
    }
}

fn strongest_dispatch_feature(
    expression: &SemanticExpr,
    placeholders: &BTreeSet<String>,
) -> Option<DispatchFeature> {
    let mut features = BTreeSet::new();
    collect_dispatch_features(expression, placeholders, &mut features);
    features.into_iter().min_by_key(dispatch_feature_priority)
}

fn expression_dispatch_features(expression: &SemanticExpr) -> BTreeSet<DispatchFeature> {
    let mut features = BTreeSet::new();
    collect_dispatch_features(expression, &BTreeSet::new(), &mut features);
    features
}

fn collect_dispatch_features(
    expression: &SemanticExpr,
    placeholders: &BTreeSet<String>,
    output: &mut BTreeSet<DispatchFeature>,
) {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) if placeholders.contains(symbol) => {}
        SemanticExprKind::Negate(inner) => collect_dispatch_features(inner, placeholders, output),
        SemanticExprKind::Power(base, exponent) => {
            output.insert(DispatchFeature::Power);
            collect_dispatch_features(base, placeholders, output);
            collect_dispatch_features(exponent, placeholders, output);
        }
        SemanticExprKind::Sum(items) => {
            output.insert(DispatchFeature::Sum(items.len()));
            for item in items {
                collect_dispatch_features(item, placeholders, output);
            }
        }
        SemanticExprKind::Product(items) => {
            output.insert(DispatchFeature::Product(items.len()));
            for item in items {
                collect_dispatch_features(item, placeholders, output);
            }
        }
        SemanticExprKind::Fraction(left, right) => {
            output.insert(DispatchFeature::Fraction);
            collect_dispatch_features(left, placeholders, output);
            collect_dispatch_features(right, placeholders, output);
        }
        SemanticExprKind::Dot(left, right) => {
            output.insert(DispatchFeature::Dot);
            collect_dispatch_features(left, placeholders, output);
            collect_dispatch_features(right, placeholders, output);
        }
        SemanticExprKind::Cross(left, right) => {
            output.insert(DispatchFeature::Cross);
            collect_dispatch_features(left, placeholders, output);
            collect_dispatch_features(right, placeholders, output);
        }
        SemanticExprKind::Derivative { expression, .. } => {
            output.insert(DispatchFeature::Derivative);
            collect_dispatch_features(expression, placeholders, output);
        }
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => {
            output.insert(DispatchFeature::Apply(operator.clone()));
            if arguments.len() == 1 {
                output.insert(DispatchFeature::Product(2));
            }
            for argument in arguments {
                collect_dispatch_features(argument, placeholders, output);
            }
        }
        SemanticExprKind::Relation { left, right, .. } => {
            collect_dispatch_features(left, placeholders, output);
            collect_dispatch_features(right, placeholders, output);
        }
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Unknown(_) => {}
    }
}

fn dispatch_feature_priority(feature: &DispatchFeature) -> u8 {
    match feature {
        DispatchFeature::Apply(_) => 0,
        DispatchFeature::Cross | DispatchFeature::Derivative | DispatchFeature::Dot => 1,
        DispatchFeature::Fraction | DispatchFeature::Power => 2,
        DispatchFeature::Product(_) | DispatchFeature::Sum(_) => 3,
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LawObservations {
    recognitions: Vec<LawRecognition>,
    equivalence_states: u32,
    guard_checks: u32,
    visited_rules: u32,
}

struct RecognitionContext<'a> {
    shapes: &'a ShapeObservations,
    quantities: &'a QuantityObservations,
    consistency: &'a RoleObservations,
    assumptions: &'a [AssumptionInfo],
    external: &'a ExternalTypeEnvironment,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalTypeEnvironment {
    roles: BTreeMap<u32, BTreeMap<String, Vec<RoleInfo>>>,
    quantities: BTreeMap<u32, BTreeMap<String, Vec<QuantityInfo>>>,
    shapes: BTreeMap<u32, BTreeMap<String, Vec<ShapeInfo>>>,
}

impl ExternalTypeEnvironment {
    pub fn add_role(&mut self, offset: u32, role: RoleInfo) {
        self.roles
            .entry(offset)
            .or_default()
            .entry(role.symbol.clone())
            .or_default()
            .push(role);
    }

    pub fn add_quantity(&mut self, offset: u32, quantity: QuantityInfo) {
        self.quantities
            .entry(offset)
            .or_default()
            .entry(quantity.symbol.clone())
            .or_default()
            .push(quantity);
    }

    pub fn add_shape(&mut self, offset: u32, shape: ShapeInfo) {
        self.shapes
            .entry(offset)
            .or_default()
            .entry(shape.symbol.clone())
            .or_default()
            .push(shape);
    }

    fn has_role(&self, offset: u32, symbol: &str, role: &str) -> bool {
        self.roles_at(offset, symbol)
            .iter()
            .any(|info| info.concept_id == role)
    }

    fn has_quantity(&self, offset: u32, symbol: &str, quantity: &str) -> bool {
        self.quantities_at(offset, symbol)
            .iter()
            .any(|info| info.quantity_kind_id.as_deref() == Some(quantity))
    }

    fn has_shape(&self, offset: u32, symbol: &str, shape: &str) -> bool {
        self.shapes_at(offset, symbol)
            .iter()
            .any(|info| info.kind == shape)
    }

    pub fn roles_at(&self, offset: u32, symbol: &str) -> Vec<RoleInfo> {
        facts_at(&self.roles, offset, symbol)
    }

    pub fn quantities_at(&self, offset: u32, symbol: &str) -> Vec<QuantityInfo> {
        facts_at(&self.quantities, offset, symbol)
    }

    pub fn shapes_at(&self, offset: u32, symbol: &str) -> Vec<ShapeInfo> {
        facts_at(&self.shapes, offset, symbol)
    }
}

fn facts_at<T: Clone>(
    facts: &BTreeMap<u32, BTreeMap<String, Vec<T>>>,
    offset: u32,
    symbol: &str,
) -> Vec<T> {
    facts
        .get(&offset)
        .and_then(|symbols| symbols.get(symbol))
        .cloned()
        .unwrap_or_default()
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

    pub fn equivalence_states(&self) -> u32 {
        self.equivalence_states
    }

    pub fn guard_checks(&self) -> u32 {
        self.guard_checks
    }
}

pub(crate) fn observe_laws(
    canonical_expressions: &[SemanticExpr],
    semantic_evidence: &ScientificSemanticEvidence,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> LawObservations {
    let mut recognitions = Vec::new();
    let mut equivalence_states = 0;
    let mut guard_checks = 0;
    let mut visited_rules = 0;
    for actual in canonical_expressions {
        if !semantic_evidence.formula_is_asserted(&actual.range)
            || !formula_operations_are_well_typed(actual, semantic_evidence, shapes)
        {
            continue;
        }
        for compiled in LAW_DISPATCH.candidates(actual) {
            if recognitions.len() >= MAX_LAW_MATCHES {
                break;
            }
            visited_rules += 1;
            let activation =
                semantic_evidence.law_activation(compiled.pack_id, &compiled.law.id, &actual.range);
            if !compiled.law.activation_phrases.is_empty() && activation.is_none() {
                continue;
            }
            equivalence_states += compiled.forms.len() as u32;
            let candidates = compiled
                .forms
                .iter()
                .flat_map(|form| {
                    unify_all(
                        &form.expression,
                        actual,
                        &compiled.placeholders,
                        &BTreeMap::new(),
                    )
                    .into_iter()
                    .map(move |bindings| (Some(form), bindings))
                })
                .chain(variadic_balance(compiled, actual).map(|bindings| (None, bindings)))
                .collect::<Vec<_>>();
            let Some((matched_form, bindings)) = candidates.into_iter().find(|(_, bindings)| {
                let supported = roles_are_supported(
                    &compiled.law.roles,
                    bindings,
                    actual.range.start_offset,
                    shapes,
                    quantities,
                    consistency,
                    external,
                );
                let typed = expression_is_well_typed(actual, shapes);
                supported && typed
            }) else {
                continue;
            };
            guard_checks += matched_form.map_or(0, |form| form.guards.len() as u32);
            let mut recognized = recognition(
                compiled,
                actual,
                bindings,
                matched_form,
                &RecognitionContext {
                    shapes,
                    quantities,
                    consistency,
                    assumptions,
                    external,
                },
            );
            if let Some(activation) = activation {
                recognized.evidence.push(activation.evidence.clone());
            }
            recognitions.push(recognized);
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
        equivalence_states,
        guard_checks,
        visited_rules,
    }
}

fn formula_operations_are_well_typed(
    expression: &SemanticExpr,
    evidence: &ScientificSemanticEvidence,
    shapes: &ShapeObservations,
) -> bool {
    evidence
        .formula_operations(&expression.range)
        .all(|operation| match operation.operation {
            FormulaOperationKind::VectorDotProduct => vector_dot_operands(expression)
                .is_some_and(|(left, right)| {
                    matches!(expression_shape(left, shapes), ShapeInference::Known(shape) if shape.first().is_some_and(|kind| kind == "vector"))
                        && matches!(expression_shape(right, shapes), ShapeInference::Known(shape) if shape.first().is_some_and(|kind| kind == "vector"))
                }),
        })
}

fn vector_dot_operands(expression: &SemanticExpr) -> Option<(&SemanticExpr, &SemanticExpr)> {
    match &expression.kind {
        SemanticExprKind::Dot(left, right) => Some((left, right)),
        SemanticExprKind::Relation { left, right, .. } => {
            vector_dot_operands(left).or_else(|| vector_dot_operands(right))
        }
        SemanticExprKind::Negate(inner) => vector_dot_operands(inner),
        SemanticExprKind::Sum(items) | SemanticExprKind::Product(items) => {
            items.iter().find_map(vector_dot_operands)
        }
        _ => None,
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
        SemanticExprKind::Sum(terms) => additive_shape(terms, shapes),
        SemanticExprKind::Product(factors) => factors.iter().fold(
            ShapeInference::Known(vec!["scalar".into()]),
            |left, right| combine_product_shapes(left, expression_shape(right, shapes)),
        ),
        SemanticExprKind::Dot(left, right) => match (
            expression_shape(left, shapes),
            expression_shape(right, shapes),
        ) {
            (ShapeInference::Known(left), ShapeInference::Known(right))
                if left.first().is_some_and(|kind| kind == "scalar")
                    || right.first().is_some_and(|kind| kind == "scalar") =>
            {
                multiply_shapes(left, right).map_or(ShapeInference::Invalid, ShapeInference::Known)
            }
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
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if matches!(operator.as_str(), "integral" | "partial-derivative") => arguments
            .first()
            .map_or(ShapeInference::Unknown, |argument| {
                expression_shape(argument, shapes)
            }),
        SemanticExprKind::Relation { left, right, .. } => {
            if is_additive_zero(left) {
                expression_shape(right, shapes)
            } else if is_additive_zero(right) {
                expression_shape(left, shapes)
            } else {
                combine_equal_shapes([
                    expression_shape(left, shapes),
                    expression_shape(right, shapes),
                ])
            }
        }
        _ => ShapeInference::Unknown,
    }
}

fn additive_shape(terms: &[SemanticExpr], shapes: &ShapeObservations) -> ShapeInference {
    let nonzero = terms
        .iter()
        .filter(|term| !is_additive_zero(term))
        .map(|term| expression_shape(term, shapes))
        .collect::<Vec<_>>();
    if nonzero.is_empty() {
        ShapeInference::Known(vec!["scalar".into()])
    } else {
        combine_equal_shapes(nonzero)
    }
}

fn is_additive_zero(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Number(value) => value == "0",
        SemanticExprKind::Negate(inner) => is_additive_zero(inner),
        _ => false,
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
    let mut candidates = match (&template.kind, &actual.kind) {
        (SemanticExprKind::Symbol(left), SemanticExprKind::Symbol(right)) if left == right => {
            vec![bindings.clone()]
        }
        (SemanticExprKind::Number(left), SemanticExprKind::Number(right)) if left == right => {
            vec![bindings.clone()]
        }
        (SemanticExprKind::Negate(left), SemanticExprKind::Negate(right)) => {
            unify_all(left, right, placeholders, bindings)
        }
        (SemanticExprKind::Power(lb, le), SemanticExprKind::Power(rb, re)) if matches!(&rb.kind, SemanticExprKind::Apply { operator, arguments } if operator == "norm" && arguments.len() == 1) =>
        {
            let SemanticExprKind::Apply { arguments, .. } = &rb.kind else {
                unreachable!()
            };
            unify_sequence(
                [lb.as_ref(), le.as_ref()],
                [&arguments[0], re.as_ref()],
                placeholders,
                bindings,
            )
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
            let reversed = (left_operator == "equals")
                .then(|| {
                    unify_sequence(
                        [left.as_ref(), right.as_ref()],
                        [actual_right.as_ref(), actual_left.as_ref()],
                        placeholders,
                        bindings,
                    )
                })
                .into_iter()
                .flatten();
            direct.into_iter().chain(reversed).collect()
        }
        (SemanticExprKind::Sum(left), SemanticExprKind::Sum(right))
            if left.len() == right.len() =>
        {
            commutative_unify_all(left, right, placeholders, bindings)
        }
        (SemanticExprKind::Product(left), SemanticExprKind::Product(right))
            if left.len() == right.len() =>
        {
            unify_sequence(left.iter(), right.iter(), placeholders, bindings)
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
            if matches!(left_operator.as_str(), "intersection" | "union") {
                commutative_unify_all(left, right, placeholders, bindings)
            } else {
                unify_sequence(left.iter(), right.iter(), placeholders, bindings)
            }
        }
        (
            SemanticExprKind::Product(template),
            SemanticExprKind::Apply {
                operator,
                arguments,
            },
        ) if arguments.len() == 1 && !is_structural_application(operator) => {
            let operator = SemanticExpr {
                kind: SemanticExprKind::Symbol(operator.clone()),
                range: actual.range.clone(),
                provenance: actual.provenance.clone(),
            };
            commutative_unify_all(
                template,
                &[operator, arguments[0].clone()],
                placeholders,
                bindings,
            )
        }
        (SemanticExprKind::Unknown(left), SemanticExprKind::Unknown(right)) if left == right => {
            vec![bindings.clone()]
        }
        _ => Vec::new(),
    };
    if candidates.is_empty()
        && matches!(template.kind, SemanticExprKind::Product(_))
        && let Some(expanded) = expand_ambiguous_juxtaposition(actual)
    {
        candidates = unify_all(template, &expanded, placeholders, bindings);
    }
    candidates
        .into_iter()
        .take(MAX_UNIFICATION_CANDIDATES)
        .collect()
}

fn expand_ambiguous_juxtaposition(expression: &SemanticExpr) -> Option<SemanticExpr> {
    let mut changed = false;
    let factors = match &expression.kind {
        SemanticExprKind::Product(items) => items
            .iter()
            .flat_map(|item| {
                if let Some(expanded) = ambiguous_factor(item) {
                    changed = true;
                    expanded
                } else {
                    vec![item.clone()]
                }
            })
            .collect::<Vec<_>>(),
        _ => {
            changed = true;
            ambiguous_factor(expression)?
        }
    };
    changed.then(|| SemanticExpr {
        kind: SemanticExprKind::Product(factors),
        range: expression.range.clone(),
        provenance: expression.provenance.clone(),
    })
}

fn ambiguous_factor(expression: &SemanticExpr) -> Option<Vec<SemanticExpr>> {
    match &expression.kind {
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if arguments.len() == 1 && !is_structural_application(operator) => Some(vec![
            SemanticExpr {
                kind: SemanticExprKind::Symbol(operator.clone()),
                range: expression.range.clone(),
                provenance: expression.provenance.clone(),
            },
            arguments[0].clone(),
        ]),
        SemanticExprKind::Power(base, exponent) => {
            let SemanticExprKind::Apply {
                operator,
                arguments,
            } = &base.kind
            else {
                return None;
            };
            (arguments.len() == 1 && !is_structural_application(operator)).then(|| {
                vec![
                    SemanticExpr {
                        kind: SemanticExprKind::Symbol(operator.clone()),
                        range: base.range.clone(),
                        provenance: base.provenance.clone(),
                    },
                    SemanticExpr {
                        kind: SemanticExprKind::Power(
                            Box::new(arguments[0].clone()),
                            exponent.clone(),
                        ),
                        range: expression.range.clone(),
                        provenance: expression.provenance.clone(),
                    },
                ]
            })
        }
        _ => None,
    }
}

fn is_structural_application(operator: &str) -> bool {
    matches!(
        operator,
        "compose" | "intersection" | "norm" | "sum" | "transpose" | "union"
    )
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
                }) || left_operator == "equals"
                    && transaction(bindings, |candidate| {
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
                && if matches!(left_operator.as_str(), "intersection" | "union") {
                    commutative_unify(left, right, placeholders, bindings)
                } else {
                    left.iter()
                        .zip(right)
                        .all(|(left, right)| unify(left, right, placeholders, bindings))
                }
        }
        (
            SemanticExprKind::Product(template),
            SemanticExprKind::Apply {
                operator,
                arguments,
            },
        ) if arguments.len() == 1 => {
            let operator = SemanticExpr {
                kind: SemanticExprKind::Symbol(operator.clone()),
                range: actual.range.clone(),
                provenance: actual.provenance.clone(),
            };
            commutative_unify(
                template,
                &[operator, arguments[0].clone()],
                placeholders,
                bindings,
            )
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
) -> bool {
    let notation_symbol = symbol.split('_').next().unwrap_or(symbol);
    if let Some(quantity) = role.quantity.as_deref()
        && !quantity_is_supported(
            quantity,
            symbol,
            notation_symbol,
            offset,
            quantities,
            external,
        )
    {
        return false;
    }
    let declared_roles = consistency.roles_at(symbol, offset).0;
    let comparable_roles = declared_roles
        .iter()
        .filter(|claim| concepts_are_comparable(&role.concept, &claim.concept_id))
        .collect::<Vec<_>>();
    if !comparable_roles.is_empty()
        && comparable_roles
            .iter()
            .all(|claim| roles_conflict(&role.concept, &claim.concept_id))
    {
        return false;
    }
    if let Some(expected_shape) = role.shape.as_deref() {
        let mut explicit = shapes.claims_at(symbol, offset).0;
        if notation_symbol != symbol {
            explicit.extend(shapes.claims_at(notation_symbol, offset).0);
        }
        let mut imported = external.shapes_at(offset, symbol);
        if notation_symbol != symbol {
            imported.extend(external.shapes_at(offset, notation_symbol));
        }
        if explicit
            .iter()
            .chain(&imported)
            .any(|shape| shape.kind != expected_shape)
        {
            return false;
        }
        match shapes
            .shape_at(symbol, offset)
            .or_else(|| shapes.shape_at(notation_symbol, offset))
        {
            Some(shape) if shape.kind != expected_shape => return false,
            Some(_) => {}
            None if !(role.quantity.is_some()
                || role.concept.starts_with("quantities-units:")
                || external.has_shape(offset, symbol, expected_shape)) =>
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
        return quantity_is_supported(
            &role.concept,
            symbol,
            notation_symbol,
            offset,
            quantities,
            external,
        );
    }
    if role.concept == "linear-algebra:linear-operator" {
        return shapes
            .shape_at(symbol, offset)
            .is_some_and(|shape| shape.kind == "matrix")
            || external.has_shape(offset, symbol, "matrix");
    }
    consistency
        .roles_at(symbol, offset)
        .0
        .iter()
        .any(|claim| claim.concept_id == role.concept)
        || external.has_role(offset, symbol, &role.concept)
}

fn concepts_are_comparable(left: &str, right: &str) -> bool {
    let one_is_quantity =
        left.starts_with("quantities-units:") != right.starts_with("quantities-units:");
    !(one_is_quantity
        && [left, right]
            .iter()
            .any(|concept| concept.split(':').next_back() == Some("variable")))
}

#[allow(clippy::too_many_arguments)]
fn quantity_is_supported(
    expected: &str,
    symbol: &str,
    notation_symbol: &str,
    offset: u32,
    quantities: &QuantityObservations,
    external: &ExternalTypeEnvironment,
) -> bool {
    let mut local = quantities.at(symbol, offset).0;
    if notation_symbol != symbol {
        local.extend(quantities.at(notation_symbol, offset).0);
    }
    if !local.is_empty() {
        let declared = local
            .iter()
            .filter_map(|quantity| quantity.quantity_kind_id.as_deref())
            .collect::<Vec<_>>();
        return !declared.is_empty() && declared.iter().all(|kind| *kind == expected);
    }
    external.has_quantity(offset, symbol, expected)
}

fn recognition(
    compiled: &CompiledLaw,
    actual: &SemanticExpr,
    bindings: BTreeMap<String, SemanticExpr>,
    matched_form: Option<&GuardedForm>,
    context: &RecognitionContext<'_>,
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
                    kind: SemanticConstraintKind::Expression,
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
    let mut conditions: Vec<LawConditionInfo> = compiled
        .law
        .conditions
        .iter()
        .map(|condition| {
            let condition_bindings = formula_bindings
                .iter()
                .filter(|binding| condition.subjects.contains(&binding.parameter))
                .collect::<Vec<_>>();
            let (evidence, mechanically_verified) = condition_evidence(
                condition.kind,
                &condition.subjects,
                &bindings,
                actual.range.start_offset,
                context.shapes,
                context.quantities,
                context.consistency,
                context.external,
            );
            LawConditionInfo {
                condition_id: condition.id.clone(),
                kind: scientific_constraint_kind(condition.kind),
                subjects: condition_bindings
                    .iter()
                    .map(|binding| binding.symbol.clone())
                    .collect(),
                label: condition.label.clone(),
                status: condition_status(
                    condition.kind,
                    condition_bindings.len(),
                    mechanically_verified,
                ),
                evidence,
            }
        })
        .collect();
    if let Some(form) = matched_form {
        conditions.extend(equivalence_conditions(
            form,
            &bindings,
            context.assumptions,
            &actual.range,
        ));
    }
    let status = if conditions
        .iter()
        .any(|condition| condition.status == ConstraintStatus::Conflicting)
    {
        LawRecognitionStatus::Conflicting
    } else if conditions.iter().any(|condition| {
        matches!(
            condition.status,
            ConstraintStatus::Required | ConstraintStatus::Unsupported
        )
    }) {
        LawRecognitionStatus::ConditionMissing
    } else if conditions.is_empty() {
        LawRecognitionStatus::Recognized
    } else {
        LawRecognitionStatus::Verified
    };
    let mut evidence = vec![formula_evidence.clone()];
    if let Some(form) = matched_form {
        evidence.extend(form.steps.iter().map(|step| Evidence {
            rule_id: format!("guarded-equivalence/{}", step.id()),
            kind: "equivalence-proof".into(),
            strength: "hard".into(),
            source_ranges: vec![actual.range.clone()],
        }));
    }
    LawRecognition {
        law_id: compiled.law.id.clone(),
        title: compiled.law.title.clone(),
        description: compiled.law.description.clone(),
        description_key: compiled.law.id.clone(),
        maturity: "recognition".into(),
        status,
        pack_id: compiled.pack_id.into(),
        pack_version: compiled.pack_version.into(),
        range: actual.range.clone(),
        bindings: formula_bindings,
        result: SemanticConstraint {
            kind: SemanticConstraintKind::Proposition,
            concepts: Vec::new(),
            dimensions: Vec::new(),
            refinements: vec!["typed-law-instance".into()],
        },
        conditions,
        relation: Some(RelationInfo {
            relation_id: format!("{}:{}", compiled.pack_id, compiled.law.id),
            title: compiled.law.title.clone(),
            description: compiled.law.description.clone(),
            roles: relation_roles,
            conditions: compiled
                .law
                .conditions
                .iter()
                .map(|condition| condition.label.clone())
                .collect(),
            evidence: evidence.clone(),
            range: actual.range.clone(),
        }),
        evidence,
        rank: 100,
    }
}

fn equivalence_conditions(
    form: &GuardedForm,
    bindings: &BTreeMap<String, SemanticExpr>,
    assumptions: &[AssumptionInfo],
    formula_range: &crate::SourceRange,
) -> Vec<LawConditionInfo> {
    form.guards
        .iter()
        .enumerate()
        .map(|(index, guard)| match instantiate_guard(guard, bindings) {
            EquivalenceGuard::Nonzero(subject) => {
                let symbols = semantic_symbols(&subject)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let (status, mut evidence) = nonzero_status(&subject, &symbols, assumptions);
                evidence.push(Evidence {
                    rule_id: "guarded-equivalence/nonzero".into(),
                    kind: "equivalence-guard".into(),
                    strength: "hard".into(),
                    source_ranges: vec![formula_range.clone()],
                });
                LawConditionInfo {
                    condition_id: format!("equivalence-nonzero-{index}"),
                    kind: ScientificConstraintKind::Nonzero,
                    subjects: symbols.clone(),
                    label: if symbols.is_empty() {
                        "The isolated divisor must be nonzero.".into()
                    } else {
                        format!("{} must be nonzero.", symbols.join(" and "))
                    },
                    status,
                    evidence,
                }
            }
        })
        .collect()
}

fn nonzero_status(
    subject: &SemanticExpr,
    symbols: &[String],
    assumptions: &[AssumptionInfo],
) -> (ConstraintStatus, Vec<Evidence>) {
    if let SemanticExprKind::Number(value) = &subject.kind {
        return if value.parse::<f64>().ok() == Some(0.0) {
            (ConstraintStatus::Conflicting, Vec::new())
        } else {
            (ConstraintStatus::Verified, Vec::new())
        };
    }
    let supporting = assumptions
        .iter()
        .filter(|assumption| {
            matches!(
                assumption.value.as_str(),
                "nonzero" | "positive" | "strictly-positive"
            ) && symbols
                .iter()
                .all(|symbol| assumption.subjects.contains(symbol))
        })
        .map(|assumption| assumption.evidence.clone())
        .collect::<Vec<_>>();
    if !symbols.is_empty() && !supporting.is_empty() {
        (ConstraintStatus::Verified, supporting)
    } else {
        (ConstraintStatus::Required, supporting)
    }
}

fn scientific_constraint_kind(kind: PackConditionKind) -> ScientificConstraintKind {
    match kind {
        PackConditionKind::Assumption => ScientificConstraintKind::Assumption,
        PackConditionKind::Differentiable => ScientificConstraintKind::Differentiable,
        PackConditionKind::DomainMembership => ScientificConstraintKind::DomainMembership,
        PackConditionKind::Positive => ScientificConstraintKind::Positive,
        PackConditionKind::SameContext => ScientificConstraintKind::SameContext,
        PackConditionKind::ShapeCompatible => ScientificConstraintKind::ShapeCompatible,
        PackConditionKind::SignConvention => ScientificConstraintKind::SignConvention,
        PackConditionKind::Uniform => ScientificConstraintKind::Uniform,
    }
}

fn condition_status(
    kind: PackConditionKind,
    resolved_subjects: usize,
    mechanically_verified: bool,
) -> ConstraintStatus {
    if resolved_subjects == 0 {
        return ConstraintStatus::Unsupported;
    }
    match kind {
        PackConditionKind::DomainMembership | PackConditionKind::ShapeCompatible
            if mechanically_verified =>
        {
            ConstraintStatus::Verified
        }
        PackConditionKind::Assumption
        | PackConditionKind::Differentiable
        | PackConditionKind::Positive
        | PackConditionKind::SameContext
        | PackConditionKind::SignConvention
        | PackConditionKind::Uniform
        | PackConditionKind::DomainMembership
        | PackConditionKind::ShapeCompatible => ConstraintStatus::Required,
    }
}

#[allow(clippy::too_many_arguments)]
fn condition_evidence(
    kind: PackConditionKind,
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
) -> (Vec<Evidence>, bool) {
    let mut evidence = Vec::new();
    let mut proved_subjects = 0;
    for subject in subjects {
        let Some(expression) = bindings.get(subject) else {
            continue;
        };
        push_evidence(
            &mut evidence,
            Evidence {
                rule_id: format!("typed-law-role/{subject}"),
                kind: "canonical-binding".into(),
                strength: "hard".into(),
                source_ranges: vec![expression.range.clone()],
            },
        );
        let symbols = semantic_symbols(expression);
        let proved = !symbols.is_empty()
            && symbols.iter().all(|symbol| {
                let facts = match kind {
                    PackConditionKind::ShapeCompatible => shapes
                        .shape_at(symbol, offset)
                        .into_iter()
                        .map(|shape| shape.evidence)
                        .chain(
                            external
                                .shapes_at(offset, symbol)
                                .into_iter()
                                .map(|shape| shape.evidence),
                        )
                        .collect::<Vec<_>>(),
                    PackConditionKind::DomainMembership => consistency
                        .roles_at(symbol, offset)
                        .0
                        .into_iter()
                        .map(|role| role.evidence)
                        .chain(
                            quantities
                                .at(symbol, offset)
                                .0
                                .into_iter()
                                .map(|quantity| quantity.evidence),
                        )
                        .chain(
                            external
                                .roles_at(offset, symbol)
                                .into_iter()
                                .map(|role| role.evidence),
                        )
                        .chain(
                            external
                                .quantities_at(offset, symbol)
                                .into_iter()
                                .map(|quantity| quantity.evidence),
                        )
                        .collect(),
                    _ => Vec::new(),
                };
                let has_facts = !facts.is_empty();
                for fact in facts {
                    push_evidence(&mut evidence, fact);
                }
                has_facts
            });
        proved_subjects += usize::from(proved);
    }
    (evidence, proved_subjects == subjects.len())
}

fn push_evidence(items: &mut Vec<Evidence>, evidence: Evidence) {
    if !items.contains(&evidence) {
        items.push(evidence);
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

    use super::{COMPILED_LAWS, LAW_DISPATCH, LawDispatch, observe_laws, unify, unify_all};
    use crate::canonical::{SemanticExpr, lower_document_region, lower_template};
    use crate::consistency::observe_roles;
    use crate::parser::{ParsedMath, parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::quantity::observe_quantities;
    use crate::shape::observe_shapes;
    use crate::{
        ConstraintStatus, DocumentLanguage, LawRecognition, LawRecognitionStatus, ProjectDocument,
        ScientificConstraintKind,
    };

    fn canonical_expressions(
        document: &ProjectDocument,
        parsed: &[ParsedMath],
    ) -> Vec<SemanticExpr> {
        parsed
            .iter()
            .map(|math| {
                let mut expression = lower_document_region(document, &math.region.content_range);
                expression.range = math.region.content_range.clone();
                expression
            })
            .collect()
    }

    #[test]
    fn indexed_dispatch_is_complete_against_exhaustive_unification() {
        for compiled in &*COMPILED_LAWS {
            for actual in &compiled.forms {
                let exhaustive = COMPILED_LAWS
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| {
                        candidate.forms.iter().any(|form| {
                            !unify_all(
                                &form.expression,
                                &actual.expression,
                                &candidate.placeholders,
                                &BTreeMap::new(),
                            )
                            .is_empty()
                        })
                    })
                    .map(|(index, _)| index)
                    .collect::<BTreeSet<_>>();
                let indexed = LAW_DISPATCH
                    .candidate_indices(&actual.expression)
                    .into_iter()
                    .filter(|index| {
                        let candidate = &COMPILED_LAWS[*index];
                        candidate.forms.iter().any(|form| {
                            !unify_all(
                                &form.expression,
                                &actual.expression,
                                &candidate.placeholders,
                                &BTreeMap::new(),
                            )
                            .is_empty()
                        })
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(indexed, exhaustive, "{}", compiled.law.id);
            }
        }
    }

    #[test]
    fn dispatch_stays_structurally_bounded_at_hundreds_of_synthetic_packs() {
        for pack_count in [100, 500] {
            let forms = (0..pack_count)
                .map(|index| lower_template(&format!("synthetic{index}(x)")))
                .collect::<Vec<_>>();
            let mut dispatch = LawDispatch::default();
            for (index, form) in forms.iter().enumerate() {
                dispatch.insert(
                    index,
                    &[crate::equivalence::GuardedForm {
                        expression: form.clone(),
                        guards: Vec::new(),
                        steps: Vec::new(),
                    }],
                    &BTreeSet::new(),
                    false,
                );
            }
            for (index, form) in forms.iter().enumerate() {
                assert_eq!(dispatch.candidate_indices(form), [index]);
            }
        }
    }

    #[test]
    fn dispatch_indexes_the_expanded_shape_of_ambiguous_juxtaposition() {
        let actual = lower_template("energy = 1 / 2 mass(velocity)^2");
        let kinetic = COMPILED_LAWS
            .iter()
            .position(|compiled| compiled.law.id == "kinetic-energy-definition")
            .expect("kinetic energy law");
        assert!(LAW_DISPATCH.candidate_indices(&actual).contains(&kinetic));
    }

    #[test]
    fn equality_and_declared_scalar_products_are_presentation_independent() {
        let template = lower_template("force = mass acceleration");
        let actual = lower_template("a m = F");
        let placeholders = ["force", "mass", "acceleration"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let forms = crate::equivalence::compile_guarded_forms(template, &placeholders);
        let bindings = forms
            .iter()
            .find_map(|form| {
                let mut bindings = BTreeMap::new();
                unify(&form.expression, &actual, &placeholders, &mut bindings).then_some(bindings)
            })
            .expect("a guarded scalar permutation should match");
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
    fn directional_relations_remain_ordered_while_set_union_is_commutative() {
        let placeholders = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        assert!(!unify(
            &lower_template("x \\in A"),
            &lower_template("A \\in x"),
            &placeholders,
            &mut bindings,
        ));
        assert!(unify(
            &lower_template("A \\cup B"),
            &lower_template("B \\cup A"),
            &placeholders,
            &mut BTreeMap::new(),
        ));
    }

    #[test]
    fn conventional_notation_without_typed_evidence_is_refused() {
        assert!(recognized_laws("The asserted device law is \\[(V)=(R\\,I)\\].").is_empty());
        assert!(
            recognized_laws(
                "Let $x$ and $u$ be vectors and $A$ and $B$ matrices. $\\dot{x}=Ax+Bu$",
            )
            .is_empty()
        );
    }

    #[test]
    fn a_capacitor_refusal_can_still_be_a_valid_resistor_law() {
        let source = "Let $i$ denote electric current, $V$ voltage, and $R$ resistance. The equation $i=V/R$ is a resistor current law, not a capacitor derivative law.";
        let recognized = recognized_law_observations(source);
        assert_eq!(recognized[0].law_id, "ohm-law");
        assert_eq!(recognized[0].status, LawRecognitionStatus::ConditionMissing);
        assert!(recognized[0].conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::Nonzero
                && condition.status == ConstraintStatus::Required
                && condition.subjects == ["R"]
        }));
    }

    #[test]
    fn explicit_nonzero_evidence_verifies_an_isolated_scalar_law() {
        let source = "Let $i$ denote electric current, $V$ voltage, and $R$ nonzero resistance. The resistor relation is $i=V/R$.";
        let recognized = recognized_law_observations(source);
        assert_eq!(recognized[0].law_id, "ohm-law");
        assert!(recognized[0].conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::Nonzero
                && condition.status == ConstraintStatus::Verified
                && condition.subjects == ["R"]
        }));
    }

    #[test]
    fn recognizes_typed_mechanical_power_without_a_law_specific_matcher() {
        let source = "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\mathbf{F}\\cdot\\mathbf{v}$";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 6,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let laws = observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &shapes,
            &quantities,
            &roles,
            &prose.assumptions,
            &Default::default(),
        );
        assert_eq!(laws.all()[0].law_id, "mechanical-power");
    }

    #[test]
    fn recognizes_a_typed_continuous_state_equation() {
        let source = "Let $x$ be an n-dimensional state vector. Let $u$ be an m-dimensional control input vector. Let $A$ be an n by n state matrix. Let $B$ be an n by m input matrix. $\\dot{x}=Ax+Bu$";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 6,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let laws = observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &shapes,
            &quantities,
            &roles,
            &prose.assumptions,
            &Default::default(),
        );
        assert_eq!(laws.all()[0].law_id, "continuous-state-equation");
    }

    #[test]
    fn recognizes_coordinated_state_space_declarations() {
        let source = "Let $x$ be an $n$-dimensional state vector, $u$ an $m$-dimensional control input vector, $A$ an $n$ by $n$ state matrix, and $B$ an $n$ by $m$ input matrix. $\\dot{x}=Ax+Bu$";
        assert_eq!(recognized_laws(source), ["continuous-state-equation"]);
    }

    #[test]
    fn recognizes_symbolic_state_space_declarations() {
        let source = "In a continuous state-space model, $z\\in\\mathbb R^p$, $v\\in\\mathbb R^r$, $F\\in\\mathbb R^{p\\times p}$, and $G\\in\\mathbb R^{p\\times r}$. Here $z$ denotes the state, $v$ the control input, $F$ the state matrix, and $G$ the input matrix. $\\dot{z} = Fz + Gv$.";
        assert_eq!(recognized_laws(source), ["continuous-state-equation"]);
    }

    #[test]
    fn recognizes_reordered_kinetic_energy() {
        let source = "Here $K$ denotes kinetic energy, $m$ denotes mass, and $v$ denotes speed. $\\frac{1}{2}mv^2=K$";
        assert_eq!(recognized_laws(source), ["kinetic-energy-definition"]);
        let grouped = "Here $K$ denotes kinetic energy, $m$ denotes mass, and $v$ denotes speed. $K=\\frac{1}{2}m(v)^2$";
        assert_eq!(recognized_laws(grouped), ["kinetic-energy-definition"]);
    }

    #[test]
    fn recognizes_remaining_canonical_variants() {
        for (source, expected) in [
            (
                "During the launch segment, let $F_{n09}$ stand for net force, $m_{n09}$ for mass, and $a_{n09}$ for acceleration. The same balance is presented as $$(m_{n09}a_{n09})=F_{n09}$$",
                "newton-second-law",
            ),
            (
                "During the coasting interval, $K_{e07}$ is measured in joules, $m_{e07}$ in kilograms, and $v_{e07}$ in metres per second. Here $K_{e07}$ denotes kinetic energy, $m_{e07}$ mass, and $v_{e07}$ velocity. The definition gives $$K_{e07}=\\frac12m_{e07}v_{e07}^{2}$$",
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
    fn recognizes_foundation_cases_with_membership_units_and_source_ordered_roles() {
        for (source, expected) in [
            (
                "Let $a\\in\\mathbb R^n$, $b\\in\\mathbb R^m$, $F_a\\in\\mathbb R^{n\\times n}$, and $G_a\\in\\mathbb R^{n\\times m}$. Here $a$ denotes the state, $b$ the control input, $F_a$ the state matrix, and $G_a$ the input matrix.\n\\[\\dot{a}=F_a a+G_a b\\]",
                "continuous-state-equation",
            ),
            (
                "Let $\\rho\\in\\mathbb R^n$, $\\sigma\\in\\mathbb R^m$, $U\\in\\mathbb R^{n\\times n}$, and $V\\in\\mathbb R^{n\\times m}$. We write $\\rho$ for state vector. We write $\\sigma$ for control input vector. We write $U$ for state matrix. We write $V$ for input matrix.\n\\[\\dot{\\rho}=U\\rho+V\\sigma\\]",
                "continuous-state-equation",
            ),
            (
                "During steady translation, $P$ is measured in watts, $F$ in newtons, and $v$ in metres per second. Here $P$ denotes power, $F$ force, and $v$ velocity. Thus \\[P=F\\,v\\]",
                "mechanical-power",
            ),
            (
                "Here $K$ denotes kinetic energy, $m$ denotes mass, and $v$ denotes speed. The calculation shows \\[K=\\tfrac{1}{2}m(v)^{2}\\]",
                "kinetic-energy-definition",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    #[test]
    fn recognizes_grouped_subscripted_state_equation() {
        let source = "Let $s_1\\in\\mathbb R^d$, $v_1\\in\\mathbb R^c$, $K_1\\in\\mathbb R^{d\\times d}$, and $L_1\\in\\mathbb R^{d\\times c}$. Here $s_1$ denotes the state, $v_1$ the control input, $K_1$ the state matrix, and $L_1$ the input matrix.\n\\[\\dot{s_1}=\\left(K_1s_1\\right)+\\left(L_1v_1\\right)\\]";
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
    fn typed_matrix_application_can_use_grouped_juxtaposition_without_hiding_function_calls() {
        assert_eq!(
            recognized_laws(
                "Let $A$ be an m by n matrix. Let $x$ be an n-dimensional vector. Let $y$ be an m-dimensional vector. $y=A(x)$",
            ),
            ["matrix-vector-product"],
        );
        assert!(
            recognized_laws("Let $I$ be a function and $R$ be resistance. $V=I(R)$").is_empty()
        );
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
                "This example states a first derivative. Let $f$ be a function of $x$, $x$ the differentiation variable, and $g$ its first derivative. $g=\\frac{d f}{d x}$",
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
                "This example states a gradient descent update. Let $y$, $x$, $\\alpha$, and $g$ denote iterate vector, iterate vector, step size scalar, and gradient vector, respectively. $y=x-\\alpha g$",
                "gradient-descent-update",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    #[test]
    fn engineering_verticals_use_composed_role_and_quantity_constraints() {
        for (source, expected) in [
            (
                "For Linear momentum, let $p$ denote momentum vector, $m$ denote mass scalar, and $v$ denote velocity vector. $p=mv$",
                "linear-momentum-definition",
            ),
            (
                "For Inductor voltage law, let $v$ denote voltage scalar, $L$ denote inductance scalar, $i$ denote current scalar, and $t$ denote time scalar. $v=L\\frac{di}{dt}$",
                "inductor-voltage-law",
            ),
            (
                "For Inductor voltage law, suppose $r$ is inductor terminal voltage, $a$ is inductance scalar, $b$ is inductor current, and $n$ is time variable. $r=a\\frac{db}{dn}$",
                "inductor-voltage-law",
            ),
            (
                "For Linear output equation, let $y$ denote output vector, $C$ denote output matrix, $x$ denote state vector, $D$ denote feedthrough matrix, and $u$ denote control input vector. $y=Cx+Du$",
                "linear-output-equation",
            ),
            (
                "Let $C$, $P_c$, and $Q_c$ be $n\\times n$ matrices. We write $C$ for state matrix. We write $P_c$ for Lyapunov certificate matrix. We write $Q_c$ for forcing matrix. $\\left(C^\\top P_c\\right)+\\left(P_cC\\right)=-Q_c$",
                "continuous-lyapunov-equation",
            ),
            (
                "For Angular frequency, let $w$ denote angular frequency scalar, $p$ denote the circle constant pi scalar, and $f$ denote cyclic frequency scalar. $w=2pf$",
                "angular-frequency-definition",
            ),
            (
                "For wave propagation, let $v$ denote wave speed, $f$ cyclic frequency, and $\\lambda$ wavelength. Then $v=f\\lambda$.",
                "wave-speed-relation",
            ),
            (
                "For Wave speed relation, let $y$ denote wave propagation speed, $c$ denote cyclic frequency scalar, and $x$ denote wavelength scalar. $y=cx$",
                "wave-speed-relation",
            ),
            (
                "For Wave speed relation, let $y$ denote wave propagation speed scalar, $c$ denote cyclic frequency scalar, and $x$ denote wavelength scalar. $y=c\\cdot x$",
                "wave-speed-relation",
            ),
            (
                "For Electric force, let $F$ denote force vector, $q$ denote electric charge scalar, and $E$ denote electric field vector. $F=qE$",
                "electric-force-law",
            ),
            (
                "For Electric potential energy, let $y$ denote electric potential energy scalar, $c$ denote electric charge scalar, and $x$ denote electric potential relative to the stated reference scalar. $y=cx$",
                "electric-potential-energy",
            ),
            (
                "For Sensible heat, let $Q$ denote heat transfer scalar, $m$ denote mass scalar, $c$ denote specific heat scalar, and $T$ denote temperature change scalar. $Q=mcT$",
                "sensible-heat-relation",
            ),
            (
                "Let $h$ be heat transfer, $m$ mass, $s$ specific heat, and $d$ temperature change. Then $h=msd$.",
                "sensible-heat-relation",
            ),
            (
                "For Plane-wall conduction rate, let $y$ denote heat-transfer rate scalar, $c$ denote thermal conductivity scalar, $x$ denote area normal to heat flow scalar, $t$ denote temperature difference scalar, and $z$ denote wall thickness scalar. $y=c x t/z$",
                "plane-wall-conduction-rate",
            ),
            (
                "For Closed-system first law, let $y$ denote change in internal energy scalar, $c$ denote heat added to the system scalar, and $x$ denote work done by the system scalar. $y=c-x$",
                "closed-system-first-law",
            ),
            (
                "For Mass flow rate, let $M$ denote mass flow rate scalar, $r$ denote density scalar, $A$ denote area scalar, and $v$ denote velocity scalar. $M=rAv$",
                "mass-flow-rate",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    #[test]
    fn recognizes_typed_zero_balance_lyapunov_form() {
        let source = "Let $B$ be a state matrix, $L_b$ a Lyapunov certificate matrix, and $N_b$ a forcing matrix. $0=B^\\top L_b+L_bB+N_b$";
        assert_eq!(recognized_laws(source), ["continuous-lyapunov-equation"]);
    }

    #[test]
    fn resolves_typed_conditions_to_bound_symbols_and_source_evidence() {
        let derivative = recognized_law_observations(
            "This example states a first derivative. Let $f$ be a function of $x$, $x$ the differentiation variable, and $g$ its first derivative. $g=\\frac{d f}{d x}$",
        );
        let condition = &derivative[0].conditions[0];
        assert_eq!(condition.condition_id, "function-differentiable");
        assert_eq!(condition.kind, ScientificConstraintKind::Differentiable);
        assert_eq!(condition.status, ConstraintStatus::Required);
        assert_eq!(condition.subjects, ["f", "x"]);
        assert_eq!(condition.evidence.len(), 2);
        assert!(
            condition
                .evidence
                .iter()
                .all(|item| !item.source_ranges.is_empty())
        );

        let matrix = recognized_law_observations(
            "Let $A$ be an m by n matrix. Let $x$ be an n-dimensional vector. Let $y$ be an m-dimensional vector. $y=Ax$",
        );
        assert_eq!(matrix[0].conditions[0].status, ConstraintStatus::Verified);
        let json = serde_json::to_value(&matrix[0].conditions[0]).unwrap();
        assert_eq!(json["kind"], "shape-compatible");
        assert_eq!(json["status"], "verified");
    }

    fn recognized_laws(source: &str) -> Vec<String> {
        recognized_law_observations(source)
            .iter()
            .map(|law| law.law_id.clone())
            .collect()
    }

    fn recognized_law_observations(source: &str) -> Vec<LawRecognition> {
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: source.into(),
            document_version: 1,
            schema_version: 6,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            declarations: Vec::new(),
            math_regions: regions.clone(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let parsed = parse_regions(source, &regions);
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &shapes,
            &quantities,
            &roles,
            &prose.assumptions,
            &Default::default(),
        )
        .all()
        .to_vec()
    }
}
