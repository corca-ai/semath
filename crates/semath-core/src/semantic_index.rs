use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::SourceRange;

const MAX_DOCUMENT_OCCURRENCES: usize = 100_000;
const MAX_DOCUMENT_CLAIMS: usize = 50_000;
const MAX_DERIVATION_DEPTH: u8 = 8;
const MAX_RESOLUTION_CANDIDATES: usize = 32;

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
    Subscript,
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
    pub scope_path: Vec<u32>,
    pub structural_path: Vec<u32>,
    /// Monotonic project-snapshot order assigned by the lowering boundary.
    /// Cross-file visibility must never compare unrelated file-local offsets.
    pub availability_order: u64,
    pub surface: String,
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
    HasQuantity,
    HasUnit,
    Assumes,
    Relates,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ClaimObject {
    Entity(EntityId),
    Occurrence(SourceOccurrenceId),
    Text(String),
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
}

#[derive(Clone, Debug, Default)]
pub struct ProjectSemanticIndex {
    document_versions: BTreeMap<String, u64>,
    occurrences: BTreeMap<SourceOccurrenceId, SourceOccurrence>,
    entities: BTreeSet<EntityId>,
    mentions: BTreeMap<SourceOccurrenceId, Mention>,
    evidence: BTreeMap<EvidenceId, EvidenceRecord>,
    claims: BTreeMap<ClaimId, Claim>,
    dependents: BTreeMap<ClaimId, BTreeSet<ClaimId>>,
    binding_claims: BTreeMap<String, BTreeSet<ClaimId>>,
    invalidated_claims: u32,
}

impl ProjectSemanticIndex {
    pub fn replace_document(&mut self, facts: DocumentSemanticFacts) -> Result<(), String> {
        let mut next = self.clone();
        next.retract_document(&facts.file_id);
        next.validate_and_insert(facts)?;
        next.rebuild_indexes();
        *self = next;
        Ok(())
    }

    pub fn remove_document(&mut self, file_id: &str) {
        self.retract_document(file_id);
        self.rebuild_indexes();
    }

