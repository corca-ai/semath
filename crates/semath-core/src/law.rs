use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::canonical::{SemanticExpr, SemanticExprKind, expression_children, lower_template};
use crate::consistency::{RoleObservations, roles_conflict};
use crate::domain::{DomainObservations, support_rank};
use crate::domain_signature::{is_capability_pack, laws_share_collision};
use crate::equivalence::{EquivalenceGuard, GuardedForm, compile_guarded_forms, instantiate_guard};
use crate::pack::{PackConditionKind, PackLaw, PackLawCondition, PackLawRole, built_in_packs};
use crate::prose::{FormulaOperationKind, LawActivationEvidence, ScientificSemanticEvidence};
use crate::quantity::QuantityObservations;
use crate::shape::ShapeObservations;
use crate::source_index::SourceIndex;
use crate::{
    AssumptionInfo, ConstraintStatus, DomainSupportTier, Evidence, LawBinding, LawConditionInfo,
    LawRecognition, LawRecognitionStatus, QuantityInfo, RelationInfo, RelationRoleInfo, RoleInfo,
    ScientificConstraintKind, SemanticConstraint, SemanticConstraintKind, ShapeInfo, SourceRange,
};

const MAX_LAW_MATCHES: usize = 16;
const MAX_UNIFICATION_CANDIDATES: usize = 64;

struct CompiledLaw {
    pack_id: &'static str,
    pack_version: &'static str,
    law: &'static PackLaw,
    plan: LawMatchPlan,
}

struct LawMatchPlan {
    forms: Vec<GuardedForm>,
    placeholders: BTreeSet<String>,
    variadic: bool,
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
    variadic_candidates: Vec<usize>,
}

impl LawDispatch {
    fn compile(laws: &[CompiledLaw]) -> Self {
        let mut dispatch = Self::default();
        for (index, compiled) in laws.iter().enumerate() {
            dispatch.insert(
                index,
                &compiled.plan.forms,
                &compiled.plan.placeholders,
                compiled.plan.variadic,
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
        let keys = forms
            .iter()
            .map(|form| DispatchKey {
                root: dispatch_root(&form.expression),
                feature: strongest_dispatch_feature(&form.expression, placeholders),
                operands: dispatch_template_operands(&form.expression, placeholders),
            })
            .collect::<BTreeSet<_>>();
        if variadic {
            self.variadic_candidates.push(index);
        }
        for key in keys {
            self.candidates.entry(key).or_default().push(index);
        }
    }

    #[cfg(test)]
    fn candidate_indices(&self, expression: &SemanticExpr) -> Vec<usize> {
        self.candidate_indices_for(&structural_alternatives(expression))
    }

    fn candidate_indices_for(&self, alternatives: &[SemanticExpr]) -> Vec<usize> {
        let mut indices = alternatives
            .iter()
            .flat_map(|expression| self.candidate_indices_exact(expression))
            .collect::<BTreeSet<_>>();
        if alternatives.iter().any(is_variadic_balance_candidate) {
            indices.extend(self.variadic_candidates.iter().copied());
        }
        indices.into_iter().collect()
    }

    fn candidate_indices_exact(&self, expression: &SemanticExpr) -> Vec<usize> {
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
        let indices = keys
            .into_iter()
            .filter_map(|key| self.candidates.get(&key))
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        indices.into_iter().collect()
    }

    fn candidates_for(&self, alternatives: &[SemanticExpr]) -> Vec<&'static CompiledLaw> {
        self.candidate_indices_for(alternatives)
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
    let left = dispatch_operand(left, &BTreeSet::new());
    let right = dispatch_operand(right, &BTreeSet::new());
    for pair in [
        (left.clone(), right.clone()),
        (DispatchOperand::Any, right),
        (left, DispatchOperand::Any),
        (DispatchOperand::Any, DispatchOperand::Any),
    ] {
        pairs.push(Some(pair.clone()));
        if operator == "equals" {
            pairs.push(Some((pair.1, pair.0)));
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
        SemanticExprKind::Apply { operator, .. } if placeholders.contains(operator.as_str()) => {
            DispatchOperand::Any
        }
        SemanticExprKind::Apply { operator, .. } => DispatchOperand::Apply(operator.value.clone()),
        SemanticExprKind::Cross(_, _) => DispatchOperand::Cross,
        SemanticExprKind::Derivative { .. } => DispatchOperand::Derivative,
        SemanticExprKind::Dot(_, _) => DispatchOperand::Dot,
        SemanticExprKind::Fraction(_, _) => DispatchOperand::Fraction,
        SemanticExprKind::Negate(_) => DispatchOperand::Negate,
        SemanticExprKind::Power(_, _) => DispatchOperand::Power,
        SemanticExprKind::Product(items) => DispatchOperand::Product(items.len()),
        SemanticExprKind::Sum(items) => DispatchOperand::Sum(items.len()),
        SemanticExprKind::Relation { .. } => DispatchOperand::Atom,
        SemanticExprKind::Index { .. } | SemanticExprKind::Condition { .. } => {
            DispatchOperand::Atom
        }
        SemanticExprKind::Binder { .. }
        | SemanticExprKind::System(_)
        | SemanticExprKind::Piecewise(_) => DispatchOperand::Atom,
    }
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
                        law,
                        plan: LawMatchPlan {
                            forms: law
                                .relations()
                                .flat_map(|form| {
                                    compile_guarded_forms(
                                        lower_template(form),
                                        &scalar_placeholders,
                                    )
                                })
                                .collect(),
                            placeholders: law.roles.iter().map(|role| role.id.clone()).collect(),
                            variadic: law.roles.iter().any(|role| role.variadic),
                        },
                    }
                })
        })
        .collect()
});

static LAW_DISPATCH: LazyLock<LawDispatch> =
    LazyLock::new(|| LawDispatch::compile(COMPILED_LAWS.as_slice()));

static NESTED_LAW_APPLICATIONS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    COMPILED_LAWS
        .iter()
        .flat_map(|compiled| &compiled.plan.forms)
        .filter_map(|form| match &form.expression.kind {
            SemanticExprKind::Apply { operator, .. } => Some(operator.value.clone()),
            _ => None,
        })
        .collect()
});

fn dispatch_root(expression: &SemanticExpr) -> DispatchRoot {
    match &expression.kind {
        SemanticExprKind::Relation { operator, .. } => {
            DispatchRoot::Relation(operator.value.clone())
        }
        SemanticExprKind::Apply { operator, .. } => DispatchRoot::Apply(operator.value.clone()),
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
            let ambiguous_applications = items
                .iter()
                .filter(|item| {
                    matches!(
                        &item.kind,
                        SemanticExprKind::Apply { operator, arguments }
                            if arguments.len() == 1
                                && !is_structural_application(operator.as_str())
                    )
                })
                .count();
            for additional in 1..=ambiguous_applications {
                output.insert(DispatchFeature::Product(items.len() + additional));
            }
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
            if !placeholders.contains(operator.as_str()) {
                output.insert(DispatchFeature::Apply(operator.value.clone()));
            }
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
        SemanticExprKind::Index { base, indices } => {
            collect_dispatch_features(base, placeholders, output);
            for index in indices {
                collect_dispatch_features(index, placeholders, output);
            }
        }
        SemanticExprKind::Condition { value, predicate } => {
            collect_dispatch_features(value, placeholders, output);
            collect_dispatch_features(predicate, placeholders, output);
        }
        SemanticExprKind::Binder {
            operator,
            variables,
            lower,
            upper,
            body,
        } => {
            output.insert(DispatchFeature::Apply(operator.value.clone()));
            for expression in variables
                .iter()
                .chain(lower.iter().map(Box::as_ref))
                .chain(upper.iter().map(Box::as_ref))
                .chain(std::iter::once(body.as_ref()))
            {
                collect_dispatch_features(expression, placeholders, output);
            }
        }
        SemanticExprKind::System(equations) => {
            for equation in equations {
                collect_dispatch_features(equation, placeholders, output);
            }
        }
        SemanticExprKind::Piecewise(branches) => {
            for branch in branches {
                collect_dispatch_features(&branch.value, placeholders, output);
                if let Some(condition) = &branch.condition {
                    collect_dispatch_features(condition, placeholders, output);
                }
            }
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
    pack_frontier_candidates: u32,
    pack_latent_candidates: u32,
    pack_latent_fallbacks: u32,
}

struct RecognitionContext<'a> {
    source: &'a str,
    source_index: &'a SourceIndex,
    shapes: &'a ShapeObservations,
    quantities: &'a QuantityObservations,
    consistency: &'a RoleObservations,
    assumptions: &'a [AssumptionInfo],
    external: &'a ExternalTypeEnvironment,
}

pub(crate) struct LawAnalysisContext<'a> {
    pub(crate) source: &'a str,
    pub(crate) formula_ranges: &'a [SourceRange],
    pub(crate) shapes: &'a ShapeObservations,
    pub(crate) quantities: &'a QuantityObservations,
    pub(crate) consistency: &'a RoleObservations,
    pub(crate) assumptions: &'a [AssumptionInfo],
    pub(crate) external: &'a ExternalTypeEnvironment,
    pub(crate) domains: &'a DomainObservations,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalTypeEnvironment {
    formula_ends: BTreeMap<u32, u32>,
    assumptions: BTreeMap<u32, Vec<AssumptionInfo>>,
    law_activations: BTreeMap<u32, Vec<LawActivationEvidence>>,
    roles: BTreeMap<u32, BTreeMap<String, Vec<RoleInfo>>>,
    quantities: BTreeMap<u32, BTreeMap<String, Vec<QuantityInfo>>>,
    shapes: BTreeMap<u32, BTreeMap<String, Vec<ShapeInfo>>>,
}

impl ExternalTypeEnvironment {
    pub fn begin_formula(&mut self, range: &SourceRange) {
        self.formula_ends
            .insert(range.start_offset, range.end_offset);
    }

    fn formula_offset(&self, offset: u32) -> u32 {
        self.formula_ends
            .range(..=offset)
            .next_back()
            .filter(|(_, end)| offset < **end)
            .map_or(offset, |(start, _)| *start)
    }

    pub fn add_assumption(&mut self, offset: u32, assumption: AssumptionInfo) {
        self.assumptions.entry(offset).or_default().push(assumption);
    }

    fn assumptions_at(&self, offset: u32) -> &[AssumptionInfo] {
        self.assumptions
            .get(&self.formula_offset(offset))
            .map_or(&[], Vec::as_slice)
    }

