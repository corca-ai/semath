use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::SourceRange;
use crate::constraint::{ConstraintInputClaim, PlannedConflict, plan_constraint_derivations};
use crate::entity_policy::{EntityEvidenceDecision, MAX_ENTITY_SURFACE_OCCURRENCES, decide_entity};
use crate::scope::scope_visible;

const MAX_DOCUMENT_OCCURRENCES: usize = 100_000;
const MAX_DOCUMENT_CLAIMS: usize = 50_000;
const MAX_DOCUMENT_CANDIDATES: usize = 50_000;
const MAX_DERIVATION_DEPTH: u8 = 8;
const MAX_RESOLUTION_CANDIDATES: usize = 32;
const MAX_CANDIDATE_EVIDENCE: usize = 32;

type OccurrencesByScope = BTreeMap<Vec<u32>, BTreeSet<SourceOccurrenceId>>;
type OccurrencesBySelection = BTreeMap<(String, String), OccurrencesByScope>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct SourceOccurrenceId {
    pub file_id: String,
    pub document_version: u64,
    pub local_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct EntityId {
    pub component_id: String,
    pub scope_path: Vec<u32>,
    pub kind: String,
    pub anchor: SourceOccurrenceId,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OccurrenceKind {
    Notation,
    Prose,
    MacroDeclaration,
    ResourceDeclaration,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NotationComponent {
    Identifier { value: String },
    NamedSurface { value: String },
    Modifier { name: String },
    Style { name: String },
    Subscript { base: String, index: String },
    Superscript,
    Argument { role: String },
    Delimiter { value: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceOccurrence {
    pub id: SourceOccurrenceId,
    pub component_id: String,
    pub kind: OccurrenceKind,
    pub range: SourceRange,
    pub selection_range: SourceRange,
    pub scope_path: Vec<u32>,
    pub structural_path: Vec<u32>,
    /// Monotonic project-snapshot order assigned by the lowering boundary.
    /// Cross-file visibility must never compare unrelated file-local offsets.
    pub availability_order: u64,
    pub surface: String,
    pub source_text: String,
    /// Exact authored text of the editable selection. This is indexed
    /// separately from the richer semantic surface so rename collision checks
    /// never scan the whole project or flatten decorated notation.
    pub selection_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notation: Vec<NotationComponent>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MentionModality {
    Notation,
    Prose,
    Declaration,
    Resource,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    pub occurrence_id: SourceOccurrenceId,
    pub modality: MentionModality,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CandidateId(pub String);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateFamily {
    Application,
    Binder,
    Bracketed,
    Decoration,
    Differential,
    Juxtaposition,
    Operator,
    Script,
    Style,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticCandidateClaim {
    pub id: CandidateId,
    pub occurrence_id: SourceOccurrenceId,
    pub family: CandidateFamily,
    pub interpretation: String,
    pub range: SourceRange,
    pub supporting_claims: Vec<ClaimId>,
    pub rejecting_claims: Vec<ClaimId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum EvidencePolarity {
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceModality {
    Asserted,
    Hypothetical,
    Hedged,
    Quoted,
    Cited,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceOrigin {
    Explicit,
    Derived,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceTier {
    Source = 0,
    ExplicitClaim = 1,
    Resolution = 2,
    Constraint = 3,
    DerivedLaw = 4,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRecord {
    pub id: EvidenceId,
    pub source: SourceOccurrenceId,
    pub scope_path: Vec<u32>,
    pub available_after: u64,
    pub polarity: EvidencePolarity,
    pub modality: EvidenceModality,
    pub origin: EvidenceOrigin,
    pub provenance: Vec<SourceOccurrenceId>,
    pub parent_claims: Vec<ClaimId>,
    pub rule_id: String,
    pub rule_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimPredicate {
    Defines,
    Names,
    Abbreviates,
    Aliases,
    HasRole,
    HasType,
    HasShape,
    HasDimension,
    HasQuantity,
    HasUnit,
    Assumes,
    Relates,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ClaimShape {
    Scalar,
    Vector(Vec<ClaimExtent>),
    Matrix(Vec<ClaimExtent>),
    Tensor(Vec<ClaimExtent>),
    Function {
        domain: Box<ClaimShape>,
        codomain: Box<ClaimShape>,
    },
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ClaimExtent {
    Known { value: u64 },
    Symbolic { entity: EntityId, display: String },
    Unknown { display: String },
}

impl ClaimExtent {
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Known { value } => value.to_string(),
            Self::Symbolic { display, .. } | Self::Unknown { display } => display.clone(),
        }
    }
}

impl From<&str> for ClaimExtent {
    fn from(display: &str) -> Self {
        display.parse::<u64>().map_or_else(
            |_| Self::Unknown {
                display: display.to_owned(),
            },
            |value| Self::Known { value },
        )
    }
}

impl From<String> for ClaimExtent {
    fn from(display: String) -> Self {
        Self::from(display.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct DimensionExponent {
    pub base: String,
    pub numerator: i16,
    pub denominator: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ClaimCondition {
    Nonzero(EntityId),
    Positive(EntityId),
    Nonnegative(EntityId),
    Invertible(EntityId),
    Member { entity: EntityId, set: EntityId },
    Named(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimComparison {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ClaimRelation {
    Comparison {
        operator: ClaimComparison,
        left: EntityId,
        right: EntityId,
        canonical_digest: String,
    },
    Sum {
        result: EntityId,
        terms: Vec<EntityId>,
        canonical_digest: String,
    },
    Product {
        result: EntityId,
        factors: Vec<EntityId>,
        canonical_digest: String,
    },
    Quotient {
        result: EntityId,
        numerator: EntityId,
        denominator: EntityId,
        canonical_digest: String,
    },
    Operation {
        result: EntityId,
        operator: ClaimOperation,
        operands: Vec<EntityId>,
        canonical_digest: String,
    },
    Application {
        result: EntityId,
        function: EntityId,
        arguments: Vec<EntityId>,
        canonical_digest: String,
    },
    Derivative {
        result: EntityId,
        operand: EntityId,
        variable: Option<EntityId>,
        order: u8,
        canonical_digest: String,
    },
    Integral {
        result: EntityId,
        integrand: EntityId,
        variable: Option<EntityId>,
        canonical_digest: String,
    },
}

impl ClaimRelation {
    fn canonical_digest(&self) -> &str {
        match self {
            Self::Comparison {
                canonical_digest, ..
            }
            | Self::Sum {
                canonical_digest, ..
            }
            | Self::Product {
                canonical_digest, ..
            }
            | Self::Quotient {
                canonical_digest, ..
            }
            | Self::Operation {
                canonical_digest, ..
            }
            | Self::Application {
                canonical_digest, ..
            }
            | Self::Derivative {
                canonical_digest, ..
            }
            | Self::Integral {
                canonical_digest, ..
            } => canonical_digest,
        }
    }

    pub(crate) fn entities(&self) -> Vec<&EntityId> {
        match self {
            Self::Comparison { left, right, .. } => vec![left, right],
            Self::Sum { result, terms, .. } => std::iter::once(result).chain(terms).collect(),
            Self::Product {
                result, factors, ..
            } => std::iter::once(result).chain(factors).collect(),
            Self::Quotient {
                result,
                numerator,
                denominator,
                ..
            } => vec![result, numerator, denominator],
            Self::Operation {
                result, operands, ..
            } => std::iter::once(result).chain(operands).collect(),
            Self::Application {
                result,
                function,
                arguments,
                ..
            } => std::iter::once(result)
                .chain(std::iter::once(function))
                .chain(arguments)
                .collect(),
            Self::Derivative {
                result,
                operand,
                variable,
                ..
            } => std::iter::once(result)
                .chain(std::iter::once(operand))
                .chain(variable)
                .collect(),
            Self::Integral {
                result,
                integrand,
                variable,
                ..
            } => std::iter::once(result)
                .chain(std::iter::once(integrand))
                .chain(variable)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimOperation {
    Negate,
    Transpose,
    Dot,
    Cross,
    Power,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ClaimValue {
    Concept(String),
    Role(String),
    Type(String),
    Shape(ClaimShape),
    Dimension(Vec<DimensionExponent>),
    Unit(String),
    QuantityKind(String),
    Condition(ClaimCondition),
    Relation(Box<ClaimRelation>),
    Scalar(String),
    Text(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ClaimObject {
    Entity(EntityId),
    Occurrence(SourceOccurrenceId),
    Value(ClaimValue),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub id: ClaimId,
    pub subject: EntityId,
    pub predicate: ClaimPredicate,
    pub object: ClaimObject,
    pub evidence_id: EvidenceId,
    pub tier: InferenceTier,
    pub derivation_depth: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSemanticFacts {
    pub file_id: String,
    pub document_version: u64,
    pub source_utf16_length: u32,
    pub occurrences: Vec<SourceOccurrence>,
    pub entities: Vec<EntityId>,
    pub mentions: Vec<Mention>,
    pub evidence: Vec<EvidenceRecord>,
    pub claims: Vec<Claim>,
    #[serde(default)]
    pub candidates: Vec<SemanticCandidateClaim>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionStatus {
    Established,
    Ambiguous,
    Conflicting,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionCandidate {
    pub entity_id: EntityId,
    pub supporting_claims: Vec<ClaimId>,
    pub rejecting_claims: Vec<ClaimId>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub occurrence_id: SourceOccurrenceId,
    pub status: ResolutionStatus,
    pub candidates: Vec<ResolutionCandidate>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexStats {
    pub occurrences: u32,
    pub entities: u32,
    pub mentions: u32,
    pub claims: u32,
    pub evidence: u32,
    pub dependency_edges: u32,
    pub invalidated_claims: u32,
    pub candidates: u32,
    pub constraint_work: u32,
    pub derived_claims: u32,
    pub constraint_truncated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectSemanticIndex {
    document_versions: BTreeMap<String, u64>,
    occurrences: BTreeMap<SourceOccurrenceId, SourceOccurrence>,
    entities: BTreeSet<EntityId>,
    mentions: BTreeMap<SourceOccurrenceId, Mention>,
    evidence: BTreeMap<EvidenceId, EvidenceRecord>,
    claims: BTreeMap<ClaimId, Claim>,
    candidates: BTreeMap<SourceOccurrenceId, Vec<SemanticCandidateClaim>>,
    dependents: BTreeMap<ClaimId, BTreeSet<ClaimId>>,
    claims_by_file_dependency: BTreeMap<String, BTreeSet<ClaimId>>,
    binding_claims: BTreeMap<String, BTreeSet<ClaimId>>,
    occurrences_by_binding: BTreeMap<String, BTreeSet<SourceOccurrenceId>>,
    occurrences_by_selection: OccurrencesBySelection,
    claims_by_entity: BTreeMap<EntityId, BTreeSet<ClaimId>>,
    established_occurrences_by_entity: BTreeMap<EntityId, BTreeSet<SourceOccurrenceId>>,
    invalidated_claims: u32,
    constraint_work: u32,
    constraint_truncated: bool,
    constraint_conflicts: Vec<PlannedConflict>,
}

struct SemanticRollback {
    document_versions: BTreeMap<String, u64>,
    occurrences: BTreeMap<SourceOccurrenceId, SourceOccurrence>,
    entities: BTreeSet<EntityId>,
    mentions: BTreeMap<SourceOccurrenceId, Mention>,
    evidence: BTreeMap<EvidenceId, EvidenceRecord>,
    claims: BTreeMap<ClaimId, Claim>,
    candidates: BTreeMap<SourceOccurrenceId, Vec<SemanticCandidateClaim>>,
    invalidated_claims: u32,
    constraint_work: u32,
    constraint_truncated: bool,
    constraint_conflicts: Vec<PlannedConflict>,
}

impl ProjectSemanticIndex {
    pub fn replace_document(&mut self, facts: DocumentSemanticFacts) -> Result<(), String> {
        self.replace_documents(vec![facts])
    }

    pub fn replace_documents(
        &mut self,
        documents: Vec<DocumentSemanticFacts>,
    ) -> Result<(), String> {
        let affected_files = documents
            .iter()
            .map(|facts| facts.file_id.clone())
            .collect::<BTreeSet<_>>();
        let mut affected_bindings = self.binding_keys_depending_on(&affected_files);
        let mut rollback = Some(self.snapshot_affected(&affected_files));
        for facts in documents {
            self.retract_document(&facts.file_id);
            if let Err(error) = self.validate_and_insert(facts) {
                self.restore_affected(
                    &affected_files,
                    rollback.take().expect("rollback state is available"),
                );
                return Err(error);
            }
        }
        if let Err(error) = self.recompute_constraints(&affected_files) {
            self.restore_affected(
                &affected_files,
                rollback.take().expect("rollback state is available"),
            );
            return Err(error);
        }
        affected_bindings.extend(self.binding_keys_depending_on(&affected_files));
        self.refresh_resolution_index(&affected_bindings);
        Ok(())
    }

    pub fn remove_document(&mut self, file_id: &str) {
        let affected_files = BTreeSet::from([file_id.to_owned()]);
        let affected_bindings = self.binding_keys_depending_on(&affected_files);
        self.retract_document(file_id);
        self.recompute_constraints(&affected_files)
            .expect("existing semantic facts must produce valid constraints");
        self.refresh_resolution_index(&affected_bindings);
    }

    pub fn resolve(&self, occurrence_id: &SourceOccurrenceId) -> Resolution {
        let Some(occurrence) = self.occurrences.get(occurrence_id) else {
            return unsupported(occurrence_id.clone());
        };
        if !self.mentions.contains_key(occurrence_id) {
            return unsupported(occurrence_id.clone());
        }
        let normalized = occurrence_binding_key(occurrence);
        let mut by_entity = BTreeMap::<EntityId, ResolutionCandidate>::new();
        let mut visible = self
            .binding_claims
            .get(&normalized)
            .into_iter()
            .flatten()
            .filter_map(|claim_id| {
                let claim = &self.claims[claim_id];
                let evidence = &self.evidence[&claim.evidence_id];
                if !scope_visible(&evidence.scope_path, &occurrence.scope_path)
                    || (evidence.available_after > occurrence.availability_order
                        && claim.subject.anchor != occurrence.id)
                    || evidence.modality != EvidenceModality::Asserted
                    || claim.subject.component_id != occurrence.component_id
                {
                    return None;
                }
                Some((claim, evidence))
            })
            .collect::<Vec<_>>();
        let local_scope = visible
            .iter()
            .filter(|(_, evidence)| evidence.source.file_id == occurrence.id.file_id)
            .map(|(_, evidence)| evidence.scope_path.len())
            .max();
        if let Some(scope_depth) = local_scope {
            let latest = visible
                .iter()
                .filter(|(_, evidence)| {
                    evidence.source.file_id == occurrence.id.file_id
                        && evidence.scope_path.len() == scope_depth
                })
                .map(|(_, evidence)| evidence.available_after)
                .max()
                .expect("local binding has an availability order");
            visible.retain(|(_, evidence)| {
                evidence.source.file_id == occurrence.id.file_id
                    && evidence.scope_path.len() == scope_depth
                    && evidence.available_after == latest
            });
        }
        for (claim, evidence) in visible {
            let candidate =
                by_entity
                    .entry(claim.subject.clone())
                    .or_insert_with(|| ResolutionCandidate {
                        entity_id: claim.subject.clone(),
                        supporting_claims: Vec::new(),
                        rejecting_claims: Vec::new(),
                    });
            match evidence.polarity {
                EvidencePolarity::Positive => candidate.supporting_claims.push(claim.id.clone()),
                EvidencePolarity::Negative => candidate.rejecting_claims.push(claim.id.clone()),
            }
        }
        let mut candidates = by_entity
            .into_values()
            .filter(|candidate| {
                !candidate.supporting_claims.is_empty() || !candidate.rejecting_claims.is_empty()
            })
            .collect::<Vec<_>>();
        for candidate in &mut candidates {
            candidate.supporting_claims.sort();
            candidate.supporting_claims.dedup();
            candidate.rejecting_claims.sort();
            candidate.rejecting_claims.dedup();
        }
        let truncated = candidates.len() > MAX_RESOLUTION_CANDIDATES;
        candidates.truncate(MAX_RESOLUTION_CANDIDATES);
        let positive = candidates
            .iter()
            .filter(|candidate| !candidate.supporting_claims.is_empty())
            .count();
        let has_conflict = candidates.iter().any(|candidate| {
            !candidate.supporting_claims.is_empty() && !candidate.rejecting_claims.is_empty()
        });
        let status = if candidates.is_empty() {
            ResolutionStatus::Unsupported
        } else if has_conflict {
            ResolutionStatus::Conflicting
        } else if positive == 1 {
            ResolutionStatus::Established
        } else if positive > 1 {
            ResolutionStatus::Ambiguous
        } else {
            ResolutionStatus::Unsupported
        };
        Resolution {
            occurrence_id: occurrence_id.clone(),
            status,
            candidates,
            truncated,
        }
    }

    pub(crate) fn has_future_external_binding_evidence(
        &self,
        occurrence_id: &SourceOccurrenceId,
    ) -> bool {
        let Some(occurrence) = self.occurrences.get(occurrence_id) else {
            return false;
        };
        let normalized = occurrence_binding_key(occurrence);
        self.binding_claims
            .get(&normalized)
            .into_iter()
            .flatten()
            .filter_map(|claim_id| {
                let claim = self.claims.get(claim_id)?;
                let evidence = self.evidence.get(&claim.evidence_id)?;
                (claim.subject.anchor != occurrence.id).then_some((claim, evidence))
            })
            .any(|(claim, evidence)| {
                claim.subject.component_id == occurrence.component_id
                    && evidence.source.file_id != occurrence.id.file_id
                    && scope_visible(&evidence.scope_path, &occurrence.scope_path)
                    && evidence.available_after > occurrence.availability_order
                    && evidence.modality == EvidenceModality::Asserted
                    && evidence.polarity == EvidencePolarity::Positive
            })
    }

    pub fn stats(&self) -> SemanticIndexStats {
        SemanticIndexStats {
            occurrences: self.occurrences.len() as u32,
            entities: self.entities.len() as u32,
            mentions: self.mentions.len() as u32,
            claims: self.claims.len() as u32,
            evidence: self.evidence.len() as u32,
            dependency_edges: self.dependents.values().map(BTreeSet::len).sum::<usize>() as u32,
            invalidated_claims: self.invalidated_claims,
            candidates: self.candidates.values().map(Vec::len).sum::<usize>() as u32,
            constraint_work: self.constraint_work,
            derived_claims: self
                .evidence
                .values()
                .filter(|evidence| evidence.rule_id.starts_with("semath/constraint/"))
                .count() as u32,
            constraint_truncated: self.constraint_truncated,
        }
    }

    pub fn occurrence(&self, id: &SourceOccurrenceId) -> Option<&SourceOccurrence> {
        self.occurrences.get(id)
    }

    pub fn occurrences(&self) -> impl Iterator<Item = &SourceOccurrence> {
        self.occurrences.values()
    }

    pub(crate) fn occurrences_for_file<'a>(
        &'a self,
        file_id: &str,
    ) -> impl Iterator<Item = &'a SourceOccurrence> + 'a {
        let first = SourceOccurrenceId {
            file_id: file_id.to_owned(),
            document_version: 0,
            local_id: 0,
        };
        let last = SourceOccurrenceId {
            file_id: file_id.to_owned(),
            document_version: u64::MAX,
            local_id: u32::MAX,
        };
        self.occurrences
            .range(first..=last)
            .map(|(_, occurrence)| occurrence)
    }

    pub(crate) fn entity_decision(
        &self,
        occurrence_id: &SourceOccurrenceId,
    ) -> EntityEvidenceDecision {
        decide_entity(&self.resolve(occurrence_id))
    }

    #[cfg(test)]
    pub(crate) fn established_occurrences_for_entity(
        &self,
        entity: &EntityId,
    ) -> Vec<&SourceOccurrence> {
        self.established_occurrences_by_entity
            .get(entity)
            .into_iter()
            .flatten()
            .filter_map(|occurrence_id| self.occurrences.get(occurrence_id))
            .collect()
    }

    pub(crate) fn bounded_established_occurrences_for_entity(
        &self,
        entity: &EntityId,
    ) -> Result<Vec<SourceOccurrence>, ()> {
        let mut occurrences = self
            .established_occurrences_by_entity
            .get(entity)
            .into_iter()
            .flatten()
            .take(MAX_ENTITY_SURFACE_OCCURRENCES + 1)
            .filter_map(|occurrence_id| self.occurrences.get(occurrence_id))
            .cloned()
            .collect::<Vec<_>>();
        if occurrences.len() > MAX_ENTITY_SURFACE_OCCURRENCES {
            return Err(());
        }
        occurrences.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(occurrences)
    }

    pub(crate) fn established_selection_would_merge(
        &self,
        target: &EntityId,
        replacement: &str,
        target_occurrences: &[SourceOccurrence],
    ) -> Result<bool, ()> {
        let mut visited = 0usize;
        let target_ids = target_occurrences
            .iter()
            .map(|occurrence| occurrence.id.clone())
            .collect::<BTreeSet<_>>();
        for (candidate_scope, occurrences) in self
            .occurrences_by_selection
            .get(&(target.component_id.clone(), replacement.to_owned()))
            .into_iter()
            .flatten()
        {
            visited += occurrences.len();
            if visited > MAX_ENTITY_SURFACE_OCCURRENCES {
                return Err(());
            }
            if occurrences
                .iter()
                .all(|occurrence_id| target_ids.contains(occurrence_id))
            {
                continue;
            }
            if target_occurrences.iter().any(|occurrence| {
                occurrence.component_id == target.component_id
                    && (scope_visible(candidate_scope, &occurrence.scope_path)
                        || scope_visible(&occurrence.scope_path, candidate_scope))
            }) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn claim(&self, id: &ClaimId) -> Option<&Claim> {
        self.claims.get(id)
    }

    pub fn evidence(&self, id: &EvidenceId) -> Option<&EvidenceRecord> {
        self.evidence.get(id)
    }

    pub(crate) fn relation_is_retracted(
        &self,
        canonical_digest: &str,
        target_occurrence: &SourceOccurrenceId,
    ) -> bool {
        let Some(target) = self.occurrences.get(target_occurrence) else {
            return false;
        };
        let mut positive = Vec::new();
        let mut negative = Vec::new();
        for claim in self.claims.values() {
            let ClaimObject::Value(ClaimValue::Relation(relation)) = &claim.object else {
                continue;
            };
            if claim.predicate != ClaimPredicate::Relates
                || claim.tier != InferenceTier::ExplicitClaim
                || relation.canonical_digest() != canonical_digest
            {
                continue;
            }
            let Some(evidence) = self.evidence.get(&claim.evidence_id) else {
                continue;
            };
            let Some(source) = self.occurrences.get(&evidence.source) else {
                continue;
            };
            if evidence.modality != EvidenceModality::Asserted {
                continue;
            }
            if evidence.rule_id != "semath/canonical-relation" {
                continue;
            }
            if source.component_id != target.component_id
                || !scope_visible(&evidence.scope_path, &target.scope_path)
            {
                continue;
            }
            match evidence.polarity {
                EvidencePolarity::Positive => positive.push(evidence.available_after),
                EvidencePolarity::Negative => negative.push(evidence.available_after),
            }
        }
        let Some(latest_negative) = negative.into_iter().max() else {
            return false;
        };
        if positive.iter().any(|order| *order > latest_negative) {
            return false;
        }
        positive
            .into_iter()
            .filter(|order| *order < latest_negative)
            .count()
            == 1
    }

    pub fn claims_for_entity(&self, entity: &EntityId) -> Vec<&Claim> {
        self.claims_by_entity
            .get(entity)
            .into_iter()
            .flatten()
            .filter_map(|claim_id| self.claims.get(claim_id))
            .collect()
    }

    pub fn claims_for_entity_at(
        &self,
        entity: &EntityId,
        occurrence: &SourceOccurrence,
    ) -> Vec<&Claim> {
        self.claims_for_entity(entity)
            .into_iter()
            .filter(|claim| {
                self.evidence
                    .get(&claim.evidence_id)
                    .is_some_and(|evidence| {
                        (evidence.available_after <= occurrence.availability_order
                            || evidence.provenance.contains(&occurrence.id))
                            && scope_visible(&evidence.scope_path, &occurrence.scope_path)
                            && claim.subject.component_id == occurrence.component_id
                    })
            })
            .collect()
    }

    pub(crate) fn occurrence_has_source_meaning(&self, occurrence_id: &SourceOccurrenceId) -> bool {
        let Some(occurrence) = self.occurrences.get(occurrence_id) else {
            return false;
        };
        self.resolve(occurrence_id)
            .candidates
            .into_iter()
            .any(|candidate| {
                self.claims_for_entity_at(&candidate.entity_id, occurrence)
                    .into_iter()
                    .any(|claim| {
                        let Some(evidence) = self.evidence.get(&claim.evidence_id) else {
                            return false;
                        };
                        evidence.polarity == EvidencePolarity::Positive
                            && evidence.modality == EvidenceModality::Asserted
                            && match claim.predicate {
                                ClaimPredicate::Names => {
                                    evidence.rule_id != "semath/canonical-symbol-identity"
                                }
                                ClaimPredicate::Defines
                                | ClaimPredicate::Abbreviates
                                | ClaimPredicate::Aliases
                                | ClaimPredicate::HasRole
                                | ClaimPredicate::HasType
                                | ClaimPredicate::HasShape
                                | ClaimPredicate::HasDimension
                                | ClaimPredicate::HasQuantity
                                | ClaimPredicate::HasUnit => true,
                                ClaimPredicate::Assumes | ClaimPredicate::Relates => false,
                            }
                    })
            })
    }

    pub(crate) fn occurrence_has_explicit_identity(
        &self,
        occurrence_id: &SourceOccurrenceId,
    ) -> bool {
        let Some(occurrence) = self.occurrences.get(occurrence_id) else {
            return false;
        };
        self.binding_claims
            .get(&occurrence_binding_key(occurrence))
            .into_iter()
            .flatten()
            .filter_map(|claim_id| self.claims.get(claim_id))
            .any(|claim| {
                claim.predicate == ClaimPredicate::Names
                    && matches!(&claim.object, ClaimObject::Occurrence(id) if id == occurrence_id)
                    && self
                        .evidence
                        .get(&claim.evidence_id)
                        .is_some_and(|evidence| {
                            evidence.origin == EvidenceOrigin::Explicit
                                && evidence.modality == EvidenceModality::Asserted
                                && evidence.polarity == EvidencePolarity::Positive
                        })
            })
    }

    pub(crate) fn constraint_conflicts_for(&self, file_id: &str) -> Vec<&PlannedConflict> {
        self.constraint_conflicts
            .iter()
            .filter(|conflict| conflict.subject.anchor.file_id == file_id)
            .collect()
    }

    pub fn candidates_for(
        &self,
        occurrence_id: &SourceOccurrenceId,
    ) -> Vec<SemanticCandidateClaim> {
        let entities = self
            .resolve(occurrence_id)
            .candidates
            .into_iter()
            .filter(|candidate| !candidate.supporting_claims.is_empty())
            .map(|candidate| candidate.entity_id)
            .collect::<BTreeSet<_>>();
        let mut candidates = self
            .candidates
            .get(occurrence_id)
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        for candidate in &mut candidates {
            for entity in &entities {
                for claim_id in self
                    .claims_by_entity
                    .get(entity)
                    .into_iter()
                    .flatten()
                    .take(MAX_CANDIDATE_EVIDENCE)
                {
                    let claim = &self.claims[claim_id];
                    if !candidate_claim_matches(candidate, claim) {
                        continue;
                    }
                    let evidence = &self.evidence[&claim.evidence_id];
                    if evidence.modality != EvidenceModality::Asserted {
                        continue;
                    }
                    match evidence.polarity {
                        EvidencePolarity::Positive => {
                            candidate.supporting_claims.push(claim.id.clone())
                        }
                        EvidencePolarity::Negative => {
                            candidate.rejecting_claims.push(claim.id.clone())
                        }
                    }
                }
            }
            candidate.supporting_claims.sort();
            candidate.supporting_claims.dedup();
            candidate.rejecting_claims.sort();
            candidate.rejecting_claims.dedup();
        }
        candidates.sort_by_key(|candidate| {
            (
                candidate.supporting_claims.is_empty(),
                !candidate.rejecting_claims.is_empty(),
                candidate.family,
                candidate.interpretation.clone(),
            )
        });
        candidates
    }

    fn validate_and_insert(&mut self, facts: DocumentSemanticFacts) -> Result<(), String> {
        if facts.occurrences.len() > MAX_DOCUMENT_OCCURRENCES {
            return Err("document occurrence cap exceeded".to_owned());
        }
        if facts.claims.len() > MAX_DOCUMENT_CLAIMS {
            return Err("document claim cap exceeded".to_owned());
        }
        if facts.candidates.len() > MAX_DOCUMENT_CANDIDATES {
            return Err("document candidate cap exceeded".to_owned());
        }
        let mut occurrence_ids = BTreeSet::new();
        for occurrence in &facts.occurrences {
            if occurrence.id.file_id != facts.file_id
                || occurrence.id.document_version != facts.document_version
            {
                return Err("occurrence identity does not match document revision".to_owned());
            }
            if occurrence.range.start_offset >= occurrence.range.end_offset
                || occurrence.range.end_offset > facts.source_utf16_length
            {
                return Err("occurrence range is not a real non-empty source span".to_owned());
            }
            if !occurrence_ids.insert(occurrence.id.clone()) {
                return Err("duplicate source occurrence identity".to_owned());
            }
        }
        let mut new_entities = BTreeSet::new();
        for entity in &facts.entities {
            if !new_entities.insert(entity.clone()) {
                return Err(format!("duplicate entity identity: {entity:?}"));
            }
        }
        for entity in &facts.entities {
            if entity.anchor.file_id != facts.file_id {
                return Err("document cannot re-own an entity anchored in another file".to_owned());
            }
            if !self.occurrences.contains_key(&entity.anchor)
                && !occurrence_ids.contains(&entity.anchor)
            {
                return Err("entity anchor is not a source occurrence".to_owned());
            }
            let anchor = self
                .occurrences
                .get(&entity.anchor)
                .or_else(|| {
                    facts
                        .occurrences
                        .iter()
                        .find(|item| item.id == entity.anchor)
                })
                .expect("known entity anchor");
            if entity.component_id != anchor.component_id || entity.scope_path != anchor.scope_path
            {
                return Err("entity component or scope differs from its anchor".to_owned());
            }
            if entity.component_id.trim().is_empty() || entity.kind.trim().is_empty() {
                return Err("entity identity has an empty component or kind".to_owned());
            }
        }
        let mut mention_ids = BTreeSet::new();
        for mention in &facts.mentions {
            if mention.occurrence_id.file_id != facts.file_id {
                return Err("document cannot re-own a mention in another file".to_owned());
            }
            if !self.occurrences.contains_key(&mention.occurrence_id)
                && !occurrence_ids.contains(&mention.occurrence_id)
            {
                return Err("mention is not source-linked".to_owned());
            }
            if !mention_ids.insert(mention.occurrence_id.clone()) {
                return Err("duplicate mention identity".to_owned());
            }
        }
        let new_evidence = facts
            .evidence
            .iter()
            .map(|item| (item.id.clone(), item))
            .collect::<BTreeMap<_, _>>();
        if new_evidence.len() != facts.evidence.len() {
            return Err("duplicate evidence identity".to_owned());
        }
        if new_evidence.keys().any(|id| self.evidence.contains_key(id)) {
            return Err("evidence identity is already owned by another document".to_owned());
        }
        let new_claims = facts
            .claims
            .iter()
            .map(|item| (item.id.clone(), item))
            .collect::<BTreeMap<_, _>>();
        if new_claims.len() != facts.claims.len() {
            return Err("duplicate claim identity".to_owned());
        }
        if new_claims.keys().any(|id| self.claims.contains_key(id)) {
            return Err("claim identity is already owned by another document".to_owned());
        }
        for evidence in &facts.evidence {
            if evidence.source.file_id != facts.file_id {
                return Err("document cannot re-own evidence from another file".to_owned());
            }
            if (!self.occurrences.contains_key(&evidence.source)
                && !occurrence_ids.contains(&evidence.source))
                || evidence.provenance.iter().any(|source| {
                    !self.occurrences.contains_key(source) && !occurrence_ids.contains(source)
                })
            {
                return Err("evidence provenance is not source-linked".to_owned());
            }
            let source = self
                .occurrences
                .get(&evidence.source)
                .or_else(|| {
                    facts
                        .occurrences
                        .iter()
                        .find(|item| item.id == evidence.source)
                })
                .expect("known evidence source");
            if evidence.scope_path != source.scope_path
                || evidence.available_after < source.availability_order
            {
                return Err(format!(
                    "evidence {} scope or availability precedes source {:?}: evidence {:?}/{} source {:?}/{}",
                    evidence.id.0,
                    evidence.source,
                    evidence.scope_path,
                    evidence.available_after,
                    source.scope_path,
                    source.availability_order
                ));
            }
            if evidence.rule_id.trim().is_empty() || evidence.rule_version == 0 {
                return Err("evidence extraction rule must be versioned".to_owned());
            }
            if evidence.origin == EvidenceOrigin::Explicit && !evidence.parent_claims.is_empty() {
                return Err("explicit evidence cannot depend on derived claims".to_owned());
            }
        }
        for claim in &facts.claims {
            if !self.entities.contains(&claim.subject) && !new_entities.contains(&claim.subject) {
                return Err("claim subject is not a known entity".to_owned());
            }
            if claim.derivation_depth > MAX_DERIVATION_DEPTH {
                return Err("claim derivation depth cap exceeded".to_owned());
            }
            let evidence = new_evidence
                .get(&claim.evidence_id)
                .copied()
                .or_else(|| self.evidence.get(&claim.evidence_id))
                .ok_or_else(|| "claim evidence is missing".to_owned())?;
            if let ClaimObject::Occurrence(occurrence) = &claim.object
                && !self.occurrences.contains_key(occurrence)
                && !occurrence_ids.contains(occurrence)
            {
                return Err("claim object is not a source occurrence".to_owned());
            }
            if let ClaimObject::Entity(entity) = &claim.object
                && !self.entities.contains(entity)
                && !new_entities.contains(entity)
            {
                return Err("claim object is not a known entity".to_owned());
            }
            if let ClaimObject::Value(ClaimValue::Relation(relation)) = &claim.object
                && relation.entities().iter().any(|entity| {
                    !self.entities.contains(*entity) && !new_entities.contains(*entity)
                })
            {
                return Err("relation references an unknown entity".to_owned());
            }
            validate_claim_object(claim)?;
            if evidence.origin == EvidenceOrigin::Explicit
                && claim.tier != InferenceTier::ExplicitClaim
            {
                return Err("explicit claim must use the explicit-claim tier".to_owned());
            }
            if evidence.origin == EvidenceOrigin::Derived {
                if evidence.parent_claims.is_empty()
                    || claim.derivation_depth == 0
                    || claim.tier <= InferenceTier::ExplicitClaim
                {
                    return Err("derived claim must name prior parents and depth".to_owned());
                }
                for parent_id in &evidence.parent_claims {
                    let parent = new_claims
                        .get(parent_id)
                        .copied()
                        .or_else(|| self.claims.get(parent_id))
                        .ok_or_else(|| "derived claim parent is missing".to_owned())?;
                    if parent.id == claim.id
                        || parent.tier >= claim.tier
                        || parent.derivation_depth >= claim.derivation_depth
                    {
                        return Err(
                            "derived claim dependency is cyclic or tier-reversing".to_owned()
                        );
                    }
                }
            }
        }
        let mut candidate_ids = BTreeSet::new();
        for candidate in &facts.candidates {
            if !candidate_ids.insert(candidate.id.clone()) {
                return Err("duplicate semantic candidate identity".to_owned());
            }
            let occurrence = self.occurrences.get(&candidate.occurrence_id).or_else(|| {
                facts
                    .occurrences
                    .iter()
                    .find(|item| item.id == candidate.occurrence_id)
            });
            if candidate.occurrence_id.file_id != facts.file_id
                || occurrence.is_none()
                || candidate.range.start_offset >= candidate.range.end_offset
                || candidate.range.end_offset > facts.source_utf16_length
                || occurrence.is_some_and(|source| {
                    candidate.range.start_offset < source.range.start_offset
                        || candidate.range.end_offset > source.range.end_offset
                })
                || candidate.interpretation.trim().is_empty()
                || candidate
                    .supporting_claims
                    .iter()
                    .chain(&candidate.rejecting_claims)
                    .any(|claim| {
                        !self.claims.contains_key(claim) && !new_claims.contains_key(claim)
                    })
            {
                return Err("semantic candidate has an invalid source or claim".to_owned());
            }
        }
        self.document_versions
            .insert(facts.file_id, facts.document_version);
        for occurrence in facts.occurrences {
            self.occurrences_by_binding
                .entry(occurrence_binding_key(&occurrence))
                .or_default()
                .insert(occurrence.id.clone());
            self.index_occurrence_selection(&occurrence);
            self.occurrences.insert(occurrence.id.clone(), occurrence);
        }
        self.entities.extend(facts.entities);
        self.mentions.extend(
            facts
                .mentions
                .into_iter()
                .map(|item| (item.occurrence_id.clone(), item)),
        );
        self.evidence.extend(
            facts
                .evidence
                .into_iter()
                .map(|item| (item.id.clone(), item)),
        );
        let inserted_claim_ids = facts
            .claims
            .iter()
            .map(|claim| claim.id.clone())
            .collect::<Vec<_>>();
        self.claims
            .extend(facts.claims.into_iter().map(|item| (item.id.clone(), item)));
        for claim_id in inserted_claim_ids {
            self.index_claim(&claim_id);
        }
        for candidate in facts.candidates {
            self.candidates
                .entry(candidate.occurrence_id.clone())
                .or_default()
                .push(candidate);
        }
        Ok(())
    }

    fn retract_document(&mut self, file_id: &str) {
        self.document_versions.remove(file_id);
        let mut queue = self
            .claims_by_file_dependency
            .get(file_id)
            .into_iter()
            .flatten()
            .cloned()
            .collect::<VecDeque<_>>();
        let mut removed = BTreeSet::new();
        while let Some(claim_id) = queue.pop_front() {
            if !removed.insert(claim_id.clone()) {
                continue;
            }
            queue.extend(
                self.dependents
                    .get(&claim_id)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        self.invalidated_claims = self.invalidated_claims.saturating_add(removed.len() as u32);
        for claim_id in &removed {
            self.unindex_claim(claim_id);
        }
        self.claims.retain(|id, _| !removed.contains(id));
        self.candidates
            .retain(|occurrence, _| occurrence.file_id != file_id);
        self.evidence.retain(|_, evidence| {
            evidence.source.file_id != file_id
                && evidence
                    .provenance
                    .iter()
                    .all(|source| source.file_id != file_id)
        });
        self.mentions.retain(|id, _| id.file_id != file_id);
        self.entities
            .retain(|entity| entity.anchor.file_id != file_id);
        let occurrence_ids = self
            .occurrences
            .keys()
            .filter(|id| id.file_id == file_id)
            .cloned()
            .collect::<Vec<_>>();
        for occurrence_id in occurrence_ids {
            if let Some(occurrence) = self.occurrences.remove(&occurrence_id) {
                remove_occurrence_index_value(
                    &mut self.occurrences_by_binding,
                    &occurrence_binding_key(&occurrence),
                    &occurrence_id,
                );
                remove_occurrence_selection_index(&mut self.occurrences_by_selection, &occurrence);
            }
        }
    }

    fn snapshot_affected(&self, affected_files: &BTreeSet<String>) -> SemanticRollback {
        let mut queue = self
            .claim_ids_depending_on(affected_files)
            .into_iter()
            .collect::<VecDeque<_>>();
        let mut claim_ids = BTreeSet::new();
        while let Some(claim_id) = queue.pop_front() {
            if !claim_ids.insert(claim_id.clone()) {
                continue;
            }
            queue.extend(
                self.dependents
                    .get(&claim_id)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        SemanticRollback {
            document_versions: self
                .document_versions
                .iter()
                .filter(|(file_id, _)| affected_files.contains(*file_id))
                .map(|(file_id, version)| (file_id.clone(), *version))
                .collect(),
            occurrences: self
                .occurrences
                .iter()
                .filter(|(id, _)| affected_files.contains(&id.file_id))
                .map(|(id, occurrence)| (id.clone(), occurrence.clone()))
                .collect(),
            entities: self
                .entities
                .iter()
                .filter(|entity| affected_files.contains(&entity.anchor.file_id))
                .cloned()
                .collect(),
            mentions: self
                .mentions
                .iter()
                .filter(|(id, _)| affected_files.contains(&id.file_id))
                .map(|(id, mention)| (id.clone(), mention.clone()))
                .collect(),
            evidence: self
                .evidence
                .iter()
                .filter(|(_, evidence)| {
                    affected_files.contains(&evidence.source.file_id)
                        || evidence
                            .provenance
                            .iter()
                            .any(|source| affected_files.contains(&source.file_id))
                })
                .map(|(id, evidence)| (id.clone(), evidence.clone()))
                .collect(),
            claims: claim_ids
                .into_iter()
                .filter_map(|id| self.claims.get(&id).cloned().map(|claim| (id, claim)))
                .collect(),
            candidates: self
                .candidates
                .iter()
                .filter(|(id, _)| affected_files.contains(&id.file_id))
                .map(|(id, candidates)| (id.clone(), candidates.clone()))
                .collect(),
            invalidated_claims: self.invalidated_claims,
            constraint_work: self.constraint_work,
            constraint_truncated: self.constraint_truncated,
            constraint_conflicts: self.constraint_conflicts.clone(),
        }
    }

    fn restore_affected(&mut self, affected_files: &BTreeSet<String>, rollback: SemanticRollback) {
        for file_id in affected_files {
            self.retract_document(file_id);
        }
        self.document_versions.extend(rollback.document_versions);
        self.occurrences.extend(rollback.occurrences);
        self.entities.extend(rollback.entities);
        self.mentions.extend(rollback.mentions);
        self.evidence.extend(rollback.evidence);
        self.claims.extend(rollback.claims);
        self.candidates.extend(rollback.candidates);
        self.invalidated_claims = rollback.invalidated_claims;
        self.constraint_work = rollback.constraint_work;
        self.constraint_truncated = rollback.constraint_truncated;
        self.constraint_conflicts = rollback.constraint_conflicts;
        self.rebuild_indexes();
    }

    fn rebuild_indexes(&mut self) {
        self.dependents.clear();
        self.claims_by_file_dependency.clear();
        self.binding_claims.clear();
        self.occurrences_by_binding.clear();
        self.occurrences_by_selection.clear();
        self.claims_by_entity.clear();
        for occurrence in self.occurrences.values() {
            self.occurrences_by_binding
                .entry(occurrence_binding_key(occurrence))
                .or_default()
                .insert(occurrence.id.clone());
        }
        let occurrences = self.occurrences.values().cloned().collect::<Vec<_>>();
        for occurrence in &occurrences {
            self.index_occurrence_selection(occurrence);
        }
        let claim_ids = self.claims.keys().cloned().collect::<Vec<_>>();
        for claim_id in claim_ids {
            self.index_claim(&claim_id);
        }
        self.rebuild_resolution_index();
    }

    fn rebuild_resolution_index(&mut self) {
        self.established_occurrences_by_entity.clear();
        let resolved = self
            .occurrences
            .keys()
            .filter_map(|occurrence_id| match self.entity_decision(occurrence_id) {
                EntityEvidenceDecision::Established(entity) => {
                    Some((entity, occurrence_id.clone()))
                }
                EntityEvidenceDecision::Ambiguous
                | EntityEvidenceDecision::Conflicting
                | EntityEvidenceDecision::Unsupported
                | EntityEvidenceDecision::EngineLimited => None,
            })
            .collect::<Vec<_>>();
        for (entity, occurrence_id) in resolved {
            self.established_occurrences_by_entity
                .entry(entity.clone())
                .or_default()
                .insert(occurrence_id.clone());
        }
    }

    fn binding_keys_depending_on(&self, affected_files: &BTreeSet<String>) -> BTreeSet<String> {
        let mut keys = self
            .occurrences
            .values()
            .filter(|occurrence| affected_files.contains(&occurrence.id.file_id))
            .map(occurrence_binding_key)
            .collect::<BTreeSet<_>>();
        keys.extend(
            self.claim_ids_depending_on(affected_files)
                .iter()
                .filter_map(|claim_id| self.claims.get(claim_id))
                .filter_map(|claim| binding_key_for_claim(claim, &self.occurrences)),
        );
        keys
    }

    fn claim_ids_depending_on(&self, affected_files: &BTreeSet<String>) -> BTreeSet<ClaimId> {
        affected_files
            .iter()
            .flat_map(|file_id| {
                self.claims_by_file_dependency
                    .get(file_id)
                    .into_iter()
                    .flatten()
            })
            .cloned()
            .collect()
    }

    fn refresh_resolution_index(&mut self, affected_bindings: &BTreeSet<String>) {
        if affected_bindings.is_empty() {
            return;
        }
        let affected_occurrences = affected_bindings
            .iter()
            .flat_map(|binding| {
                self.occurrences_by_binding
                    .get(binding)
                    .into_iter()
                    .flatten()
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        self.established_occurrences_by_entity
            .retain(|_, occurrences| {
                occurrences.retain(|occurrence| {
                    self.occurrences.contains_key(occurrence)
                        && !affected_occurrences.contains(occurrence)
                });
                !occurrences.is_empty()
            });
        let resolved = affected_occurrences
            .iter()
            .filter_map(|occurrence_id| match self.entity_decision(occurrence_id) {
                EntityEvidenceDecision::Established(entity) => {
                    Some((entity, occurrence_id.clone()))
                }
                EntityEvidenceDecision::Ambiguous
                | EntityEvidenceDecision::Conflicting
                | EntityEvidenceDecision::Unsupported
                | EntityEvidenceDecision::EngineLimited => None,
            })
            .collect::<Vec<_>>();
        for (entity, occurrence_id) in resolved {
            self.established_occurrences_by_entity
                .entry(entity)
                .or_default()
                .insert(occurrence_id);
        }
    }

    fn index_occurrence_selection(&mut self, occurrence: &SourceOccurrence) {
        if occurrence.kind != OccurrenceKind::Notation {
            return;
        }
        self.occurrences_by_selection
            .entry((
                occurrence.component_id.clone(),
                occurrence.selection_text.clone(),
            ))
            .or_default()
            .entry(occurrence.scope_path.clone())
            .or_default()
            .insert(occurrence.id.clone());
    }

    fn index_claim(&mut self, claim_id: &ClaimId) {
        let Some(claim) = self.claims.get(claim_id) else {
            return;
        };
        let subject = claim.subject.clone();
        let parents = self
            .evidence
            .get(&claim.evidence_id)
            .map_or_else(Vec::new, |evidence| evidence.parent_claims.clone());
        let dependency_files = claim_dependency_files(claim, self.evidence.get(&claim.evidence_id));
        let binding = if matches!(
            claim.predicate,
            ClaimPredicate::Defines
                | ClaimPredicate::Names
                | ClaimPredicate::Abbreviates
                | ClaimPredicate::Aliases
        ) && let ClaimObject::Occurrence(occurrence_id) = &claim.object
        {
            self.occurrences
                .get(occurrence_id)
                .map(occurrence_binding_key)
        } else {
            None
        };

        self.claims_by_entity
            .entry(subject)
            .or_default()
            .insert(claim_id.clone());
        for parent in parents {
            self.dependents
                .entry(parent)
                .or_default()
                .insert(claim_id.clone());
        }
        for file_id in dependency_files {
            self.claims_by_file_dependency
                .entry(file_id)
                .or_default()
                .insert(claim_id.clone());
        }
        if let Some(binding) = binding {
            self.binding_claims
                .entry(binding)
                .or_default()
                .insert(claim_id.clone());
        }
    }

    fn unindex_claim(&mut self, claim_id: &ClaimId) {
        let Some(claim) = self.claims.get(claim_id) else {
            return;
        };
        let subject = claim.subject.clone();
        let parents = self
            .evidence
            .get(&claim.evidence_id)
            .map_or_else(Vec::new, |evidence| evidence.parent_claims.clone());
        let dependency_files = claim_dependency_files(claim, self.evidence.get(&claim.evidence_id));
        let binding = if matches!(
            claim.predicate,
            ClaimPredicate::Defines
                | ClaimPredicate::Names
                | ClaimPredicate::Abbreviates
                | ClaimPredicate::Aliases
        ) && let ClaimObject::Occurrence(occurrence_id) = &claim.object
        {
            self.occurrences
                .get(occurrence_id)
                .map(occurrence_binding_key)
        } else {
            None
        };

        remove_index_value(&mut self.claims_by_entity, &subject, claim_id);
        for parent in parents {
            remove_index_value(&mut self.dependents, &parent, claim_id);
        }
        for file_id in dependency_files {
            remove_index_value(&mut self.claims_by_file_dependency, &file_id, claim_id);
        }
        self.dependents.remove(claim_id);
        if let Some(binding) = binding {
            remove_index_value(&mut self.binding_claims, &binding, claim_id);
        }
    }

    fn recompute_constraints(&mut self, affected_files: &BTreeSet<String>) -> Result<(), String> {
        let generated_evidence = self
            .evidence
            .iter()
            .filter(|(_, evidence)| {
                evidence.rule_id.starts_with("semath/constraint/")
                    && affected_files.contains(&evidence.source.file_id)
            })
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let generated_claims = self
            .claims
            .values()
            .filter(|claim| generated_evidence.contains(&claim.evidence_id))
            .map(|claim| claim.id.clone())
            .collect::<Vec<_>>();
        for claim_id in &generated_claims {
            self.unindex_claim(claim_id);
        }
        self.claims
            .retain(|_, claim| !generated_evidence.contains(&claim.evidence_id));
        self.evidence
            .retain(|id, _| !generated_evidence.contains(id));

        let input = self
            .claims
            .values()
            .filter(|claim| affected_files.contains(&claim.subject.anchor.file_id))
            .filter_map(|claim| {
                Some(ConstraintInputClaim {
                    binding_key: self
                        .occurrences
                        .get(&claim.subject.anchor)
                        .map(occurrence_binding_key),
                    claim: claim.clone(),
                    evidence: self.evidence.get(&claim.evidence_id)?.clone(),
                })
            })
            .collect::<Vec<_>>();
        let plan = plan_constraint_derivations(&input);
        self.constraint_work = plan.work_items;
        self.constraint_truncated = plan.truncated;
        self.constraint_conflicts
            .retain(|conflict| !affected_files.contains(&conflict.subject.anchor.file_id));
        self.constraint_conflicts.extend(plan.conflicts);
        self.constraint_conflicts.sort();
        self.constraint_conflicts.dedup();
        for derivation in plan.derivations {
            let digest = constraint_digest(
                &derivation.subject,
                &derivation.predicate,
                &derivation.value,
            );
            let evidence_id = EvidenceId(format!("semath:constraint:evidence:{digest}"));
            let claim_id = ClaimId(format!("semath:constraint:claim:{digest}"));
            if self.claims.contains_key(&claim_id) || self.evidence.contains_key(&evidence_id) {
                return Err("stable constraint identity collision".into());
            }
            validate_claim_value(&derivation.value)?;
            if derivation
                .parent_claims
                .iter()
                .any(|parent| !self.claims.contains_key(parent))
            {
                return Err("constraint proof parent is missing".into());
            }
            let evidence = EvidenceRecord {
                id: evidence_id.clone(),
                source: derivation.subject.anchor.clone(),
                scope_path: derivation.subject.scope_path.clone(),
                available_after: derivation.available_after,
                polarity: EvidencePolarity::Positive,
                modality: EvidenceModality::Asserted,
                origin: EvidenceOrigin::Derived,
                provenance: derivation.provenance,
                parent_claims: derivation.parent_claims,
                rule_id: derivation.rule_id,
                rule_version: 1,
            };
            let claim = Claim {
                id: claim_id.clone(),
                subject: derivation.subject,
                predicate: derivation.predicate,
                object: ClaimObject::Value(derivation.value),
                evidence_id: evidence_id.clone(),
                tier: InferenceTier::Constraint,
                derivation_depth: 1,
            };
            self.evidence.insert(evidence_id, evidence);
            self.claims.insert(claim_id.clone(), claim);
            self.index_claim(&claim_id);
        }
        Ok(())
    }
}

fn remove_index_value<K: Ord + Clone>(
    index: &mut BTreeMap<K, BTreeSet<ClaimId>>,
    key: &K,
    value: &ClaimId,
) {
    let remove_key = index.get_mut(key).is_some_and(|values| {
        values.remove(value);
        values.is_empty()
    });
    if remove_key {
        index.remove(key);
    }
}

fn remove_occurrence_index_value<K: Ord + Clone>(
    index: &mut BTreeMap<K, BTreeSet<SourceOccurrenceId>>,
    key: &K,
    value: &SourceOccurrenceId,
) {
    let remove_key = index.get_mut(key).is_some_and(|values| {
        values.remove(value);
        values.is_empty()
    });
    if remove_key {
        index.remove(key);
    }
}

fn remove_occurrence_selection_index(
    index: &mut OccurrencesBySelection,
    occurrence: &SourceOccurrence,
) {
    if occurrence.kind != OccurrenceKind::Notation {
        return;
    }
    let key = (
        occurrence.component_id.clone(),
        occurrence.selection_text.clone(),
    );
    let remove_key = index.get_mut(&key).is_some_and(|by_scope| {
        let remove_scope = by_scope
            .get_mut(&occurrence.scope_path)
            .is_some_and(|occurrences| {
                occurrences.remove(&occurrence.id);
                occurrences.is_empty()
            });
        if remove_scope {
            by_scope.remove(&occurrence.scope_path);
        }
        by_scope.is_empty()
    });
    if remove_key {
        index.remove(&key);
    }
}

fn constraint_digest(subject: &EntityId, predicate: &ClaimPredicate, value: &ClaimValue) -> String {
    let identity = format!("{subject:?}|{predicate:?}|{value:?}");
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in identity.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn candidate_claim_matches(candidate: &SemanticCandidateClaim, claim: &Claim) -> bool {
    let ClaimObject::Value(value) = &claim.object else {
        return false;
    };
    let value = match value {
        ClaimValue::Concept(value)
        | ClaimValue::Role(value)
        | ClaimValue::Type(value)
        | ClaimValue::Scalar(value)
        | ClaimValue::Text(value) => value,
        _ => return false,
    };
    let normalized = value.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    if normalized == candidate.interpretation {
        return true;
    }
    match candidate.family {
        CandidateFamily::Application => matches!(
            normalized.as_str(),
            "function" | "operator" | "map" | "application" | "metric"
        ),
        CandidateFamily::Binder => matches!(normalized.as_str(), "binder" | "bound-variable"),
        CandidateFamily::Differential => {
            matches!(
                normalized.as_str(),
                "differential" | "derivative" | "gradient"
            )
        }
        CandidateFamily::Decoration => normalized == candidate.interpretation,
        CandidateFamily::Style => normalized == candidate.interpretation,
        CandidateFamily::Script => normalized == candidate.interpretation,
        CandidateFamily::Bracketed | CandidateFamily::Juxtaposition | CandidateFamily::Operator => {
            normalized == candidate.interpretation
        }
    }
}

fn validate_claim_object(claim: &Claim) -> Result<(), String> {
    let valid = matches!(
        (&claim.predicate, &claim.object),
        (
            ClaimPredicate::Defines
                | ClaimPredicate::Names
                | ClaimPredicate::Abbreviates
                | ClaimPredicate::Aliases,
            ClaimObject::Occurrence(_) | ClaimObject::Entity(_),
        ) | (
            ClaimPredicate::HasRole,
            ClaimObject::Value(ClaimValue::Role(_))
        ) | (
            ClaimPredicate::HasRole,
            ClaimObject::Value(ClaimValue::Concept(_))
        ) | (
            ClaimPredicate::HasType,
            ClaimObject::Value(ClaimValue::Type(_))
        ) | (
            ClaimPredicate::HasShape,
            ClaimObject::Value(ClaimValue::Shape(_))
        ) | (
            ClaimPredicate::HasDimension,
            ClaimObject::Value(ClaimValue::Dimension(_))
        ) | (
            ClaimPredicate::HasQuantity,
            ClaimObject::Value(ClaimValue::QuantityKind(_))
        ) | (
            ClaimPredicate::HasUnit,
            ClaimObject::Value(ClaimValue::Unit(_))
        ) | (
            ClaimPredicate::Assumes,
            ClaimObject::Value(ClaimValue::Condition(_))
        ) | (
            ClaimPredicate::Relates,
            ClaimObject::Value(ClaimValue::Relation(_))
        ) | (ClaimPredicate::Relates, ClaimObject::Entity(_))
    );
    if !valid {
        return Err("claim predicate and typed object are incompatible".to_owned());
    }
    if let ClaimObject::Value(value) = &claim.object {
        validate_claim_value(value)?;
    }
    Ok(())
}

fn validate_claim_value(value: &ClaimValue) -> Result<(), String> {
    const MAX_TEXT_LENGTH: usize = 256;
    let text = match value {
        ClaimValue::Concept(value)
        | ClaimValue::Role(value)
        | ClaimValue::Type(value)
        | ClaimValue::Unit(value)
        | ClaimValue::QuantityKind(value)
        | ClaimValue::Scalar(value)
        | ClaimValue::Text(value) => Some(value.as_str()),
        ClaimValue::Condition(ClaimCondition::Named(value)) => Some(value.as_str()),
        ClaimValue::Relation(value) => Some(value.canonical_digest()),
        _ => None,
    };
    if text.is_some_and(|value| value.trim().is_empty() || value.len() > MAX_TEXT_LENGTH) {
        return Err("typed claim value is empty or exceeds its bound".to_owned());
    }
    if let ClaimValue::Dimension(exponents) = value
        && (exponents.len() > 16
            || exponents.iter().any(|exponent| {
                exponent.base.trim().is_empty()
                    || exponent.denominator == 0
                    || exponent.base.len() > 64
            }))
    {
        return Err("dimension claim is invalid or exceeds its bound".to_owned());
    }
    if let ClaimValue::Relation(relation) = value
        && (relation.entities().len() > 32 || relation.canonical_digest().trim().is_empty())
    {
        return Err("relation claim is invalid or exceeds its bound".to_owned());
    }
    if let ClaimValue::Shape(shape) = value
        && !valid_claim_shape(shape, MAX_TEXT_LENGTH)
    {
        return Err("shape extents are invalid or exceed their bound".to_owned());
    }
    Ok(())
}

fn valid_claim_shape(shape: &ClaimShape, max_text_length: usize) -> bool {
    match shape {
        ClaimShape::Scalar | ClaimShape::Unknown => true,
        ClaimShape::Vector(extents) | ClaimShape::Matrix(extents) | ClaimShape::Tensor(extents) => {
            extents.len() <= 16
                && extents.iter().all(|extent| match extent {
                    ClaimExtent::Known { .. } => true,
                    ClaimExtent::Symbolic { display, .. } | ClaimExtent::Unknown { display } => {
                        !display.trim().is_empty() && display.len() <= max_text_length
                    }
                })
        }
        ClaimShape::Function { domain, codomain } => {
            valid_claim_shape(domain, max_text_length)
                && valid_claim_shape(codomain, max_text_length)
        }
    }
}

fn claim_dependency_files(claim: &Claim, evidence: Option<&EvidenceRecord>) -> BTreeSet<String> {
    let mut files = BTreeSet::from([claim.subject.anchor.file_id.clone()]);
    match &claim.object {
        ClaimObject::Occurrence(id) => {
            files.insert(id.file_id.clone());
        }
        ClaimObject::Entity(entity) => {
            files.insert(entity.anchor.file_id.clone());
        }
        ClaimObject::Value(value) => collect_claim_value_files(value, &mut files),
    }
    if let Some(evidence) = evidence {
        files.insert(evidence.source.file_id.clone());
        files.extend(
            evidence
                .provenance
                .iter()
                .map(|source| source.file_id.clone()),
        );
    }
    files
}

fn collect_claim_value_files(value: &ClaimValue, files: &mut BTreeSet<String>) {
    match value {
        ClaimValue::Condition(condition) => match condition {
            ClaimCondition::Nonzero(entity)
            | ClaimCondition::Positive(entity)
            | ClaimCondition::Nonnegative(entity)
            | ClaimCondition::Invertible(entity) => {
                files.insert(entity.anchor.file_id.clone());
            }
            ClaimCondition::Member { entity, set } => {
                files.insert(entity.anchor.file_id.clone());
                files.insert(set.anchor.file_id.clone());
            }
            ClaimCondition::Named(_) => {}
        },
        ClaimValue::Relation(relation) => files.extend(
            relation
                .entities()
                .into_iter()
                .map(|entity| entity.anchor.file_id.clone()),
        ),
        ClaimValue::Shape(shape) => collect_claim_shape_files(shape, files),
        ClaimValue::Concept(_)
        | ClaimValue::Role(_)
        | ClaimValue::Type(_)
        | ClaimValue::Dimension(_)
        | ClaimValue::Unit(_)
        | ClaimValue::QuantityKind(_)
        | ClaimValue::Scalar(_)
        | ClaimValue::Text(_) => {}
    }
}

fn collect_claim_shape_files(shape: &ClaimShape, files: &mut BTreeSet<String>) {
    match shape {
        ClaimShape::Vector(extents) | ClaimShape::Matrix(extents) | ClaimShape::Tensor(extents) => {
            files.extend(extents.iter().filter_map(|extent| {
                let ClaimExtent::Symbolic { entity, .. } = extent else {
                    return None;
                };
                Some(entity.anchor.file_id.clone())
            }))
        }
        ClaimShape::Function { domain, codomain } => {
            collect_claim_shape_files(domain, files);
            collect_claim_shape_files(codomain, files);
        }
        ClaimShape::Scalar | ClaimShape::Unknown => {}
    }
}

pub(crate) fn occurrence_binding_key(occurrence: &SourceOccurrence) -> String {
    if occurrence.notation.is_empty() {
        return occurrence.surface.trim().to_owned();
    }
    serde_json::to_string(&occurrence.notation)
        .expect("notation components always serialize to a binding key")
}

fn binding_key_for_claim(
    claim: &Claim,
    occurrences: &BTreeMap<SourceOccurrenceId, SourceOccurrence>,
) -> Option<String> {
    if !matches!(
        claim.predicate,
        ClaimPredicate::Defines
            | ClaimPredicate::Names
            | ClaimPredicate::Abbreviates
            | ClaimPredicate::Aliases
    ) {
        return None;
    }
    let ClaimObject::Occurrence(occurrence_id) = &claim.object else {
        return None;
    };
    occurrences.get(occurrence_id).map(occurrence_binding_key)
}

fn unsupported(occurrence_id: SourceOccurrenceId) -> Resolution {
    Resolution {
        occurrence_id,
        status: ResolutionStatus::Unsupported,
        candidates: Vec::new(),
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn occurrence(
        file_id: &str,
        version: u64,
        local_id: u32,
        start: u32,
        order: u64,
        scope_path: &[u32],
        surface: &str,
        notation: Vec<NotationComponent>,
    ) -> SourceOccurrence {
        SourceOccurrence {
            id: SourceOccurrenceId {
                file_id: file_id.to_owned(),
                document_version: version,
                local_id,
            },
            component_id: file_id.to_owned(),
            kind: OccurrenceKind::Notation,
            range: SourceRange {
                start_offset: start,
                end_offset: start + surface.encode_utf16().count().max(1) as u32,
            },
            selection_range: SourceRange {
                start_offset: start,
                end_offset: start + surface.encode_utf16().count().max(1) as u32,
            },
            scope_path: scope_path.to_vec(),
            structural_path: vec![local_id],
            availability_order: order,
            surface: surface.to_owned(),
            source_text: surface.to_owned(),
            selection_text: surface.to_owned(),
            notation,
        }
    }

    fn entity(occurrence: &SourceOccurrence, kind: &str) -> EntityId {
        EntityId {
            component_id: occurrence.component_id.clone(),
            scope_path: occurrence.scope_path.clone(),
            kind: kind.to_owned(),
            anchor: occurrence.id.clone(),
        }
    }

    fn evidence(
        id: &str,
        source: &SourceOccurrence,
        polarity: EvidencePolarity,
        modality: EvidenceModality,
    ) -> EvidenceRecord {
        EvidenceRecord {
            id: EvidenceId(id.to_owned()),
            source: source.id.clone(),
            scope_path: source.scope_path.clone(),
            available_after: source.availability_order,
            polarity,
            modality,
            origin: EvidenceOrigin::Explicit,
            provenance: vec![source.id.clone()],
            parent_claims: Vec::new(),
            rule_id: "test-explicit-declaration".to_owned(),
            rule_version: 1,
        }
    }

    fn claim(
        id: &str,
        subject: &EntityId,
        predicate: ClaimPredicate,
        object: ClaimObject,
        evidence_id: &str,
    ) -> Claim {
        Claim {
            id: ClaimId(id.to_owned()),
            subject: subject.clone(),
            predicate,
            object,
            evidence_id: EvidenceId(evidence_id.to_owned()),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        }
    }

    fn facts(
        file_id: &str,
        version: u64,
        occurrences: Vec<SourceOccurrence>,
        entities: Vec<EntityId>,
        mentions: Vec<SourceOccurrenceId>,
        evidence: Vec<EvidenceRecord>,
        claims: Vec<Claim>,
    ) -> DocumentSemanticFacts {
        DocumentSemanticFacts {
            file_id: file_id.to_owned(),
            document_version: version,
            source_utf16_length: 10_000,
            occurrences,
            entities,
            mentions: mentions
                .into_iter()
                .map(|occurrence_id| Mention {
                    occurrence_id,
                    modality: MentionModality::Notation,
                })
                .collect(),
            evidence,
            claims,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn notation_components_do_not_create_entity_equivalence() {
        let plain = occurrence(
            "main.tex",
            1,
            1,
            0,
            1,
            &[],
            "y",
            vec![NotationComponent::Identifier {
                value: "y".to_owned(),
            }],
        );
        let hat = occurrence(
            "main.tex",
            1,
            2,
            10,
            2,
            &[],
            "y",
            vec![
                NotationComponent::Modifier {
                    name: "hat".to_owned(),
                },
                NotationComponent::Identifier {
                    value: "y".to_owned(),
                },
            ],
        );
        let bold = occurrence(
            "main.tex",
            1,
            3,
            20,
            3,
            &[],
            "y",
            vec![
                NotationComponent::Style {
                    name: "bold".to_owned(),
                },
                NotationComponent::Identifier {
                    value: "y".to_owned(),
                },
            ],
        );
        let calligraphic = occurrence(
            "main.tex",
            1,
            4,
            30,
            4,
            &[],
            "y",
            vec![NotationComponent::Style {
                name: "calligraphic".to_owned(),
            }],
        );
        let occurrences = vec![
            plain.clone(),
            hat.clone(),
            bold.clone(),
            calligraphic.clone(),
        ];
        let entities = occurrences
            .iter()
            .enumerate()
            .map(|(index, item)| entity(item, &format!("entity-{index}")))
            .collect();
        let mentions = occurrences.iter().map(|item| item.id.clone()).collect();
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "main.tex",
                1,
                occurrences,
                entities,
                mentions,
                Vec::new(),
                Vec::new(),
            ))
            .unwrap();

        assert_eq!(index.stats().entities, 4);
        for item in [plain, hat, bold, calligraphic] {
            assert_eq!(
                index.resolve(&item.id).status,
                ResolutionStatus::Unsupported
            );
        }
    }

    #[test]
    fn established_occurrence_index_replaces_and_retracts_atomically() {
        let declaration = occurrence("main.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let reference = occurrence("main.tex", 1, 2, 5, 2, &[], "x", Vec::new());
        let target_occurrences = vec![declaration.clone(), reference.clone()];
        let declared_entity = entity(&declaration, "definition");
        let source_evidence = evidence(
            "identity",
            &declaration,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let claims = vec![
            claim(
                "declaration-name",
                &declared_entity,
                ClaimPredicate::Names,
                ClaimObject::Occurrence(declaration.id.clone()),
                "identity",
            ),
            claim(
                "reference-name",
                &declared_entity,
                ClaimPredicate::Names,
                ClaimObject::Occurrence(reference.id.clone()),
                "identity",
            ),
        ];
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "main.tex",
                1,
                vec![declaration.clone(), reference.clone()],
                vec![declared_entity.clone()],
                vec![declaration.id.clone(), reference.id.clone()],
                vec![source_evidence],
                claims,
            ))
            .unwrap();
        assert_eq!(
            index
                .established_occurrences_for_entity(&declared_entity)
                .len(),
            2
        );
        assert_eq!(
            index.established_selection_would_merge(&declared_entity, "x", &target_occurrences,),
            Ok(false)
        );
        let unrelated_occurrence = SourceOccurrence {
            id: SourceOccurrenceId {
                file_id: "other.tex".into(),
                document_version: 1,
                local_id: 0,
            },
            component_id: declared_entity.component_id.clone(),
            ..declaration.clone()
        };
        let unrelated_target = EntityId {
            kind: "other-definition".into(),
            anchor: unrelated_occurrence.id.clone(),
            ..declared_entity.clone()
        };
        assert_eq!(
            index.established_selection_would_merge(
                &unrelated_target,
                "x",
                std::slice::from_ref(&unrelated_occurrence),
            ),
            Ok(true)
        );

        let replacement = occurrence("main.tex", 2, 1, 0, 1, &[], "y", Vec::new());
        index
            .replace_document(facts(
                "main.tex",
                2,
                vec![replacement.clone()],
                Vec::new(),
                vec![replacement.id],
                Vec::new(),
                Vec::new(),
            ))
            .unwrap();
        assert!(
            index
                .established_occurrences_for_entity(&declared_entity)
                .is_empty()
        );
        assert_eq!(
            index.established_selection_would_merge(
                &unrelated_target,
                "x",
                &[unrelated_occurrence],
            ),
            Ok(false)
        );
        index.remove_document("main.tex");
        assert!(
            index
                .established_occurrences_for_entity(&declared_entity)
                .is_empty()
        );
    }

    #[test]
    fn bounded_entity_surface_never_materializes_cap_plus_one() {
        let mut index = ProjectSemanticIndex::default();
        let anchor = occurrence("main.tex", 1, 0, 0, 1, &[], "x", Vec::new());
        let target = entity(&anchor, "definition");
        for local_id in 0..=MAX_ENTITY_SURFACE_OCCURRENCES as u32 {
            let item = occurrence(
                "main.tex",
                1,
                local_id,
                local_id.saturating_mul(2),
                u64::from(local_id.saturating_mul(2).saturating_add(1)),
                &[],
                "x",
                Vec::new(),
            );
            index
                .established_occurrences_by_entity
                .entry(target.clone())
                .or_default()
                .insert(item.id.clone());
            index.occurrences.insert(item.id.clone(), item);
        }
        assert_eq!(
            index.bounded_established_occurrences_for_entity(&target),
            Err(())
        );
    }

    #[test]
    fn replacing_a_declaration_refreshes_only_its_binding_fanout() {
        let mut declaration = occurrence("definitions.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        declaration.component_id = "project".to_owned();
        let mut reference = occurrence("usage.tex", 1, 1, 0, 2, &[], "x", Vec::new());
        reference.component_id = "project".to_owned();
        let original_entity = entity(&declaration, "definition-v1");
        let source_evidence = evidence(
            "identity-v1",
            &declaration,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let source_claim = claim(
            "declaration-v1",
            &original_entity,
            ClaimPredicate::Defines,
            ClaimObject::Occurrence(declaration.id.clone()),
            "identity-v1",
        );
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_documents(vec![
                facts(
                    "definitions.tex",
                    1,
                    vec![declaration],
                    vec![original_entity.clone()],
                    Vec::new(),
                    vec![source_evidence],
                    vec![source_claim],
                ),
                facts(
                    "usage.tex",
                    1,
                    vec![reference.clone()],
                    Vec::new(),
                    vec![reference.id.clone()],
                    Vec::new(),
                    Vec::new(),
                ),
            ])
            .unwrap();
        assert_eq!(
            index.entity_decision(&reference.id),
            EntityEvidenceDecision::Established(original_entity.clone())
        );

        let mut replacement = occurrence("definitions.tex", 2, 1, 0, 1, &[], "x", Vec::new());
        replacement.component_id = "project".to_owned();
        let replacement_entity = entity(&replacement, "definition-v2");
        let replacement_evidence = evidence(
            "identity-v2",
            &replacement,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let replacement_claim = claim(
            "declaration-v2",
            &replacement_entity,
            ClaimPredicate::Defines,
            ClaimObject::Occurrence(replacement.id.clone()),
            "identity-v2",
        );
        index
            .replace_document(facts(
                "definitions.tex",
                2,
                vec![replacement],
                vec![replacement_entity.clone()],
                Vec::new(),
                vec![replacement_evidence],
                vec![replacement_claim],
            ))
            .unwrap();

        assert!(
            index
                .established_occurrences_for_entity(&original_entity)
                .is_empty()
        );
        assert_eq!(
            index.entity_decision(&reference.id),
            EntityEvidenceDecision::Established(replacement_entity.clone())
        );
        assert_eq!(
            index.established_occurrences_for_entity(&replacement_entity),
            vec![&reference]
        );
        let incremental = index.established_occurrences_by_entity.clone();
        index.rebuild_resolution_index();
        assert_eq!(index.established_occurrences_by_entity, incremental);
    }

    #[test]
    fn relation_between_hat_y_and_y_does_not_merge_them() {
        let plain = occurrence("main.tex", 1, 1, 0, 1, &[], "y", Vec::new());
        let hat = occurrence(
            "main.tex",
            1,
            2,
            10,
            2,
            &[],
            "y",
            vec![NotationComponent::Modifier {
                name: "hat".to_owned(),
            }],
        );
        let plain_entity = entity(&plain, "plain-y");
        let hat_entity = entity(&hat, "hat-y");
        let relation_evidence = evidence(
            "estimate-evidence",
            &hat,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let relation = claim(
            "estimate-claim",
            &hat_entity,
            ClaimPredicate::Relates,
            ClaimObject::Entity(plain_entity.clone()),
            "estimate-evidence",
        );
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "main.tex",
                1,
                vec![plain.clone(), hat.clone()],
                vec![plain_entity, hat_entity],
                vec![plain.id.clone(), hat.id.clone()],
                vec![relation_evidence],
                vec![relation],
            ))
            .unwrap();

        assert_eq!(
            index.resolve(&plain.id).status,
            ResolutionStatus::Unsupported
        );
        assert_eq!(index.resolve(&hat.id).status, ResolutionStatus::Unsupported);
    }

    #[test]
    fn acronym_binding_is_case_sensitive_scoped_and_source_ordered() {
        let declaration = occurrence("paper.tex", 1, 1, 0, 10, &[0], "ECE", Vec::new());
        let formula = occurrence("paper.tex", 1, 2, 20, 20, &[0, 1], "ECE", Vec::new());
        let lowercase = occurrence("paper.tex", 1, 3, 30, 30, &[0, 1], "ece", Vec::new());
        let sibling = occurrence("paper.tex", 1, 4, 40, 40, &[1], "ECE", Vec::new());
        let before = occurrence("paper.tex", 1, 5, 50, 5, &[0], "ECE", Vec::new());
        let metric = entity(&declaration, "expected-calibration-error");
        let binding_evidence = evidence(
            "ece-definition",
            &declaration,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let binding = claim(
            "ece-binding",
            &metric,
            ClaimPredicate::Abbreviates,
            ClaimObject::Occurrence(declaration.id.clone()),
            "ece-definition",
        );
        let mentions = vec![
            declaration.id.clone(),
            formula.id.clone(),
            lowercase.id.clone(),
            sibling.id.clone(),
            before.id.clone(),
        ];
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "paper.tex",
                1,
                vec![
                    declaration,
                    formula.clone(),
                    lowercase.clone(),
                    sibling.clone(),
                    before.clone(),
                ],
                vec![metric.clone()],
                mentions,
                vec![binding_evidence],
                vec![binding],
            ))
            .unwrap();

        let resolved = index.resolve(&formula.id);
        assert_eq!(resolved.status, ResolutionStatus::Established);
        assert_eq!(resolved.candidates[0].entity_id, metric);
        assert_eq!(
            index.resolve(&lowercase.id).status,
            ResolutionStatus::Unsupported
        );
        assert_eq!(
            index.resolve(&sibling.id).status,
            ResolutionStatus::Unsupported
        );
        assert_eq!(
            index.resolve(&before.id).status,
            ResolutionStatus::Unsupported
        );
    }

    #[test]
    fn incremental_claim_indexes_match_a_full_rebuild() {
        let mut index = ProjectSemanticIndex::default();
        for version in [1, 2] {
            let declaration = occurrence("paper.tex", version, 1, 0, 10, &[], "ECE", Vec::new());
            let metric = entity(&declaration, "expected-calibration-error");
            let binding_evidence = evidence(
                &format!("ece-definition-{version}"),
                &declaration,
                EvidencePolarity::Positive,
                EvidenceModality::Asserted,
            );
            let binding = claim(
                &format!("ece-binding-{version}"),
                &metric,
                ClaimPredicate::Abbreviates,
                ClaimObject::Occurrence(declaration.id.clone()),
                &binding_evidence.id.0,
            );
            let mut derived_evidence = evidence(
                &format!("ece-type-evidence-{version}"),
                &declaration,
                EvidencePolarity::Positive,
                EvidenceModality::Asserted,
            );
            derived_evidence.origin = EvidenceOrigin::Derived;
            derived_evidence.parent_claims = vec![binding.id.clone()];
            derived_evidence.rule_id = "test/derived-type".into();
            let derived = Claim {
                id: ClaimId(format!("ece-type-{version}")),
                subject: metric.clone(),
                predicate: ClaimPredicate::HasType,
                object: ClaimObject::Value(ClaimValue::Type("metric".into())),
                evidence_id: derived_evidence.id.clone(),
                tier: InferenceTier::Constraint,
                derivation_depth: 1,
            };
            index
                .replace_document(facts(
                    "paper.tex",
                    version,
                    vec![declaration.clone()],
                    vec![metric],
                    vec![declaration.id],
                    vec![binding_evidence, derived_evidence],
                    vec![binding, derived],
                ))
                .unwrap();

            let dependents = index.dependents.clone();
            let claims_by_file_dependency = index.claims_by_file_dependency.clone();
            let binding_claims = index.binding_claims.clone();
            let occurrences_by_binding = index.occurrences_by_binding.clone();
            let claims_by_entity = index.claims_by_entity.clone();
            let established = index.established_occurrences_by_entity.clone();
            index.rebuild_indexes();
            assert_eq!(index.dependents, dependents);
            assert_eq!(index.claims_by_file_dependency, claims_by_file_dependency);
            assert_eq!(index.binding_claims, binding_claims);
            assert_eq!(index.occurrences_by_binding, occurrences_by_binding);
            assert_eq!(index.claims_by_entity, claims_by_entity);
            assert_eq!(index.established_occurrences_by_entity, established);
        }

        index.remove_document("paper.tex");
        assert!(index.dependents.is_empty());
        assert!(index.claims_by_file_dependency.is_empty());
        assert!(index.binding_claims.is_empty());
        assert!(index.occurrences_by_binding.is_empty());
        assert!(index.claims_by_entity.is_empty());
    }

    #[test]
    fn non_asserting_modalities_cannot_establish_a_binding() {
        for (index_number, modality) in [
            EvidenceModality::Hypothetical,
            EvidenceModality::Hedged,
            EvidenceModality::Quoted,
            EvidenceModality::Cited,
        ]
        .into_iter()
        .enumerate()
        {
            let file = format!("modality-{index_number}.tex");
            let declaration = occurrence(&file, 1, 1, 0, 1, &[], "x", Vec::new());
            let usage = occurrence(&file, 1, 2, 10, 2, &[], "x", Vec::new());
            let declared_entity = entity(&declaration, "x");
            let declaration_evidence = evidence(
                "declaration",
                &declaration,
                EvidencePolarity::Positive,
                modality,
            );
            let binding = claim(
                "binding",
                &declared_entity,
                ClaimPredicate::Defines,
                ClaimObject::Occurrence(declaration.id.clone()),
                "declaration",
            );
            let mut semantic_index = ProjectSemanticIndex::default();
            semantic_index
                .replace_document(facts(
                    &file,
                    1,
                    vec![declaration, usage.clone()],
                    vec![declared_entity],
                    vec![usage.id.clone()],
                    vec![declaration_evidence],
                    vec![binding],
                ))
                .unwrap();
            assert_eq!(
                semantic_index.resolve(&usage.id).status,
                ResolutionStatus::Unsupported
            );
        }
    }

    #[test]
    fn a_declaration_anchor_owns_its_entity_before_trailing_prose_finishes() {
        let declaration = occurrence("main.tex", 1, 1, 1, 1, &[], "x", Vec::new());
        let premature = occurrence("main.tex", 1, 2, 20, 1, &[], "x", Vec::new());
        let usage = occurrence("main.tex", 1, 3, 40, 3, &[], "x", Vec::new());
        let declared_entity = entity(&declaration, "x");
        let mut declaration_evidence = evidence(
            "declaration",
            &declaration,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        declaration_evidence.available_after = 2;
        let binding = claim(
            "binding",
            &declared_entity,
            ClaimPredicate::Defines,
            ClaimObject::Occurrence(declaration.id.clone()),
            "declaration",
        );
        let mut semantic_index = ProjectSemanticIndex::default();
        semantic_index
            .replace_document(facts(
                "main.tex",
                1,
                vec![declaration.clone(), premature.clone(), usage.clone()],
                vec![declared_entity.clone()],
                vec![
                    declaration.id.clone(),
                    premature.id.clone(),
                    usage.id.clone(),
                ],
                vec![declaration_evidence],
                vec![binding],
            ))
            .unwrap();

        assert_eq!(
            semantic_index.resolve(&declaration.id).candidates[0].entity_id,
            declared_entity
        );
        assert_eq!(
            semantic_index.resolve(&premature.id).status,
            ResolutionStatus::Unsupported
        );
        assert_eq!(
            semantic_index.resolve(&usage.id).status,
            ResolutionStatus::Established
        );
    }

    #[test]
    fn removing_source_evidence_retracts_the_minimal_derived_closure() {
        let source = occurrence("definitions.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let source_entity = entity(&source, "x");
        let source_evidence = evidence(
            "source-evidence",
            &source,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let source_claim = claim(
            "source-claim",
            &source_entity,
            ClaimPredicate::Defines,
            ClaimObject::Occurrence(source.id.clone()),
            "source-evidence",
        );
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "definitions.tex",
                1,
                vec![source.clone()],
                vec![source_entity.clone()],
                vec![source.id.clone()],
                vec![source_evidence],
                vec![source_claim],
            ))
            .unwrap();

        let derived_source = occurrence("analysis.tex", 1, 1, 0, 2, &[], "x", Vec::new());
        let derived_entity = entity(&derived_source, "typed-x");
        let derived_evidence = EvidenceRecord {
            id: EvidenceId("derived-evidence".to_owned()),
            source: derived_source.id.clone(),
            scope_path: Vec::new(),
            available_after: 2,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Derived,
            provenance: vec![source.id.clone(), derived_source.id.clone()],
            parent_claims: vec![ClaimId("source-claim".to_owned())],
            rule_id: "test-derived-type".to_owned(),
            rule_version: 1,
        };
        let derived_claim = Claim {
            id: ClaimId("derived-claim".to_owned()),
            subject: derived_entity.clone(),
            predicate: ClaimPredicate::HasType,
            object: ClaimObject::Value(ClaimValue::Type("real".to_owned())),
            evidence_id: derived_evidence.id.clone(),
            tier: InferenceTier::Constraint,
            derivation_depth: 1,
        };
        index
            .replace_document(facts(
                "analysis.tex",
                1,
                vec![derived_source],
                vec![derived_entity],
                Vec::new(),
                vec![derived_evidence],
                vec![derived_claim],
            ))
            .unwrap();

        index.remove_document("definitions.tex");

        assert!(index.claim(&ClaimId("source-claim".to_owned())).is_none());
        assert!(index.claim(&ClaimId("derived-claim".to_owned())).is_none());
        assert!(
            index
                .evidence(&EvidenceId("derived-evidence".to_owned()))
                .is_none()
        );
        assert_eq!(index.stats().occurrences, 1);
        assert_eq!(index.stats().invalidated_claims, 2);
    }

    #[test]
    fn tier_reversal_and_non_increasing_depth_are_rejected_atomically() {
        let source = occurrence("main.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let declared_entity = entity(&source, "x");
        let source_evidence = evidence(
            "source-evidence",
            &source,
            EvidencePolarity::Positive,
            EvidenceModality::Asserted,
        );
        let source_claim = claim(
            "source-claim",
            &declared_entity,
            ClaimPredicate::Defines,
            ClaimObject::Occurrence(source.id.clone()),
            "source-evidence",
        );
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "main.tex",
                1,
                vec![source.clone()],
                vec![declared_entity.clone()],
                vec![source.id.clone()],
                vec![source_evidence],
                vec![source_claim],
            ))
            .unwrap();
        let before = index.stats();

        let derived_source = occurrence("bad.tex", 1, 1, 0, 2, &[], "x", Vec::new());
        let bad_evidence = EvidenceRecord {
            id: EvidenceId("bad-evidence".to_owned()),
            source: derived_source.id.clone(),
            scope_path: Vec::new(),
            available_after: 2,
            polarity: EvidencePolarity::Positive,
            modality: EvidenceModality::Asserted,
            origin: EvidenceOrigin::Derived,
            provenance: vec![source.id, derived_source.id.clone()],
            parent_claims: vec![ClaimId("source-claim".to_owned())],
            rule_id: "bad-rule".to_owned(),
            rule_version: 1,
        };
        let bad_claim = Claim {
            id: ClaimId("bad-claim".to_owned()),
            subject: declared_entity,
            predicate: ClaimPredicate::HasType,
            object: ClaimObject::Value(ClaimValue::Type("real".to_owned())),
            evidence_id: bad_evidence.id.clone(),
            tier: InferenceTier::ExplicitClaim,
            derivation_depth: 0,
        };
        let result = index.replace_document(facts(
            "bad.tex",
            1,
            vec![derived_source],
            Vec::new(),
            Vec::new(),
            vec![bad_evidence],
            vec![bad_claim],
        ));

        assert!(result.is_err());
        assert_eq!(index.stats(), before);
    }

    #[test]
    fn invalid_source_ranges_do_not_partially_replace_a_document() {
        let original = occurrence("main.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "main.tex",
                1,
                vec![original.clone()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ))
            .unwrap();
        let mut invalid = occurrence("main.tex", 2, 1, 0, 2, &[], "y", Vec::new());
        invalid.range.end_offset = 20_000;

        assert!(
            index
                .replace_document(facts(
                    "main.tex",
                    2,
                    vec![invalid],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ))
                .is_err()
        );
        assert!(index.occurrence(&original.id).is_some());
        assert_eq!(index.stats().occurrences, 1);
    }

    #[test]
    fn invalid_late_batch_document_restores_every_affected_document() {
        let original_a = occurrence("a.tex", 1, 1, 0, 1, &[], "a", Vec::new());
        let original_b = occurrence("b.tex", 1, 1, 0, 2, &[], "b", Vec::new());
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_documents(vec![
                facts(
                    "a.tex",
                    1,
                    vec![original_a.clone()],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                facts(
                    "b.tex",
                    1,
                    vec![original_b.clone()],
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            ])
            .unwrap();
        let before = index.stats();
        let replacement_a = occurrence("a.tex", 2, 1, 0, 3, &[], "x", Vec::new());
        let mut invalid_b = occurrence("b.tex", 2, 1, 0, 4, &[], "y", Vec::new());
        invalid_b.range.end_offset = 20_000;

        assert!(
            index
                .replace_documents(vec![
                    facts(
                        "a.tex",
                        2,
                        vec![replacement_a],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                    facts(
                        "b.tex",
                        2,
                        vec![invalid_b],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ])
                .is_err()
        );
        assert!(index.occurrence(&original_a.id).is_some());
        assert!(index.occurrence(&original_b.id).is_some());
        assert_eq!(index.stats(), before);
    }

    #[test]
    fn resolution_candidates_are_deterministic_and_bounded() {
        let usage = occurrence("many.tex", 1, 100, 5_000, 1_000, &[], "x", Vec::new());
        let mut occurrences = vec![usage.clone()];
        let mut entities = Vec::new();
        let mut evidence_records = Vec::new();
        let mut claims = Vec::new();
        for number in (0..40).rev() {
            let declaration = occurrence(
                "many.tex",
                1,
                number + 1,
                number * 10,
                1,
                &[],
                "x",
                Vec::new(),
            );
            let declared_entity = entity(&declaration, &format!("candidate-{number:02}"));
            let evidence_id = format!("evidence-{number:02}");
            let claim_id = format!("claim-{number:02}");
            evidence_records.push(evidence(
                &evidence_id,
                &declaration,
                EvidencePolarity::Positive,
                EvidenceModality::Asserted,
            ));
            claims.push(claim(
                &claim_id,
                &declared_entity,
                ClaimPredicate::Defines,
                ClaimObject::Occurrence(declaration.id.clone()),
                &evidence_id,
            ));
            occurrences.push(declaration);
            entities.push(declared_entity);
        }
        let mut index = ProjectSemanticIndex::default();
        index
            .replace_document(facts(
                "many.tex",
                1,
                occurrences,
                entities,
                vec![usage.id.clone()],
                evidence_records,
                claims,
            ))
            .unwrap();

        let first = index.resolve(&usage.id);
        let second = index.resolve(&usage.id);
        assert_eq!(first, second);
        assert_eq!(first.status, ResolutionStatus::Ambiguous);
        assert!(first.truncated);
        assert_eq!(first.candidates.len(), MAX_RESOLUTION_CANDIDATES);
        assert!(
            first
                .candidates
                .windows(2)
                .all(|pair| pair[0].entity_id < pair[1].entity_id)
        );
    }

    #[test]
    fn structural_candidates_rank_only_from_asserted_typed_claims() {
        let notation = vec![NotationComponent::NamedSurface {
            value: "ECE".into(),
        }];
        let declaration = occurrence("metric.tex", 1, 1, 0, 1, &[], "ECE", notation.clone());
        let usage = occurrence("metric.tex", 1, 2, 20, 20, &[], "ECE", notation);
        let metric = entity(&declaration, "metric");
        let mut document = facts(
            "metric.tex",
            1,
            vec![declaration.clone(), usage.clone()],
            vec![metric.clone()],
            vec![usage.id.clone()],
            vec![
                evidence(
                    "definition-evidence",
                    &declaration,
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                ),
                evidence(
                    "type-evidence",
                    &declaration,
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                ),
            ],
            vec![
                claim(
                    "definition",
                    &metric,
                    ClaimPredicate::Defines,
                    ClaimObject::Occurrence(declaration.id.clone()),
                    "definition-evidence",
                ),
                claim(
                    "metric-type",
                    &metric,
                    ClaimPredicate::HasType,
                    ClaimObject::Value(ClaimValue::Type("metric".into())),
                    "type-evidence",
                ),
            ],
        );
        document.candidates = vec![SemanticCandidateClaim {
            id: CandidateId("application-candidate".into()),
            occurrence_id: usage.id.clone(),
            family: CandidateFamily::Application,
            interpretation: "application".into(),
            range: usage.range.clone(),
            supporting_claims: Vec::new(),
            rejecting_claims: Vec::new(),
        }];
        let mut index = ProjectSemanticIndex::default();
        index.replace_document(document).unwrap();

        let candidates = index.candidates_for(&usage.id);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].supporting_claims,
            [ClaimId("metric-type".into())]
        );
        assert!(candidates[0].rejecting_claims.is_empty());
    }

    #[test]
    fn typed_claim_values_serialize_deterministically_and_validate_by_predicate() {
        let source = occurrence("typed.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let subject = entity(&source, "x");
        let value = ClaimObject::Value(ClaimValue::Shape(ClaimShape::Vector(vec!["n".into()])));
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#"{"kind":"value","value":{"kind":"shape","value":{"kind":"vector","value":[{"kind":"unknown","display":"n"}]}}}"#
        );
        let mut index = ProjectSemanticIndex::default();
        let invalid = claim(
            "wrong-kind",
            &subject,
            ClaimPredicate::HasUnit,
            value,
            "typed-evidence",
        );
        let error = index
            .replace_document(facts(
                "typed.tex",
                1,
                vec![source.clone()],
                vec![subject],
                vec![source.id.clone()],
                vec![evidence(
                    "typed-evidence",
                    &source,
                    EvidencePolarity::Positive,
                    EvidenceModality::Asserted,
                )],
                vec![invalid],
            ))
            .unwrap_err();
        assert_eq!(error, "claim predicate and typed object are incompatible");
        assert_eq!(index.stats(), SemanticIndexStats::default());
    }

    #[test]
    fn typed_dimensions_reject_zero_denominators_atomically() {
        let source = occurrence("dimension.tex", 1, 1, 0, 1, &[], "x", Vec::new());
        let subject = entity(&source, "x");
        let invalid = claim(
            "invalid-dimension",
            &subject,
            ClaimPredicate::HasDimension,
            ClaimObject::Value(ClaimValue::Dimension(vec![DimensionExponent {
                base: "length".into(),
                numerator: 1,
                denominator: 0,
            }])),
            "dimension-evidence",
        );
        let mut index = ProjectSemanticIndex::default();
        assert_eq!(
            index
                .replace_document(facts(
                    "dimension.tex",
                    1,
                    vec![source.clone()],
                    vec![subject],
                    vec![source.id.clone()],
                    vec![evidence(
                        "dimension-evidence",
                        &source,
                        EvidencePolarity::Positive,
                        EvidenceModality::Asserted,
                    )],
                    vec![invalid],
                ))
                .unwrap_err(),
            "dimension claim is invalid or exceeds its bound"
        );
        assert_eq!(index.stats(), SemanticIndexStats::default());
    }
}