    pub fn resolve(&self, occurrence_id: &SourceOccurrenceId) -> Resolution {
        let Some(occurrence) = self.occurrences.get(occurrence_id) else {
            return unsupported(occurrence_id.clone());
        };
        if !self.mentions.contains_key(occurrence_id) {
            return unsupported(occurrence_id.clone());
        }
        let normalized = binding_key(occurrence);
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
                    || evidence.available_after > occurrence.availability_order
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

    pub fn stats(&self) -> SemanticIndexStats {
        SemanticIndexStats {
            occurrences: self.occurrences.len() as u32,
            entities: self.entities.len() as u32,
            mentions: self.mentions.len() as u32,
            claims: self.claims.len() as u32,
            evidence: self.evidence.len() as u32,
            dependency_edges: self.dependents.values().map(BTreeSet::len).sum::<usize>() as u32,
            invalidated_claims: self.invalidated_claims,
        }
    }

    pub fn occurrence(&self, id: &SourceOccurrenceId) -> Option<&SourceOccurrence> {
        self.occurrences.get(id)
    }

    pub fn claim(&self, id: &ClaimId) -> Option<&Claim> {
        self.claims.get(id)
    }

    pub fn evidence(&self, id: &EvidenceId) -> Option<&EvidenceRecord> {
        self.evidence.get(id)
    }

    fn validate_and_insert(&mut self, facts: DocumentSemanticFacts) -> Result<(), String> {
        if facts.occurrences.len() > MAX_DOCUMENT_OCCURRENCES {
            return Err("document occurrence cap exceeded".to_owned());
        }
        if facts.claims.len() > MAX_DOCUMENT_CLAIMS {
            return Err("document claim cap exceeded".to_owned());
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
        let known_occurrences = self
            .occurrences
            .keys()
            .cloned()
            .chain(occurrence_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let new_entities = facts.entities.iter().cloned().collect::<BTreeSet<_>>();
        if new_entities.len() != facts.entities.len() {
            return Err("duplicate entity identity".to_owned());
        }
        for entity in &facts.entities {
            if entity.anchor.file_id != facts.file_id {
                return Err("document cannot re-own an entity anchored in another file".to_owned());
            }
            if !known_occurrences.contains(&entity.anchor) {
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
        let known_entities = self
            .entities
            .iter()
            .cloned()
            .chain(facts.entities.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut mention_ids = BTreeSet::new();
        for mention in &facts.mentions {
            if mention.occurrence_id.file_id != facts.file_id {
                return Err("document cannot re-own a mention in another file".to_owned());
            }
            if !known_occurrences.contains(&mention.occurrence_id) {
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
            if !known_occurrences.contains(&evidence.source)
                || evidence
                    .provenance
                    .iter()
                    .any(|source| !known_occurrences.contains(source))
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
                return Err("evidence scope or availability precedes its source".to_owned());
            }
            if evidence.rule_id.trim().is_empty() || evidence.rule_version == 0 {
                return Err("evidence extraction rule must be versioned".to_owned());
            }
            if evidence.origin == EvidenceOrigin::Explicit && !evidence.parent_claims.is_empty() {
                return Err("explicit evidence cannot depend on derived claims".to_owned());
            }
        }
        for claim in &facts.claims {
            if !known_entities.contains(&claim.subject) {
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
                && !known_occurrences.contains(occurrence)
            {
                return Err("claim object is not a source occurrence".to_owned());
            }
            if let ClaimObject::Entity(entity) = &claim.object
                && !known_entities.contains(entity)
            {
                return Err("claim object is not a known entity".to_owned());
            }
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
        self.document_versions
            .insert(facts.file_id, facts.document_version);
        self.occurrences.extend(
            facts
                .occurrences
                .into_iter()
                .map(|item| (item.id.clone(), item)),
        );
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
        self.claims
            .extend(facts.claims.into_iter().map(|item| (item.id.clone(), item)));
        Ok(())
    }

    fn retract_document(&mut self, file_id: &str) {
        self.document_versions.remove(file_id);
        let mut queue = self
            .claims
            .values()
            .filter(|claim| claim_depends_on_file(claim, &self.evidence, file_id))
            .map(|claim| claim.id.clone())
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
        self.claims.retain(|id, _| !removed.contains(id));
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
        self.occurrences.retain(|id, _| id.file_id != file_id);
    }

    fn rebuild_indexes(&mut self) {
        self.dependents.clear();
        self.binding_claims.clear();
        for claim in self.claims.values() {
            if let Some(evidence) = self.evidence.get(&claim.evidence_id) {
                for parent in &evidence.parent_claims {
                    self.dependents
                        .entry(parent.clone())
                        .or_default()
                        .insert(claim.id.clone());
                }
            }
            if matches!(
                claim.predicate,
                ClaimPredicate::Defines
                    | ClaimPredicate::Names
                    | ClaimPredicate::Abbreviates
                    | ClaimPredicate::Aliases
            ) && let ClaimObject::Occurrence(occurrence_id) = &claim.object
                && let Some(occurrence) = self.occurrences.get(occurrence_id)
            {
                self.binding_claims
                    .entry(binding_key(occurrence))
                    .or_default()
                    .insert(claim.id.clone());
            }
        }
    }
}

fn claim_depends_on_file(
    claim: &Claim,
    evidence: &BTreeMap<EvidenceId, EvidenceRecord>,
    file_id: &str,
) -> bool {
    claim.subject.anchor.file_id == file_id
        || matches!(&claim.object, ClaimObject::Occurrence(id) if id.file_id == file_id)
        || matches!(&claim.object, ClaimObject::Entity(id) if id.anchor.file_id == file_id)
        || evidence.get(&claim.evidence_id).is_some_and(|item| {
            item.source.file_id == file_id
                || item
                    .provenance
                    .iter()
                    .any(|source| source.file_id == file_id)
        })
}

fn binding_key(occurrence: &SourceOccurrence) -> String {
    if occurrence.notation.is_empty() {
        return occurrence.surface.trim().to_owned();
    }
    serde_json::to_string(&occurrence.notation)
        .expect("notation components always serialize to a binding key")
}

fn scope_visible(declaration: &[u32], occurrence: &[u32]) -> bool {
    declaration.len() <= occurrence.len()
        && declaration
            .iter()
            .zip(occurrence)
            .all(|(left, right)| left == right)
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
            scope_path: scope_path.to_vec(),
            structural_path: vec![local_id],
            availability_order: order,
            surface: surface.to_owned(),
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
            object: ClaimObject::Text("real".to_owned()),
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
            object: ClaimObject::Text("real".to_owned()),
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
}