    pub fn add_law_activation(&mut self, offset: u32, activation: LawActivationEvidence) {
        self.law_activations
            .entry(offset)
            .or_default()
            .push(activation);
    }

    fn law_activation(
        &self,
        offset: u32,
        pack_id: &str,
        law_id: &str,
    ) -> Option<&LawActivationEvidence> {
        self.law_activations
            .get(&self.formula_offset(offset))?
            .iter()
            .filter(|activation| {
                activation.pack_id == pack_id
                    && activation.law_id == law_id
                    && activation.frame.establishes()
            })
            .max_by_key(|activation| activation.identifies_attached_formula)
    }

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
        facts_at(&self.roles, self.formula_offset(offset), symbol)
    }

    pub fn quantities_at(&self, offset: u32, symbol: &str) -> Vec<QuantityInfo> {
        facts_at(&self.quantities, self.formula_offset(offset), symbol)
    }

    pub fn shapes_at(&self, offset: u32, symbol: &str) -> Vec<ShapeInfo> {
        facts_at(&self.shapes, self.formula_offset(offset), symbol)
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

    pub fn overlapping(&self, range: &SourceRange) -> Vec<LawRecognition> {
        self.recognitions
            .iter()
            .filter(|recognition| {
                recognition.range.start_offset < range.end_offset
                    && range.start_offset < recognition.range.end_offset
            })
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

    pub fn pack_frontier_candidates(&self) -> u32 {
        self.pack_frontier_candidates
    }

    pub fn pack_latent_candidates(&self) -> u32 {
        self.pack_latent_candidates
    }

    pub fn pack_latent_fallbacks(&self) -> u32 {
        self.pack_latent_fallbacks
    }
}

pub(crate) fn observe_laws(
    canonical_expressions: &[SemanticExpr],
    semantic_evidence: &ScientificSemanticEvidence,
    context: &LawAnalysisContext<'_>,
) -> LawObservations {
    let source_index = SourceIndex::new(context.source);
    let recognition_context = RecognitionContext {
        source: context.source,
        source_index: &source_index,
        shapes: context.shapes,
        quantities: context.quantities,
        consistency: context.consistency,
        assumptions: context.assumptions,
        external: context.external,
    };
    let mut recognitions = Vec::<LawRecognition>::new();
    let mut equivalence_states = 0;
    let mut guard_checks = 0;
    let mut visited_rules = 0;
    let mut pack_frontier_candidates = 0;
    let mut pack_latent_candidates = 0;
    let mut pack_latent_fallbacks = 0;
    let mut actuals = Vec::new();
    for expression in canonical_expressions {
        let formula_range = context
            .formula_ranges
            .iter()
            .filter(|range| {
                range.start_offset <= expression.range.start_offset
                    && expression.range.end_offset <= range.end_offset
            })
            .min_by_key(|range| range.end_offset - range.start_offset);
        collect_law_expressions(expression, formula_range, &mut actuals);
    }
    for (actual, source_envelope) in actuals {
        let source_envelope =
            strip_formula_presentation(&source_envelope, context.source, &source_index);
        if !semantic_evidence.formula_is_asserted(&actual.range)
            || !formula_operations_are_well_typed(actual, semantic_evidence, context.shapes)
        {
            continue;
        }
        let alternatives = structural_alternatives(actual);
        let mut frontier = LAW_DISPATCH.candidates_for(&alternatives);
        let dominant_context_pack =
            dominant_frontier_context_pack(&frontier, context.domains, actual.range.start_offset);
        let recognition_start = recognitions.len();
        let mut traversed_latent = false;
        pack_frontier_candidates += frontier.len() as u32;
        frontier.sort_by_key(|compiled| {
            (
                !plan_matches_exact(&compiled.plan, actual),
                context
                    .domains
                    .relevance(compiled.pack_id, actual.range.start_offset)
                    .map_or(
                        if is_capability_pack(compiled.pack_id) {
                            25
                        } else {
                            30
                        },
                        |relevance| support_rank(relevance.support),
                    ),
            )
        });
        for compiled in frontier {
            if recognitions.len() >= MAX_LAW_MATCHES {
                break;
            }
            let relevance = context
                .domains
                .relevance(compiled.pack_id, actual.range.start_offset);
            let activation = semantic_evidence
                .law_activation(compiled.pack_id, &compiled.law.id, &actual.range)
                .or_else(|| {
                    context.external.law_activation(
                        actual.range.start_offset,
                        compiled.pack_id,
                        &compiled.law.id,
                    )
                });
            let role_context_activated = relevance.as_ref().is_some_and(|relevance| {
                matches!(
                    relevance.support,
                    DomainSupportTier::Explicit | DomainSupportTier::Supported
                )
            }) || dominant_context_pack == Some(compiled.pack_id);
            let exact_match = plan_matches_exact(&compiled.plan, actual);
            if !exact_match && recognitions.len() > recognition_start {
                continue;
            }
            let match_alternatives = if exact_match {
                std::slice::from_ref(actual)
            } else {
                alternatives.as_slice()
            };
            equivalence_states += (compiled.plan.forms.len() * match_alternatives.len()) as u32;
            let candidates = compiled
                .plan
                .forms
                .iter()
                .flat_map(|form| {
                    match_alternatives.iter().flat_map(move |alternative| {
                        unify_exact_all(
                            &form.expression,
                            alternative,
                            &compiled.plan.placeholders,
                            &BTreeMap::new(),
                        )
                        .into_iter()
                        .map(move |bindings| (Some(form), bindings))
                    })
                })
                .chain(variadic_balance(compiled, actual).map(|bindings| (None, bindings)))
                .take(MAX_UNIFICATION_CANDIDATES)
                .collect::<Vec<_>>();
            let attached_role_support = candidates.iter().any(|(_, bindings)| {
                bindings_have_formula_attached_declared_roles(
                    &compiled.law.roles,
                    bindings,
                    &actual.range,
                    context.consistency,
                    context.external,
                )
            });
            if !compiled.law.activation_phrases.is_empty()
                && activation.is_none()
                && (!attached_role_support
                    || recognitions[recognition_start..].iter().any(|recognized| {
                        laws_share_collision(
                            &recognized.pack_id,
                            &recognized.law_id,
                            compiled.pack_id,
                            &compiled.law.id,
                        )
                    }))
            {
                continue;
            }
            let latent = relevance.is_none()
                && !is_capability_pack(compiled.pack_id)
                && !attached_role_support;
            if latent
                && recognitions[recognition_start..].iter().all(|recognized| {
                    !laws_share_collision(
                        &recognized.pack_id,
                        &recognized.law_id,
                        compiled.pack_id,
                        &compiled.law.id,
                    )
                })
                && recognitions.len() > recognition_start
            {
                continue;
            }
            visited_rules += 1;
            if latent {
                pack_latent_candidates += 1;
                if !traversed_latent {
                    pack_latent_fallbacks += 1;
                    traversed_latent = true;
                }
            }
            let Some((matched_form, bindings)) = candidates.into_iter().find(|(_, bindings)| {
                let inferred_role = actual_output_role(actual, bindings);
                let supported = roles_are_supported(
                    &compiled.law.roles,
                    bindings,
                    inferred_role.as_deref(),
                    actual.range.start_offset,
                    role_context_activated,
                    activation.is_some(),
                    activation.is_some_and(|activation| activation.identifies_attached_formula),
                    context.shapes,
                    context.quantities,
                    context.consistency,
                    context.external,
                );
                let typed = expression_is_well_typed(actual, context.shapes);
                supported
                    && typed
                    && !law_conditions_refuted(
                        compiled.law,
                        bindings,
                        context.assumptions,
                        context.external.assumptions_at(actual.range.start_offset),
                    )
            }) else {
                continue;
            };
            guard_checks += matched_form.map_or(0, |form| form.guards.len() as u32);
            let mut recognized = recognition(
                compiled,
                actual,
                &source_envelope,
                bindings,
                matched_form,
                &recognition_context,
                activation,
            );
            recognized.rank = relevance.as_ref().map_or(
                if is_capability_pack(compiled.pack_id) {
                    25
                } else {
                    30
                },
                |value| support_rank(value.support),
            );
            recognized.relevance = relevance;
            recognitions.push(recognized);
        }
    }
    recognitions.sort_by_key(|recognition| {
        (
            recognition.range.start_offset,
            recognition.rank,
            recognition.pack_id.clone(),
            recognition.law_id.clone(),
        )
    });
    LawObservations {
        recognitions,
        equivalence_states,
        guard_checks,
        visited_rules,
        pack_frontier_candidates,
        pack_latent_candidates,
        pack_latent_fallbacks,
    }
}

fn dominant_frontier_context_pack<'a>(
    frontier: &[&'a CompiledLaw],
    domains: &DomainObservations,
    offset: u32,
) -> Option<&'a str> {
    let mut ranked = frontier
        .iter()
        .filter_map(|compiled| {
            domains
                .relevance(compiled.pack_id, offset)
                .map(|relevance| (support_rank(relevance.support), compiled.pack_id))
        })
        .collect::<Vec<_>>();
    ranked.sort_unstable();
    ranked.dedup();
    let best_rank = ranked.first()?.0;
    let mut best = ranked
        .iter()
        .filter(|(rank, _)| *rank == best_rank)
        .map(|(_, pack_id)| *pack_id);
    let pack_id = best.next()?;
    best.next().is_none().then_some(pack_id)
}

fn strip_formula_presentation(
    range: &SourceRange,
    source: &str,
    source_index: &SourceIndex,
) -> SourceRange {
    let mut start = source_index.byte_for_utf16(range.start_offset);
    let mut end = source_index.byte_for_utf16(range.end_offset);
    trim_formula_whitespace(source, &mut start, &mut end);

    while let Some(command_end) = presentation_command_end(source, start, end) {
        start = command_end;
        trim_formula_whitespace(source, &mut start, &mut end);
    }

    loop {
        let candidate = ["\\label", "\\tag"]
            .into_iter()
            .filter_map(|command| source[start..end].rfind(command).map(|at| start + at))
            .max();
        let Some(candidate) = candidate else { break };
        if presentation_command_end(source, candidate, end)
            .is_some_and(|after| source[after..end].chars().all(char::is_whitespace))
        {
            end = candidate;
            trim_formula_whitespace(source, &mut start, &mut end);
        } else {
            break;
        }
    }

    SourceRange {
        start_offset: source_index.utf16_for_byte(start),
        end_offset: source_index.utf16_for_byte(end),
    }
}

