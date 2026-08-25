use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::canonical::{
    SemanticExpr, SemanticExprKind, expression_children, lower_template, render_canonical,
};
use crate::concept::concepts_share_lineage;
use crate::consistency::{RoleObservations, role_shape_conflict, roles_conflict};
use crate::domain::{DomainObservations, support_rank};
use crate::domain_signature::{
    is_capability_pack, laws_share_collision, pack_requires_explicit_law_activation,
};
use crate::equivalence::{EquivalenceGuard, GuardedForm, compile_guarded_forms, instantiate_guard};
use crate::interpretation::normalize_source_anchors;
use crate::pack::{
    PackConditionKind, PackLaw, PackLawCondition, PackLawRole, PackOperatorProperty,
    RoleSourceProjection, built_in_packs,
};
use crate::prose::{
    FormulaOperationKind, LawActivationEvidence, ScientificSemanticEvidence,
    assumption_formula_targets, assumption_public_evidence, assumption_value_and_target,
};
use crate::quantity::QuantityObservations;
use crate::scope::{ScopeGraph, scope_visible};
use crate::shape::ShapeObservations;
use crate::source_index::SourceIndex;
use crate::{
    AssumptionInfo, ConstraintStatus, DomainSupportTier, Evidence, LawBinding, LawBindingProof,
    LawConditionInfo, LawRecognition, LawRecognitionStatus,
    MathInterpretationEvidenceSourceAnchorInfo, MeaningConflict, OperatorProperty, QuantityInfo,
    RelationInfo, RelationRoleInfo, RoleInfo, ScientificConstraintKind, SemanticConstraint,
    SemanticConstraintKind, ShapeInfo, SourceRange,
};

const MAX_LAW_MATCHES_PER_EXPRESSION: usize = 16;
const MAX_UNIFICATION_CANDIDATES: usize = 64;
const MAX_COMPOSITE_SOURCE_LABEL_CHARS: usize = 256;

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

struct CompiledOperatorType {
    operator: String,
    operand_concepts: Vec<String>,
    result_concept: String,
    result_shape: Option<String>,
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

static OPERATOR_TYPES: LazyLock<Vec<CompiledOperatorType>> = LazyLock::new(|| {
    built_in_packs()
        .iter()
        .flat_map(|pack| {
            pack.operators.iter().flat_map(|entry| {
                entry.notation.iter().filter_map(|notation| {
                    let result_concept = entry.result_concept.clone()?;
                    let expression = lower_template(notation);
                    let (operator, arity) = match expression.kind {
                        SemanticExprKind::Apply {
                            operator,
                            arguments,
                        } => (operator.value, arguments.len()),
                        SemanticExprKind::Dot(_, _) => ("dot".into(), 2),
                        _ => return None,
                    };
                    (arity == entry.operand_concepts.len()).then(|| CompiledOperatorType {
                        operator,
                        operand_concepts: entry.operand_concepts.clone(),
                        result_concept,
                        result_shape: entry.result_shape.clone(),
                    })
                })
            })
        })
        .collect()
});

fn typed_operator_parts(expression: &SemanticExpr) -> Option<(&str, Vec<&SemanticExpr>)> {
    match &expression.kind {
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => Some((operator.as_str(), arguments.iter().collect())),
        SemanticExprKind::Dot(left, right) => Some(("dot", vec![left, right])),
        _ => None,
    }
}

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
    positive_facts: &'a [PositiveFormulaFact],
    scopes: &'a ScopeGraph,
}

#[derive(Clone, Debug)]
struct PositiveFormulaFact {
    expression: SemanticExpr,
    evidence_range: SourceRange,
}

const MAX_STRUCTURAL_FACT_NODES: usize = 256;
const MAX_POSITIVE_FACTS: usize = 256;
const MAX_POSITIVE_FACTS_PER_SYSTEM: usize = 64;

pub(crate) struct LawAnalysisContext<'a> {
    pub(crate) source: &'a str,
    pub(crate) formula_ranges: &'a [SourceRange],
    pub(crate) shapes: &'a ShapeObservations,
    pub(crate) quantities: &'a QuantityObservations,
    pub(crate) consistency: &'a RoleObservations,
    pub(crate) assumptions: &'a [AssumptionInfo],
    pub(crate) external: &'a ExternalTypeEnvironment,
    pub(crate) domains: &'a DomainObservations,
    pub(crate) scopes: &'a ScopeGraph,
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
            .any(|info| concepts_share_lineage(&info.concept_id, role))
    }

    fn has_quantity(&self, offset: u32, symbol: &str, quantity: &str) -> bool {
        self.quantities_at(offset, symbol)
            .iter()
            .any(|info| info.quantity_kind_id.as_deref() == Some(quantity))
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

    pub(crate) fn retained_roles(&self) -> Vec<(RoleInfo, SourceRange)> {
        let established = self
            .recognitions
            .iter()
            .filter(|recognition| !recognition.non_authoritative)
            .filter(|recognition| {
                matches!(
                    recognition.status,
                    LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
                )
            })
            .collect::<Vec<_>>();
        established
            .iter()
            .flat_map(|recognition| {
                recognition
                    .bindings
                    .iter()
                    .filter(|binding| {
                        binding.proof == LawBindingProof::Derived
                            && binding.evidence.kind == "law-chain-binding"
                    })
                    .filter_map(|binding| {
                        law_derived_role(recognition, binding)
                            .map(|role| (role, recognition.range.clone()))
                    })
            })
            .collect()
    }
}

const MAX_LAW_DERIVATION_DEPTH: u8 = 2;
const MAX_DERIVED_LAW_ROLES_PER_FORMULA: usize = 64;

impl ExternalTypeEnvironment {
    pub(crate) fn with_preceding_law_roles(
        &self,
        formula_ranges: &[SourceRange],
        observations: &LawObservations,
        source_anchor: &dyn Fn(&SourceRange) -> Option<MathInterpretationEvidenceSourceAnchorInfo>,
    ) -> Option<Self> {
        let mut output = None;
        for formula in formula_ranges {
            let mut derived = observations
                .all()
                .iter()
                .filter(|recognition| !recognition.non_authoritative)
                .filter(|recognition| {
                    recognition.range.end_offset <= formula.start_offset
                        && matches!(
                            recognition.status,
                            LawRecognitionStatus::Recognized | LawRecognitionStatus::Verified
                        )
                })
                .flat_map(|recognition| {
                    recognition.bindings.iter().filter_map(move |binding| {
                        let mut role = law_derived_role(recognition, binding)?;
                        if let Some(anchor) = source_anchor(&recognition.range) {
                            role.evidence.source_anchors.push(anchor);
                            normalize_source_anchors(&mut role.evidence.source_anchors);
                        }
                        let shape = law_derived_shape(binding, &role.evidence);
                        Some((role, shape))
                    })
                })
                .take(MAX_DERIVED_LAW_ROLES_PER_FORMULA)
                .collect::<Vec<_>>();
            derived.sort_by(|left, right| {
                (&left.0.symbol, &left.0.concept_id).cmp(&(&right.0.symbol, &right.0.concept_id))
            });
            derived.dedup_by(|left, right| {
                left.0.symbol == right.0.symbol && left.0.concept_id == right.0.concept_id
            });
            for (role, shape) in derived {
                let environment = output.get_or_insert_with(|| self.clone());
                if let Some(shape) = shape {
                    environment.add_shape(formula.start_offset, shape);
                }
                environment.add_role(formula.start_offset, role);
            }
        }
        output
    }
}

fn law_derived_shape(binding: &LawBinding, evidence: &Evidence) -> Option<ShapeInfo> {
    let kind = match binding.constraint.kind {
        SemanticConstraintKind::Scalar => "scalar",
        SemanticConstraintKind::Vector => "vector",
        SemanticConstraintKind::Matrix => "matrix",
        SemanticConstraintKind::Tensor => "tensor",
        SemanticConstraintKind::Function => "function",
        _ => return None,
    };
    let dimensions = binding.constraint.dimensions.clone();
    let display = if dimensions.is_empty() {
        kind.to_owned()
    } else {
        format!("{kind}[{}]", dimensions.join(" × "))
    };
    Some(ShapeInfo {
        symbol: binding.symbol.clone(),
        kind: kind.into(),
        dimensions,
        refinements: binding.constraint.refinements.clone(),
        display,
        evidence: evidence.clone(),
    })
}

fn law_derived_role(recognition: &LawRecognition, binding: &LawBinding) -> Option<RoleInfo> {
    let concept_id = binding.constraint.concepts.first()?.clone();
    simple_binding_symbol(&binding.symbol)?;
    let mut source_ranges = recognition
        .evidence
        .iter()
        .flat_map(|evidence| evidence.source_ranges.iter().cloned())
        .chain(binding.evidence.source_ranges.iter().cloned())
        .collect::<Vec<_>>();
    source_ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    source_ranges.dedup();
    let mut source_anchors = recognition
        .evidence
        .iter()
        .flat_map(|evidence| evidence.source_anchors.iter().cloned())
        .chain(binding.evidence.source_anchors.iter().cloned())
        .collect::<Vec<_>>();
    normalize_source_anchors(&mut source_anchors);
    Some(RoleInfo {
        symbol: binding.symbol.clone(),
        concept_id,
        description: format!(
            "Role established by {}:{}.",
            recognition.pack_id, recognition.law_id
        ),
        evidence: Evidence {
            rule_id: format!(
                "law-chain/{}/{}:{}",
                MAX_LAW_DERIVATION_DEPTH, recognition.pack_id, recognition.law_id
            ),
            kind: "law-derived-role".into(),
            strength: "strong".into(),
            source_ranges,
            source_anchors,
        },
    })
}

fn simple_binding_symbol(symbol: &str) -> Option<&str> {
    (!symbol.is_empty()
        && symbol.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '\\' | '_' | '{' | '}' | '\'' | '′')
        }))
    .then_some(symbol)
}

pub(crate) fn observe_laws(
    canonical_expressions: &[SemanticExpr],
    semantic_evidence: &ScientificSemanticEvidence,
    context: &LawAnalysisContext<'_>,
) -> LawObservations {
    let source_index = SourceIndex::new(context.source);
    let positive_facts = collect_positive_formula_facts(canonical_expressions);
    let recognition_context = RecognitionContext {
        source: context.source,
        source_index: &source_index,
        shapes: context.shapes,
        quantities: context.quantities,
        consistency: context.consistency,
        assumptions: context.assumptions,
        external: context.external,
        positive_facts: &positive_facts,
        scopes: context.scopes,
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
    for (actual, source_envelope, formula_envelope, ownership_range) in actuals {
        let source_envelope =
            strip_formula_presentation(&source_envelope, context.source, &source_index);
        let ownership_range =
            strip_formula_presentation(&ownership_range, context.source, &source_index);
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
            if recognitions.len().saturating_sub(recognition_start)
                >= MAX_LAW_MATCHES_PER_EXPRESSION
            {
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
            let attached_declared_role_support = candidates.iter().any(|(_, bindings)| {
                bindings_have_formula_attached_declared_roles(
                    &compiled.law.roles,
                    bindings,
                    &actual.range,
                    context.consistency,
                    context.external,
                )
            });
            let attached_formula_role_support = candidates.iter().any(|(_, bindings)| {
                compiled.law.roles.iter().all(|role| {
                    bindings.get(&role.id).is_some_and(|expression| {
                        formula_operator_role_support(role, expression, actual).is_proven()
                    })
                })
            });
            let attached_role_support =
                attached_declared_role_support || attached_formula_role_support;
            let context_only_admission = !compiled.law.activation_phrases.is_empty()
                && activation.is_none()
                && !attached_role_support
                && role_context_activated;
            if !compiled.law.activation_phrases.is_empty()
                && pack_requires_explicit_law_activation(compiled.pack_id)
                && activation.is_none()
                && !attached_declared_role_support
            {
                continue;
            }
            if !compiled.law.activation_phrases.is_empty()
                && activation.is_none()
                && ((!attached_role_support && !role_context_activated)
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
            let Some((matched_form, bindings, role_support)) =
                candidates.into_iter().find_map(|(matched_form, bindings)| {
                    let inferred_role = actual_output_role(actual, &bindings);
                    let role_support = plan_role_support(
                        &compiled.law.roles,
                        &bindings,
                        actual,
                        inferred_role.as_deref(),
                        actual.range.start_offset,
                        role_context_activated || activation.is_some(),
                        activation.is_some(),
                        activation.is_some_and(|activation| activation.identifies_attached_formula),
                        context.shapes,
                        context.quantities,
                        context.consistency,
                        context.assumptions,
                        context.external,
                    )?;
                    let typed = expression_is_well_typed(actual, context.shapes);
                    (typed
                        && !law_has_admission_blocking_refutation(
                            compiled.law,
                            &bindings,
                            &actual.range,
                            context.assumptions,
                            context.external.assumptions_at(actual.range.start_offset),
                            context.scopes,
                        ))
                    .then_some((matched_form, bindings, role_support))
                })
            else {
                continue;
            };
            guard_checks += matched_form.map_or(0, |form| form.guards.len() as u32);
            let mut recognized = recognition(
                compiled,
                actual,
                &source_envelope,
                &formula_envelope,
                &ownership_range,
                bindings,
                &role_support,
                matched_form,
                &recognition_context,
                activation,
            );
            recognized.conventional_candidate = context_only_admission
                || (activation.is_none()
                    && role_context_activated
                    && !attached_role_support
                    && recognized.bindings.iter().any(|binding| {
                        binding.proof == LawBindingProof::Asserted
                            && compiled.law.roles.iter().any(|role| {
                                let PackLawRole { id, notation, .. } = role;
                                id == &binding.parameter
                                    && notation.iter().any(|candidate| {
                                        notation_matches_symbol(candidate, &binding.symbol)
                                    })
                            })
                    }));
            recognized.non_authoritative = context_only_admission;
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
        .filter(|compiled| {
            compiled.law.activation_phrases.is_empty()
                || !pack_requires_explicit_law_activation(compiled.pack_id)
        })
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
    output: &mut Vec<(&'a SemanticExpr, SourceRange, SourceRange, SourceRange)>,
) {
    if let SemanticExprKind::System(expressions) = &expression.kind {
        for expression in expressions {
            collect_law_expressions_with_envelope(
                expression,
                Some(&expression.range),
                formula_range,
                output,
            );
        }
    } else {
        collect_law_expressions_with_envelope(
            expression,
            Some(&expression.range),
            formula_range,
            output,
        );
    }
}

fn collect_law_expressions_with_envelope<'a>(
    expression: &'a SemanticExpr,
    relation_range: Option<&SourceRange>,
    formula_range: Option<&SourceRange>,
    output: &mut Vec<(&'a SemanticExpr, SourceRange, SourceRange, SourceRange)>,
) {
    let relation_envelope = relation_range
        .cloned()
        .unwrap_or_else(|| expression.range.clone());
    let formula_envelope = formula_range
        .cloned()
        .unwrap_or_else(|| relation_envelope.clone());
    output.push((
        expression,
        relation_envelope.clone(),
        formula_envelope.clone(),
        relation_envelope.clone(),
    ));
    for child in expression_children(expression) {
        collect_nested_law_expressions(
            child,
            &relation_envelope,
            &formula_envelope,
            matches!(expression.kind, SemanticExprKind::Relation { .. }),
            output,
        );
    }
}

fn collect_nested_law_expressions<'a>(
    expression: &'a SemanticExpr,
    source_envelope: &SourceRange,
    formula_envelope: &SourceRange,
    owns_relation_operand: bool,
    output: &mut Vec<(&'a SemanticExpr, SourceRange, SourceRange, SourceRange)>,
) {
    if matches!(
        &expression.kind,
        SemanticExprKind::Apply { operator, .. }
            if NESTED_LAW_APPLICATIONS.contains(operator.as_str())
    ) {
        output.push((
            expression,
            source_envelope.clone(),
            formula_envelope.clone(),
            if owns_relation_operand {
                source_envelope.clone()
            } else {
                expression.range.clone()
            },
        ));
    }
    let child_owns_relation_operand = match &expression.kind {
        SemanticExprKind::Relation { .. } => true,
        SemanticExprKind::Apply { arguments, .. } if arguments.len() == 1 => owns_relation_operand,
        _ => false,
    };
    for child in expression_children(expression) {
        collect_nested_law_expressions(
            child,
            source_envelope,
            formula_envelope,
            child_owns_relation_operand,
            output,
        );
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
        (
            SemanticExprKind::Binder {
                operator: left_operator,
                variables: left_variables,
                lower: left_lower,
                upper: left_upper,
                body: left_body,
            },
            SemanticExprKind::Binder {
                operator: right_operator,
                variables: right_variables,
                lower: right_lower,
                upper: right_upper,
                body: right_body,
            },
        ) if left_operator == right_operator
            && left_variables.len() == right_variables.len()
            && left_lower.is_some() == right_lower.is_some()
            && left_upper.is_some() == right_upper.is_some() =>
        {
            let left = left_variables
                .iter()
                .chain(left_lower.iter().map(Box::as_ref))
                .chain(left_upper.iter().map(Box::as_ref))
                .chain(std::iter::once(left_body.as_ref()));
            let right = right_variables
                .iter()
                .chain(right_lower.iter().map(Box::as_ref))
                .chain(right_upper.iter().map(Box::as_ref))
                .chain(std::iter::once(right_body.as_ref()));
            unify_sequence(left, right, placeholders, bindings)
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

fn relation_without_side_sign(
    expression: &SemanticExpr,
    strip_left: bool,
) -> Option<(bool, SemanticExpr)> {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &expression.kind
    else {
        return None;
    };
    let target = if strip_left {
        left.as_ref()
    } else {
        right.as_ref()
    };
    let (negative, unsigned) = match &target.kind {
        SemanticExprKind::Negate(inner) => (true, inner.as_ref().clone()),
        SemanticExprKind::Product(items) if matches!(items.first().map(|item| &item.kind), Some(SemanticExprKind::Symbol(sign)) if sign == "+") =>
        {
            let remaining = &items[1..];
            let unsigned = if remaining.len() == 1 {
                remaining[0].clone()
            } else {
                SemanticExpr {
                    kind: SemanticExprKind::Product(remaining.to_vec()),
                    range: target.range.clone(),
                    provenance: target.provenance.clone(),
                }
            };
            (false, unsigned)
        }
        _ => (false, target.clone()),
    };
    Some((
        negative,
        SemanticExpr {
            kind: SemanticExprKind::Relation {
                operator: operator.clone(),
                left: if strip_left {
                    Box::new(unsigned.clone())
                } else {
                    left.clone()
                },
                right: if strip_left {
                    right.clone()
                } else {
                    Box::new(unsigned)
                },
            },
            range: expression.range.clone(),
            provenance: expression.provenance.clone(),
        },
    ))
}

fn differs_by_one_explicit_relation_sign(
    template: &SemanticExpr,
    actual: &SemanticExpr,
    placeholders: &BTreeSet<String>,
) -> bool {
    [true, false].into_iter().any(|strip_left| {
        let Some((template_negative, template_unsigned)) =
            relation_without_side_sign(template, strip_left)
        else {
            return false;
        };
        let Some((actual_negative, actual_unsigned)) =
            relation_without_side_sign(actual, strip_left)
        else {
            return false;
        };
        template_negative != actual_negative
            && !unify_exact_all(
                &template_unsigned,
                &actual_unsigned,
                placeholders,
                &BTreeMap::new(),
            )
            .is_empty()
    })
}

pub(crate) fn rejected_formula_sign_conflicts(
    actual: &SemanticExpr,
    semantic_evidence: &ScientificSemanticEvidence,
) -> Vec<MeaningConflict> {
    if !semantic_evidence.formula_is_explicitly_retracted(&actual.range) {
        return Vec::new();
    }
    COMPILED_LAWS
        .iter()
        .filter_map(|compiled| {
            let activation = semantic_evidence
                .law_activation(compiled.pack_id, &compiled.law.id, &actual.range)
                .or_else(|| {
                    semantic_evidence
                        .law_activations
                        .iter()
                        .filter(|activation| {
                            activation.pack_id == compiled.pack_id
                                && activation.law_id == compiled.law.id
                                && activation.frame.establishes()
                                && activation.clause_range.end_offset <= actual.range.start_offset
                                && actual
                                    .range
                                    .start_offset
                                    .saturating_sub(activation.clause_range.end_offset)
                                    <= MAX_ASSUMPTION_DISTANCE
                        })
                        .max_by_key(|activation| activation.clause_range.end_offset)
                })?;
            compiled
                .plan
                .forms
                .iter()
                .any(|form| {
                    differs_by_one_explicit_relation_sign(
                        &form.expression,
                        actual,
                        &compiled.plan.placeholders,
                    )
                })
                .then(|| MeaningConflict {
                    conflict_id: format!(
                        "{}:{}/explicit-sign-mismatch",
                        compiled.pack_id, compiled.law.id
                    ),
                    label: format!(
                        "The rejected formula has the opposite explicit sign from {}.",
                        compiled.law.title
                    ),
                    evidence: vec![
                        activation.evidence.clone(),
                        Evidence {
                            rule_id: "semantic-law/explicit-sign-mismatch".into(),
                            kind: "canonical-math".into(),
                            strength: "hard".into(),
                            source_ranges: vec![actual.range.clone()],
                            source_anchors: Vec::new(),
                        },
                    ],
                })
        })
        .collect()
}

pub(crate) fn refuted_law_condition_conflicts(formulas: &[LawRecognition]) -> Vec<MeaningConflict> {
    formulas
        .iter()
        .filter(|formula| formula.status == LawRecognitionStatus::Conflicting)
        .filter(|formula| formula.relation.is_some())
        .filter(|formula| {
            !formula.bindings.is_empty()
                && formula.bindings.iter().all(|binding| {
                    matches!(
                        binding.proof,
                        LawBindingProof::Typed | LawBindingProof::Derived
                    )
                })
        })
        .flat_map(|formula| {
            formula
                .conditions
                .iter()
                .filter(|condition| {
                    condition.kind == ScientificConstraintKind::SignConvention
                        && condition.status == ConstraintStatus::Conflicting
                        && formula.conditions.iter().all(|other| {
                            other.condition_id == condition.condition_id
                                || other.status == ConstraintStatus::Verified
                        })
                })
                .filter_map(|condition| {
                    let formula_evidence = formula.evidence.iter().find(|evidence| {
                        evidence.kind == "canonical-math"
                            && evidence.strength == "hard"
                            && evidence
                                .source_ranges
                                .iter()
                                .any(|range| source_ranges_overlap(range, &formula.range))
                    })?;
                    let refutation_evidence = condition.evidence.iter().find(|evidence| {
                        evidence.rule_id == "english-scientific-assumption"
                            && matches!(evidence.kind.as_str(), "explicit-prose" | "attached-prose")
                            && evidence.source_ranges.iter().any(|range| {
                                range.start_offset < range.end_offset
                                    && !source_ranges_overlap(range, &formula.range)
                            })
                    })?;
                    Some(MeaningConflict {
                        conflict_id: format!(
                            "{}:{}/condition/{}/explicit-refutation",
                            formula.pack_id, formula.law_id, condition.condition_id
                        ),
                        label: format!(
                            "Source evidence explicitly refutes condition \"{}\" for {}.",
                            condition.label, formula.title
                        ),
                        evidence: vec![formula_evidence.clone(), refutation_evidence.clone()],
                    })
                })
        })
        .collect()
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
fn plan_role_support(
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
    inferred_role: Option<&str>,
    offset: u32,
    notation_context_activated: bool,
    law_explicitly_activated: bool,
    formula_identified: bool,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> Option<RoleSupportPlan> {
    let mut supported = 0;
    let mut supported_roles = BTreeMap::new();
    let mut unresolved_roles = BTreeSet::new();
    for role in roles {
        let bound_expression = bindings.get(&role.id)?;
        let projected_expression = match (&bound_expression.kind, role.source_projection) {
            (SemanticExprKind::Apply { operator, .. }, RoleSourceProjection::Head) => {
                Some(SemanticExpr {
                    kind: SemanticExprKind::Symbol(operator.value.clone()),
                    range: operator.range.clone(),
                    provenance: operator.provenance.clone(),
                })
            }
            (SemanticExprKind::Apply { operator, .. }, RoleSourceProjection::Expression)
                if notation_context_activated
                    && role
                        .notation
                        .iter()
                        .any(|notation| notation_matches_symbol(notation, operator.as_str())) =>
            {
                Some(SemanticExpr {
                    kind: SemanticExprKind::Symbol(operator.value.clone()),
                    range: operator.range.clone(),
                    provenance: operator.provenance.clone(),
                })
            }
            _ => None,
        };
        let expression = projected_expression.as_ref().unwrap_or(bound_expression);
        let output_support = relation_operator_output_role_support(
            role,
            bound_expression,
            actual,
            offset,
            consistency,
            assumptions,
            external,
        );
        if output_support.is_proven() {
            if expression_constraints_refuted(
                role, expression, offset, shapes, quantities, external,
            ) {
                return None;
            }
            supported += 1;
            supported_roles.insert(role.id.as_str(), RoleBindingProof::DerivedFromTypes);
            continue;
        }
        match structural_operator_role_support(
            role,
            expression,
            offset,
            consistency,
            assumptions,
            external,
        ) {
            RoleSupport::Typed | RoleSupport::Derived => {
                supported += 1;
                supported_roles.insert(role.id.as_str(), RoleBindingProof::DerivedFromTypes);
                continue;
            }
            RoleSupport::Asserted => {
                supported += 1;
                supported_roles.insert(role.id.as_str(), RoleBindingProof::Asserted);
                continue;
            }
            RoleSupport::Refuted => return None,
            RoleSupport::Unresolved => {}
        }
        if role.shape.as_deref() == Some("scalar") && is_numeric_scalar(expression) {
            supported += 1;
            supported_roles.insert(role.id.as_str(), RoleBindingProof::Derived);
            continue;
        }
        let symbols = semantic_symbols(expression);
        if symbols.is_empty() || !(role.variadic || role_expression_is_atomic(expression)) {
            return None;
        }
        let mut role_support = RoleSupport::Typed;
        for symbol in &symbols {
            role_support = role_support.and(role_symbol_support(
                role,
                symbol,
                &expression.range,
                offset,
                notation_context_activated,
                law_explicitly_activated,
                shapes,
                quantities,
                consistency,
                assumptions,
                external,
            ));
        }
        match role_support {
            RoleSupport::Typed => {
                supported += 1;
                supported_roles.insert(role.id.as_str(), RoleBindingProof::Typed);
            }
            RoleSupport::Derived => {
                supported += 1;
                let proof = if symbols.iter().any(|symbol| {
                    external.roles_at(offset, symbol).iter().any(|claim| {
                        claim.evidence.kind == "law-derived-role"
                            && concepts_share_lineage(&claim.concept_id, &role.concept)
                    })
                }) {
                    RoleBindingProof::DerivedFromLaw
                } else {
                    RoleBindingProof::Derived
                };
                supported_roles.insert(role.id.as_str(), proof);
            }
            RoleSupport::Asserted => {
                supported += 1;
                supported_roles.insert(role.id.as_str(), RoleBindingProof::Asserted);
            }
            RoleSupport::Unresolved => {
                if formula_operator_role_support(role, expression, actual).is_proven() {
                    supported += 1;
                    supported_roles.insert(role.id.as_str(), RoleBindingProof::Derived);
                } else {
                    unresolved_roles.insert(role.id.as_str());
                }
            }
            RoleSupport::Refuted => return None,
        }
    }
    let unresolved = unresolved_roles.len();
    let unresolved_role = (unresolved == 1)
        .then(|| unresolved_roles.first().copied())
        .flatten();
    let asserted_roles = supported_roles
        .iter()
        .filter_map(|(role, proof)| (*proof == RoleBindingProof::Asserted).then_some(*role))
        .collect::<Vec<_>>();
    let asserted_role = (asserted_roles.len() == 1).then(|| asserted_roles[0].to_owned());
    let inferable_role = if unresolved == 1 && asserted_roles.is_empty() {
        unresolved_role
    } else if unresolved == 0 && asserted_roles.len() == 1 {
        asserted_role.as_deref()
    } else {
        None
    };
    let proved = supported_roles
        .values()
        .filter(|proof| {
            matches!(
                proof,
                RoleBindingProof::Typed
                    | RoleBindingProof::Derived
                    | RoleBindingProof::DerivedFromTypes
            )
        })
        .count();
    let inferred = inferable_role.is_some()
        && proved >= 2
        && ((inferable_role == inferred_role && roles.len() <= 3)
            || (roles.len() <= 3
                && inferable_role.is_some_and(|unresolved| {
                    roles
                        .iter()
                        .any(|role| role.id == unresolved && role.quantity.is_some())
                }))
            || (inferable_role != inferred_role && proved >= 3));
    let admitted_by_assertion = (law_explicitly_activated
        && (inferred_role.map_or(supported >= 2, |role| supported_roles.contains_key(role))
            || (roles.len() == 2 && supported == 1 && unresolved == 1)))
        || formula_identified;
    if unresolved != 0 && !inferred && !admitted_by_assertion {
        return None;
    }
    let mut proofs = supported_roles
        .into_iter()
        .map(|(role, proof)| (role.to_owned(), proof))
        .collect::<BTreeMap<_, _>>();
    if inferred && let Some(role) = asserted_role {
        proofs.insert(role, RoleBindingProof::Derived);
    }
    for role in unresolved_roles {
        let proof = if inferred && Some(role) == unresolved_role {
            RoleBindingProof::Derived
        } else if formula_identified {
            RoleBindingProof::Asserted
        } else {
            RoleBindingProof::Candidate
        };
        proofs.insert(role.to_owned(), proof);
    }
    Some(RoleSupportPlan { proofs })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoleBindingProof {
    Typed,
    Derived,
    DerivedFromTypes,
    DerivedFromLaw,
    Asserted,
    Candidate,
}

#[derive(Clone, Debug)]
struct RoleSupportPlan {
    proofs: BTreeMap<String, RoleBindingProof>,
}

impl RoleSupportPlan {
    fn proof_for(&self, role: &str) -> RoleBindingProof {
        self.proofs
            .get(role)
            .copied()
            .unwrap_or(RoleBindingProof::Candidate)
    }
}

fn has_differentiable_function_evidence(assumptions: &[AssumptionInfo], symbol: &str) -> bool {
    assumptions
        .iter()
        .any(|assumption| is_differentiable_function_evidence(assumption, symbol))
}

fn is_differentiable_function_evidence(assumption: &AssumptionInfo, symbol: &str) -> bool {
    assumption.kind == "regularity"
        && assumption.value == "differentiable"
        && assumption.subjects.iter().any(|subject| subject == symbol)
}

fn role_binding_evidence_ranges(
    expression: &SemanticExpr,
    proof: RoleBindingProof,
    activation: Option<&LawActivationEvidence>,
    mut ranges: Vec<SourceRange>,
) -> Vec<SourceRange> {
    if ranges.is_empty() {
        ranges.push(expression.range.clone());
    }
    if proof == RoleBindingProof::Asserted
        && let Some(activation) = activation
    {
        ranges.extend(activation.evidence.source_ranges.iter().cloned());
    }
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    ranges
}

fn role_binding_source_anchors(
    proof: RoleBindingProof,
    activation: Option<&LawActivationEvidence>,
    mut anchors: Vec<MathInterpretationEvidenceSourceAnchorInfo>,
) -> Vec<MathInterpretationEvidenceSourceAnchorInfo> {
    if proof == RoleBindingProof::Asserted
        && let Some(activation) = activation
    {
        anchors.extend(activation.evidence.source_anchors.iter().cloned());
    }
    normalize_source_anchors(&mut anchors);
    anchors
}

fn align_source_ranges_with_anchors(
    ranges: Vec<SourceRange>,
    anchors: &[MathInterpretationEvidenceSourceAnchorInfo],
) -> Vec<SourceRange> {
    if anchors.is_empty() {
        return ranges;
    }
    let mut unmatched_anchor_ranges = anchors
        .iter()
        .map(|anchor| anchor.location.range.clone())
        .collect::<Vec<_>>();
    let mut unanchored_ranges = Vec::new();
    for range in ranges {
        if let Some(index) = unmatched_anchor_ranges
            .iter()
            .position(|anchor_range| anchor_range == &range)
        {
            unmatched_anchor_ranges.remove(index);
        } else {
            unanchored_ranges.push(range);
        }
    }
    let mut aligned = anchors
        .iter()
        .map(|anchor| anchor.location.range.clone())
        .collect::<Vec<_>>();
    aligned.extend(unanchored_ranges);
    aligned
}

#[allow(clippy::too_many_arguments)]
fn role_source_evidence_ranges(
    role: &PackLawRole,
    expression: &SemanticExpr,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> Vec<SourceRange> {
    let mut ranges = Vec::new();
    for symbol in semantic_symbols(expression) {
        ranges.extend(consistency.occurrence_role_evidence_ranges(
            &symbol,
            &expression.range,
            &role.concept,
        ));
        ranges.extend(
            consistency
                .roles_at(&symbol, offset)
                .0
                .into_iter()
                .filter(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
                .flat_map(|claim| claim.evidence.source_ranges),
        );
        if concepts_share_lineage("semath:function", &role.concept) {
            ranges.extend(
                assumptions
                    .iter()
                    .filter(|assumption| is_differentiable_function_evidence(assumption, &symbol))
                    .flat_map(|assumption| assumption.evidence.source_ranges.iter().cloned()),
            );
        }
        ranges.extend(
            external
                .roles_at(offset, &symbol)
                .into_iter()
                .filter(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
                .flat_map(|claim| claim.evidence.source_ranges),
        );
        if let Some(quantity) = role.quantity.as_deref().or_else(|| {
            role.concept
                .starts_with("quantities-units:")
                .then_some(role.concept.as_str())
        }) {
            ranges.extend(
                quantities
                    .at(&symbol, offset)
                    .0
                    .into_iter()
                    .filter(|claim| claim.quantity_kind_id.as_deref() == Some(quantity))
                    .flat_map(|claim| claim.evidence.source_ranges),
            );
            ranges.extend(
                external
                    .quantities_at(offset, &symbol)
                    .into_iter()
                    .filter(|claim| claim.quantity_kind_id.as_deref() == Some(quantity))
                    .flat_map(|claim| claim.evidence.source_ranges),
            );
        }
        if let Some(shape) = role.shape.as_deref() {
            ranges.extend(
                shapes
                    .claims_at(&symbol, offset)
                    .0
                    .into_iter()
                    .filter(|claim| claim.kind == shape)
                    .flat_map(|claim| claim.evidence.source_ranges),
            );
            ranges.extend(
                external
                    .shapes_at(offset, &symbol)
                    .into_iter()
                    .filter(|claim| claim.kind == shape)
                    .flat_map(|claim| claim.evidence.source_ranges),
            );
        }
    }
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    ranges
}

#[allow(clippy::too_many_arguments)]
fn role_source_evidence_anchors(
    role: &PackLawRole,
    expression: &SemanticExpr,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> Vec<MathInterpretationEvidenceSourceAnchorInfo> {
    let mut anchors = Vec::new();
    for symbol in semantic_symbols(expression) {
        anchors.extend(
            consistency
                .roles_at(&symbol, offset)
                .0
                .into_iter()
                .filter(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
                .flat_map(|claim| claim.evidence.source_anchors),
        );
        if concepts_share_lineage("semath:function", &role.concept) {
            anchors.extend(
                assumptions
                    .iter()
                    .filter(|assumption| is_differentiable_function_evidence(assumption, &symbol))
                    .flat_map(|assumption| assumption.evidence.source_anchors.iter().cloned()),
            );
        }
        anchors.extend(
            external
                .roles_at(offset, &symbol)
                .into_iter()
                .filter(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
                .flat_map(|claim| claim.evidence.source_anchors),
        );
        if let Some(quantity) = role.quantity.as_deref().or_else(|| {
            role.concept
                .starts_with("quantities-units:")
                .then_some(role.concept.as_str())
        }) {
            anchors.extend(
                quantities
                    .at(&symbol, offset)
                    .0
                    .into_iter()
                    .filter(|claim| claim.quantity_kind_id.as_deref() == Some(quantity))
                    .flat_map(|claim| claim.evidence.source_anchors),
            );
            anchors.extend(
                external
                    .quantities_at(offset, &symbol)
                    .into_iter()
                    .filter(|claim| claim.quantity_kind_id.as_deref() == Some(quantity))
                    .flat_map(|claim| claim.evidence.source_anchors),
            );
        }
        if let Some(shape) = role.shape.as_deref() {
            anchors.extend(
                shapes
                    .claims_at(&symbol, offset)
                    .0
                    .into_iter()
                    .filter(|claim| claim.kind == shape)
                    .flat_map(|claim| claim.evidence.source_anchors),
            );
            anchors.extend(
                external
                    .shapes_at(offset, &symbol)
                    .into_iter()
                    .filter(|claim| claim.kind == shape)
                    .flat_map(|claim| claim.evidence.source_anchors),
            );
        }
    }
    anchors
}

fn is_numeric_scalar(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Number(_) => true,
        SemanticExprKind::Negate(inner) => is_numeric_scalar(inner),
        _ => false,
    }
}

fn formula_operator_role_support(
    role: &PackLawRole,
    expression: &SemanticExpr,
    formula: &SemanticExpr,
) -> RoleSupport {
    let expected_symbols = semantic_leaf_symbols(expression);
    if expected_symbols.is_empty() {
        return RoleSupport::Unresolved;
    }
    let supported = expression_any(formula, |candidate| {
        let Some((operator, arguments)) = typed_operator_parts(candidate) else {
            return false;
        };
        OPERATOR_TYPES.iter().any(|signature| {
            signature.operator == operator
                && signature.operand_concepts.len() == arguments.len()
                && arguments
                    .iter()
                    .zip(&signature.operand_concepts)
                    .any(|(argument, concept)| {
                        if !concepts_share_lineage(concept, &role.concept) {
                            return false;
                        }
                        let argument_symbols = semantic_leaf_symbols(argument);
                        expected_symbols
                            .iter()
                            .all(|symbol| argument_symbols.contains(symbol))
                    })
        })
    });
    if supported {
        RoleSupport::Derived
    } else {
        RoleSupport::Unresolved
    }
}

fn expression_any(
    expression: &SemanticExpr,
    mut predicate: impl FnMut(&SemanticExpr) -> bool,
) -> bool {
    let mut pending = vec![expression];
    for _ in 0..MAX_STRUCTURAL_FACT_NODES {
        let Some(candidate) = pending.pop() else {
            return false;
        };
        if predicate(candidate) {
            return true;
        }
        pending.extend(expression_children(candidate).into_iter().rev());
    }
    false
}

fn structural_operator_role_support(
    role: &PackLawRole,
    expression: &SemanticExpr,
    offset: u32,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
    if matches!(expression.kind, SemanticExprKind::Derivative { .. })
        && role.concept.split(':').next_back() == Some("derivative")
    {
        return RoleSupport::Derived;
    }
    let Some((operator, arguments)) = typed_operator_parts(expression) else {
        return RoleSupport::Unresolved;
    };
    let mut matched = false;
    let mut support = RoleSupport::Unresolved;
    for signature in OPERATOR_TYPES.iter().filter(|signature| {
        signature.operator == operator
            && concepts_share_lineage(&signature.result_concept, &role.concept)
            && signature.operand_concepts.len() == arguments.len()
            && role
                .shape
                .as_deref()
                .is_none_or(|shape| signature.result_shape.as_deref() == Some(shape))
    }) {
        matched = true;
        let candidate = arguments.iter().zip(&signature.operand_concepts).fold(
            RoleSupport::Typed,
            |support, (argument, concept)| {
                support.and(expression_concept_support(
                    argument,
                    concept,
                    offset,
                    consistency,
                    assumptions,
                    external,
                ))
            },
        );
        if candidate.is_proven() {
            return RoleSupport::Derived;
        }
        support = support.and(candidate);
    }
    if matched {
        support
    } else {
        RoleSupport::Unresolved
    }
}

fn relation_operator_output_role_support(
    role: &PackLawRole,
    expression: &SemanticExpr,
    relation: &SemanticExpr,
    offset: u32,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &relation.kind
    else {
        return RoleSupport::Unresolved;
    };
    if operator != "equals" {
        return RoleSupport::Unresolved;
    }
    let left_matches = equivalent(expression, left);
    let right_matches = equivalent(expression, right);
    let operator_expression = match (left_matches, right_matches) {
        (true, false) => right.as_ref(),
        (false, true) => left.as_ref(),
        _ => return RoleSupport::Unresolved,
    };
    structural_operator_role_support(
        role,
        operator_expression,
        offset,
        consistency,
        assumptions,
        external,
    )
}

fn expression_constraints_refuted(
    role: &PackLawRole,
    expression: &SemanticExpr,
    offset: u32,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    external: &ExternalTypeEnvironment,
) -> bool {
    semantic_symbols(expression).into_iter().any(|symbol| {
        let role_shape_refuted =
            role_shape_constraints_refuted(&role.concept, &symbol, offset, shapes, external);
        let shape_refuted = role.shape.as_deref().is_some_and(|expected| {
            shapes
                .claims_at(&symbol, offset)
                .0
                .into_iter()
                .chain(external.shapes_at(offset, &symbol))
                .any(|shape| shape.kind != expected)
                || shapes
                    .shape_at(&symbol, offset)
                    .is_some_and(|shape| shape.kind != expected)
        });
        let quantity_refuted = role.quantity.as_deref().is_some_and(|expected| {
            quantity_support(expected, &symbol, &symbol, offset, quantities, external)
                == RoleSupport::Refuted
        });
        role_shape_refuted || shape_refuted || quantity_refuted
    })
}

fn role_shape_constraints_refuted(
    role: &str,
    symbol: &str,
    offset: u32,
    shapes: &ShapeObservations,
    external: &ExternalTypeEnvironment,
) -> bool {
    shapes
        .claims_at(symbol, offset)
        .0
        .into_iter()
        .chain(external.shapes_at(offset, symbol))
        .any(|shape| role_shape_conflict(role, &shape.kind))
}

fn expression_concept_support(
    expression: &SemanticExpr,
    expected: &str,
    offset: u32,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
    let symbols = match &expression.kind {
        SemanticExprKind::Apply { operator, .. }
            if concepts_share_lineage("semath:function", expected) =>
        {
            vec![operator.value.clone()]
        }
        _ => semantic_leaf_symbols(expression),
    };
    if symbols.is_empty() {
        return RoleSupport::Unresolved;
    }
    symbols.iter().fold(RoleSupport::Typed, |support, symbol| {
        let declared = consistency
            .roles_at(symbol, offset)
            .0
            .into_iter()
            .chain(external.roles_at(offset, symbol));
        let mut found = false;
        let mut conflicting = false;
        for role in declared {
            found |= concepts_share_lineage(&role.concept_id, expected);
            conflicting |= roles_conflict(expected, &role.concept_id);
        }
        let asserted_function = concepts_share_lineage("semath:function", expected)
            && has_differentiable_function_evidence(assumptions, symbol);
        support.and(if conflicting {
            RoleSupport::Refuted
        } else if found || asserted_function {
            RoleSupport::Typed
        } else {
            RoleSupport::Unresolved
        })
    })
}

fn semantic_leaf_symbols(expression: &SemanticExpr) -> Vec<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. } => {
            semantic_symbol(expression).into_iter().collect()
        }
        SemanticExprKind::Derivative { expression, .. } => semantic_leaf_symbols(expression),
        _ => expression_children(expression)
            .into_iter()
            .flat_map(semantic_leaf_symbols)
            .collect(),
    }
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
                        .any(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
                        || external.has_role(offset, &symbol, &role.concept)
                })
        })
    });
    let one_role_is_attached_to_formula = roles.iter().any(|role| {
        bindings.get(&role.id).is_some_and(|expression| {
            semantic_symbols(expression).into_iter().any(|symbol| {
                consistency.roles_at(&symbol, offset).0.iter().any(|claim| {
                    concepts_share_lineage(&claim.concept_id, &role.concept)
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
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Index { .. } => true,
        SemanticExprKind::Negate(inner) => role_expression_is_atomic(inner),
        SemanticExprKind::Power(base, exponent) if is_decorative_star(exponent) => {
            role_expression_is_atomic(base)
        }
        SemanticExprKind::Derivative { expression, .. } => role_expression_is_atomic(expression),
        SemanticExprKind::Apply { arguments, .. } => {
            arguments.iter().all(role_expression_is_atomic)
        }
        SemanticExprKind::Product(factors) => {
            semantic_symbols(expression).len() == 1
                && factors.iter().all(|factor| {
                    matches!(factor.kind, SemanticExprKind::Number(_))
                        || role_expression_is_atomic(factor)
                })
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoleSupport {
    Typed,
    Derived,
    Asserted,
    Unresolved,
    Refuted,
}

impl RoleSupport {
    fn is_proven(self) -> bool {
        matches!(self, Self::Typed | Self::Derived)
    }

    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Refuted, _) | (_, Self::Refuted) => Self::Refuted,
            (Self::Unresolved, _) | (_, Self::Unresolved) => Self::Unresolved,
            (Self::Asserted, _) | (_, Self::Asserted) => Self::Asserted,
            (Self::Derived, _) | (_, Self::Derived) => Self::Derived,
            (Self::Typed, Self::Typed) => Self::Typed,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn role_symbol_support(
    role: &PackLawRole,
    symbol: &str,
    symbol_range: &SourceRange,
    offset: u32,
    notation_context_activated: bool,
    law_explicitly_activated: bool,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    external: &ExternalTypeEnvironment,
) -> RoleSupport {
    if role_shape_constraints_refuted(&role.concept, symbol, offset, shapes, external) {
        return RoleSupport::Refuted;
    }
    if consistency.has_occurrence_role(symbol, symbol_range, &role.concept) {
        return RoleSupport::Typed;
    }
    let notation_symbol = symbol;
    if role.concept == "quantities-units:dimensionless"
        && role.shape.as_deref() == Some("scalar")
        && symbol.trim_start_matches('\\') == "pi"
    {
        return RoleSupport::Derived;
    }
    if role.shape.as_deref() == Some("scalar")
        && role.quantity.as_deref().is_some_and(|quantity| {
            crate::quantity::unit_symbol_supports_quantity(symbol, quantity)
        })
    {
        return RoleSupport::Derived;
    }
    let activated_notation_support = notation_context_activated
        && role
            .notation
            .iter()
            .any(|notation| notation_matches_symbol(notation, symbol));
    let required_quantity = role
        .quantity
        .as_deref()
        .map_or(RoleSupport::Typed, |quantity| {
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
    let notation_support = || {
        if law_explicitly_activated {
            RoleSupport::Derived
        } else {
            RoleSupport::Asserted
        }
    };
    let required_quantity =
        if required_quantity == RoleSupport::Unresolved && activated_notation_support {
            notation_support()
        } else {
            required_quantity
        };
    let declared_roles = consistency.roles_at(symbol, offset).0;
    let has_exact_role = declared_roles
        .iter()
        .any(|claim| concepts_share_lineage(&claim.concept_id, &role.concept));
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
    let mut shape_support = RoleSupport::Unresolved;
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
            Some(_) => shape_support = RoleSupport::Typed,
            None => {}
        }
        if shape_support == RoleSupport::Unresolved {
            shape_support = imported
                .iter()
                .filter(|shape| shape.kind == expected_shape)
                .map(|shape| {
                    if shape.evidence.kind == "law-derived-role" {
                        RoleSupport::Derived
                    } else {
                        RoleSupport::Typed
                    }
                })
                .min_by_key(|support| match support {
                    RoleSupport::Typed => 0,
                    RoleSupport::Derived => 1,
                    _ => 2,
                })
                .unwrap_or(RoleSupport::Unresolved);
        }
        let shape_proves_concept = role.concept.split(':').next_back() == Some(expected_shape)
            || (role.concept == "linear-algebra:linear-operator" && expected_shape == "matrix");
        if shape_proves_concept {
            return required_quantity.and(
                if shape_support == RoleSupport::Unresolved && activated_notation_support {
                    notation_support()
                } else {
                    shape_support
                },
            );
        }
        if activated_notation_support && shape_support.is_proven() {
            return required_quantity.and(notation_support());
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
            notation_support()
        } else {
            support
        }
    } else if role.concept == "linear-algebra:linear-operator" {
        let local_matrix = shapes
            .shape_at(symbol, offset)
            .is_some_and(|shape| shape.kind == "matrix");
        if local_matrix {
            RoleSupport::Typed
        } else if shape_support.is_proven() {
            shape_support
        } else if activated_notation_support {
            notation_support()
        } else {
            RoleSupport::Unresolved
        }
    } else if has_exact_role
        || (concepts_share_lineage("semath:function", &role.concept)
            && has_differentiable_function_evidence(assumptions, symbol))
    {
        RoleSupport::Typed
    } else if let Some(imported) = external
        .roles_at(offset, symbol)
        .into_iter()
        .find(|claim| concepts_share_lineage(&claim.concept_id, &role.concept))
    {
        if imported.evidence.kind == "law-derived-role" {
            RoleSupport::Derived
        } else {
            RoleSupport::Typed
        }
    } else if activated_notation_support {
        notation_support()
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
    if crate::quantity::unit_symbol_supports_quantity(symbol, expected) {
        return RoleSupport::Derived;
    }
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
            RoleSupport::Typed
        } else {
            RoleSupport::Refuted
        };
    }
    if external.has_quantity(offset, symbol, expected) {
        RoleSupport::Typed
    } else {
        RoleSupport::Unresolved
    }
}

#[allow(clippy::too_many_arguments)]
fn recognition(
    compiled: &CompiledLaw,
    actual: &SemanticExpr,
    source_envelope: &SourceRange,
    formula_envelope: &SourceRange,
    ownership_range: &SourceRange,
    bindings: BTreeMap<String, SemanticExpr>,
    role_support: &RoleSupportPlan,
    matched_form: Option<&GuardedForm>,
    context: &RecognitionContext<'_>,
    activation: Option<&LawActivationEvidence>,
) -> LawRecognition {
    let formula_range = source_envelope.clone();
    let formula_evidence_range = formula_source_range(formula_envelope, context);
    let formula_evidence = Evidence {
        rule_id: "semantic-law-unification".into(),
        kind: "canonical-math".into(),
        strength: "hard".into(),
        source_ranges: vec![formula_evidence_range],
        source_anchors: Vec::new(),
    };
    let formula_bindings = compiled
        .law
        .roles
        .iter()
        .filter_map(|role| {
            let expression = bindings.get(&role.id)?;
            let planned_proof = role_support.proof_for(&role.id);
            let mut proof_ranges = match planned_proof {
                RoleBindingProof::Typed
                | RoleBindingProof::DerivedFromTypes
                | RoleBindingProof::DerivedFromLaw => role_source_evidence_ranges(
                    role,
                    expression,
                    actual.range.start_offset,
                    context.shapes,
                    context.quantities,
                    context.consistency,
                    context.assumptions,
                    context.external,
                ),
                RoleBindingProof::Candidate => vec![actual.range.clone()],
                RoleBindingProof::Derived | RoleBindingProof::Asserted => {
                    vec![expression.range.clone()]
                }
            };
            let proof_anchors = match planned_proof {
                RoleBindingProof::Typed
                | RoleBindingProof::DerivedFromTypes
                | RoleBindingProof::DerivedFromLaw => role_source_evidence_anchors(
                    role,
                    expression,
                    actual.range.start_offset,
                    context.shapes,
                    context.quantities,
                    context.consistency,
                    context.assumptions,
                    context.external,
                ),
                RoleBindingProof::Candidate
                | RoleBindingProof::Derived
                | RoleBindingProof::Asserted => Vec::new(),
            };
            let proof = if planned_proof == RoleBindingProof::Typed && proof_ranges.is_empty() {
                RoleBindingProof::Asserted
            } else {
                planned_proof
            };
            let symbol = if role.variadic {
                variadic_labels(expression, role.source_projection, context).join("; ")
            } else {
                role_source_label(expression, role.source_projection, context)?
            };
            let source_anchors = role_binding_source_anchors(proof, activation, proof_anchors);
            let source_ranges = align_source_ranges_with_anchors(
                role_binding_evidence_ranges(
                    expression,
                    proof,
                    activation,
                    std::mem::take(&mut proof_ranges),
                ),
                &source_anchors,
            );
            Some(LawBinding {
                parameter: role.id.clone(),
                symbol,
                constraint: role_semantic_constraint(
                    role,
                    expression,
                    actual.range.start_offset,
                    context,
                ),
                proof: match proof {
                    RoleBindingProof::Typed => LawBindingProof::Typed,
                    RoleBindingProof::Derived
                    | RoleBindingProof::DerivedFromTypes
                    | RoleBindingProof::DerivedFromLaw => LawBindingProof::Derived,
                    RoleBindingProof::Asserted => LawBindingProof::Asserted,
                    RoleBindingProof::Candidate => LawBindingProof::Candidate,
                },
                evidence: Evidence {
                    rule_id: match proof {
                        RoleBindingProof::Typed => format!("typed-law-role/{}", role.id),
                        RoleBindingProof::Derived | RoleBindingProof::DerivedFromTypes => {
                            format!("derived-law-role/{}", role.id)
                        }
                        RoleBindingProof::DerivedFromLaw => {
                            format!("law-chain-role/{}", role.id)
                        }
                        RoleBindingProof::Asserted => format!("asserted-law-role/{}", role.id),
                        RoleBindingProof::Candidate => {
                            format!("unresolved-law-role/{}", role.id)
                        }
                    },
                    kind: match proof {
                        RoleBindingProof::Typed => "canonical-binding",
                        RoleBindingProof::Derived | RoleBindingProof::DerivedFromTypes => {
                            "derived-binding"
                        }
                        RoleBindingProof::DerivedFromLaw => "law-chain-binding",
                        RoleBindingProof::Asserted => "asserted-binding",
                        RoleBindingProof::Candidate => "candidate-binding",
                    }
                    .into(),
                    strength: match proof {
                        RoleBindingProof::Typed => "hard",
                        RoleBindingProof::Derived
                        | RoleBindingProof::DerivedFromTypes
                        | RoleBindingProof::DerivedFromLaw
                        | RoleBindingProof::Asserted => "strong",
                        RoleBindingProof::Candidate => "weak",
                    }
                    .into(),
                    source_ranges,
                    source_anchors,
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
                variadic_labels(expression, role.source_projection, context)
            } else {
                vec![role_source_label(
                    expression,
                    role.source_projection,
                    context,
                )?]
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
    let relation_id = format!("{}:{}", compiled.pack_id, compiled.law.id);
    let mut conditions: Vec<LawConditionInfo> = compiled
        .law
        .conditions
        .iter()
        .map(|condition| {
            let condition_bindings = formula_bindings
                .iter()
                .filter(|binding| condition.subjects.contains(&binding.parameter))
                .collect::<Vec<_>>();
            let (evidence, mechanically_verified, explicitly_refuted) = condition_evidence(
                &relation_id,
                condition,
                &compiled.law.roles,
                &bindings,
                actual,
                &actual.range,
                context.shapes,
                context.quantities,
                context.consistency,
                context.assumptions,
                role_support,
                context.external,
                context.positive_facts,
                context.scopes,
            );
            LawConditionInfo {
                condition_id: condition.id.clone(),
                kind: scientific_constraint_kind(condition.kind),
                subjects: condition_bindings
                    .iter()
                    .map(|binding| binding.symbol.clone())
                    .collect(),
                label: condition.label.clone(),
                operator_property: condition.operator_property.map(operator_property),
                status: condition_status(
                    condition.kind,
                    condition_bindings.len(),
                    mechanically_verified,
                    explicitly_refuted,
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
            source_anchors: Vec::new(),
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
            range: ownership_range.clone(),
        }),
        evidence,
        relevance: None,
        rank: 100,
        conventional_candidate: false,
        non_authoritative: false,
    }
}

fn role_semantic_constraint(
    role: &PackLawRole,
    expression: &SemanticExpr,
    offset: u32,
    context: &RecognitionContext<'_>,
) -> SemanticConstraint {
    let observed = semantic_symbols(expression).into_iter().find_map(|symbol| {
        context
            .shapes
            .shape_at(&symbol, offset)
            .into_iter()
            .chain(context.external.shapes_at(offset, &symbol))
            .find(|shape| {
                role.shape
                    .as_deref()
                    .is_none_or(|expected| shape.kind == expected)
            })
    });
    let kind = role
        .shape
        .as_deref()
        .or_else(|| observed.as_ref().map(|shape| shape.kind.as_str()))
        .and_then(semantic_constraint_kind)
        .unwrap_or(SemanticConstraintKind::Expression);
    SemanticConstraint {
        kind,
        concepts: vec![role.concept.clone()],
        dimensions: observed
            .as_ref()
            .map(|shape| shape.dimensions.clone())
            .unwrap_or_default(),
        refinements: observed.map(|shape| shape.refinements).unwrap_or_default(),
    }
}

fn semantic_constraint_kind(kind: &str) -> Option<SemanticConstraintKind> {
    Some(match kind {
        "function" => SemanticConstraintKind::Function,
        "matrix" => SemanticConstraintKind::Matrix,
        "scalar" => SemanticConstraintKind::Scalar,
        "tensor" => SemanticConstraintKind::Tensor,
        "vector" => SemanticConstraintKind::Vector,
        _ => return None,
    })
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
                    source_anchors: Vec::new(),
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
                    operator_property: None,
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
        PackConditionKind::MapsBetween => ScientificConstraintKind::MapsBetween,
        PackConditionKind::OperatorProperty => ScientificConstraintKind::OperatorProperty,
        PackConditionKind::Positive => ScientificConstraintKind::Positive,
        PackConditionKind::RankCompatible => ScientificConstraintKind::RankCompatible,
        PackConditionKind::SameContext => ScientificConstraintKind::SameContext,
        PackConditionKind::ShapeCompatible => ScientificConstraintKind::ShapeCompatible,
        PackConditionKind::SignConvention => ScientificConstraintKind::SignConvention,
        PackConditionKind::Uniform => ScientificConstraintKind::Uniform,
    }
}

fn operator_property(property: PackOperatorProperty) -> OperatorProperty {
    match property {
        PackOperatorProperty::Adjoint => OperatorProperty::Adjoint,
        PackOperatorProperty::Bilinear => OperatorProperty::Bilinear,
        PackOperatorProperty::Gradient => OperatorProperty::Gradient,
        PackOperatorProperty::Hessian => OperatorProperty::Hessian,
        PackOperatorProperty::InnerProduct => OperatorProperty::InnerProduct,
        PackOperatorProperty::Jacobian => OperatorProperty::Jacobian,
        PackOperatorProperty::Linear => OperatorProperty::Linear,
        PackOperatorProperty::Norm => OperatorProperty::Norm,
    }
}

fn condition_status(
    kind: PackConditionKind,
    resolved_subjects: usize,
    mechanically_verified: bool,
    explicitly_refuted: bool,
) -> ConstraintStatus {
    if resolved_subjects == 0 {
        return ConstraintStatus::Unsupported;
    }
    if explicitly_refuted {
        return ConstraintStatus::Conflicting;
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
        | PackConditionKind::MapsBetween
        | PackConditionKind::OperatorProperty
        | PackConditionKind::RankCompatible
        | PackConditionKind::ShapeCompatible => ConstraintStatus::Required,
    }
}

#[allow(clippy::too_many_arguments)]
fn condition_evidence(
    relation_id: &str,
    condition: &PackLawCondition,
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
    formula_range: &SourceRange,
    shapes: &ShapeObservations,
    quantities: &QuantityObservations,
    consistency: &RoleObservations,
    assumptions: &[AssumptionInfo],
    role_support: &RoleSupportPlan,
    external: &ExternalTypeEnvironment,
    positive_facts: &[PositiveFormulaFact],
    scopes: &ScopeGraph,
) -> (Vec<Evidence>, bool, bool) {
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
        relation_id,
        condition,
        bindings,
        formula_range,
        assumptions,
        external.assumptions_at(offset),
        scopes,
    );
    if let Some(condition_evidence) = &semantic_condition.supporting {
        push_evidence(&mut evidence, condition_evidence.clone());
    }
    if let Some(condition_evidence) = &semantic_condition.refuting {
        push_evidence(&mut evidence, condition_evidence.clone());
    }
    let structural_condition =
        structural_condition_evidence(condition, roles, bindings, actual, role_support);
    if let Some(condition_evidence) = &structural_condition {
        push_evidence(&mut evidence, condition_evidence.clone());
    }
    let formula_fact = (kind == PackConditionKind::Positive)
        .then(|| {
            positive_condition_evidence(subjects, bindings, actual, formula_range, positive_facts)
        })
        .flatten();
    if let Some(condition_evidence) = &formula_fact {
        push_evidence(&mut evidence, condition_evidence.clone());
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
                source_anchors: Vec::new(),
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
        semantic_condition.supporting.is_some()
            || structural_condition.is_some()
            || formula_fact.is_some()
            || proved_subjects == subjects.len(),
        semantic_condition.refuting.is_some(),
    )
}

fn structural_condition_evidence(
    condition: &PackLawCondition,
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
    role_support: &RoleSupportPlan,
) -> Option<Evidence> {
    if condition.kind == PackConditionKind::DomainMembership
        && condition.subjects.iter().all(|subject| {
            matches!(
                role_support.proof_for(subject),
                RoleBindingProof::Typed | RoleBindingProof::DerivedFromTypes
            )
        })
    {
        return Some(Evidence {
            rule_id: "typed-operator/domain-membership".into(),
            kind: "canonical-binding".into(),
            strength: "hard".into(),
            source_ranges: vec![actual.range.clone()],
            source_anchors: Vec::new(),
        });
    }
    if condition.kind == PackConditionKind::Differentiable && condition.subjects.len() == 2 {
        let function = bindings.get(&condition.subjects[0])?;
        let variable = bindings.get(&condition.subjects[1])?;
        if expression_asserts_derivative(actual, function, variable) {
            return Some(Evidence {
                rule_id: "canonical-regularity/asserted-derivative".into(),
                kind: "canonical-binding".into(),
                strength: "hard".into(),
                source_ranges: vec![actual.range.clone()],
                source_anchors: Vec::new(),
            });
        }
    }
    if condition.kind == PackConditionKind::SameContext
        && application_roles_share_arguments(&condition.subjects, bindings, actual)
    {
        return Some(Evidence {
            rule_id: "canonical-context/shared-application-arguments".into(),
            kind: "canonical-binding".into(),
            strength: "hard".into(),
            source_ranges: vec![actual.range.clone()],
            source_anchors: Vec::new(),
        });
    }
    if condition.kind == PackConditionKind::SameContext
        && typed_operator_groups_roles(&condition.subjects, roles, bindings, actual)
    {
        return Some(Evidence {
            rule_id: "typed-operator/shared-context".into(),
            kind: "canonical-binding".into(),
            strength: "hard".into(),
            source_ranges: vec![actual.range.clone()],
            source_anchors: Vec::new(),
        });
    }
    if condition.kind != PackConditionKind::SameContext || condition.subjects.len() != 2 {
        return None;
    }
    let first = bindings.get(&condition.subjects[0])?;
    let second = bindings.get(&condition.subjects[1])?;
    let SemanticExprKind::Relation {
        operator,
        left,
        right,
    } = &actual.kind
    else {
        return None;
    };
    if operator != "equals" {
        return None;
    }
    let transpose_pair = |result: &SemanticExpr, source: &SemanticExpr| {
        let SemanticExprKind::Apply {
            operator,
            arguments,
        } = &source.kind
        else {
            return false;
        };
        operator == "transpose"
            && arguments.len() == 1
            && ((equivalent(result, first) && equivalent(&arguments[0], second))
                || (equivalent(result, second) && equivalent(&arguments[0], first)))
    };
    (transpose_pair(left, right) || transpose_pair(right, left)).then(|| Evidence {
        rule_id: "canonical-context-preserving/transpose".into(),
        kind: "canonical-binding".into(),
        strength: "hard".into(),
        source_ranges: vec![actual.range.clone()],
        source_anchors: Vec::new(),
    })
}

fn typed_operator_groups_roles(
    subjects: &[String],
    roles: &[PackLawRole],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
) -> bool {
    if subjects.len() < 2 {
        return false;
    }
    let subject_roles = subjects
        .iter()
        .map(|subject| {
            Some((
                roles.iter().find(|role| role.id == *subject)?,
                bindings.get(subject)?,
            ))
        })
        .collect::<Option<Vec<_>>>();
    let Some(subject_roles) = subject_roles else {
        return false;
    };
    expression_any(actual, |candidate| {
        let Some((operator, arguments)) = typed_operator_parts(candidate) else {
            return false;
        };
        OPERATOR_TYPES.iter().any(|signature| {
            signature.operator == operator
                && signature.operand_concepts.len() == arguments.len()
                && subject_roles.iter().all(|(role, binding)| {
                    arguments
                        .iter()
                        .zip(&signature.operand_concepts)
                        .any(|(argument, expected)| {
                            let available = semantic_leaf_symbols(argument);
                            concepts_share_lineage(&role.concept, expected)
                                && semantic_leaf_symbols(binding)
                                    .iter()
                                    .all(|symbol| available.contains(symbol))
                        })
                })
        })
    })
}

fn collect_positive_formula_facts(expressions: &[SemanticExpr]) -> Vec<PositiveFormulaFact> {
    let mut facts = Vec::new();
    let mut visited = 0;
    for expression in expressions {
        collect_positive_facts_from_expression(expression, &mut facts, &mut visited);
        if facts.len() >= MAX_POSITIVE_FACTS || visited >= MAX_STRUCTURAL_FACT_NODES {
            break;
        }
    }
    facts
}

fn collect_positive_facts_from_expression(
    expression: &SemanticExpr,
    output: &mut Vec<PositiveFormulaFact>,
    visited: &mut usize,
) {
    if output.len() >= MAX_POSITIVE_FACTS || *visited >= MAX_STRUCTURAL_FACT_NODES {
        return;
    }
    *visited += 1;
    let relations = match &expression.kind {
        SemanticExprKind::System(items) => items
            .iter()
            .take(MAX_POSITIVE_FACTS_PER_SYSTEM)
            .collect::<Vec<_>>(),
        SemanticExprKind::Relation { .. } => vec![expression],
        _ => Vec::new(),
    };
    let equalities = relations
        .iter()
        .filter_map(|relation| {
            let SemanticExprKind::Relation {
                operator,
                left,
                right,
            } = &relation.kind
            else {
                return None;
            };
            (operator == "equals").then_some((left.as_ref(), right.as_ref()))
        })
        .collect::<Vec<_>>();
    let mut positive = relations
        .iter()
        .filter_map(|relation| {
            let SemanticExprKind::Relation {
                operator,
                left,
                right,
            } = &relation.kind
            else {
                return None;
            };
            if operator == "greater-than" && is_additive_zero(right) {
                Some((*left.clone(), relation.range.clone()))
            } else if operator == "less-than" && is_additive_zero(left) {
                Some((*right.clone(), relation.range.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let mut cursor = 0;
    while cursor < positive.len() && positive.len() < MAX_POSITIVE_FACTS_PER_SYSTEM {
        let (known, range) = positive[cursor].clone();
        for (left, right) in &equalities {
            let propagated = if equivalent(&known, left) {
                Some((*right).clone())
            } else if equivalent(&known, right) {
                Some((*left).clone())
            } else {
                None
            };
            if let Some(propagated) = propagated
                && !positive
                    .iter()
                    .any(|(candidate, _)| equivalent(candidate, &propagated))
            {
                positive.push((propagated, range.clone()));
            }
        }
        cursor += 1;
    }
    output.extend(
        positive
            .into_iter()
            .take(MAX_POSITIVE_FACTS - output.len())
            .map(|(expression, evidence_range)| PositiveFormulaFact {
                expression,
                evidence_range,
            }),
    );
    for child in expression_children(expression) {
        if !matches!(expression.kind, SemanticExprKind::System(_))
            || matches!(child.kind, SemanticExprKind::System(_))
        {
            collect_positive_facts_from_expression(child, output, visited);
        }
    }
}

fn positive_condition_evidence(
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
    formula_range: &SourceRange,
    facts: &[PositiveFormulaFact],
) -> Option<Evidence> {
    let subject = bindings.get(subjects.first()?)?;
    let subject_symbols = semantic_leaf_symbols(subject);
    let fact = facts.iter().find(|fact| {
        fact.evidence_range.end_offset <= formula_range.start_offset
            && (equivalent(subject, &fact.expression)
                || application_with_symbols_matches(actual, &subject_symbols, &fact.expression))
    })?;
    Some(Evidence {
        rule_id: "canonical-propagation/positive-equality".into(),
        kind: "canonical-binding".into(),
        strength: "hard".into(),
        source_ranges: vec![fact.evidence_range.clone()],
        source_anchors: Vec::new(),
    })
}

fn application_with_symbols_matches(
    expression: &SemanticExpr,
    symbols: &[String],
    expected: &SemanticExpr,
) -> bool {
    expression_any(expression, |candidate| {
        if !matches!(candidate.kind, SemanticExprKind::Apply { .. }) {
            return false;
        }
        let available = semantic_leaf_symbols(candidate);
        symbols.iter().all(|symbol| available.contains(symbol)) && equivalent(candidate, expected)
    })
}

fn application_roles_share_arguments(
    subjects: &[String],
    bindings: &BTreeMap<String, SemanticExpr>,
    actual: &SemanticExpr,
) -> bool {
    if subjects.len() < 2 {
        return false;
    }
    let argument_lists = subjects
        .iter()
        .map(|subject| {
            let operator = bindings.get(subject).and_then(semantic_symbol)?;
            let mut matches = Vec::new();
            collect_application_arguments(actual, &operator, &mut matches);
            (matches.len() == 1).then(|| matches.pop().unwrap())
        })
        .collect::<Option<Vec<_>>>();
    let Some((first, rest)) = argument_lists
        .as_deref()
        .and_then(|items| items.split_first())
    else {
        return false;
    };
    !first.is_empty()
        && rest.iter().all(|arguments| {
            arguments.len() == first.len()
                && arguments
                    .iter()
                    .zip(first.iter())
                    .all(|(left, right)| equivalent(left, right))
        })
}

fn collect_application_arguments<'a>(
    expression: &'a SemanticExpr,
    expected_operator: &str,
    output: &mut Vec<&'a [SemanticExpr]>,
) {
    if let SemanticExprKind::Apply {
        operator,
        arguments,
    } = &expression.kind
        && operator == expected_operator
    {
        output.push(arguments);
    }
    for child in expression_children(expression) {
        collect_application_arguments(child, expected_operator, output);
    }
}

fn expression_asserts_derivative(
    expression: &SemanticExpr,
    function: &SemanticExpr,
    variable: &SemanticExpr,
) -> bool {
    if let SemanticExprKind::Derivative {
        expression,
        variable: derivative_variable,
        ..
    } = &expression.kind
        && equivalent(expression, function)
        && semantic_symbol(variable).is_some_and(|name| name == derivative_variable.value)
    {
        return true;
    }
    crate::canonical::expression_children(expression)
        .into_iter()
        .any(|child| expression_asserts_derivative(child, function, variable))
}

const MAX_ASSUMPTION_DISTANCE: u32 = 640;
const MAX_ATTACHED_ASSUMPTION_GAP: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypedAssumption {
    Differentiable,
    Positive,
    SignConvention,
    OpposedSignConvention,
    Uniform,
    SameContext,
    DifferentContext,
    MapsBetween,
    OperatorProperty(PackOperatorProperty),
    RankCompatible,
    Other,
}

fn typed_assumption(assumption: &AssumptionInfo) -> TypedAssumption {
    let value = assumption_value_and_target(&assumption.value).0;
    match (assumption.kind.as_str(), value) {
        ("regularity", "differentiable") => TypedAssumption::Differentiable,
        ("sign", "positive" | "strictly-positive") => TypedAssumption::Positive,
        ("sign-convention", value) if value.starts_with("not-") => {
            TypedAssumption::OpposedSignConvention
        }
        ("sign-convention", _) => TypedAssumption::SignConvention,
        ("uniformity", "uniform") => TypedAssumption::Uniform,
        ("context", "different-context") => TypedAssumption::DifferentContext,
        ("context", _) => TypedAssumption::SameContext,
        ("mapping", "maps-between") => TypedAssumption::MapsBetween,
        ("operator-property" | "algebraic-property", "adjoint") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Adjoint)
        }
        ("operator-property" | "algebraic-property", "bilinear") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Bilinear)
        }
        ("operator-property" | "algebraic-property", "gradient") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Gradient)
        }
        ("operator-property" | "algebraic-property", "hessian") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Hessian)
        }
        ("operator-property" | "algebraic-property", "inner-product") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::InnerProduct)
        }
        ("operator-property" | "algebraic-property", "jacobian") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Jacobian)
        }
        ("operator-property" | "algebraic-property", "linear") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Linear)
        }
        ("operator-property" | "algebraic-property", "norm") => {
            TypedAssumption::OperatorProperty(PackOperatorProperty::Norm)
        }
        ("rank", "compatible") => TypedAssumption::RankCompatible,
        _ => TypedAssumption::Other,
    }
}

#[derive(Default)]
struct AssumptionConditionEvidence {
    supporting: Option<Evidence>,
    refuting: Option<Evidence>,
}

fn assumption_condition_evidence(
    relation_id: &str,
    condition: &PackLawCondition,
    bindings: &BTreeMap<String, SemanticExpr>,
    formula_range: &SourceRange,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
    scopes: &ScopeGraph,
) -> AssumptionConditionEvidence {
    let symbols = bound_condition_symbols(&condition.subjects, bindings);
    let subjects_match = |assumption: &&AssumptionInfo| {
        let (assumption_value, target_relation_id) = assumption_value_and_target(&assumption.value);
        if target_relation_id.is_some_and(|target| target != relation_id) {
            return false;
        }
        if assumption.subjects.is_empty() {
            return true;
        }
        if assumption_value == condition.id
            || assumption_value.strip_prefix("not-") == Some(condition.id.as_str())
        {
            return assumption
                .subjects
                .iter()
                .all(|subject| condition_symbols_contain(&symbols, subject));
        }
        symbols
            .iter()
            .all(|symbol| condition_symbols_contain(&assumption.subjects, symbol))
    };
    let mut resolution = AssumptionConditionEvidence::default();
    for assumption in assumptions
        .iter()
        .filter(|assumption| {
            let source_ranges = &assumption.evidence.source_ranges;
            let start = source_ranges
                .iter()
                .map(|range| range.start_offset)
                .min()
                .unwrap_or_default();
            let end = source_ranges
                .iter()
                .map(|range| range.end_offset)
                .max()
                .unwrap_or_default();
            let explicit_targets = assumption_formula_targets(assumption);
            let positional_preceding_limit = if condition.kind == PackConditionKind::SignConvention
                && explicit_targets.is_empty()
                && assumption.evidence.kind == "attached-prose"
            {
                MAX_ATTACHED_ASSUMPTION_GAP
            } else {
                MAX_ASSUMPTION_DISTANCE
            };
            let precedes_formula = end <= formula_range.start_offset
                && formula_range.start_offset - end <= positional_preceding_limit;
            let immediately_follows_formula = formula_range.end_offset <= start
                && start - formula_range.end_offset <= MAX_ATTACHED_ASSUMPTION_GAP;
            let targets_formula = if explicit_targets.is_empty() {
                condition.kind != PackConditionKind::SignConvention
                    && source_ranges.iter().any(|range| {
                        range.start_offset < formula_range.end_offset
                            && formula_range.start_offset < range.end_offset
                    })
            } else {
                explicit_targets
                    .iter()
                    .any(|range| source_ranges_overlap(range, formula_range))
            };
            // Scientific prose stores subject ranges first, explicit formula targets
            // next, and the reviewed phrase last. A target-bound assumption may not
            // fall back by distance onto a different formula.
            let targets_another_formula = !explicit_targets.is_empty()
                && explicit_targets
                    .iter()
                    .all(|range| !source_ranges_overlap(range, formula_range));
            let visible = assumption
                .evidence
                .source_ranges
                .last()
                .is_some_and(|phrase| {
                    scope_visible(
                        &scopes.path_at(phrase.start_offset),
                        &scopes.path_at(formula_range.start_offset),
                    )
                });
            let positionally_attached =
                !targets_another_formula && (precedes_formula || immediately_follows_formula);
            (positionally_attached || targets_formula) && visible && subjects_match(assumption)
        })
        .chain(external_assumptions.iter().filter(subjects_match))
    {
        if resolution.supporting.is_none() && assumption_supports_condition(condition, assumption) {
            resolution.supporting = Some(assumption_public_evidence(assumption));
        }
        if resolution.refuting.is_none()
            && condition.kind == PackConditionKind::SignConvention
            && typed_assumption(assumption) == TypedAssumption::OpposedSignConvention
        {
            resolution.refuting = Some(assumption_public_evidence(assumption));
        }
        if resolution.supporting.is_some() && resolution.refuting.is_some() {
            break;
        }
    }
    resolution
}

fn assumption_supports_condition(
    condition: &PackLawCondition,
    assumption: &AssumptionInfo,
) -> bool {
    let assumption_value = assumption_value_and_target(&assumption.value).0;
    if assumption_value == condition.id {
        return true;
    }
    match (condition.kind, typed_assumption(assumption)) {
        (PackConditionKind::Differentiable, TypedAssumption::Differentiable)
        | (PackConditionKind::Positive, TypedAssumption::Positive)
        | (PackConditionKind::SignConvention, TypedAssumption::SignConvention)
        | (PackConditionKind::Uniform, TypedAssumption::Uniform) => true,
        (PackConditionKind::MapsBetween, TypedAssumption::MapsBetween)
        | (PackConditionKind::RankCompatible, TypedAssumption::RankCompatible) => true,
        (PackConditionKind::OperatorProperty, TypedAssumption::OperatorProperty(property)) => {
            condition.operator_property == Some(property)
        }
        (
            PackConditionKind::Assumption
            | PackConditionKind::DomainMembership
            | PackConditionKind::SameContext
            | PackConditionKind::ShapeCompatible,
            _,
        ) => false,
        _ => false,
    }
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
            source_anchors: Vec::new(),
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
            typed_assumption(assumption) == TypedAssumption::SameContext
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
        source_anchors: Vec::new(),
    })
}

fn evidence_ranges_overlap(left: &Evidence, right: &Evidence) -> bool {
    left.source_ranges.iter().any(|left| {
        right.source_ranges.iter().any(|right| {
            left.start_offset < right.end_offset && right.start_offset < left.end_offset
        })
    })
}

fn source_ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

fn law_has_admission_blocking_refutation(
    law: &PackLaw,
    bindings: &BTreeMap<String, SemanticExpr>,
    formula_range: &SourceRange,
    assumptions: &[AssumptionInfo],
    external_assumptions: &[AssumptionInfo],
    scopes: &ScopeGraph,
) -> bool {
    law.conditions.iter().any(|condition| {
        let symbols = bound_condition_symbols(&condition.subjects, bindings);
        if symbols.is_empty() {
            return false;
        }
        assumptions
            .iter()
            .filter(|assumption| {
                assumption
                    .evidence
                    .source_ranges
                    .last()
                    .is_some_and(|phrase| {
                        scope_visible(
                            &scopes.path_at(phrase.start_offset),
                            &scopes.path_at(formula_range.start_offset),
                        )
                    })
            })
            .chain(external_assumptions)
            .any(|assumption| {
                let refutes = match condition.kind {
                    PackConditionKind::SameContext => {
                        typed_assumption(assumption) == TypedAssumption::DifferentContext
                    }
                    PackConditionKind::SignConvention => false,
                    _ => false,
                };
                refutes
                    && (assumption.subjects.is_empty()
                        || assumption
                            .subjects
                            .iter()
                            .all(|subject| condition_symbols_contain(&symbols, subject)))
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
        .map(|symbol| symbol.trim_start_matches('\\').to_owned())
        .collect()
}

fn condition_symbols_contain<'a>(
    symbols: impl IntoIterator<Item = &'a String>,
    target: &str,
) -> bool {
    symbols.into_iter().any(|symbol| {
        symbol == target
            || notation_matches_symbol(symbol, target)
            || notation_matches_symbol(target, symbol)
    })
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
        SemanticExprKind::Number(_) => Vec::new(),
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
        SemanticExprKind::Number(number) => Some(number.clone()),
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
        SemanticExprKind::Product(items) => Some(
            items
                .iter()
                .map(expression_label)
                .collect::<Option<Vec<_>>>()?
                .join(" "),
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
        SemanticExprKind::Apply { .. }
            | SemanticExprKind::Derivative { .. }
            | SemanticExprKind::Symbol(_)
    ) && context.source.as_bytes().get(end) == Some(&b')')
    {
        end += 1;
    }
    let authored = context
        .source
        .get(start..end)
        .map(str::trim)
        .map(|label| label.trim_end_matches(['.', ',', ';', ':']).trim_end())
        .map(strip_source_group)
        .filter(|label| source_label_matches_expression(expression, label));
    Some(authored.unwrap_or(&canonical).to_owned())
}

fn role_source_label(
    expression: &SemanticExpr,
    projection: RoleSourceProjection,
    context: &RecognitionContext<'_>,
) -> Option<String> {
    if projection == RoleSourceProjection::Expression {
        return source_expression_label(expression, context);
    }
    let SemanticExprKind::Apply { operator, .. } = &expression.kind else {
        return source_expression_label(expression, context);
    };
    let start = context
        .source_index
        .byte_for_utf16(operator.range.start_offset);
    let end = context
        .source_index
        .byte_for_utf16(operator.range.end_offset);
    let authored = context
        .source
        .get(start..end)
        .map(str::trim)
        .filter(|label| {
            label
                .chars()
                .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
                .count()
                <= MAX_COMPOSITE_SOURCE_LABEL_CHARS
                && semantic_symbol(&lower_template(label)).as_deref() == Some(operator.as_str())
        });
    Some(authored.unwrap_or(operator.as_str()).to_owned())
}

fn strip_source_group(mut label: &str) -> &str {
    loop {
        let bytes = label.as_bytes();
        if bytes.first() != Some(&b'{') || bytes.last() != Some(&b'}') {
            return label;
        }
        let mut depth = 0_u32;
        let balanced = label.char_indices().all(|(offset, character)| {
            match character {
                '{' => depth += 1,
                '}' => {
                    let Some(next) = depth.checked_sub(1) else {
                        return false;
                    };
                    depth = next;
                    if depth == 0 && offset + character.len_utf8() < label.len() {
                        return false;
                    }
                }
                _ => {}
            }
            true
        });
        if !balanced || depth != 0 {
            return label;
        }
        label = label[1..label.len() - 1].trim();
    }
}

fn source_label_matches_expression(expression: &SemanticExpr, label: &str) -> bool {
    if label.is_empty() {
        return false;
    }
    if !expression.provenance.is_empty() {
        return label.strip_prefix('\\').is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        }) && label
            .chars()
            .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
            .count()
            <= MAX_COMPOSITE_SOURCE_LABEL_CHARS;
    }
    match &expression.kind {
        SemanticExprKind::Symbol(symbol) => {
            label == symbol
                || label.strip_prefix('\\') == Some(symbol.as_str())
                || source_label_is_structural_operator_application(label, symbol)
        }
        SemanticExprKind::Index { .. } => {
            label
                .chars()
                .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
                .count()
                <= MAX_COMPOSITE_SOURCE_LABEL_CHARS
                && render_canonical(&lower_template(label)) == render_canonical(expression)
        }
        SemanticExprKind::Derivative { .. } => {
            label.starts_with("\\dot")
                || label.starts_with("\\ddot")
                || label.starts_with("\\frac")
                || label.contains('\'')
        }
        SemanticExprKind::Power(_, exponent) if is_decorative_star(exponent) => {
            label
                .chars()
                .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
                .count()
                <= MAX_COMPOSITE_SOURCE_LABEL_CHARS
                && render_canonical(&lower_template(label)) == render_canonical(expression)
        }
        SemanticExprKind::Apply { .. }
        | SemanticExprKind::Fraction(_, _)
        | SemanticExprKind::Negate(_)
        | SemanticExprKind::Power(_, _)
        | SemanticExprKind::Product(_)
        | SemanticExprKind::Sum(_) => {
            label
                .chars()
                .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
                .count()
                <= MAX_COMPOSITE_SOURCE_LABEL_CHARS
                && render_canonical(&lower_template(label)) == render_canonical(expression)
        }
        _ => false,
    }
}

fn source_label_is_structural_operator_application(label: &str, symbol: &str) -> bool {
    if symbol != "transpose" {
        return false;
    }
    if label
        .chars()
        .take(MAX_COMPOSITE_SOURCE_LABEL_CHARS + 1)
        .count()
        > MAX_COMPOSITE_SOURCE_LABEL_CHARS
    {
        return false;
    }
    let lowered = lower_template(label);
    matches!(
        &lowered.kind,
        SemanticExprKind::Apply { operator, arguments }
            if operator.as_str() == symbol
                && arguments.len() == 1
    )
}

fn is_decorative_star(expression: &SemanticExpr) -> bool {
    matches!(
        &expression.kind,
        SemanticExprKind::Symbol(value) | SemanticExprKind::Unknown(value) if value == "*"
    )
}

fn variadic_labels(
    expression: &SemanticExpr,
    projection: RoleSourceProjection,
    context: &RecognitionContext<'_>,
) -> Vec<String> {
    match &expression.kind {
        SemanticExprKind::Sum(items) => items
            .iter()
            .flat_map(|item| variadic_labels(item, projection, context))
            .collect(),
        SemanticExprKind::Negate(inner) => variadic_labels(inner, projection, context),
        SemanticExprKind::Product(items) if contains_sum_operator(expression) => items
            .iter()
            .filter(|item| !contains_sum_operator(item))
            .filter_map(|item| role_source_label(item, projection, context))
            .collect(),
        _ => role_source_label(expression, projection, context)
            .into_iter()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        COMPILED_LAWS, ExternalTypeEnvironment, LAW_DISPATCH, LawAnalysisContext, LawDispatch,
        LawObservations, TypedAssumption, collect_law_expressions,
        differs_by_one_explicit_relation_sign, observe_laws, rejected_formula_sign_conflicts,
        source_label_matches_expression, strip_formula_presentation, structural_alternatives,
        typed_assumption, unify_all,
    };
    use crate::canonical::{SemanticExpr, SemanticExprKind, lower_document_region, lower_template};
    use crate::consistency::observe_roles;
    use crate::domain_signature::laws_share_collision;
    use crate::pack::{PackConditionKind, PackLawCondition, PackOperatorProperty};
    use crate::parser::{ParsedMath, parse_regions, test_math_regions};
    use crate::prose::observe_prose;
    use crate::quantity::observe_quantities;
    use crate::shape::observe_shapes;
    use crate::{
        AssumptionInfo, ConstraintStatus, DocumentLanguage, Evidence, LawBindingProof,
        LawRecognition, LawRecognitionStatus, ProjectDocument, ScientificConstraintKind,
        SourceIndex, SourceRange,
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
    fn condition_assumptions_lower_to_closed_typed_values() {
        let assumption = |kind: &str, value: &str| AssumptionInfo {
            kind: kind.into(),
            value: value.into(),
            subjects: Vec::new(),
            evidence: Evidence {
                rule_id: "test".into(),
                kind: "display-only".into(),
                strength: "display-only".into(),
                source_ranges: Vec::new(),
                source_anchors: Vec::new(),
            },
        };
        assert_eq!(
            typed_assumption(&assumption("regularity", "differentiable")),
            TypedAssumption::Differentiable
        );
        assert_eq!(
            typed_assumption(&assumption("context", "different-context")),
            TypedAssumption::DifferentContext
        );
        assert_eq!(
            typed_assumption(&assumption("sign-convention", "not-clockwise")),
            TypedAssumption::OpposedSignConvention
        );
        assert_eq!(
            typed_assumption(&assumption("mapping", "maps-between")),
            TypedAssumption::MapsBetween
        );
        assert_eq!(
            typed_assumption(&assumption("operator-property", "jacobian")),
            TypedAssumption::OperatorProperty(PackOperatorProperty::Jacobian)
        );
        assert_eq!(
            typed_assumption(&assumption("rank", "compatible")),
            TypedAssumption::RankCompatible
        );
    }

    #[test]
    fn explicit_relation_sign_mismatch_is_structural_and_symmetric() {
        let placeholders = ["electric-field", "magnetic-field", "time"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let negative = lower_template(
            "\\nabla \\times electric-field = -\\frac{\\partial magnetic-field}{\\partial time}",
        );
        let positive = lower_template("\\nabla \\times E = \\frac{\\partial B}{\\partial t}");
        assert!(differs_by_one_explicit_relation_sign(
            &negative,
            &positive,
            &placeholders
        ));
        assert!(!differs_by_one_explicit_relation_sign(
            &negative,
            &negative,
            &placeholders
        ));
    }

    #[test]
    fn rejected_formula_conflicts_with_an_attached_activated_law_sign() {
        let source = "Our laboratory convention assigns the conventional minus sign to the magnetic-field derivative in Faraday's law.\nThe following formula is rejected:\n$\\nabla\\times E=+\\frac{\\partial B}{\\partial t}$.";
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
        assert!(
            prose
                .semantic_evidence
                .formula_is_rejected(&canonical[0].range)
        );
        assert!(
            prose
                .semantic_evidence
                .law_activations
                .iter()
                .any(|activation| {
                    activation.law_id == "faraday-law" && activation.frame.establishes()
                }),
            "{:#?}",
            prose.semantic_evidence.law_activations
        );
        let conflicts = rejected_formula_sign_conflicts(&canonical[0], &prose.semantic_evidence);
        assert_eq!(conflicts.len(), 1, "canonical={:#?}", canonical[0]);
        assert_eq!(
            conflicts[0].conflict_id,
            "electromagnetism:faraday-law/explicit-sign-mismatch"
        );

        let source = "Our convention assigns the minus sign in Faraday's law.\nThe application is not stated. Consider $\\nabla\\times E=+\\frac{\\partial B}{\\partial t}$.";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
        let document = ProjectDocument {
            content: source.into(),
            math_regions: regions.clone(),
            ..document
        };
        let parsed = parse_regions(source, &regions);
        let canonical = canonical_expressions(&document, &parsed);
        let prose = observe_prose(&document, &parsed, &canonical);
        assert!(
            prose
                .semantic_evidence
                .formula_is_rejected(&canonical[0].range)
        );
        assert!(
            !prose
                .semantic_evidence
                .formula_is_explicitly_retracted(&canonical[0].range)
        );
        assert!(
            rejected_formula_sign_conflicts(&canonical[0], &prose.semantic_evidence).is_empty(),
            "a missing application is not an explicit sign refutation"
        );
    }

    #[test]
    fn condition_proof_does_not_depend_on_presentation_evidence_kind() {
        let condition = PackLawCondition {
            id: "positive-input".into(),
            kind: PackConditionKind::Positive,
            subjects: vec!["input".into()],
            label: "input is positive".into(),
            operator_property: None,
            evidence_phrases: Vec::new(),
        };
        let bindings = BTreeMap::from([(
            "input".into(),
            SemanticExpr {
                kind: SemanticExprKind::Symbol("x".into()),
                range: SourceRange {
                    start_offset: 20,
                    end_offset: 21,
                },
                provenance: Vec::new(),
            },
        )]);
        let mut assumption = AssumptionInfo {
            kind: "sign".into(),
            value: "positive".into(),
            subjects: vec!["x".into()],
            evidence: Evidence {
                rule_id: "test".into(),
                kind: "attached-prose".into(),
                strength: "strong".into(),
                source_ranges: vec![SourceRange {
                    start_offset: 35,
                    end_offset: 45,
                }],
                source_anchors: Vec::new(),
            },
        };
        let formula_range = SourceRange {
            start_offset: 10,
            end_offset: 30,
        };
        let document = ProjectDocument {
            prose_annotations: Vec::new(),
            file_id: "main".into(),
            path: "main.tex".into(),
            language: DocumentLanguage::Latex,
            content: " ".repeat(64),
            document_version: 1,
            schema_version: 8,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
            blocks: Vec::new(),
            declarations: Vec::new(),
            math_regions: Vec::new(),
            macros: Vec::new(),
            includes: Vec::new(),
        };
        let scopes = crate::scope::ScopeGraph::new(&document);
        let first = super::assumption_condition_evidence(
            "test:law",
            &condition,
            &bindings,
            &formula_range,
            std::slice::from_ref(&assumption),
            &[],
            &scopes,
        );
        assumption.evidence.kind = "display-only".into();
        assumption.evidence.strength = "display-only".into();
        let mutated = super::assumption_condition_evidence(
            "test:law",
            &condition,
            &bindings,
            &formula_range,
            std::slice::from_ref(&assumption),
            &[],
            &scopes,
        );
        assert!(first.supporting.is_some());
        assert!(mutated.supporting.is_some());
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
    fn bounded_integrals_bind_variables_limits_and_integrands_once() {
        let template = lower_template("\\int_{lower}^{upper} density(variable) \\, d variable = 1");
        let actual = lower_template("\\int_a^b f(x) \\, d x = 1");
        let captured = lower_template("\\int_a^b f(y) \\, d x = 1");
        let placeholders = ["density", "variable", "lower", "upper"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let bindings = unify_all(&template, &actual, &placeholders, &BTreeMap::new());

        assert_eq!(bindings.len(), 1, "{bindings:?}");
        assert_eq!(bindings[0].len(), 4);
        assert!(
            unify_all(&template, &captured, &placeholders, &BTreeMap::new()).is_empty(),
            "a bound variable must agree with its integrand occurrence"
        );
    }

    #[test]
    fn generated_notation_projects_its_authored_macro_call() {
        let expression = SemanticExpr {
            kind: SemanticExprKind::Symbol("DeltaT".into()),
            range: SourceRange {
                start_offset: 2,
                end_offset: 8,
            },
            provenance: vec![SourceRange {
                start_offset: 2,
                end_offset: 8,
            }],
        };

        assert!(source_label_matches_expression(&expression, "\\dtemp"));
        assert!(!source_label_matches_expression(
            &expression,
            "\\dtemp extra"
        ));
        assert!(!source_label_matches_expression(&expression, "\\dtemp{T}"));
    }

    #[test]
    fn atomic_role_label_does_not_absorb_a_following_grouped_factor() {
        let expression = SemanticExpr {
            kind: SemanticExprKind::Symbol("C_n".into()),
            range: SourceRange {
                start_offset: 0,
                end_offset: 25,
            },
            provenance: Vec::new(),
        };

        assert!(source_label_matches_expression(&expression, "C_n"));
        assert!(!source_label_matches_expression(
            &expression,
            "C_n\\left(\\frac{dv_n}{dt}"
        ));
        assert!(!source_label_matches_expression(&expression, "C_n(s)"));
    }

    #[test]
    fn structural_transpose_operator_accepts_only_its_application() {
        let expression = SemanticExpr {
            kind: SemanticExprKind::Symbol("transpose".into()),
            range: SourceRange {
                start_offset: 0,
                end_offset: 8,
            },
            provenance: Vec::new(),
        };

        assert!(source_label_matches_expression(&expression, "M^{\\top}"));
        assert!(!source_label_matches_expression(&expression, "M(s)"));
    }

    #[test]
    fn recognizes_a_command_transpose_without_accepting_arbitrary_calls() {
        let source = "Let $M$ and $N$ be matrices. The transposed matrix satisfies Let $N$ and $M$ denote linear operator matrix and linear operator matrix, respectively. $N=M^{\\top}$";
        assert_eq!(recognized_laws(source), ["matrix-transpose-definition"]);
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
        let bindings = unify_all(&template, &actual, &placeholders, &BTreeMap::new());
        assert!(
            !bindings.is_empty(),
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
        let authored = recognized_law_observations(
            "Let $A$ denote a request timing out and $B$ the request having entered the retry path. In sampled traffic, $P(A\\cap B)=0.012$ and $P(B)=0.08>0$. Among those requests, $P(A\\mid B)=\\frac{P(A\\cap B)}{P(B)}=0.15$.",
        )
        .into_iter()
        .find(|law| law.law_id == "conditional-probability")
        .unwrap();
        assert_eq!(authored.status, LawRecognitionStatus::Verified);
        assert!(
            authored
                .conditions
                .iter()
                .flat_map(|condition| &condition.evidence)
                .any(|evidence| {
                    matches!(
                        evidence.rule_id.as_str(),
                        "typed-operator/shared-context" | "canonical-propagation/positive-equality"
                    )
                })
        );
        let missing_positive = recognized_law_observations(
            "Use $P(A\\mid B)=\\frac{P(A\\cap B)}{P(B)}$ without a positivity premise.",
        )
        .into_iter()
        .find(|law| law.law_id == "conditional-probability")
        .unwrap();
        assert_eq!(
            missing_positive.status,
            LawRecognitionStatus::ConditionMissing
        );
        let combined_premise = recognized_law_observations(
            "Let $A$ denote a request timing out and $B$ the retry event. The sample gives $P(A\\cap B)=0.012,\\qquad P(B)=0.08>0$. Then $P(A\\mid B)=\\frac{P(A\\cap B)}{P(B)}=0.15$.",
        )
        .into_iter()
        .find(|law| law.law_id == "conditional-probability")
        .unwrap();
        assert_eq!(combined_premise.status, LawRecognitionStatus::Verified);
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
    fn nested_law_recognition_owns_only_its_exact_application_range() {
        let source = "Let $A$ and $B$ be events. The reported value is $p=P(A\\cap B)/P(B)$.";
        let intersection = recognized_law_observations(source)
            .into_iter()
            .find(|law| law.law_id == "event-intersection")
            .expect("expected the nested event intersection law");
        let expected_start = source.find("A\\cap B").unwrap() as u32;
        let expected_end = source.find(")/P").unwrap() as u32;

        assert_eq!(
            intersection
                .relation
                .as_ref()
                .expect("expected public relation")
                .range,
            SourceRange {
                start_offset: expected_start,
                end_offset: expected_end,
            }
        );
        assert!(
            intersection
                .evidence
                .iter()
                .find(|evidence| evidence.rule_id == "semantic-law-unification")
                .is_some_and(|evidence| evidence.source_ranges.iter().any(|range| {
                    range.start_offset < expected_start && expected_end <= range.end_offset
                })),
            "the exact query range must retain the enclosing formula as evidence"
        );
    }

    #[test]
    fn unary_application_preserves_nested_law_relation_ownership() {
        let source = "Let $A$ and $B$ be events. The observed probability is $P(A\\cap B)=0.012$.";
        let intersection = recognized_law_observations(source)
            .into_iter()
            .find(|law| law.law_id == "event-intersection")
            .expect("expected the nested event intersection law");
        let expected_start = source.find("P(A\\cap B)").unwrap() as u32;
        let expected_end = source.find("=0.012").unwrap() as u32 + "=0.012".len() as u32;

        assert_eq!(
            intersection.range,
            SourceRange {
                start_offset: expected_start,
                end_offset: expected_end,
            }
        );
        let relation = intersection.relation.expect("expected public relation");
        assert_eq!(
            &source[relation.range.start_offset as usize..relation.range.end_offset as usize],
            "P(A\\cap B)=0.012"
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
    fn probability_operators_accept_square_bracket_application() {
        assert_eq!(
            recognized_laws(
                "For integrable random variables $X$ and $Y$ and scalars $a,b$, linearity of expectation gives $\\mathbb E[aX+bY]=a\\mathbb E[X]+b\\mathbb E[Y]$.",
            ),
            ["expectation-linearity"],
        );
        assert!(
            recognized_laws(
                "Let $E$ be a matrix and $X$ a vector. The untyped expression is $E[X]$.",
            )
            .is_empty()
        );
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

        let period =
            recognized_law_observations("For a periodic signal, the asserted relation is $f=1/T$.")
                .into_iter()
                .find(|law| law.law_id == "period-frequency-reciprocity")
                .expect("reviewed notation supplies the frequency quantity roles");
        assert_eq!(period.status, LawRecognitionStatus::ConditionMissing);
        assert!(
            period
                .bindings
                .iter()
                .all(|binding| binding.proof == LawBindingProof::Asserted),
            "domain routing and formula-level assertion may expose a candidate but cannot type its roles: {period:?}"
        );
        assert!(period.conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::Positive
                && condition.status == ConstraintStatus::Required
        }));
    }

    #[test]
    fn structural_derivatives_are_derived_while_declared_operands_remain_typed() {
        let recognition = recognized_law_observations(
            "Let $y$ be a function. Let $x$ be a variable. The function $y$ is differentiable in $x$. $y'(x)=\\frac{dy}{dx}(x)$",
        )
        .into_iter()
        .find(|law| law.law_id == "first-derivative-relation")
        .expect("the canonical derivative relation remains visible");
        let proofs = recognition
            .bindings
            .iter()
            .map(|binding| (binding.parameter.as_str(), binding.proof))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(proofs["derivative"], LawBindingProof::Derived);
        assert_eq!(proofs["function"], LawBindingProof::Typed);
        assert_eq!(proofs["variable"], LawBindingProof::Typed);
        for binding in recognition
            .bindings
            .iter()
            .filter(|binding| binding.proof == LawBindingProof::Typed)
        {
            assert!(
                binding
                    .evidence
                    .source_ranges
                    .iter()
                    .all(|range| range.end_offset <= recognition.range.start_offset),
                "typed proof must retain its independent declaration roots: {binding:?}"
            );
        }
        let derivative = recognition
            .bindings
            .iter()
            .find(|binding| binding.parameter == "derivative")
            .unwrap();
        assert!(
            derivative
                .evidence
                .source_ranges
                .iter()
                .any(|range| recognition.range.contains(range.start_offset))
        );
    }

    #[test]
    fn explicit_measurement_context_verifies_frequency_conversions() {
        let source = r"The measured-period stage estimates the oscillator period from successive rising edges.
The computed ordinary-frequency stage uses
\[f=1/T.\]
The same oscillator's converted angular-frequency stage then supplies the phase accumulator with
\[\omega=2\pi f.\]
This conversion is performed once per accepted timing sample so the accumulator and diagnostic display share one estimate.";
        let recognized = recognized_law_observations(source);
        for law_id in [
            "period-frequency-reciprocity",
            "angular-frequency-definition",
        ] {
            let law = recognized
                .iter()
                .find(|law| law.law_id == law_id)
                .unwrap_or_else(|| panic!("missing {law_id}: {recognized:?}"));
            assert_eq!(law.status, LawRecognitionStatus::Verified, "{law:?}");
        }
    }

    #[test]
    fn recognizes_a_frequency_conversion_with_an_explicit_unit_literal() {
        let actual = lower_template("\\omega_c=2\\pi(20\\,\\mathrm{Hz})");
        let template = lower_template("angular-frequency = 2 pi ordinary-frequency");
        let placeholders = ["angular-frequency", "ordinary-frequency"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let bindings = unify_all(&template, &actual, &placeholders, &BTreeMap::new());
        assert!(
            !bindings.is_empty(),
            "template={} actual={}",
            crate::canonical::render_canonical(&template),
            crate::canonical::render_canonical(&actual)
        );
        let source = "A 20 Hz crossover is converted as $\\omega_c=2\\pi(20\\,\\mathrm{Hz})$.";
        let recognized = recognized_law_observations(source);
        let law = recognized
            .iter()
            .find(|law| law.law_id == "angular-frequency-definition")
            .unwrap_or_else(|| panic!("missing angular-frequency-definition: {recognized:?}"));
        assert_eq!(law.status, LawRecognitionStatus::Verified, "{law:?}");
        assert!(
            recognized_law_observations(
                "A symbolic product is not a measured frequency: $\\omega=2\\pi(ab)$."
            )
            .iter()
            .all(|law| law.law_id != "angular-frequency-definition")
        );
    }

    #[test]
    fn semantic_law_title_heads_select_candidates_but_do_not_prove_conditions() {
        let recognition =
            recognized_law_observations("The Reynolds number is $R_D=\\frac{\\rho vD}{\\mu}$.")
                .into_iter()
                .find(|recognition| recognition.law_id == "reynolds-number-definition")
                .expect("the exact named relation should remain visible");
        assert_eq!(
            recognition.status,
            LawRecognitionStatus::ConditionMissing,
            "{recognition:?}"
        );
    }

    #[test]
    fn an_explicit_law_name_does_not_turn_unresolved_roles_into_hard_evidence() {
        let recognition = recognized_law_observations(
            "The Newtonian shear relation is $x=y\\dot z$, but the report does not identify these symbols.",
        )
        .into_iter()
        .find(|recognition| recognition.law_id == "newtonian-shear")
        .expect("the named exact relation should remain a candidate");

        assert!(recognition.bindings.iter().any(|binding| {
            binding.proof == LawBindingProof::Asserted && binding.evidence.source_ranges.len() >= 2
        }));
        assert!(
            recognition
                .bindings
                .iter()
                .all(|binding| binding.proof != LawBindingProof::Typed)
        );
    }

    #[test]
    fn declarative_condition_phrases_can_prove_a_named_law_condition() {
        let recognition = recognized_law_observations(
            "For a Newtonian fluid, the constitutive relation is $\\tau=\\mu\\dot\\gamma$.",
        )
        .into_iter()
        .find(|recognition| recognition.law_id == "newtonian-shear")
        .expect("the exact named relation should remain visible");
        assert_eq!(
            recognition.status,
            LawRecognitionStatus::Verified,
            "{recognition:?}"
        );
        assert!(recognition.conditions.iter().any(|condition| {
            condition.condition_id == "newtonian-fluid"
                && condition.status == ConstraintStatus::Verified
                && condition
                    .evidence
                    .iter()
                    .any(|evidence| evidence.rule_id == "english-scientific-assumption")
        }));
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
    fn transpose_structure_proves_its_shared_scalar_context() {
        let recognized = recognized_law_observations(
            "Let the centered data table satisfy $A\\in\\mathbb R^{r\\times s}$. The matrix transpose is $B=A^{\\mathsf T}$.",
        );
        let transpose = recognized
            .iter()
            .find(|law| law.law_id == "matrix-transpose-definition")
            .expect("matrix transpose");
        assert_eq!(transpose.status, LawRecognitionStatus::Verified);
        assert!(transpose.conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::SameContext
                && condition.status == ConstraintStatus::Verified
                && condition
                    .evidence
                    .iter()
                    .any(|evidence| evidence.rule_id == "canonical-context-preserving/transpose")
        }));
    }

    #[test]
    fn typed_dot_signature_establishes_mechanical_power_roles_and_context() {
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
        let role_definitions = prose
            .definitions
            .iter()
            .chain(&prose.semantic_role_definitions)
            .cloned()
            .collect::<Vec<_>>();
        let roles = observe_roles(&document, &role_definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let scopes = crate::scope::ScopeGraph::new(&document);
        let domains = crate::domain::observe_domains(
            &document,
            scopes.clone(),
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
                scopes: &scopes,
            },
        );
        let recognition = &laws.all()[0];
        assert_eq!(recognition.law_id, "mechanical-power");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
        assert!(recognition.conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::SameContext
                && condition.status == ConstraintStatus::Verified
                && condition
                    .evidence
                    .iter()
                    .any(|evidence| evidence.rule_id == "typed-operator/shared-context")
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
        let role_definitions = prose
            .definitions
            .iter()
            .chain(&prose.semantic_role_definitions)
            .cloned()
            .collect::<Vec<_>>();
        let roles = observe_roles(&document, &role_definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let scopes = crate::scope::ScopeGraph::new(&document);
        let domains = crate::domain::observe_domains(
            &document,
            scopes.clone(),
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
                scopes: &scopes,
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
    fn law_bindings_project_typed_shapes_and_symbolic_extents() {
        let laws = recognized_law_observations(
            "Let $H$ be an m by n matrix, $q$ an n-dimensional vector, and $z$ an m-dimensional vector. Then $z=Hq$.",
        );
        let law = laws
            .iter()
            .find(|law| law.law_id == "matrix-vector-product")
            .unwrap();
        let operator = law
            .bindings
            .iter()
            .find(|binding| binding.parameter == "operator")
            .unwrap();
        let input = law
            .bindings
            .iter()
            .find(|binding| binding.parameter == "vector")
            .unwrap();
        assert_eq!(
            operator.constraint.kind,
            crate::SemanticConstraintKind::Matrix
        );
        assert_eq!(operator.constraint.dimensions, ["m", "n"]);
        assert_eq!(input.constraint.kind, crate::SemanticConstraintKind::Vector);
        assert_eq!(input.constraint.dimensions, ["n"]);
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
    fn draft_proposals_do_not_establish_nested_laws() {
        let source = "Let $A$ and $B$ be events. The draft calculation proposed \
            $P(A\\cup B)=P(A)+P(B)=0.29$.";

        assert!(
            recognized_laws(source)
                .iter()
                .all(|law| law != "event-union"),
        );
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
    fn set_laws_remain_visible_inside_logical_equivalences() {
        assert_eq!(
            recognized_laws(
                "Let $A$ and $B$ be sets in one universe. For every $x$, $x\\in A\\cup B\\iff (x\\in A)\\lor(x\\in B)$.",
            ),
            ["set-union"],
        );
        assert!(
            recognized_laws(
                "For two event sets $A$ and $B$, $x\\in A\\cup B\\iff (x\\in A)\\lor(x\\in B)$.",
            )
            .iter()
            .any(|law| law == "set-union")
        );
        assert!(
            recognized_laws(
                "Without set declarations, the surface is $x\\in Q\\cup R\\iff (x\\in Q)\\lor(x\\in R)$.",
            )
            .is_empty()
        );
    }

    #[test]
    fn set_laws_remain_visible_inside_cardinality_identities() {
        let source = "Assume $A$ and $B$ are finite sets. $|A\\cup B|=|A|+|B|-|A\\cap B|$.";
        let recognized = recognized_laws(source);
        assert!(recognized.iter().any(|law| law == "set-union"));
        assert!(recognized.iter().any(|law| law == "set-intersection"));
        assert!(
            recognized
                .iter()
                .any(|law| law == "two-set-inclusion-exclusion")
        );
        let inclusion_exclusion = recognized_law_observations(source)
            .into_iter()
            .find(|law| law.law_id == "two-set-inclusion-exclusion")
            .unwrap();
        assert_eq!(
            inclusion_exclusion
                .bindings
                .iter()
                .map(|binding| binding.symbol.as_str())
                .collect::<Vec<_>>(),
            ["|A\\cup B|", "|A|", "|B|", "|A\\cap B|"]
        );

        let untyped = recognized_laws("$|A\\cup B|=|A|+|B|-|A\\cap B|$.");
        assert!(
            !untyped
                .iter()
                .any(|law| law == "two-set-inclusion-exclusion")
        );

        let scalar = recognized_laws(
            "Assume $A$ and $B$ are scalar quantities. $|A\\cup B|=|A|+|B|-|A\\cap B|$.",
        );
        assert!(
            !scalar
                .iter()
                .any(|law| law == "two-set-inclusion-exclusion")
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
    fn neighboring_relations_keep_exact_ownership_and_shared_formula_evidence() {
        let source = "B=A^T,\\qquad C=AB";
        let expression = lower_template(source);
        let formula_range = SourceRange {
            start_offset: 0,
            end_offset: source.encode_utf16().count() as u32,
        };
        let mut actuals = Vec::new();
        collect_law_expressions(&expression, Some(&formula_range), &mut actuals);
        let (ranges, envelopes): (Vec<_>, Vec<_>) = actuals
            .iter()
            .filter_map(|(expression, range, envelope, _)| {
                matches!(expression.kind, SemanticExprKind::Relation { .. })
                    .then_some((range.clone(), envelope.clone()))
            })
            .unzip();
        assert_eq!(ranges.len(), 2);
        assert!(ranges[0].end_offset <= ranges[1].start_offset);
        assert_eq!(envelopes, [formula_range.clone(), formula_range]);
    }

    #[test]
    fn neighboring_law_recognitions_have_disjoint_query_ranges() {
        let source = "Let $K$ be energy, $p$ momentum, $m$ mass, and $v$ velocity. $K=\\tfrac12 mv^2, \\qquad p=mv$.";
        let recognized = recognized_law_observations(source);
        let kinetic = recognized
            .iter()
            .find(|law| law.law_id == "kinetic-energy-definition")
            .unwrap();
        let momentum = recognized
            .iter()
            .find(|law| law.law_id == "linear-momentum-definition")
            .unwrap();
        assert!(kinetic.range.end_offset <= momentum.range.start_offset);
        assert_eq!(
            &source[kinetic.range.start_offset as usize..kinetic.range.end_offset as usize],
            "K=\\tfrac12 mv^2"
        );
        assert_eq!(
            &source[momentum.range.start_offset as usize..momentum.range.end_offset as usize],
            "p=mv"
        );
        let kinetic_formula = kinetic
            .evidence
            .iter()
            .find(|evidence| evidence.rule_id == "semantic-law-unification")
            .unwrap();
        let momentum_formula = momentum
            .evidence
            .iter()
            .find(|evidence| evidence.rule_id == "semantic-law-unification")
            .unwrap();
        assert_eq!(
            kinetic_formula.source_ranges,
            momentum_formula.source_ranges
        );
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
    fn sign_convention_condition_preserves_explicit_refutation() {
        assert_eq!(
            recognized_laws(
                "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Under the passive sign convention, $i=C\\frac{dv}{dt}$."
            ),
            ["capacitor-current-law"]
        );
        let public_capacitor = recognized_law_observations(
            "Let $C>0$ be a constant capacitance, let $v_C(t)$ be the voltage from the marked positive terminal to the marked negative terminal, and let $i_C(t)$ enter the positive terminal. Under this passive sign convention, the capacitor law is\n\\[\ni_C(t)=C\\frac{dv_C}{dt}(t).\n\\]",
        );
        let public_capacitor = public_capacitor
            .iter()
            .find(|recognition| recognition.law_id == "capacitor-current-law")
            .expect("public capacitor law");
        assert_eq!(public_capacitor.status, LawRecognitionStatus::Verified);
        let public_condition = public_capacitor
            .conditions
            .iter()
            .find(|condition| condition.condition_id == "passive-sign-convention")
            .expect("public passive sign condition");
        assert_eq!(public_condition.status, ConstraintStatus::Verified);
        let public_prose = public_condition
            .evidence
            .iter()
            .find(|evidence| evidence.rule_id == "english-scientific-assumption")
            .expect("public sign convention prose");
        assert_eq!(public_prose.source_ranges.len(), 1);
        for descriptor in ["ohm", "kirchhoff", "electric", "closed"] {
            let source = format!(
                "Let $C>0$ be a constant capacitance, let $v_C(t)$ be voltage, let $i_C(t)$ be electric current, and let $t$ be time. Under this passive sign convention, the {descriptor} law is $i_C(t)=C\\frac{{dv_C}}{{dt}}(t)$."
            );
            let capacitor = recognized_law_observations(&source)
                .into_iter()
                .find(|recognition| recognition.law_id == "capacitor-current-law")
                .expect("bounded capacitor candidate");
            assert_ne!(
                capacitor.status,
                LawRecognitionStatus::Verified,
                "{descriptor}"
            );
            assert!(capacitor.conditions.iter().any(|condition| {
                condition.condition_id == "passive-sign-convention"
                    && condition.status == ConstraintStatus::Required
            }));
        }
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
        let source = "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Without adopting the passive sign convention, consider $i=C\\frac{dv}{dt}$.";
        let refuted = recognized_law_observations(source);
        let capacitor = refuted
            .iter()
            .find(|recognition| recognition.law_id == "capacitor-current-law")
            .expect("the refuted law remains inspectable");
        assert_eq!(capacitor.status, LawRecognitionStatus::Conflicting);
        let condition = capacitor
            .conditions
            .iter()
            .find(|condition| condition.condition_id == "passive-sign-convention")
            .expect("sign convention condition");
        assert_eq!(condition.status, ConstraintStatus::Conflicting);
        let phrase_start = source.find("Without").unwrap() as u32;
        let phrase_end = source.find("passive sign convention").unwrap() as u32
            + "passive sign convention".len() as u32;
        assert!(condition.evidence.iter().any(|evidence| {
            evidence.rule_id == "english-scientific-assumption"
                && matches!(evidence.kind.as_str(), "explicit-prose" | "attached-prose")
                && evidence.source_ranges.iter().any(|range| {
                    range.start_offset == phrase_start && range.end_offset == phrase_end
                })
        }));
        let conflicts = super::refuted_law_condition_conflicts(std::slice::from_ref(capacitor));
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].evidence.iter().any(|evidence| {
            evidence.kind == "canonical-math"
                && evidence.source_ranges.iter().any(|range| {
                    range.start_offset < capacitor.range.end_offset
                        && capacitor.range.start_offset < range.end_offset
                })
        }));
        assert!(conflicts[0].evidence.iter().any(|evidence| {
            evidence.rule_id == "english-scientific-assumption"
                && evidence.source_ranges.iter().any(|range| {
                    range.start_offset == phrase_start && range.end_offset == phrase_end
                })
        }));

        let mut asserted = capacitor.clone();
        asserted.bindings[0].proof = LawBindingProof::Asserted;
        assert!(super::refuted_law_condition_conflicts(&[asserted]).is_empty());
    }

    #[test]
    fn non_authoritative_or_distant_sign_refutations_do_not_create_conflicts() {
        let declarations =
            "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. ";
        for source in [
            format!(
                "{declarations}If the passive sign convention were not adopted, one might compare $i=C\\frac{{dv}}{{dt}}$."
            ),
            format!(
                "{declarations}According to the cited note, the passive sign convention is not adopted. Consider $i=C\\frac{{dv}}{{dt}}$."
            ),
            format!(
                "Without adopting the passive sign convention. {} {declarations}Consider $i=C\\frac{{dv}}{{dt}}$.",
                "background ".repeat(80)
            ),
            format!(
                "\\section{{Rejected convention}}\nWithout adopting the passive sign convention.\n\\section{{Current model}}\n{declarations}Consider $i=C\\frac{{dv}}{{dt}}$."
            ),
        ] {
            let observations = recognized_law_observations(&source);
            assert!(
                super::refuted_law_condition_conflicts(&observations).is_empty(),
                "{source}: {observations:#?}"
            );
        }
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
    fn prime_voltage_derivative_matches_the_capacitor_law() {
        assert_eq!(
            recognized_laws(
                "Let $i_m$ denote electric current scalar, $K_m$ capacitance scalar, $v_m$ voltage scalar, and $t$ duration scalar. $i_m=K_m v_m'$.",
            ),
            ["capacitor-current-law"],
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
            (
                "Let $u$, $t$, and $\\kappa$ denote pde field function, variable, and diffusivity scalar, respectively. The reviewed law context states diffusion equation for $\\frac{\\partial u}{\\partial t}=\\kappa\\nabla^2u$",
                "diffusion-equation",
            ),
            (
                "Let $u$, $t$, and $F$ denote pde field function, variable, and conservation flux vector, respectively. The reviewed law context states conservation-form pde for $\\frac{\\partial u}{\\partial t}+\\operatorname{div}(F)=0$",
                "conservation-form-equation",
            ),
            (
                "Let $L$, $u$, and $\\lambda$ denote evolution operator, pde field function, and eigenvalue scalar, respectively. The reviewed law context states differential operator eigenproblem for $L(u)=\\lambda u$",
                "differential-operator-eigenproblem",
            ),
            (
                "Let $f$, $x$, $a$, and $b$ denote density function, variable, integration bound, and integration bound, respectively. The reviewed law context states density normalization for $\\int_a^b f(x)\\,dx=1$",
                "density-normalization",
            ),
            (
                "Let $c$, $X$, and $Y$ denote covariance scalar, random variable, and random variable, respectively. The reviewed law context states covariance definition for $c=\\operatorname{Cov}(X,Y)$",
                "covariance-value-definition",
            ),
            (
                "Let $y$, $X$, $b$, and $e$ denote regression response, linear operator, regression parameter, and regression error, respectively. The reviewed law context states linear regression model for $y=Xb+e$",
                "linear-regression-model",
            ),
            (
                "Let $x_1$, $A$, $x_0$, and $w$ denote state, state matrix, state, and process noise, respectively. The reviewed law context states stochastic state transition for $x_1=Ax_0+w$",
                "stochastic-state-transition",
            ),
        ] {
            assert_eq!(recognized_laws(source), [expected], "{source}");
        }
    }

    #[test]
    fn recognition_budget_is_scoped_to_each_expression() {
        let source = (0..20)
            .map(|index| {
                format!(
                    "Here $C_{index}$, $A_{index}$, and $B_{index}$ denote linear operator matrix, linear operator matrix, and linear operator matrix, respectively. The reviewed law context states matrix addition for $C_{index}=A_{index}+B_{index}$."
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let recognized = recognized_law_observations(&source)
            .into_iter()
            .filter(|law| law.law_id == "matrix-addition")
            .count();
        assert_eq!(recognized, 20);
    }

    #[test]
    fn callable_role_bindings_project_their_entity_heads() {
        let source = "The transfer function is $H(s)=\\frac{Y(s)}{X(s)}$.";
        let transfer = recognized_law_observations(source)
            .into_iter()
            .find(|law| law.law_id == "transfer-function")
            .unwrap();
        assert_eq!(
            transfer
                .bindings
                .iter()
                .map(|binding| binding.symbol.as_str())
                .collect::<Vec<_>>(),
            ["H", "Y", "X"]
        );
        assert_eq!(transfer.status, LawRecognitionStatus::Verified);
        assert!(transfer.conditions[0].evidence.iter().any(|evidence| {
            evidence.rule_id == "canonical-context/shared-application-arguments"
        }));

        assert!(
            recognized_law_observations("The transfer function is $H(s)=\\frac{Y(z)}{X(s)}$.")
                .into_iter()
                .all(|law| law.law_id != "transfer-function")
        );
    }

    #[test]
    fn declarative_head_projection_preserves_styled_field_notation() {
        let source = "The fitted $\\epsilon_{\\rm eff}$ is the effective permittivity, $\\mathbf E$ is the macroscopic electric field, and $\\mathbf D$ is the electric displacement field. We imposed $\\mathbf D(\\mathbf x)=\\epsilon_{\\rm eff}\\,\\mathbf E(\\mathbf x)$.";
        let constitutive = recognized_law_observations(source)
            .into_iter()
            .find(|law| law.law_id == "linear-electric-constitutive-law")
            .unwrap();
        assert_eq!(
            constitutive
                .bindings
                .iter()
                .map(|binding| binding.symbol.as_str())
                .collect::<Vec<_>>(),
            ["\\mathbf D", "epsilon_eff", "\\mathbf E"]
        );
    }

    #[test]
    fn reviewed_mean_velocity_phrase_proves_the_mass_flow_condition() {
        let positive = recognized_law_observations(
            "Let $\\rho$ be density and $A$ area. Particle tracking supplied the area-mean exit speed $v_e$. The mass flow rate is $\\dot m=\\rho A v_e$.",
        )
        .into_iter()
        .find(|law| law.law_id == "mass-flow-rate")
        .unwrap();
        assert_eq!(positive.status, LawRecognitionStatus::Verified);
        assert!(
            positive.conditions[0]
                .evidence
                .iter()
                .any(|evidence| evidence.rule_id == "english-scientific-assumption")
        );

        let unsupported = recognized_law_observations(
            "Let $\\rho$ be density, $A$ area, and $v_e$ exit speed. The mass flow rate is $\\dot m=\\rho A v_e$.",
        )
        .into_iter()
        .find(|law| law.law_id == "mass-flow-rate")
        .unwrap();
        assert_eq!(unsupported.status, LawRecognitionStatus::ConditionMissing);
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
    fn recognizes_source_grounded_closed_system_balance_without_a_law_specific_path() {
        let observations = recognized_law_observations(
            "The vessel is a closed system. Heat into the system and work done by the system are positive. The balance is $\\Delta U=Q-W$.",
        );
        let recognition = observations
            .iter()
            .find(|recognition| recognition.law_id == "closed-system-first-law")
            .expect("generic compiled law recognition");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
    }

    #[test]
    fn recognizes_bracketed_blackboard_expectation_linearity() {
        let template = lower_template("E(a X + b Y) = a E(X) + b E(Y)");
        let actual = lower_template("E(2 X - 3 Y) = 2 E(X) - 3 E(Y)");
        let placeholders = ["a", "X", "b", "Y"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            !unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty(),
            "template={} actual={}",
            crate::canonical::render_canonical(&template),
            crate::canonical::render_canonical(&actual),
        );
        let observations = recognized_law_observations(
            "The random variables $X$ and $Y$ are integrable. Linearity of expectation gives $\\mathbb E[2X-3Y]=2\\mathbb E[X]-3\\mathbb E[Y]$.",
        );
        assert!(
            observations
                .iter()
                .any(|recognition| recognition.law_id == "expectation-linearity"),
            "{observations:#?}"
        );
    }

    #[test]
    fn recognizes_authored_binary_cross_entropy_notation() {
        let template =
            lower_template("loss = -label log probability - (1 - label) log (1 - probability)");
        let actual = lower_template("L(y,p) = - y log p - (1 - y) log (1 - p)");
        let placeholders = ["loss", "label", "probability"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert!(
            !unify_all(&template, &actual, &placeholders, &BTreeMap::new()).is_empty(),
            "template={} actual={}",
            crate::canonical::render_canonical(&template),
            crate::canonical::render_canonical(&actual),
        );
        let observations = recognized_law_observations(
            r"For each binary label $y\in\{0,1\}$, the network emits a probability $p\in(0,1)$. We computed binary cross-entropy as
\[
L(y,p)=-y\log p-(1-y)\log(1-p).
\]",
        );
        let recognition = observations
            .iter()
            .find(|recognition| recognition.law_id == "binary-cross-entropy")
            .expect("binary cross-entropy should be recognized");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
        assert!(
            recognition
                .bindings
                .iter()
                .any(|binding| binding.parameter == "loss" && binding.symbol == "L(y,p)")
        );
    }

    #[test]
    fn verifies_normal_equation_from_declared_matrix_dimensions() {
        let observations = recognized_law_observations(
            r"Let $A$ be a 2 by 3 matrix, $\theta$ a 3-dimensional vector, and $b$ a 2-dimensional vector.
Consequently every least-squares minimizer obeys the normal equation
\[
  A^\top A\theta=A^\top b.
\]",
        );
        let recognition = observations
            .iter()
            .find(|recognition| recognition.law_id == "least-squares-normal-equation")
            .expect("normal equation should be recognized");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
    }

    #[test]
    fn named_normal_equation_does_not_override_incompatible_shapes() {
        let observations = recognized_law_observations(
            r"Let $A$ be a $2$ by $3$ matrix, $\theta$ a $4$-dimensional vector, and $b$ a $2$-dimensional vector.
The normal equation is $A^\top A\theta=A^\top b$.",
        );
        assert!(
            observations
                .iter()
                .all(|recognition| recognition.law_id != "least-squares-normal-equation")
        );
    }

    #[test]
    fn named_normal_equation_does_not_prove_unknown_shapes() {
        let recognition =
            recognized_law_observations("The normal equation is $A^\\top A\\theta=A^\\top b$.")
                .into_iter()
                .find(|recognition| recognition.law_id == "least-squares-normal-equation")
                .expect("the exact named relation should remain visible");
        assert_eq!(recognition.status, LawRecognitionStatus::ConditionMissing);
        assert!(recognition.conditions.iter().any(|condition| {
            condition.kind == ScientificConstraintKind::ShapeCompatible
                && condition.status == ConstraintStatus::Required
        }));
    }

    #[test]
    fn possessive_equation_flow_uses_the_existing_role_and_law_pipeline() {
        let source = "A charge packet with signed charge $q_b$ entering a region held at potential $V_b$ has electric potential energy $U_b=q_bV_b$.";
        assert_eq!(recognized_laws(source), ["electric-potential-energy"]);
    }

    #[test]
    fn descriptive_symbol_roles_complete_a_following_formula_relation() {
        let source = r"For the dc supply, let $I_s$ be the conventional current delivered at terminal
voltage $V_s$; its output power is
\[
P_s=V_sI_s.
\]";
        let observations = law_observations(source);
        let power = observations
            .all()
            .iter()
            .find(|recognition| recognition.law_id == "electric-power-law")
            .unwrap_or_else(|| {
                panic!(
                    "the independently described power roles should identify the law: {observations:#?}"
                )
            });
        assert_eq!(power.status, LawRecognitionStatus::ConditionMissing);
        assert!(power.bindings.iter().all(|binding| {
            matches!(
                binding.proof,
                LawBindingProof::Typed | LawBindingProof::Derived
            )
        }));
    }

    #[test]
    fn occurrence_roles_preserve_explicitly_different_contexts() {
        let source = "Event $A$ belongs to the first probability space, while event $B$ belongs to a different experiment. The formal surface is $A\\cap B$.";
        assert!(
            recognized_laws(source)
                .iter()
                .all(|law| *law != "event-intersection")
        );
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
        assert_eq!(condition.status, ConstraintStatus::Verified);
        assert_eq!(condition.subjects, ["f", "x"]);
        assert!(
            condition
                .evidence
                .iter()
                .any(|evidence| { evidence.rule_id == "canonical-regularity/asserted-derivative" })
        );
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

        let circuit = recognized_law_observations(
            "Current $I$, voltage $V$, and resistance $R$ use the passive sign convention. The accepted equation is $V=RI$.",
        );
        let ohm = circuit
            .iter()
            .find(|law| law.law_id == "ohm-law")
            .expect("Ohm's law");
        assert_eq!(ohm.conditions[0].status, ConstraintStatus::Verified);

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

    #[test]
    fn composite_derivative_notation_does_not_poison_a_later_typed_law() {
        let recognized = recognized_law_observations(
            "Let $y$ be a function and $x$ a variable. $y$ is differentiable in $x$. We use $\\frac{dy}{dx}(x)$ for the derivative value. $y'(x)=\\frac{dy}{dx}(x)$",
        );
        let recognition = recognized
            .iter()
            .find(|law| law.law_id == "first-derivative-relation")
            .expect("first derivative recognition");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
        let relation = recognition.relation.as_ref().expect("recognized relation");
        assert_eq!(relation.roles.len(), 3);
        assert_eq!(
            relation
                .roles
                .iter()
                .map(|role| (role.role.as_str(), role.symbol.as_str()))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                ("derivative", "y'(x)"),
                ("function", "y"),
                ("variable", "x"),
            ])
        );

        assert!(
            recognized_law_observations(
                "Let $y$ be a function and $x$ a variable. $y$ is differentiable in $x$. We use $y$ for the derivative value. $y'(x)=\\frac{dy}{dx}(x)"
            )
            .iter()
            .all(|law| law.law_id != "first-derivative-relation"),
            "an explicit conflicting definition of y must still refuse the law"
        );
    }

    #[test]
    fn typed_structural_variants_preserve_authored_relations() {
        let failures = [
            (
                r"Let $f$ be a differentiable scalar function on an open subset of $\mathbb R^3$. We define the gradient vector field by
\[
g(x):=\nabla f(x)=\begin{pmatrix}\partial f/\partial x_1\\\partial f/\partial x_2\\\partial f/\partial x_3\end{pmatrix}(x).
\]",
                "gradient-relation",
            ),
            (
                r"Here $q_{\mathrm t}$ is the test charge, $\mathbf E_0$ is the electric field, and $\mathbf F_{\!e}$ denotes the resulting electric force. With magnetic force absent, therefore
\[
\mathbf F_{\!e}=q_{\mathrm t}\mathbf E_0.
\]",
                "electric-force-law",
            ),
        ]
        .into_iter()
        .filter_map(|(source, expected)| {
            let observations = recognized_law_observations(source);
            (!observations
                    .iter()
                    .any(|recognition| recognition.law_id == expected))
            .then(|| format!("{source}: {observations:#?}"))
        })
        .collect::<Vec<_>>();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn structural_variants_do_not_supply_missing_semantic_roles() {
        for source in [
            r"The displayed symbols are untyped: $\dot{x}=Ax+Bu$.",
            r"The displayed symbols are untyped: $g(x)=\nabla f(x)$.",
        ] {
            assert!(recognized_laws(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn styled_electric_force_uses_exact_quantity_roles_and_location_evidence() {
        let source = r"For a sufficiently small test body, the applied field is uniform over the charge distribution. Here $q_{\mathrm t}$ is the test charge, $\mathbf E_0$ is the electric field evaluated at the body's center, and $\mathbf F_{\!e}$ denotes the resulting electric force. Therefore
\[
\mathbf F_{\!e}=q_{\mathrm t}\mathbf E_0.
\]";
        let recognition = recognized_law_observations(source)
            .into_iter()
            .find(|recognition| recognition.law_id == "electric-force-law")
            .expect("electric force law");
        assert_eq!(recognition.status, LawRecognitionStatus::Verified);
        assert_eq!(
            recognition
                .relation
                .expect("public relation")
                .roles
                .into_iter()
                .map(|role| role.symbol)
                .collect::<Vec<_>>(),
            ["\\mathbf F_{\\!e}", "q_{\\mathrm t}", "\\mathbf E_0"]
        );
    }

    #[test]
    fn typed_operator_result_derives_only_its_exact_relation_output() {
        for source in [
            "Let $f$ be a scalar function. We define $g=\\nabla f$.",
            "Let $f$ be a scalar function. We define $\\nabla f=g$.",
        ] {
            let recognized = recognized_law_observations(source);
            let gradient = recognized
                .iter()
                .find(|law| law.law_id == "gradient-relation")
                .expect("gradient relation");
            let relation = gradient.relation.as_ref().expect("public relation");
            assert!(relation.roles.iter().any(|role| {
                role.role == "result"
                    && role.symbol == "g"
                    && role.concept_id.as_deref() == Some("calculus-analysis:nabla-operator")
            }));
        }

        for rejected in [
            "Let $f$ be a scalar function and $g$ a scalar. We define $g=\\nabla f$.",
            "Let $f$ be a scalar function. According to the reference, $g=\\nabla f$.",
            "Let $f$ be a scalar function. We define $g=f$.",
        ] {
            assert!(
                !recognized_laws(rejected)
                    .iter()
                    .any(|law| law == "gradient-relation"),
                "{rejected}"
            );
        }
    }

    #[test]
    fn characterized_operator_assignment_proves_its_domain_condition() {
        let source = r#"\begin{lemma}
Let $U\subset\mathbb R^n$ be open and $f:U\to\mathbb R$ differentiable. For every $x\in U$ there is a unique vector $g(x)$ such that
\[
  Df(x)[v]=g(x)\cdot v\qquad\text{for all }v\in\mathbb R^n.
\]
This vector field is the gradient of $f$; hence, after the characterization above, we set $g:=\nabla f$ on $U$.
\end{lemma}"#;
        let recognized = recognized_law_observations(source);
        let gradient = recognized
            .iter()
            .find(|law| law.law_id == "gradient-relation")
            .expect("gradient relation");

        assert_eq!(
            gradient.status,
            LawRecognitionStatus::Verified,
            "{:?}",
            gradient.bindings
        );
        assert!(gradient.conditions.iter().all(|condition| {
            condition.status == ConstraintStatus::Verified && !condition.evidence.is_empty()
        }));
    }

    #[test]
    fn withheld_pack_condition_is_not_verified() {
        let recognized = recognized_law_observations(
            r"Let $m$ be mass flow rate, $\rho$ density, $A$ area, and $u$ velocity.
The analysis withholds the uniform section condition for the displayed mass-flow relation.
\[
m=\rho A u
\]",
        );
        let flow = recognized
            .iter()
            .find(|law| law.law_id == "mass-flow-rate")
            .expect("mass-flow relation remains visible");
        let condition = flow
            .conditions
            .iter()
            .find(|condition| condition.condition_id == "uniform-section-values")
            .expect("uniform-section condition");

        assert_ne!(condition.status, ConstraintStatus::Verified, "{flow:#?}");
    }

    fn recognized_law_observations(source: &str) -> Vec<LawRecognition> {
        law_observations(source).all().to_vec()
    }

    #[test]
    fn law_roles_flow_forward_once_and_never_back_into_earlier_formulas() {
        let observations = law_observations(
            "Let $H$ be an m by n matrix, $q$ an n-dimensional vector, and $z$ an m-dimensional vector. Then $z=Hq$.",
        );
        let recognition = observations
            .all()
            .iter()
            .find(|law| law.law_id == "matrix-vector-product")
            .expect("matrix-vector product");
        let before = SourceRange {
            start_offset: recognition.range.start_offset.saturating_sub(1),
            end_offset: recognition.range.start_offset,
        };
        let after = SourceRange {
            start_offset: recognition.range.end_offset + 1,
            end_offset: recognition.range.end_offset + 2,
        };
        let environment = ExternalTypeEnvironment::default()
            .with_preceding_law_roles(&[before.clone(), after.clone()], &observations, &|_| None)
            .expect("the later formula receives derived roles");
        assert!(environment.roles_at(before.start_offset, "H").is_empty());
        assert!(
            environment
                .roles_at(after.start_offset, "H")
                .iter()
                .any(|role| role.concept_id == "linear-algebra:linear-operator"
                    && role.evidence.kind == "law-derived-role")
        );
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
        let role_definitions = prose
            .definitions
            .iter()
            .chain(&prose.semantic_role_definitions)
            .cloned()
            .collect::<Vec<_>>();
        let roles = observe_roles(&document, &role_definitions, &shapes);
        let external = ExternalTypeEnvironment::default();
        let scopes = crate::scope::ScopeGraph::new(&document);
        let domains = crate::domain::observe_domains(
            &document,
            scopes.clone(),
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
                scopes: &scopes,
            },
        )
    }
}