fn trim_formula_whitespace(source: &str, start: &mut usize, end: &mut usize) {
    while *start < *end
        && source[*start..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        *start += source[*start..].chars().next().unwrap().len_utf8();
    }
    while *start < *end
        && source[..*end]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        *end -= source[..*end].chars().next_back().unwrap().len_utf8();
    }
}

fn presentation_command_end(source: &str, start: usize, limit: usize) -> Option<usize> {
    let tail = &source[start..limit];
    let command = ["\\label", "\\tag"]
        .into_iter()
        .find(|command| tail.starts_with(command))?;
    let mut cursor = start + command.len();
    while cursor < limit
        && source[cursor..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        cursor += source[cursor..].chars().next()?.len_utf8();
    }
    if source.as_bytes().get(cursor) != Some(&b'{') {
        return None;
    }
    let mut depth = 0_u32;
    for (relative, character) in source[cursor..limit].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(cursor + relative + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_law_expressions<'a>(
    expression: &'a SemanticExpr,
    formula_range: Option<&SourceRange>,
    output: &mut Vec<(&'a SemanticExpr, SourceRange)>,
) {
    if let SemanticExprKind::System(expressions) = &expression.kind {
        for expression in expressions {
            collect_law_expressions(expression, formula_range, output);
        }
    } else {
        let source_envelope = formula_range
            .cloned()
            .unwrap_or_else(|| expression_source_envelope(expression));
        output.push((expression, source_envelope.clone()));
        for child in expression_children(expression) {
            collect_nested_law_expressions(child, &source_envelope, output);
        }
    }
}

fn expression_source_envelope(expression: &SemanticExpr) -> SourceRange {
    expression
        .provenance
        .iter()
        .fold(expression.range.clone(), |mut envelope, range| {
            envelope.start_offset = envelope.start_offset.min(range.start_offset);
            envelope.end_offset = envelope.end_offset.max(range.end_offset);
            envelope
        })
}

fn collect_nested_law_expressions<'a>(
    expression: &'a SemanticExpr,
    source_envelope: &SourceRange,
    output: &mut Vec<(&'a SemanticExpr, SourceRange)>,
) {
    if matches!(
        &expression.kind,
        SemanticExprKind::Apply { operator, .. }
            if NESTED_LAW_APPLICATIONS.contains(operator.as_str())
    ) {
        output.push((expression, source_envelope.clone()));
    }
    for child in expression_children(expression) {
        collect_nested_law_expressions(child, source_envelope, output);
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
    if !is_variadic_balance_candidate(actual) {
        return None;
    }
    let SemanticExprKind::Relation { left, right, .. } = &actual.kind else {
        unreachable!("a variadic balance candidate is an equality");
    };
    let mut terms = Vec::new();
    collect_balance_terms(left, &mut terms);
    collect_balance_terms(right, &mut terms);
    terms.retain(|term| !matches!(&term.kind, SemanticExprKind::Number(value) if value == "0"));
    Some(
        [(
            role.id.clone(),
            SemanticExpr {
                kind: SemanticExprKind::Sum(terms),
                range: actual.range.clone(),
                provenance: actual.provenance.clone(),
            },
        )]
        .into_iter()
        .collect(),
    )
}

fn is_variadic_balance_candidate(expression: &SemanticExpr) -> bool {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &expression.kind
    else {
        return false;
    };
    operator == "equals"
        && balance_expression(left)
        && balance_expression(right)
        && (balance_term_count(left) + balance_term_count(right) >= 3
            || contains_sum_operator(expression))
}

fn balance_term_count(expression: &SemanticExpr) -> usize {
    match &expression.kind {
        SemanticExprKind::Sum(terms) => terms.iter().map(balance_term_count).sum(),
        SemanticExprKind::Number(value) if value == "0" => 0,
        _ => 1,
    }
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
        SemanticExprKind::Binder { body, .. } => contains_sum_operator(body),
        SemanticExprKind::System(equations) => equations.iter().any(contains_sum_operator),
        SemanticExprKind::Piecewise(branches) => branches.iter().any(|branch| {
            contains_sum_operator(&branch.value)
                || branch.condition.as_ref().is_some_and(contains_sum_operator)
        }),
        _ => false,
    }
}

fn balance_expression(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. } => true,
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
            .map(|shape| ShapeInference::Known([vec![shape.kind], shape.dimensions].concat()))
            .unwrap_or(ShapeInference::Unknown),
        SemanticExprKind::Index { base, .. } => {
            let label = crate::canonical::expression_name(expression).unwrap_or_default();
            shapes
                .shape_at(&label, expression.range.start_offset)
                .map(|shape| ShapeInference::Known([vec![shape.kind], shape.dimensions].concat()))
                .unwrap_or_else(|| expression_shape(base, shapes))
        }
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
        SemanticExprKind::Binder { body, .. } => expression_shape(body, shapes),
        SemanticExprKind::System(equations) => {
            combine_equal_shapes(equations.iter().map(|item| expression_shape(item, shapes)))
        }
        SemanticExprKind::Piecewise(branches) => combine_equal_shapes(
            branches
                .iter()
                .map(|branch| expression_shape(&branch.value, shapes)),
        ),
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

pub(crate) fn unify_all(
    template: &SemanticExpr,
    actual: &SemanticExpr,
    placeholders: &BTreeSet<String>,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Vec<BTreeMap<String, SemanticExpr>> {
    structural_alternatives(actual)
        .iter()
        .flat_map(|alternative| unify_exact_all(template, alternative, placeholders, bindings))
        .take(MAX_UNIFICATION_CANDIDATES)
        .collect()
}

fn unify_exact_all(
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
            unify_exact_all(left, right, placeholders, bindings)
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
            left_variable.as_str(),
            right_variable.as_str(),
            placeholders,
            actual,
            bindings,
        )
        .into_iter()
        .flat_map(|candidate| unify_exact_all(left, right, placeholders, &candidate))
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
        ) if left.len() == right.len() => bind_name_all(
            left_operator.as_str(),
            right_operator.as_str(),
            placeholders,
            actual,
            bindings,
        )
        .into_iter()
        .flat_map(|candidate| {
            if matches!(left_operator.as_str(), "intersection" | "union") {
                commutative_unify_all(left, right, placeholders, &candidate)
            } else {
                unify_sequence(left.iter(), right.iter(), placeholders, &candidate)
            }
        })
        .collect(),
        (
            SemanticExprKind::Index {
                base: left_base,
                indices: left_indices,
            },
            SemanticExprKind::Index {
                base: right_base,
                indices: right_indices,
            },
        ) if left_indices.len() == right_indices.len() => unify_sequence(
            std::iter::once(left_base.as_ref()).chain(left_indices),
            std::iter::once(right_base.as_ref()).chain(right_indices),
            placeholders,
            bindings,
        ),
        (
            SemanticExprKind::Condition {
                value: left_value,
                predicate: left_predicate,
            },
            SemanticExprKind::Condition {
                value: right_value,
                predicate: right_predicate,
            },
        ) => unify_sequence(
            [left_value.as_ref(), left_predicate.as_ref()],
            [right_value.as_ref(), right_predicate.as_ref()],
            placeholders,
            bindings,
        ),
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

fn structural_alternatives(expression: &SemanticExpr) -> Vec<SemanticExpr> {
    let mut alternatives = vec![expression.clone()];
    if let Some(expanded) = expand_ambiguous_juxtaposition(expression)
        && expanded != *expression
    {
        alternatives.push(expanded);
    }
    alternatives
}

fn plan_matches_exact(plan: &LawMatchPlan, actual: &SemanticExpr) -> bool {
    plan.forms.iter().any(|form| {
        !unify_exact_all(
            &form.expression,
            actual,
            &plan.placeholders,
            &BTreeMap::new(),
        )
        .is_empty()
    }) || plan.variadic && is_variadic_balance_candidate(actual)
}

fn expand_ambiguous_juxtaposition(expression: &SemanticExpr) -> Option<SemanticExpr> {
    if let Some(factors) = ambiguous_factor(expression) {
        return Some(SemanticExpr {
            kind: SemanticExprKind::Product(factors),
            range: expression.range.clone(),
            provenance: expression.provenance.clone(),
        });
    }

    let expand = |child: &SemanticExpr| match expand_ambiguous_juxtaposition(child) {
        Some(expanded) => (expanded, true),
        None => (child.clone(), false),
    };
    let (kind, changed) = match &expression.kind {
        SemanticExprKind::Sum(items) => {
            let expanded = items.iter().map(expand).collect::<Vec<_>>();
            (
                SemanticExprKind::Sum(expanded.iter().map(|(item, _)| item.clone()).collect()),
                expanded.iter().any(|(_, changed)| *changed),
            )
        }
        SemanticExprKind::Product(items) => {
            let mut changed = false;
            let factors = items
                .iter()
                .flat_map(|item| {
                    if let Some(expanded) = ambiguous_factor(item) {
                        changed = true;
                        expanded
                    } else if let Some(expanded) = expand_ambiguous_juxtaposition(item) {
                        changed = true;
                        vec![expanded]
                    } else {
                        vec![item.clone()]
                    }
                })
                .collect();
            (SemanticExprKind::Product(factors), changed)
        }
        SemanticExprKind::Dot(left, right)
        | SemanticExprKind::Cross(left, right)
        | SemanticExprKind::Fraction(left, right)
        | SemanticExprKind::Power(left, right) => {
            let ((left, left_changed), (right, right_changed)) = (expand(left), expand(right));
            let kind = match &expression.kind {
                SemanticExprKind::Dot(_, _) => {
                    SemanticExprKind::Dot(Box::new(left), Box::new(right))
                }
                SemanticExprKind::Cross(_, _) => {
                    SemanticExprKind::Cross(Box::new(left), Box::new(right))
                }
                SemanticExprKind::Fraction(_, _) => {
                    SemanticExprKind::Fraction(Box::new(left), Box::new(right))
                }
                SemanticExprKind::Power(_, _) => {
                    SemanticExprKind::Power(Box::new(left), Box::new(right))
                }
                _ => unreachable!(),
            };
            (kind, left_changed || right_changed)
        }
        SemanticExprKind::Negate(inner) => {
            let (inner, changed) = expand(inner);
            (SemanticExprKind::Negate(Box::new(inner)), changed)
        }
        SemanticExprKind::Relation {
            operator,
            left,
            right,
        } => {
            let ((left, left_changed), (right, right_changed)) = (expand(left), expand(right));
            (
                SemanticExprKind::Relation {
                    operator: operator.clone(),
                    left: Box::new(left),
                    right: Box::new(right),
                },
                left_changed || right_changed,
            )
        }
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => {
            let arguments = arguments.iter().map(expand).collect::<Vec<_>>();
            (
                SemanticExprKind::Apply {
                    operator: operator.clone(),
                    arguments: arguments
                        .iter()
                        .map(|(argument, _)| argument.clone())
                        .collect(),
                },
                arguments.iter().any(|(_, changed)| *changed),
            )
        }
        _ => return None,
    };
    changed.then(|| SemanticExpr {
        kind,
        range: expression.range.clone(),
        provenance: expression.provenance.clone(),
    })
}

fn ambiguous_factor(expression: &SemanticExpr) -> Option<Vec<SemanticExpr>> {
    match &expression.kind {
        SemanticExprKind::Apply {
            operator,
            arguments,
        } if arguments.len() == 1 && !is_structural_application(operator.as_str()) => Some(vec![
            SemanticExpr {
                kind: SemanticExprKind::Symbol(operator.value.clone()),
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
            (arguments.len() == 1 && !is_structural_application(operator.as_str())).then(|| {
                vec![
                    SemanticExpr {
                        kind: SemanticExprKind::Symbol(operator.value.clone()),
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
        "compose"
            | "condition"
            | "curl"
            | "divergence"
            | "gradient"
            | "integral"
            | "intersection"
            | "laplacian"
            | "norm"
            | "partial-derivative"
            | "sum"
            | "transpose"
            | "union"
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
                .flat_map(|candidate| unify_exact_all(template, actual, placeholders, &candidate))
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
            for next in
                unify_exact_all(&template[index], &actual[candidate], placeholders, bindings)
            {
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

fn equivalent(left: &SemanticExpr, right: &SemanticExpr) -> bool {
    !unify_exact_all(left, right, &BTreeSet::new(), &BTreeMap::new()).is_empty()
}

#[allow(clippy::too_many_arguments)]
fn roles_are_supported(
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    inferred_role: Option<&str>,
    offset: u32,
    notation_context_activated: bool,
    law_explicitly_activated: bool,
    formula_identified: bool,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
) -> bool {
    let mut supported = 0;
    let mut supported_roles = BTreeSet::new();
    let mut unresolved = 0;
    let mut unresolved_role = None;
    for role in roles {
        let Some(expression) = bindings.get(&role.id) else {
            return false;
        };
        let symbols = semantic_symbols(expression);
        if symbols.is_empty() || !(role.variadic || role_expression_is_atomic(expression)) {
            return false;
        }
        let mut role_support = RoleSupport::Supported;
        for symbol in symbols {
            role_support = role_support.and(role_symbol_support(
                role,
                &symbol,
                offset,
                notation_context_activated,
                shapes,
                quantities,
                consistency,
                external,
            ));
        }
        match role_support {
            RoleSupport::Supported => {
                supported += 1;
                supported_roles.insert(role.id.as_str());
            }
            RoleSupport::Unresolved => {
                unresolved += 1;
                unresolved_role = Some(role.id.as_str());
            }
            RoleSupport::Refuted => return false,
        }
    }
    unresolved == 0
        || (unresolved == 1
            && supported >= 2
            && ((unresolved_role == inferred_role && roles.len() <= 3)
                || (roles.len() <= 3
                    && unresolved_role.is_some_and(|unresolved| {
                        roles
                            .iter()
                            .any(|role| role.id == unresolved && role.quantity.is_some())
                    }))
                || (unresolved_role != inferred_role && supported >= 3)))
        || (law_explicitly_activated
            && (inferred_role.map_or(supported >= 2, |role| supported_roles.contains(role))
                || (roles.len() == 2 && supported == 1 && unresolved == 1)))
        || formula_identified
}

fn actual_output_role(
    actual: &SemanticExpr,
    bindings: &BTreeMap<String, SemanticExpr>,
) -> Option<String> {
    let SemanticExprKind::Relation { left, .. } = &actual.kind else {
        return None;
    };
    let mut roles = bindings
        .iter()
        .filter(|(_, expression)| equivalent(expression, left))
        .map(|(role, _)| role.clone());
    let role = roles.next()?;
    roles.next().is_none().then_some(role)
}

fn bindings_have_formula_attached_declared_roles(
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    formula_range: &SourceRange,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
) -> bool {
    if roles.is_empty() {
        return false;
    }
    let offset = formula_range.start_offset;
    let all_roles_are_declared = roles.iter().all(|role| {
        bindings.get(&role.id).is_some_and(|expression| {
            let symbols = semantic_symbols(expression);
            !symbols.is_empty()
                && symbols.into_iter().all(|symbol| {
                    consistency
                        .roles_at(&symbol, offset)
                        .0
                        .iter()
                        .any(|claim| claim.concept_id == role.concept)
                        || external.has_role(offset, &symbol, &role.concept)
                })
        })
    });
    let one_role_is_attached_to_formula = roles.iter().any(|role| {
        bindings.get(&role.id).is_some_and(|expression| {
            semantic_symbols(expression).into_iter().any(|symbol| {
                consistency.roles_at(&symbol, offset).0.iter().any(|claim| {
                    claim.concept_id == role.concept
                        && claim.evidence.source_ranges.iter().any(|range| {
                            range.start_offset <= formula_range.start_offset
                                && formula_range.end_offset <= range.end_offset
                        })
                })
            })
        })
    });
    all_roles_are_declared && one_role_is_attached_to_formula
}

fn role_expression_is_atomic(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. } => true,
        SemanticExprKind::Power(base, exponent) if is_decorative_star(exponent) => {
            role_expression_is_atomic(base)
        }
        SemanticExprKind::Derivative { expression, .. } => role_expression_is_atomic(expression),
        SemanticExprKind::Apply { arguments, .. } => {
            arguments.iter().all(role_expression_is_atomic)
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoleSupport {
    Supported,
    Unresolved,
    Refuted,
}

impl RoleSupport {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Refuted, _) | (_, Self::Refuted) => Self::Refuted,
            (Self::Unresolved, _) | (_, Self::Unresolved) => Self::Unresolved,
            (Self::Supported, Self::Supported) => Self::Supported,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn role_symbol_support(
    role: &PackLawRole,
    symbol: &str,
    offset: u32,
    notation_context_activated: bool,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
    let notation_symbol = symbol;
    let required_quantity = role
        .quantity
        .as_deref()
        .map_or(RoleSupport::Supported, |quantity| {
            quantity_support(
                quantity,
                symbol,
                notation_symbol,
                offset,
                quantities,
                external,
            )
        });
    if required_quantity == RoleSupport::Refuted {
        return RoleSupport::Refuted;
    }
    let declared_roles = consistency.roles_at(symbol, offset).0;
    let has_exact_role = declared_roles
        .iter()
        .any(|claim| claim.concept_id == role.concept);
    let comparable_roles = declared_roles
        .iter()
        .filter(|claim| concepts_are_comparable(&role.concept, &claim.concept_id))
        .collect::<Vec<_>>();
    if comparable_roles
        .iter()
        .any(|claim| roles_conflict(&role.concept, &claim.concept_id))
    {
        return RoleSupport::Refuted;
    }
    let activated_notation_support = notation_context_activated
        && role
            .notation
            .iter()
            .any(|notation| notation_matches_symbol(notation, symbol));
    let mut matching_shape = false;
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
            return RoleSupport::Refuted;
        }
        match shapes
            .shape_at(symbol, offset)
            .or_else(|| shapes.shape_at(notation_symbol, offset))
        {
            Some(shape) if shape.kind != expected_shape => return RoleSupport::Refuted,
            Some(_) => matching_shape = true,
            None => {}
        }
        matching_shape |= external.has_shape(offset, symbol, expected_shape);
        if role.concept.split(':').next_back() == Some(expected_shape) {
            return required_quantity.and(if matching_shape {
                RoleSupport::Supported
            } else {
                RoleSupport::Unresolved
            });
        }
        if activated_notation_support && matching_shape {
            return required_quantity;
        }
    }
    let concept_support = if role.concept.starts_with("quantities-units:") {
        let support = quantity_support(
            &role.concept,
            symbol,
            notation_symbol,
            offset,
            quantities,
            external,
        );
        if support == RoleSupport::Unresolved && activated_notation_support {
            RoleSupport::Supported
        } else {
            support
        }
    } else if role.concept == "linear-algebra:linear-operator" {
        if matching_shape
            || shapes
                .shape_at(symbol, offset)
                .is_some_and(|shape| shape.kind == "matrix")
            || external.has_shape(offset, symbol, "matrix")
        {
            RoleSupport::Supported
        } else {
            RoleSupport::Unresolved
        }
    } else if has_exact_role
        || activated_notation_support
        || external.has_role(offset, symbol, &role.concept)
    {
        RoleSupport::Supported
    } else {
        RoleSupport::Unresolved
    };
    required_quantity.and(concept_support)
}

fn notation_matches_symbol(notation: &str, symbol: &str) -> bool {
    symbol == notation
        || symbol
            .strip_prefix(notation)
            .is_some_and(|suffix| matches!(suffix.chars().next(), Some('_' | '^')))
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
fn quantity_support(
    expected: &str,
    symbol: &str,
    notation_symbol: &str,
    offset: u32,
    quantities: &QuantityObservations,
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
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
            RoleSupport::Unresolved
        } else if declared.iter().all(|kind| *kind == expected) {
            RoleSupport::Supported
        } else {
            RoleSupport::Refuted
        };
    }
    if external.has_quantity(offset, symbol, expected) {
        RoleSupport::Supported
    } else {
        RoleSupport::Unresolved
    }
}

fn recognition(
    compiled: &CompiledLaw,
    actual: &SemanticExpr,
    source_envelope: &SourceRange,
    bindings: BTreeMap<String, SemanticExpr>,
    matched_form: Option<&GuardedForm>,
    context: &RecognitionContext<'_>,
    activation: Option<&LawActivationEvidence>,
) -> LawRecognition {
    let formula_range = formula_source_range(source_envelope, context);
    let formula_evidence = Evidence {
        rule_id: "semantic-law-unification".into(),
        kind: "canonical-math".into(),
        strength: "hard".into(),
        source_ranges: vec![formula_range.clone()],
    };
    let formula_bindings = compiled
        .law
        .roles
        .iter()
        .filter_map(|role| {
            let expression = bindings.get(&role.id)?;
            let symbol = if role.variadic {
                variadic_labels(expression, context).join("; ")
            } else {
                source_expression_label(expression, context)?
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
                variadic_labels(expression, context)
            } else {
                vec![source_expression_label(expression, context)?]
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
                condition,
                &bindings,
                &actual.range,
                context.shapes,
                context.quantities,
                context.consistency,
                context.assumptions,
                context.external,
                activation.map(|activation| &activation.evidence),
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
            context.external.assumptions_at(formula_range.start_offset),
            &formula_range,
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
    if let Some(activation) = activation {
        push_evidence(&mut evidence, activation.evidence.clone());
    }
    if let Some(form) = matched_form {
        evidence.extend(form.steps.iter().map(|step| Evidence {
            rule_id: format!("guarded-equivalence/{}", step.id()),
            kind: "equivalence-proof".into(),
            strength: "hard".into(),
            source_ranges: vec![formula_range.clone()],
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
        range: formula_range.clone(),
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
            range: formula_range,
        }),
        evidence,
        relevance: None,
        rank: 100,
    }
}

fn formula_source_range(range: &SourceRange, context: &RecognitionContext<'_>) -> SourceRange {
    let mut range = range.clone();
    let mut cursor = context.source_index.byte_for_utf16(range.end_offset);
    while context.source[cursor..]
        .chars()
        .next()
        .is_some_and(|character| matches!(character, ' ' | '\t'))
    {
        cursor += context.source[cursor..]
            .chars()
            .next()
            .expect("checked character")
            .len_utf8();
    }
    if let Some(character) = context.source[cursor..].chars().next()
        && matches!(character, '.' | ',' | ';' | ':')
    {
        range.end_offset = context
            .source_index
            .utf16_for_byte(cursor + character.len_utf8());
    }
    range
}

fn equivalence_conditions(
    form: &GuardedForm,
    bindings: &BTreeMap<String, SemanticExpr>,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
    formula_range: &crate::SourceRange,
) -> Vec<LawConditionInfo> {
    form.guards
        .iter()
        .enumerate()
        .map(|(index, guard)| match instantiate_guard(guard, bindings) {
            EquivalenceGuard::Nonzero(subject) => {
                let symbols = semantic_symbols(&subject);
                let (status, mut evidence) =
                    nonzero_status(&subject, &symbols, assumptions, external_assumptions);
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
    external_assumptions: &[AssumptionInfo],
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
        .chain(external_assumptions)
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
    if mechanically_verified {
        return ConstraintStatus::Verified;
    }
    match kind {
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
    condition: &PackLawCondition,
    bindings: &BTreeMap<String, SemanticExpr>,
    formula_range: &SourceRange,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
    law_activation: Option<&Evidence>,
) -> (Vec<Evidence>, bool) {
    let offset = formula_range.start_offset;
    let kind = condition.kind;
    let subjects = &condition.subjects;
    let mut evidence = Vec::new();
    let mut proved_subjects = 0;
    let shared_context = (kind == PackConditionKind::SameContext)
        .then(|| {
            same_context_evidence(
                subjects,
                bindings,
                offset,
                assumptions,
                external.assumptions_at(offset),
                consistency,
            )
        })
        .flatten();
    let semantic_condition = assumption_condition_evidence(
        condition,
        bindings,
        formula_range,
        assumptions,
        external.assumptions_at(offset),
    );
    if let Some(condition_evidence) = &semantic_condition {
        push_evidence(&mut evidence, condition_evidence.clone());
    }
    if kind == PackConditionKind::DomainMembership
        && let Some(law_activation) = law_activation
    {
        push_evidence(&mut evidence, law_activation.clone());
        return (evidence, true);
    }
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
                        .filter(|shape| shape.dimensions.iter().all(|dimension| dimension != "?"))
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
                    PackConditionKind::SameContext => shared_context.clone().into_iter().collect(),
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
    (
        evidence,
        semantic_condition.is_some() || proved_subjects == subjects.len(),
    )
}

const MAX_ASSUMPTION_DISTANCE: u32 = 640;

fn assumption_condition_evidence(
    condition: &PackLawCondition,
    bindings: &BTreeMap<String, SemanticExpr>,
    formula_range: &SourceRange,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
) -> Option<Evidence> {
    let symbols = bound_condition_symbols(&condition.subjects, bindings);
    let subjects_match = |assumption: &&AssumptionInfo| {
        assumption.subjects.is_empty()
            || assumption
                .subjects
                .iter()
                .all(|subject| symbols.contains(subject))
    };
    assumptions
        .iter()
        .filter(|assumption| {
            let start = assumption
                .evidence
                .source_ranges
                .iter()
                .map(|range| range.start_offset)
                .min()
                .unwrap_or_default();
            let end = assumption
                .evidence
                .source_ranges
                .iter()
                .map(|range| range.end_offset)
                .max()
                .unwrap_or_default();
            let precedes_formula = end <= formula_range.start_offset
                && formula_range.start_offset - end <= MAX_ASSUMPTION_DISTANCE;
            let attaches_after_formula = assumption.evidence.kind == "attached-prose"
                && formula_range.end_offset <= start
                && start - formula_range.end_offset <= MAX_ASSUMPTION_DISTANCE;
            (precedes_formula || attaches_after_formula) && subjects_match(assumption)
        })
        .chain(external_assumptions.iter().filter(subjects_match))
        .find(|assumption| {
            if assumption.value == condition.id {
                return true;
            }
            match condition.kind {
                PackConditionKind::Assumption => false,
                PackConditionKind::Differentiable => {
                    assumption.kind == "regularity" && assumption.value == "differentiable"
                }
                PackConditionKind::Positive => {
                    assumption.kind == "sign"
                        && matches!(assumption.value.as_str(), "positive" | "strictly-positive")
                }
                PackConditionKind::SignConvention => {
                    assumption.kind == "sign-convention" && !assumption.value.starts_with("not-")
                }
                PackConditionKind::Uniform => {
                    assumption.kind == "uniformity" && assumption.value == "uniform"
                }
                PackConditionKind::DomainMembership
                | PackConditionKind::SameContext
                | PackConditionKind::ShapeCompatible => false,
            }
        })
        .map(|assumption| assumption.evidence.clone())
}

fn same_context_evidence(
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
    offset: u32,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
    consistency: &RoleObservations,
) -> Option<Evidence> {
    if same_context_is_supported(subjects, bindings) {
        return Some(Evidence {
            rule_id: "canonical-shared-argument".into(),
            kind: "canonical-binding".into(),
            strength: "hard".into(),
            source_ranges: subjects
                .iter()
                .filter_map(|subject| bindings.get(subject))
                .map(|expression| expression.range.clone())
                .collect(),
        });
    }
    let symbols = bound_condition_symbols(subjects, bindings);
    if symbols.is_empty() {
        return None;
    }
    assumptions
        .iter()
        .chain(external_assumptions)
        .find(|assumption| {
            assumption.kind == "context"
                && symbols
                    .iter()
                    .all(|symbol| assumption.subjects.contains(symbol))
        })
        .map(|assumption| assumption.evidence.clone())
        .or_else(|| shared_role_context_evidence(&symbols, offset, consistency))
}

fn shared_role_context_evidence(
    symbols: &BTreeSet<String>,
    offset: u32,
    consistency: &RoleObservations,
) -> Option<Evidence> {
    let groups = symbols
        .iter()
        .map(|symbol| {
            consistency
                .roles_at(symbol, offset)
                .0
                .into_iter()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let first = groups.first()?;
    let seed = first.iter().find(|seed| {
        groups[1..].iter().all(|group| {
            group.iter().any(|other| {
                other.concept_id == seed.concept_id
                    && evidence_ranges_overlap(&seed.evidence, &other.evidence)
            })
        })
    })?;
    let mut source_ranges = seed.evidence.source_ranges.clone();
    for group in &groups[1..] {
        let other = group.iter().find(|other| {
            other.concept_id == seed.concept_id
                && evidence_ranges_overlap(&seed.evidence, &other.evidence)
        })?;
        source_ranges.extend(other.evidence.source_ranges.iter().cloned());
    }
    source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    source_ranges.dedup();
    Some(Evidence {
        rule_id: "shared-role-context".into(),
        kind: "attached-prose".into(),
        strength: "strong".into(),
        source_ranges,
    })
}

fn evidence_ranges_overlap(left: &Evidence, right: &Evidence) -> bool {
    left.source_ranges.iter().any(|left| {
        right.source_ranges.iter().any(|right| {
            left.start_offset < right.end_offset && right.start_offset < left.end_offset
        })
    })
}

fn law_conditions_refuted(
    law: &PackLaw,
    bindings: &BTreeMap<String, SemanticExpr>,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
) -> bool {
    law.conditions.iter().any(|condition| {
        let symbols = bound_condition_symbols(&condition.subjects, bindings);
        if symbols.is_empty() {
            return false;
        }
        assumptions
            .iter()
            .chain(external_assumptions)
            .any(|assumption| {
                let refutes = match condition.kind {
                    PackConditionKind::SameContext => {
                        assumption.kind == "context" && assumption.value == "different-context"
                    }
                    PackConditionKind::SignConvention => {
                        assumption.kind == "sign-convention" && assumption.value.starts_with("not-")
                    }
                    _ => false,
                };
                refutes
                    && (assumption.subjects.is_empty()
                        || assumption
                            .subjects
                            .iter()
                            .all(|subject| symbols.contains(subject)))
            })
    })
}

fn bound_condition_symbols(
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
) -> BTreeSet<String> {
    subjects
        .iter()
        .filter_map(|subject| bindings.get(subject))
        .flat_map(semantic_symbols)
        .collect()
}

fn same_context_is_supported(
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
) -> bool {
    let Some(argument_lists) = subjects
        .iter()
        .map(|subject| {
            let expression = bindings.get(subject)?;
            let SemanticExprKind::Apply { arguments, .. } = &expression.kind else {
                return None;
            };
            (!arguments.is_empty()).then_some(arguments)
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some((first, rest)) = argument_lists.split_first() else {
        return false;
    };
    rest.iter().all(|arguments| {
        arguments.len() == first.len()
            && arguments
                .iter()
                .zip(first.iter())
                .all(|(left, right)| equivalent(left, right))
    })
}

fn push_evidence(items: &mut Vec<Evidence>, evidence: Evidence) {
    if !items.contains(&evidence) {
        items.push(evidence);
    }
}

fn semantic_symbol(expression: &SemanticExpr) -> Option<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => Some(symbol.clone()),
        SemanticExprKind::Index { .. } => crate::canonical::expression_name(expression),
        SemanticExprKind::Derivative { expression, .. } => semantic_symbol(expression),
        SemanticExprKind::Apply {
            operator,
            arguments: _,
        } if operator == "sum" => None,
        SemanticExprKind::Apply {
            operator,
            arguments: _,
        } if operator != "transpose" => Some(operator.value.clone()),
        _ => None,
    }
}

fn semantic_symbols(expression: &SemanticExpr) -> Vec<String> {
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
        SemanticExprKind::Power(base, exponent) if is_decorative_star(exponent) => {
            semantic_symbols(base)
        }
        SemanticExprKind::Power(base, _) if contains_sum_operator(base) => Vec::new(),
        _ => semantic_symbol(expression).into_iter().collect(),
    }
}

fn expression_label(expression: &SemanticExpr) -> Option<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => Some(symbol.clone()),
        SemanticExprKind::Power(base, exponent) if is_decorative_star(exponent) => {
            Some(format!("{}^*", expression_label(base)?))
        }
        SemanticExprKind::Derivative { expression, .. } => expression_label(expression),
        SemanticExprKind::Index { .. } => crate::canonical::expression_name(expression),
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

fn source_expression_label(
    expression: &SemanticExpr,
    context: &RecognitionContext<'_>,
) -> Option<String> {
    let canonical = expression_label(expression)?;
    let start = context
        .source_index
        .byte_for_utf16(expression.range.start_offset);
    let mut end = context
        .source_index
        .byte_for_utf16(expression.range.end_offset);
    if matches!(
        expression.kind,
        SemanticExprKind::Apply { .. } | SemanticExprKind::Derivative { .. }
    ) && context.source.as_bytes().get(end) == Some(&b')')
    {
        end += 1;
    }
    let authored = context
        .source
        .get(start..end)
        .map(str::trim)
        .filter(|label| source_label_matches_expression(expression, label));
    Some(authored.unwrap_or(&canonical).to_owned())
}

fn source_label_matches_expression(expression: &SemanticExpr, label: &str) -> bool {
    if label.is_empty() || !expression.provenance.is_empty() {
        return false;
    }
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => {
            label == symbol || label.strip_prefix('\\') == Some(symbol.as_str())
        }
        SemanticExprKind::Index { .. } => !label.chars().any(char::is_whitespace),
        SemanticExprKind::Derivative { .. } => {
            label.starts_with("\\dot")
                || label.starts_with("\\ddot")
                || label.starts_with("\\frac")
                || label.contains('\'')
        }
        SemanticExprKind::Power(_, exponent) if is_decorative_star(exponent) => {
            !label.chars().any(char::is_whitespace)
        }
        _ => false,
    }
}

fn is_decorative_star(expression: &SemanticExpr) -> bool {
    matches!(
        &expression.kind,
        SemanticExprKind::Symbol(value) | SemanticExprKind::Unknown(value) if value == "*"
    )
}

fn variadic_labels(expression: &SemanticExpr, context: &RecognitionContext<'_>) -> Vec<String> {
    match &expression.kind {
        SemanticExprKind::Sum(items) => items
            .iter()
            .flat_map(|item| variadic_labels(item, context))
            .collect(),
        SemanticExprKind::Negate(inner) => variadic_labels(inner, context),
        SemanticExprKind::Product(items) if contains_sum_operator(expression) => items
            .iter()
            .filter(|item| !contains_sum_operator(item))
            .filter_map(|item| source_expression_label(item, context))
            .collect(),
        _ => source_expression_label(expression, context)
            .into_iter()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        COMPILED_LAWS, ExternalTypeEnvironment, LAW_DISPATCH, LawAnalysisContext, LawDispatch,
        LawObservations, collect_law_expressions, observe_laws, strip_formula_presentation,
        structural_alternatives, unify_all,
    };
    use crate::canonical::{SemanticExpr, SemanticExprKind, lower_document_region, lower_template};
    use crate::consistency::observe_roles;
    use crate::domain_signature::laws_share_collision;
    use crate::parser::{ParsedMath, parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::quantity::observe_quantities;
    use crate::shape::observe_shapes;
    use crate::{
        ConstraintStatus, DocumentLanguage, LawRecognition, LawRecognitionStatus, ProjectDocument,
        ScientificConstraintKind, SourceIndex, SourceRange,
    };

    fn canonical_expressions(
        document: &ProjectDocument,
        parsed: &[ParsedMath],
    ) -> Vec<SemanticExpr> {
        parsed
            .iter()
            .map(|math| lower_document_region(document, &math.region.content_range))
            .collect()
    }

    #[test]
    fn callable_role_placeholders_bind_the_operator_and_arguments_once() {
        let template = lower_template("objective(variable) = value");
        let actual = lower_template("f(x) = y");
        let placeholders = ["objective", "variable", "value"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let bindings = unify_all(&template, &actual, &placeholders, &BTreeMap::new());

        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].contains_key("objective"));
        assert!(bindings[0].contains_key("variable"));
        assert!(bindings[0].contains_key("value"));
    }

    #[test]
    fn attaches_an_explicit_law_name_to_the_immediately_following_formula() {
        let source = "For diffusive mass transport, let $J$ denote species flux, $D$ diffusivity, and $c$ concentration. Then $J=-D\\nabla c$.";
        let template = lower_template("flux = -diffusivity \\nabla concentration");
        let actual = lower_template("J = -D \\nabla c");
        let placeholders = ["flux", "diffusivity", "concentration"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            !unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty(),
            "template={template:?} actual={actual:?}"
        );
        assert_eq!(recognized_laws(source), ["fick-diffusion"]);

        let unrelated = "Fick's law is one diffusion model. This paragraph changes topic. Let $J$ denote species flux, $D$ diffusivity, and $c$ concentration. Then $J=-D\\nabla c$.";
        assert!(
            !recognized_laws(unrelated)
                .iter()
                .any(|law| law == "fick-diffusion")
        );
    }

    #[test]
    fn explicit_equation_references_attach_later_law_evidence_to_the_named_formula() {
        let source = "$P=Fv\\label{eq:power}$ The mesh is fixed. The time step is fixed. Equation~\\eqref{eq:power} is the mechanical power relation.";
        assert_eq!(recognized_laws(source), ["mechanical-power"]);
    }

    #[test]
    fn postposed_formula_references_attach_a_law_name_to_the_nearby_formula() {
        assert_eq!(
            recognized_laws("$P=Fv$. This relation is the mechanical power relation."),
            ["mechanical-power"]
        );
    }

    #[test]
    fn recognizes_new_probability_and_learning_relations_from_explicit_roles() {
        assert_eq!(
            recognized_laws(
                "Let $A$ and $B$ be events. Conditional probability is $P(A\\mid B)=P(A\\cap B)/P(B)$."
            ),
            ["conditional-probability", "event-intersection"]
        );
        assert_eq!(
            recognized_laws(
                "Let $A$ and $B$ be events. Conditional probability is $\\mathbb{P}(A\\mid B)=\\frac{\\mathbb{P}(A\\cap B)}{\\mathbb{P}(B)}$."
            ),
            ["conditional-probability", "event-intersection"]
        );
        assert_eq!(
            recognized_laws(
                "For label $y$, predicted probability $p$, and binary cross-entropy loss $L$, use $L=-y\\log p-(1-y)\\log(1-p)$."
            ),
            ["binary-cross-entropy"]
        );
    }

    #[test]
    fn recognizes_v025_vertical_relations() {
        let wave_template = lower_template(
            "\\nabla^2 field = \\frac{1}{speed^2} \\frac{\\partial^2 field}{\\partial time^2}",
        );
        let wave_actual =
            lower_template("\\nabla^2 u = \\frac{1}{c^2} \\frac{\\partial^2 u}{\\partial t^2}");
        let wave_placeholders = ["field", "speed", "time"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            !unify_all(
                &wave_template,
                &wave_actual,
                &wave_placeholders,
                &BTreeMap::new()
            )
            .is_empty(),
            "wave template={wave_template:?} actual={wave_actual:?}"
        );
        for (source, expected) in [
            (
                "For integrable random variables $X$ and $Y$ and scalars $a,b$, expectation obeys $\\operatorname{E}(aX+bY)=a\\operatorname{E}(X)+b\\operatorname{E}(Y)$.",
                "expectation-linearity",
            ),
            (
                "For a Newtonian fluid, let $\\tau$ be shear stress, $\\mu$ dynamic viscosity, and $\\dot{\\gamma}$ shear rate. The constitutive relation is $\\tau=\\mu\\dot{\\gamma}$.",
                "newtonian-shear",
            ),
            (
                "Let $u$ denote a scalar wave field, $c$ wave speed, and $t$ time. The homogeneous wave equation is $\\nabla^2u=\\frac{1}{c^2}\\frac{\\partial^2u}{\\partial t^2}$.",
                "scalar-wave-equation",
            ),
            (
                "Let $x_k$ be state, $u_k$ control input, $A$ state matrix, and $B$ input matrix. The discrete state equation is $x_{k+1}=Ax_k+Bu_k$.",
                "discrete-state-equation",
            ),
        ] {
            assert!(
                recognized_laws(source).iter().any(|law| law == expected),
                "{expected}: {source}"
            );
        }
    }

    #[test]
    fn recognizes_stationarity_with_a_decorated_optimum() {
        let template = lower_template("\\nabla objective(variable) = 0");
        let actual = lower_template("\\nabla f(x^*) = 0");
        let placeholders = ["objective", "variable"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            !unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty(),
            "template={template:?} actual={actual:?}"
        );
        let source = "At an unconstrained differentiable optimum $x^*$ of objective $f$, first-order stationarity requires $\\nabla f(x^*)=0$.";
        assert!(
            recognized_laws(source)
                .iter()
                .any(|law| law == "first-order-stationarity")
        );
    }

    #[test]
    fn indexed_dispatch_is_complete_against_exhaustive_unification() {
        for compiled in &*COMPILED_LAWS {
            for actual in &compiled.plan.forms {
                let exhaustive = COMPILED_LAWS
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| {
                        candidate.plan.forms.iter().any(|form| {
                            !unify_all(
                                &form.expression,
                                &actual.expression,
                                &candidate.plan.placeholders,
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
                        candidate.plan.forms.iter().any(|form| {
                            !unify_all(
                                &form.expression,
                                &actual.expression,
                                &candidate.plan.placeholders,
                                &BTreeMap::new(),
                            )
                            .is_empty()
                        })
                    })
                    .collect::<BTreeSet<_>>();
                assert_eq!(indexed, exhaustive, "{}", compiled.law.id);
                for index in exhaustive {
                    let candidate = &COMPILED_LAWS[index];
                    if candidate.pack_id != compiled.pack_id || candidate.law.id != compiled.law.id
                    {
                        assert!(
                            laws_share_collision(
                                compiled.pack_id,
                                &compiled.law.id,
                                candidate.pack_id,
                                &candidate.law.id,
                            ),
                            "collision atlas omitted {}:{} and {}:{}",
                            compiled.pack_id,
                            compiled.law.id,
                            candidate.pack_id,
                            candidate.law.id,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn dispatch_covers_unique_and_collision_heavy_hundreds_of_synthetic_packs() {
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

            let collision_forms = (0..pack_count)
                .map(|index| lower_template(&format!("out{index} = factor{index} input{index}")))
                .collect::<Vec<_>>();
            let mut collision_dispatch = LawDispatch::default();
            for (index, form) in collision_forms.iter().enumerate() {
                let placeholders = [
                    format!("out{index}"),
                    format!("factor{index}"),
                    format!("input{index}"),
                ]
                .into_iter()
                .collect::<BTreeSet<_>>();
                collision_dispatch.insert(
                    index,
                    &[crate::equivalence::GuardedForm {
                        expression: form.clone(),
                        guards: Vec::new(),
                        steps: Vec::new(),
                    }],
                    &placeholders,
                    false,
                );
            }
            assert_eq!(
                collision_dispatch
                    .candidate_indices(&collision_forms[pack_count - 1])
                    .len(),
                pack_count,
                "the collision fixture must not accidentally become uniquely keyed",
            );
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
    fn dispatch_considers_variadic_balances_only_for_sums() {
        let variadic = COMPILED_LAWS
            .iter()
            .enumerate()
            .filter_map(|(index, compiled)| {
                compiled
                    .law
                    .roles
                    .iter()
                    .any(|role| role.variadic)
                    .then_some(index)
            })
            .collect::<BTreeSet<_>>();
        assert!(!variadic.is_empty());

        let alias_candidates = LAW_DISPATCH
            .candidate_indices(&lower_template("reported = ECE"))
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert!(alias_candidates.is_disjoint(&variadic));

        let balance_candidates = LAW_DISPATCH
            .candidate_indices(&lower_template("a + b = c"))
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert!(!balance_candidates.is_disjoint(&variadic));
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
                unify_all(&form.expression, &actual, &placeholders, &BTreeMap::new())
                    .into_iter()
                    .next()
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
        assert!(
            !unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty(),
            "{actual:?}"
        );
    }

    #[test]
    fn norm_application_is_not_silently_erased_by_unification() {
        let template = lower_template("energy = state^2");
        let actual = lower_template("energy = \\lVert state \\rVert^2");
        let placeholders = ["energy"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert!(unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn structural_alternatives_expand_ambiguous_application_once_at_any_depth() {
        let actual = lower_template("output = coefficient(input)");
        let alternatives = structural_alternatives(&actual);
        assert_eq!(alternatives.len(), 2);
        assert_eq!(structural_alternatives(&alternatives[1]).len(), 1);
    }

    #[test]
    fn directional_relations_remain_ordered_while_set_union_is_commutative() {
        let placeholders = BTreeSet::new();
        assert!(
            unify_all(
                &lower_template("x \\in A"),
                &lower_template("A \\in x"),
                &placeholders,
                &BTreeMap::new(),
            )
            .is_empty()
        );
        assert!(
            !unify_all(
                &lower_template("A \\cup B"),
                &lower_template("B \\cup A"),
                &placeholders,
                &BTreeMap::new(),
            )
            .is_empty()
        );
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
    fn a_unique_frontier_domain_prior_can_activate_existing_notation_support() {
        assert_eq!(
            recognized_laws(
                "For the inductor, the passive sign convention is used. $v_L=L\\frac{di_L}{dt}$."
            ),
            ["inductor-voltage-law"]
        );
        assert!(
            recognized_laws("An electric circuit model is compared with electromagnetism. $P=VI$.")
                .is_empty()
        );
    }

    #[test]
    fn semantic_law_title_heads_activate_existing_pack_conditions() {
        assert_eq!(
            recognized_laws("The Reynolds number is $R_D=\\frac{\\rho vD}{\\mu}$."),
            ["reynolds-number-definition"]
        );
        assert_eq!(
            recognized_laws(
                "Inside the calibrated interval the Newtonian shear relation is $\\tau=\\mu\\dot\\gamma$."
            ),
            ["newtonian-shear"]
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
    fn an_explicit_law_name_can_complete_one_unrefuted_role() {
        assert_eq!(
            recognized_laws(
                "Let $T$ denote signal period. The period-frequency reciprocity is $f=1/T$."
            ),
            ["period-frequency-reciprocity"]
        );
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
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &canonical, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let domains = crate::domain::observe_domains(
            &document,
            crate::scope::ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        );
        let laws = observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &LawAnalysisContext {
                source,
                formula_ranges: &regions
                    .iter()
                    .map(|region| region.content_range.clone())
                    .collect::<Vec<_>>(),
                shapes: &shapes,
                quantities: &quantities,
                consistency: &roles,
                assumptions: &prose.assumptions,
                external: &external,
                domains: &domains,
            },
        );
        let recognition = &laws.all()[0];
        assert_eq!(recognition.law_id, "mechanical-power");
        assert_eq!(recognition.status, LawRecognitionStatus::ConditionMissing);
        assert!(recognition.conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::SameContext
                && condition.status == ConstraintStatus::Required
        }));
    }

    #[test]
    fn recognizes_reversed_typed_mechanical_power() {
        assert_eq!(
            recognized_laws(
                "At the measured instant, $P$ denotes power while $\\mathbf{F}$ and $\\mathbf{v}$ denote force and velocity vectors. Commutativity gives $P$ denotes power scalar. $\\mathbf{F}$ denotes force. $\\mathbf{v}$ denotes velocity. $\\mathbf{v}\\cdot\\mathbf{F}=P$"
            ),
            ["mechanical-power"]
        );
    }

    #[test]
    fn formula_first_where_clause_supplies_typed_roles_to_the_attached_equation() {
        let source = "$V=IR$, where $V$ denotes voltage, $I$ electric current, and $R$ resistance.";
        assert_eq!(recognized_laws(source), ["ohm-law"]);

        let observations = recognized_law_observations(
            r"The volumetric flow rate $Q$ equals the area $A$ times a measured speed.
              \[Q=Av.\] Here $v$ is the section-averaged normal speed.",
        );
        let flow = observations
            .iter()
            .find(|recognition| recognition.law_id == "volumetric-flow-rate")
            .expect("volumetric flow relation");
        assert_eq!(flow.status, LawRecognitionStatus::Verified);

        let observations = recognized_law_observations(
            r"The volumetric flow rate $Q$ equals the area $A$ times a measured speed.
              \[Q=Av.\] Later testing calls $v$ the section-averaged normal speed.",
        );
        let flow = observations
            .iter()
            .find(|recognition| recognition.law_id == "volumetric-flow-rate")
            .expect("volumetric flow relation");
        assert_eq!(flow.status, LawRecognitionStatus::ConditionMissing);
    }

    #[test]
    fn domain_ordering_prunes_only_noncolliding_latent_laws_after_a_match() {
        let source = "In control systems, $\\dot{x}=Ax+Bu$, where $x$ is the state vector, $u$ is the control input vector, $A$ is the state matrix, and $B$ is the input matrix.";
        let observations = law_observations(source);
        assert_eq!(
            observations
                .all()
                .iter()
                .map(|recognition| recognition.law_id.as_str())
                .collect::<Vec<_>>(),
            ["continuous-state-equation"]
        );
        assert_eq!(observations.pack_latent_fallbacks(), 1);
        assert!(observations.visited_rules() < observations.pack_frontier_candidates());
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
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &canonical, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let domains = crate::domain::observe_domains(
            &document,
            crate::scope::ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        );
        let laws = observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &LawAnalysisContext {
                source,
                formula_ranges: &regions
                    .iter()
                    .map(|region| region.content_range.clone())
                    .collect::<Vec<_>>(),
                shapes: &shapes,
                quantities: &quantities,
                consistency: &roles,
                assumptions: &prose.assumptions,
                external: &external,
                domains: &domains,
            },
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
    fn recognizes_a_discrete_state_equation_with_arbitrary_role_symbols() {
        let source = "For discrete state equation, suppose $r$ is n-dimensional system state vector, $a$ is n by n state matrix, $b$ is n-dimensional system state vector, $j$ is n by n input matrix, and $p$ is n-dimensional control input vector. $r = a b + j p$";
        assert_eq!(recognized_laws(source), ["discrete-state-equation"]);
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
    fn pack_compiled_expression_laws_remain_visible_inside_larger_formulas() {
        let source = "Let $A$, $B$, and $C$ be sets. The overlap is defined by $A\\cap B=C$.";
        let recognized = recognized_law_observations(source);
        assert_eq!(
            recognized
                .iter()
                .map(|recognition| recognition.law_id.as_str())
                .collect::<Vec<_>>(),
            ["set-intersection"]
        );
        let range = &recognized[0].range;
        assert_eq!(
            &source[range.start_offset as usize..range.end_offset as usize],
            "A\\cap B=C"
        );
        assert!(
            recognized_laws("No set roles are declared for $Q\\cap R=S$.")
                .iter()
                .all(|law| law != "set-intersection")
        );
    }

    #[test]
    fn formula_evidence_excludes_leading_and_trailing_presentation_commands() {
        let display = "\\label{eq:set}\n A\\cap B=C. \\tag{4}";
        let source_index = SourceIndex::new(display);
        let formula = SourceRange {
            start_offset: 0,
            end_offset: source_index.utf16_for_byte(display.len()),
        };
        let range = strip_formula_presentation(&formula, display, &source_index);
        assert_eq!(
            &display[source_index.byte_for_utf16(range.start_offset)
                ..source_index.byte_for_utf16(range.end_offset)],
            "A\\cap B=C."
        );
    }

    #[test]
    fn neighboring_relations_share_their_authored_formula_envelope() {
        let source = "B=A^T,\\qquad C=AB";
        let expression = lower_template(source);
        let formula_range = SourceRange {
            start_offset: 0,
            end_offset: source.encode_utf16().count() as u32,
        };
        let mut actuals = Vec::new();
        collect_law_expressions(&expression, Some(&formula_range), &mut actuals);
        let ranges = actuals
            .iter()
            .filter_map(|(expression, range)| {
                matches!(expression.kind, SemanticExprKind::Relation { .. })
                    .then_some(range.clone())
            })
            .collect::<Vec<_>>();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges, [formula_range.clone(), formula_range]);
    }

    #[test]
    fn same_context_condition_accepts_shared_evidence_and_refuses_separate_contexts() {
        let shared = recognized_law_observations(
            "For two events $A$ and $B$, consider their joint occurrence $A \\cap B$.",
        );
        assert_eq!(shared[0].law_id, "event-intersection");
        assert_eq!(shared[0].status, LawRecognitionStatus::Verified);
        assert_eq!(
            recognized_laws(
                "Events $A$ and $B$ belong to the same probability space. Their joint event is $A \\cap B$.",
            ),
            ["event-intersection"],
        );
        assert!(
            recognized_laws(
                "Event $A$ belongs to the first probability space, while event $B$ belongs to a different experiment. The formal surface is $A \\cap B$.",
            )
            .is_empty()
        );
    }

    #[test]
    fn sign_convention_condition_accepts_asserted_and_refuses_negated_prose() {
        assert_eq!(
            recognized_laws(
                "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Under the passive sign convention, $i=C\\frac{dv}{dt}$."
            ),
            ["capacitor-current-law"]
        );
        assert_eq!(
            recognized_laws(
                "Current $i_{\\rm out}$ is referenced leaving the positive-voltage terminal. We write $i_{\\rm out}$ for electric current scalar. We write $C$ for capacitance scalar. We write $v$ for voltage scalar. We write $t$ for duration scalar. \\[i_{\\rm out}=-C\\,dv/dt\\]."
            ),
            ["capacitor-current-law"]
        );
        assert_eq!(
            recognized_laws(
                "The next formula is presented without an assumed named law. \\begin{equation}q_{1323}\\end{equation} Current $i_{\\rm out}$ is referenced leaving the positive-voltage terminal, so We write $i_{\\rm out}$ for electric current scalar. We write $C$ for capacitance scalar. We write $v$ for voltage scalar. We write $t$ for duration scalar. \\[i_{\\rm out}=-C\\,dv/dt\\]."
            ),
            ["capacitor-current-law"]
        );
        assert!(
            recognized_laws(
                "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Without adopting the passive sign convention, consider $i=C\\frac{dv}{dt}$."
            )
            .is_empty()
        );
    }

    #[test]
    fn negative_capacitor_representation_preserves_declared_roles() {
        let actual = lower_template(r"i_{\rm out}=-C\,dv/dt");
        let capacitor = COMPILED_LAWS
            .iter()
            .position(|compiled| compiled.law.id == "capacitor-current-law")
            .expect("capacitor law");
        assert!(LAW_DISPATCH.candidate_indices(&actual).contains(&capacitor));
        assert!(COMPILED_LAWS[capacitor].plan.forms.iter().any(|form| {
            !unify_all(
                &form.expression,
                &actual,
                &COMPILED_LAWS[capacitor].plan.placeholders,
                &BTreeMap::new(),
            )
            .is_empty()
        }));
        assert_eq!(
            recognized_laws(
                "Current $i_{\\rm out}$ is referenced leaving the positive-voltage terminal. We write $i_{\\rm out}$ for electric current scalar, $C$ for capacitance scalar, $v$ for voltage scalar, and $t$ for duration scalar. $i_{\\rm out}=-C\\,dv/dt$."
            ),
            ["capacitor-current-law"]
        );
    }

    #[test]
    fn explicit_symbolic_shape_mismatch_refuses_a_structural_law() {
        assert!(
            recognized_laws(
                "Let $H$ be an m by n matrix, $q$ a k-dimensional vector with $k\\ne n$, and $z$ an m-dimensional vector. Consider $z=Hq$."
            )
            .is_empty()
        );
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
    fn an_explicit_declaration_resets_unrelated_prior_discourse_for_its_formula() {
        assert_eq!(
            recognized_laws(
                "The symbols below have no meaning beyond this standalone example. A grouped symbol is $q_{206}$. In discrete state equation, let $q$, $k$, $u$, $s$, and $w$ denote n-dimensional system state vector, n by n state matrix, n-dimensional system state vector, n by n input matrix, and n-dimensional control input vector, respectively. $q = k u + s w$"
            ),
            ["discrete-state-equation"]
        );
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
    fn equation_flow_roles_activate_existing_compiled_laws() {
        let authored = recognized_law_observations(
            "The fluid density $\\rho$ was measured. For each nozzle, the optical diameter supplied the area $A$, while particle tracking supplied the area-mean exit speed $v_e$. We computed\nthe discharged mass per unit time as\n\\[\\dot m=\\rho A v_e.\\]",
        );
        assert_eq!(authored[0].law_id, "mass-flow-rate");
        let relation = authored[0].relation.as_ref().unwrap();
        assert!(
            relation
                .roles
                .iter()
                .any(|role| { role.role == "mass-flow-rate" && role.symbol == "\\dot m" })
        );
        assert!(
            relation
                .roles
                .iter()
                .any(|role| role.role == "density" && role.symbol == "\\rho")
        );
        assert_eq!(
            recognized_laws(
                "The bore determines area $A$ and the meter reports cross-section mean speed $v$. Density was sampled at the same temperature, allowing the corresponding mass rate\nto be written as\n\\[\\dot m=\\rho Q=\\rho A v.\\]"
            ),
            ["mass-flow-rate"]
        );
        assert_eq!(
            recognized_laws(
                "For binary label $y$, the model emits predicted probability $p$. We calculated the per-example binary\ncross-entropy as\n\\[L=-y\\log p-(1-y)\\log(1-p).\\]"
            ),
            ["binary-cross-entropy"]
        );
    }

    #[test]
    fn possessive_equation_flow_uses_the_existing_role_and_law_pipeline() {
        let source = "A charge packet with signed charge $q_b$ entering a region held at potential $V_b$ has electric potential energy $U_b=q_bV_b$.";
        assert_eq!(recognized_laws(source), ["electric-potential-energy"]);
    }

    #[test]
    fn infers_one_unresolved_law_role_from_at_least_two_typed_roles() {
        assert_eq!(
            recognized_laws("Let $A$ be area and $v$ velocity. $Q=Av$."),
            ["volumetric-flow-rate"]
        );
        assert_eq!(
            recognized_laws(
                "Let $M$ be mass flow rate, $A$ area, and $v$ velocity. $M=\\rho A v$."
            ),
            ["mass-flow-rate"]
        );
        assert_eq!(
            recognized_laws(
                "Let $M$ be mass flow rate, $A$ area, and $v$ velocity. $M=\\rho Q=\\rho A v$."
            ),
            ["mass-flow-rate"]
        );
        assert_eq!(
            recognized_laws(
                "Let $c$ be wave propagation speed and $\\lambda$ wavelength. $c=f\\lambda$."
            ),
            ["wave-speed-relation"]
        );
    }

    #[test]
    fn refuses_role_completion_with_two_unknowns_or_a_conflicting_role() {
        assert!(recognized_laws("Let $A$ be area. $Q=Ax$.").is_empty());
        assert!(
            !recognized_laws("Let $Q$ be voltage, $A$ area, and $v$ velocity. $Q=Av$.")
                .iter()
                .any(|law| law == "volumetric-flow-rate")
        );
        assert!(
            recognized_laws(
                "We write $h$ for loss value scalar. We write $h$ for loss value scalar. We write $g$ for step size scalar. We write $j$ for penalty value scalar. This notation is used in a regularized objective. $v=h+gj$."
            )
            .is_empty()
        );
    }

    #[test]
    fn an_explicitly_wrong_physical_unit_refutes_a_law_role() {
        assert!(
            recognized_laws(
                "Let $K$ be energy in joules, $m$ be measured in seconds, and $v$ speed in metres per second. $K=\\frac12mv^2$."
            )
            .is_empty()
        );
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

        let unspecified = recognized_law_observations(
            "Let $x$ be a state vector, $u$ a control input vector, $A$ a state matrix, and $B$ an input matrix. The state-space equation is $\\dot{x}=Ax+Bu$.",
        );
        assert_eq!(
            unspecified[0].conditions[0].status,
            ConstraintStatus::Required
        );
    }

    fn recognized_laws(source: &str) -> Vec<String> {
        recognized_law_observations(source)
            .iter()
            .map(|law| law.law_id.clone())
            .collect()
    }

    #[test]
    fn callable_derivative_roles_preserve_the_complete_authored_notation() {
        let recognized = recognized_law_observations(
            "Let $y$ be differentiable in $x$. $y'(x)=\\frac{dy}{dx}(x)$",
        );
        let roles = &recognized
            .iter()
            .find(|law| law.law_id == "first-derivative-relation")
            .unwrap()
            .relation
            .as_ref()
            .unwrap()
            .roles;

        assert!(
            roles
                .iter()
                .any(|role| role.role == "derivative" && role.symbol == "y'(x)")
        );
        assert!(
            roles
                .iter()
                .any(|role| role.role == "function" && role.symbol == "y")
        );
        assert!(
            roles
                .iter()
                .any(|role| role.role == "variable" && role.symbol == "x")
        );
    }

    fn recognized_law_observations(source: &str) -> Vec<LawRecognition> {
        law_observations(source).all().to_vec()
    }

    fn law_observations(source: &str) -> LawObservations {
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
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        let shapes = observe_shapes(&document, &parsed, &canonical, &prose.shapes);
        let quantities = observe_quantities(&document, &parsed, &prose.definitions);
        let roles = observe_roles(&document, &prose.definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let domains = crate::domain::observe_domains(
            &document,
            crate::scope::ScopeGraph::new(&document),
            &prose.semantic_evidence,
            &[],
        );
        observe_laws(
            &canonical,
            &prose.semantic_evidence,
            &LawAnalysisContext {
                source,
                formula_ranges: &regions
                    .iter()
                    .map(|region| region.content_range.clone())
                    .collect::<Vec<_>>(),
                shapes: &shapes,
                quantities: &quantities,
                consistency: &roles,
                assumptions: &prose.assumptions,
                external: &external,
                domains: &domains,
            },
        )
    }
}
