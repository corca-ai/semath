use std::collections::{BTreeSet, HashMap};

use super::{
    EngineError, SemathEngine, canonical_expression_owner, expression_carries_formula_fact,
    formula_meaning_is_adopted, formula_meaning_owns_relation_head, index_occurrence_range,
    notation_occurrence_range, occurrence_id_at_range, relation_expression_at_cursor,
    stable_text_digest,
};
use crate::canonical::{
    SemanticExpr, SemanticExprKind, SemanticReference, lower_document_region, relation_head,
    render_canonical,
};
use crate::parser::test_math_regions;
use crate::semantic_index::{OccurrenceKind, SourceOccurrence, SourceOccurrenceId};
use crate::{
    ChangeEnvelope, CompleteSyntaxState, DocumentLanguage, Evidence, GeneratedNotationNode,
    GeneratedNotationTree, LawBindingProof, LexicalClass, MathRoot, MathRootState, MeaningDecision,
    NotationArgument, NotationNode, NotationNodeKind, NotationNodeRanges, PROTOCOL_VERSION,
    ProjectChange, ProjectDocument, ProjectInclude, ProjectMacro, ProjectMacroExpansion,
    ProjectMacroExpansionStatus, ProjectMacroKind, ProjectSnapshot, ProjectSourceRef, Query,
    QueryEnvelope, QueryValue, SourceRange, SyntaxScope, SyntaxState, VisibleProseSpan,
};

#[test]
fn formula_meaning_ownership_is_independent_of_provenance_rule_id() {
    let mut fact = crate::prose::FormulaMeaningFact {
        target_range: SourceRange {
            start_offset: 4,
            end_offset: 9,
        },
        ownership: crate::prose::FormulaMeaningOwnership::RelationHead,
        authority: crate::prose::FormulaMeaningAuthority::Adopted,
        evidence: Evidence {
            rule_id: "public/notation-provenance-v1".into(),
            kind: "attached-prose".into(),
            strength: "strong".into(),
            source_ranges: vec![SourceRange {
                start_offset: 0,
                end_offset: 3,
            }],
            source_anchors: Vec::new(),
        },
    };

    assert!(formula_meaning_owns_relation_head(&fact));
    assert!(formula_meaning_is_adopted(&fact));
    fact.evidence.rule_id = "public/notation-provenance-v2".into();
    assert!(formula_meaning_owns_relation_head(&fact));
    assert!(formula_meaning_is_adopted(&fact));
}

fn document(file_id: &str, path: &str, content: &str, version: u64) -> ProjectDocument {
    document_with_language(file_id, path, content, version, DocumentLanguage::Latex)
}

fn document_with_language(
    file_id: &str,
    path: &str,
    content: &str,
    version: u64,
    language: DocumentLanguage,
) -> ProjectDocument {
    ProjectDocument {
        prose_annotations: vec![],
        file_id: file_id.into(),
        path: path.into(),
        language,
        content: content.into(),
        document_version: version,
        schema_version: 8,
        nodes: Vec::new(),
        math_roots: Vec::new(),
        visible_prose: Vec::new(),
        scopes: Vec::new(),
        blocks: Vec::new(),
        declarations: Vec::new(),
        math_regions: test_math_regions(content, language),
        macros: Vec::new(),
        includes: Vec::new(),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceGradedDevelopmentFixture {
    schema_version: u32,
    id: String,
    provenance: EvidenceGradedDevelopmentProvenance,
    cases: Vec<EvidenceGradedDevelopmentCase>,
    independently_authored_scenario_coverage: Vec<EvidenceGradedAuthoredScenarioCoverage>,
    supplemental_lifecycle_tests: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceGradedDevelopmentProvenance {
    authoring_method: String,
    engine_blind_at_authoring: bool,
    historical_or_fresh_fixture_imported: bool,
    purpose: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceGradedDevelopmentCase {
    id: String,
    pair_id: String,
    language: DocumentLanguage,
    path: String,
    source: String,
    cursor_needle: String,
    expected_decision: String,
    expected_support: Option<String>,
    required_evidence_role: String,
    required_provenance: Vec<String>,
    required_missing_discriminator_prefix: Option<String>,
    minimum_hypotheses: usize,
    maximum_diagnostics: usize,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceGradedAuthoredScenarioCoverage {
    scenario_id: String,
    probe_id: String,
    expected_decision: String,
    facets: Vec<String>,
}

#[test]
fn public_evidence_graded_hypotheses_are_source_grounded_and_format_paired() {
    let fixture: EvidenceGradedDevelopmentFixture = serde_json::from_str(include_str!(
        "../../../fixtures/development/evidence-graded-hypotheses-v1.json"
    ))
    .expect("strict evidence-graded development fixture");
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.id, "evidence-graded-hypotheses-v1");
    assert_eq!(
        fixture.provenance.authoring_method,
        "project-original-reviewed-development"
    );
    assert!(!fixture.provenance.engine_blind_at_authoring);
    assert!(!fixture.provenance.historical_or_fresh_fixture_imported);
    assert!(fixture.provenance.purpose.contains("spent release fixture"));
    assert_eq!(fixture.cases.len(), 8);
    assert_eq!(fixture.supplemental_lifecycle_tests.len(), 7);
    assert_independently_authored_evidence_coverage(
        &fixture.independently_authored_scenario_coverage,
    );

    let mut paired = HashMap::new();
    for case in fixture.cases {
        let offset = case
            .source
            .rfind(&case.cursor_needle)
            .unwrap_or_else(|| panic!("{} has a unique cursor needle", case.id))
            as u32;
        let mut engine = SemathEngine::default();
        engine
            .reset(ProjectSnapshot {
                protocol_version: PROTOCOL_VERSION,
                epoch: format!("{}:1", case.id),
                inventory_version: 1,
                project_id: case.id.clone(),
                main_file_id: Some("main".into()),
                documents: vec![document_with_language(
                    "main",
                    &case.path,
                    &case.source,
                    1,
                    case.language,
                )],
            })
            .unwrap();
        let QueryValue::SemanticView { view } = engine
            .query(QueryEnvelope {
                protocol_version: PROTOCOL_VERSION,
                epoch: format!("{}:1", case.id),
                inventory_version: 1,
                document_version: 1,
                analysis_generation: 1,
                query: Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
            })
            .unwrap()
            .value
        else {
            panic!("{} expected semantic view", case.id)
        };
        let cursor_only_conflict = case.expected_decision == "conflicting"
            && case.expected_support.as_deref() == Some("contradicted");
        let formula_scoped_expectation = case.expected_support.as_deref() == Some("derived");
        let actual_decision = if formula_scoped_expectation {
            authoring_disposition_name(view.authoring_context.disposition)
        } else {
            meaning_decision_name(&view.decision)
        };
        assert_eq!(
            actual_decision, case.expected_decision,
            "{} decision: {view:#?}",
            case.id,
        );
        assert_eq!(
            view.authoring_context.interpretations.exhaustiveness,
            crate::MathInterpretationExhaustiveness::BoundedOpenWorld,
            "{} exhaustiveness",
            case.id
        );
        assert!(
            view.authoring_context.interpretations.hypotheses.len() >= case.minimum_hypotheses,
            "{} hypotheses: {:#?}",
            case.id,
            view.authoring_context.interpretations
        );
        assert!(
            view.diagnostics.len() <= case.maximum_diagnostics,
            "{} diagnostics: {:#?}",
            case.id,
            view.diagnostics
        );
        if cursor_only_conflict {
            assert!(matches!(
                view.decision,
                MeaningDecision::Conflicting {
                    ref conflicts,
                    ref reasons
                } if !conflicts.is_empty() && !reasons.is_empty()
            ));
            assert!(
                view.authoring_context
                    .interpretations
                    .hypotheses
                    .iter()
                    .all(|hypothesis| {
                        hypothesis.support != crate::MathInterpretationSupportTier::Contradicted
                    })
            );
        } else if let Some(expected) = case.expected_support.as_deref() {
            assert!(
                view.authoring_context
                    .interpretations
                    .hypotheses
                    .iter()
                    .any(|hypothesis| interpretation_support_name(hypothesis.support) == expected),
                "{} support: {:#?}",
                case.id,
                view.authoring_context.interpretations.hypotheses
            );
        }
        if !cursor_only_conflict {
            assert!(
                view.authoring_context
                    .interpretations
                    .hypotheses
                    .iter()
                    .flat_map(|hypothesis| &hypothesis.evidence)
                    .any(|item| interpretation_evidence_role_name(item.role)
                        == case.required_evidence_role),
                "{} evidence role {}: {:#?}",
                case.id,
                case.required_evidence_role,
                view.authoring_context.interpretations.hypotheses
            );
        }
        for expected in &case.required_provenance {
            assert!(
                view.authoring_context
                    .interpretations
                    .hypotheses
                    .iter()
                    .flat_map(|hypothesis| &hypothesis.evidence)
                    .any(|item| interpretation_provenance_name(item.provenance) == expected),
                "{} provenance {}: {:#?}",
                case.id,
                expected,
                view.authoring_context.interpretations.hypotheses
            );
        }
        if let Some(prefix) = &case.required_missing_discriminator_prefix {
            let discriminator_ids = view
                .authoring_context
                .interpretations
                .missing_discriminators
                .iter()
                .map(authoring_requirement_name)
                .collect::<Vec<_>>();
            if prefix == "declaration/"
                && view
                    .authoring_context
                    .interpretations
                    .hypotheses
                    .iter()
                    .any(|hypothesis| hypothesis.hypothesis_id == "source-meaning")
            {
                assert!(
                    discriminator_ids.iter().all(|id| !id.starts_with(prefix)),
                    "{} formula requirements must not inherit cursor declarations: {discriminator_ids:?}",
                    case.id
                );
            } else {
                assert!(
                    discriminator_ids.iter().any(|id| id.starts_with(prefix)),
                    "{} discriminators: {discriminator_ids:?}",
                    case.id
                );
            }
        }
        for hypothesis in &view.authoring_context.interpretations.hypotheses {
            assert_eq!(hypothesis.location.file_id, "main", "{} file", case.id);
            assert_eq!(hypothesis.location.path, case.path, "{} path", case.id);
            assert_eq!(hypothesis.document_version, 1, "{} version", case.id);
            assert!(
                hypothesis
                    .ordering_reasons
                    .iter()
                    .filter(|reason| {
                        reason.kind
                            != crate::MathInterpretationOrderingReasonKind::StableSourceOrder
                    })
                    .all(|reason| !reason.evidence.is_empty()),
                "{} ordering evidence: {:#?}",
                case.id,
                hypothesis.ordering_reasons
            );
        }
        let summary = (
            meaning_decision_name(&view.decision),
            view.authoring_context
                .interpretations
                .hypotheses
                .iter()
                .map(|hypothesis| {
                    (
                        hypothesis.kind,
                        hypothesis.support,
                        hypothesis.label.clone(),
                        hypothesis.missing_discriminator_ids.clone(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        if let Some(previous) = paired.insert(case.pair_id.clone(), summary.clone()) {
            assert_eq!(previous, summary, "{} TeX/Markdown parity", case.pair_id);
        }
    }
    assert_eq!(paired.len(), 4);
}

fn assert_independently_authored_evidence_coverage(
    coverage: &[EvidenceGradedAuthoredScenarioCoverage],
) {
    let authored: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/challenge/document-reasoning-development-v1.json"
    ))
    .expect("authored development fixture");
    let scenarios = authored["scenarios"]
        .as_array()
        .expect("authored scenarios");
    let probes = authored["probes"].as_array().expect("authored probes");
    let mut facets = std::collections::BTreeSet::new();
    for reference in coverage {
        let scenario = scenarios
            .iter()
            .find(|scenario| scenario["id"] == reference.scenario_id)
            .unwrap_or_else(|| panic!("missing authored scenario {}", reference.scenario_id));
        assert_eq!(scenario["provenance"]["engineBlind"], true);
        assert!(
            scenario["review"]["status"] == "accepted"
                || scenario["review"]["status"] == "corrected"
        );
        let probe = probes
            .iter()
            .find(|probe| probe["id"] == reference.probe_id)
            .unwrap_or_else(|| panic!("missing authored probe {}", reference.probe_id));
        assert_eq!(probe["scenarioId"], reference.scenario_id);
        assert_eq!(probe["expected"]["decision"], reference.expected_decision);
        facets.extend(reference.facets.iter().map(String::as_str));
    }
    for required in [
        "cross-field-interpretations",
        "leading-candidate-contradiction",
        "missing-discriminator",
        "natural-language-provenance",
        "open-world-refusal",
        "section-scope",
        "include-lifecycle",
        "retraction-lifecycle",
    ] {
        assert!(facets.contains(required), "missing public facet {required}");
    }
}

fn meaning_decision_name(decision: &MeaningDecision) -> &'static str {
    match decision {
        MeaningDecision::Established { .. } => "established",
        MeaningDecision::Partial { .. } => "partial",
        MeaningDecision::Ambiguous { .. } => "ambiguous",
        MeaningDecision::Conflicting { .. } => "conflicting",
        MeaningDecision::Unsupported { .. } => "unsupported",
    }
}

fn authoring_disposition_name(disposition: crate::MathAuthoringDisposition) -> &'static str {
    match disposition {
        crate::MathAuthoringDisposition::Established => "established",
        crate::MathAuthoringDisposition::Conventional => "conventional",
        crate::MathAuthoringDisposition::Partial => "partial",
        crate::MathAuthoringDisposition::Ambiguous => "ambiguous",
        crate::MathAuthoringDisposition::Conflicting => "conflicting",
        crate::MathAuthoringDisposition::Unsupported => "unsupported",
        crate::MathAuthoringDisposition::EngineLimited => "engine-limited",
    }
}

fn interpretation_support_name(support: crate::MathInterpretationSupportTier) -> &'static str {
    match support {
        crate::MathInterpretationSupportTier::Explicit => "explicit",
        crate::MathInterpretationSupportTier::Derived => "derived",
        crate::MathInterpretationSupportTier::Supported => "supported",
        crate::MathInterpretationSupportTier::Tentative => "tentative",
        crate::MathInterpretationSupportTier::Contradicted => "contradicted",
    }
}

fn interpretation_provenance_name(
    provenance: crate::MathInterpretationEvidenceProvenance,
) -> &'static str {
    match provenance {
        crate::MathInterpretationEvidenceProvenance::ExplicitDeclaration => "explicit-declaration",
        crate::MathInterpretationEvidenceProvenance::TypedStructure => "typed-structure",
        crate::MathInterpretationEvidenceProvenance::NaturalLanguageExtraction => {
            "natural-language-extraction"
        }
        crate::MathInterpretationEvidenceProvenance::DomainContext => "domain-context",
        crate::MathInterpretationEvidenceProvenance::ReviewedConvention => "reviewed-convention",
        crate::MathInterpretationEvidenceProvenance::DerivedEvidence => "derived-evidence",
    }
}

fn interpretation_evidence_role_name(role: crate::MathInterpretationEvidenceRole) -> &'static str {
    match role {
        crate::MathInterpretationEvidenceRole::Supporting => "supporting",
        crate::MathInterpretationEvidenceRole::Contradicting => "contradicting",
    }
}

fn authoring_requirement_name(requirement: &crate::MathInterpretationRequirementInfo) -> &str {
    match requirement {
        crate::MathInterpretationRequirementInfo::Declaration { requirement_id, .. }
        | crate::MathInterpretationRequirementInfo::RoleDeclaration { requirement_id, .. }
        | crate::MathInterpretationRequirementInfo::Condition { requirement_id, .. }
        | crate::MathInterpretationRequirementInfo::Disambiguation { requirement_id, .. } => {
            requirement_id
        }
    }
}

fn snapshot(content: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION,
        epoch: "project:1".into(),
        inventory_version: 1,
        project_id: "project".into(),
        main_file_id: Some("main".into()),
        documents: vec![document("main", "main.tex", content, 1)],
    }
}

fn semantic_view_at(content: &str, offset: u32) -> crate::SemanticViewInfo {
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    *view
}

fn assert_retracted_formula_surfaces(
    engine: &SemathEngine,
    content: &str,
    inventory_version: u64,
    document_version: u64,
) {
    let offset = content.find("V=IR").expect("formula") as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            inventory_version,
            document_version,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(view.authoring_context.lifecycle.retracted, "{view:#?}");
    assert!(!view.authoring_context.lifecycle.editable, "{view:#?}");

    for surface in [
        Query::Definition {
            file_id: "main".into(),
            offset,
        },
        Query::References {
            file_id: "main".into(),
            offset,
            include_declaration: true,
        },
        Query::PrepareRename {
            file_id: "main".into(),
            offset,
        },
        Query::Rename {
            file_id: "main".into(),
            offset,
            new_name: "W".into(),
        },
    ] {
        let result = engine
            .query(query(surface, inventory_version, document_version))
            .unwrap();
        match result.value {
            QueryValue::Locations {
                authorization,
                locations,
            } => {
                assert!(matches!(
                    authorization,
                    crate::EntitySurfaceAuthorization::Refused { .. }
                ));
                assert!(locations.is_empty());
            }
            QueryValue::RenamePreparation {
                authorization,
                range,
                placeholder,
            } => {
                assert!(matches!(
                    authorization,
                    crate::EntitySurfaceAuthorization::Refused { .. }
                ));
                assert!(range.is_none());
                assert!(placeholder.is_none());
            }
            QueryValue::EditProposal {
                authorization,
                proposal,
            } => {
                assert!(matches!(
                    authorization,
                    crate::EntitySurfaceAuthorization::Refused { .. }
                ));
                assert!(proposal.is_none());
            }
            value => panic!("unexpected entity surface: {value:#?}"),
        }
    }
}

#[test]
fn directly_withdrawn_archival_formula_is_retracted_and_noneditable() {
    let content = "The relation displayed next is withdrawn and retained only as an archival quotation.\n\\[V=IR\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    assert_retracted_formula_surfaces(&engine, content, 1, 1);
}

#[test]
fn passively_unused_formula_is_retracted_and_noneditable() {
    for content in [
        "This formula is not being used: $V=IR$.",
        "This formula must not be used: $V=IR$.",
        "Do not use $V=IR$.",
        "Do not apply $V=IR$.",
        "Cannot apply $V=IR$.",
        "$V=IR$ is not used.",
        "The report is not final but does not use $V=IR$.",
        "The report does not use $V=IR$.",
        "The report did not use $V=IR$.",
        "The report does not publish $V=IR$.",
        "The report does not assert $V=IR$.",
        "The report does not accept $V=IR$.",
        "The report does not select $V=IR$.",
        "We never use $V=IR$.",
        "Never use $V=IR$.",
        "We cannot use $V=IR$.",
        "We must not use $V=IR$.",
        "We should not publish $V=IR$.",
        "The report cannot use $V=IR$.",
        "This formula is never used: $V=IR$.",
        "This formula has not been used: $V=IR$.",
        "The report is not final, but $V=IR$ is not used.",
        "The following formula has been withdrawn: $V=IR$.",
        "The following formula was rejected: $V=IR$.",
    ] {
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        assert_retracted_formula_surfaces(&engine, content, 1, 1);
    }
}

#[test]
fn incremental_withdrawal_retracts_prior_formula_authority() {
    let before = "Let $V$ be voltage, $I$ current, and $R$ resistance. The circuit adopts Ohm's law.\n\\[V=IR\\]";
    let after = "The relation displayed next is withdrawn and retained only as an archival quotation.\n\\[V=IR\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(before)).unwrap();
    engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", after, 2)),
            }],
        })
        .unwrap();

    assert_retracted_formula_surfaces(&engine, after, 2, 2);
}

#[test]
fn a_withdrawal_targets_only_the_nearest_formula() {
    let content =
        "This relation is withdrawn: $V=IR$, while another relation remains active: $P=VI$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let lifecycle = |needle: &str| {
        let offset = content.find(needle).expect("formula") as u32;
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        view.authoring_context.lifecycle
    };

    assert!(lifecycle("V=IR").retracted);
    assert!(!lifecycle("P=VI").retracted);
}

#[test]
fn an_ordinal_withdrawal_does_not_guess_between_multiple_displays() {
    let content = "The second relation is withdrawn.\n\\[V=IR\\]\n\\[P=VI\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let offset = content.find("V=IR").expect("first formula") as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert!(!view.authoring_context.lifecycle.retracted, "{view:#?}");
}

#[test]
fn an_ordinary_negative_property_does_not_retract_a_nearby_formula() {
    for content in [
        "The controller is not stable.\n\\[V=IR\\]",
        "The state is not uniform when $Q=Av$.",
        "The ignored sensor is not stable near $V=IR$.",
        "The fallback channel is unavailable because the controller is not stable.\n\\[V=IR\\]",
        "The report is not silent but publishes $V=IR$.",
        "The draft is not incomplete and explicitly asserts $V=IR$.",
        "The report is not assertive near $V=IR$.",
        "The equation editor is unavailable beside $V=IR$.",
        "The equation is not smooth but rejection-prone near $V=IR$.",
        "The editor for the equation is unavailable beside $V=IR$.",
        "The backup sensor is not used near $V=IR$.",
        "The backup sensor is not used: $V=IR$ remains active.",
        "The fallback rule does not apply: $V=IR$ remains active.",
        "This formula is not stable but rejected input is recorded near $V=IR$.",
        "The following formula is not smooth while the backup sensor is not used near $V=IR$.",
        "The report is not final, but $V=IR$ remains active, and the backup sensor is not used.",
        "This formula is not used for calibration: $V=IR$ remains active.",
        "This formula is not used during calibration: $V=IR$ remains active.",
        "$V=IR$ is not used by the backup sensor.",
        "This formula is not published in the appendix: $V=IR$ remains active.",
        "$V=IR$ is not selected for the plot.",
        "This formula is unavailable in the mobile editor but remains valid: $V=IR$.",
        "$V=IR$ is unavailable in the appendix but valid elsewhere.",
        "The editor for the high resolution equation is unavailable: $V=IR$.",
        "We do not publish in the supplementary appendix the formula: $V=IR$.",
        "The editor concerning the equation is unavailable: $V=IR$.",
        "The editor regarding the formula is unavailable: $V=IR$.",
        "The editor handling the formula is unavailable: $V=IR$ remains valid.",
        "The report does not publish. $V=IR$ remains active.",
        "The report does not publish; $V=IR$ remains active.",
        "The report does not publish—$V=IR$ remains active.",
        "The report does not publish, $V=IR$ remains active.",
    ] {
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let formula = if content.contains("V=IR") {
            "V=IR"
        } else {
            "Q=Av"
        };
        let offset = content.find(formula).expect("formula") as u32;
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };

        assert!(
            !view.authoring_context.lifecycle.retracted,
            "{content}: {view:#?}"
        );
    }
}

#[test]
fn a_withdrawn_display_does_not_retract_the_next_display() {
    let content = "The relation displayed next is withdrawn.\n\\[V=IR\\]\n\\[P=VI\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let offset = content.find("P=VI").expect("second formula") as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert!(!view.authoring_context.lifecycle.retracted, "{view:#?}");
}

#[test]
fn unselected_alternative_shape_roots_do_not_emit_a_conflict_diagnostic() {
    let content = "Candidate amber defines $w$ as a vector. Candidate cobalt defines $w$ as a square matrix. Neither candidate is selected.\n\\[w=Az\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = result.value else {
        panic!("expected diagnostics")
    };

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "notation-shape-conflict"),
        "{diagnostics:#?}"
    );
}

#[test]
fn unrelated_selection_refusal_does_not_hide_a_shape_conflict() {
    let content = "Candidate amber defines $w$ as a vector. Candidate cobalt defines $w$ as a square matrix. The renderer does not select a color profile.\n\\[w=Az\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = result.value else {
        panic!("expected diagnostics")
    };

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "notation-shape-conflict"),
        "{diagnostics:#?}"
    );
}

#[test]
fn partially_unselected_shape_roots_still_expose_a_conflict() {
    let content = "Candidate amber defines $w$ as a vector. Candidate cobalt defines $w$ as a square matrix. One candidate is not selected.\n\\[w=Az\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = result.value else {
        panic!("expected diagnostics")
    };

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "notation-shape-conflict"),
        "{diagnostics:#?}"
    );
}

#[test]
fn qualified_unselection_does_not_hide_a_shape_conflict() {
    for selection in [
        "No candidate is not selected.",
        "No candidate is selected automatically.",
    ] {
        let content = format!(
            "Candidate amber defines $w$ as a vector. Candidate cobalt defines $w$ as a square matrix. {selection}\n\\[w=Az\\]"
        );
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(&content)).unwrap();
        let result = engine
            .query(query(
                Query::Diagnostics {
                    file_id: "main".into(),
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::Diagnostics { diagnostics } = result.value else {
            panic!("expected diagnostics")
        };

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "notation-shape-conflict"),
            "{selection}: {diagnostics:#?}"
        );
    }
}

#[test]
fn chained_equality_stores_operands_and_source_linked_relations_without_a_system_placeholder() {
    let mut engine = SemathEngine::default();
    let update = engine.reset(snapshot("$a=b=c$")).unwrap();

    assert_eq!(update.stats.semantic_entities, 5);
}

#[test]
fn wide_variadic_relation_is_bounded_without_rejecting_the_snapshot() {
    let operands = |count, separator| {
        (0..count)
            .map(|index| format!("x_{{{index}}}"))
            .collect::<Vec<_>>()
            .join(separator)
    };
    let variadic_relations = [
        ("31-term sum", operands(31, "+")),
        ("32-term sum", operands(32, "+")),
        ("31-factor product", operands(31, "\\cdot ")),
        ("32-factor product", operands(32, "\\cdot ")),
        (
            "30-argument application",
            format!("f({})", operands(30, ",")),
        ),
        (
            "31-argument application",
            format!("f({})", operands(31, ",")),
        ),
    ];
    for (label, right) in variadic_relations {
        let formula = format!("r={right}");
        let content = format!("The candidate equality ${formula}$ is not asserted.");

        let mut engine = SemathEngine::default();
        engine
            .reset(snapshot(&content))
            .unwrap_or_else(|error| panic!("{label} rejected: {error:?}"));
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: (content.find(&formula).expect("formula exists") + formula.len())
                        as u32,
                },
                1,
                1,
            ))
            .expect("the bounded formula remains queryable");
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert!(
            matches!(view.decision, MeaningDecision::Unsupported { .. }),
            "{label} must remain unsupported: {:#?}",
            view.decision
        );
    }
}

#[test]
fn chained_metric_formula_does_not_store_relation_placeholder_entities() {
    let named = "Let p denote the probability assigned to event A.\nExpected calibration error (ECE) uses confidence bins $B_m$.\n$p=\\operatorname{ECE}=\\sum_{m=1}^{M}\\frac{|B_m|}{n}\\left|\\operatorname{acc}(B_m)-\\operatorname{conf}(B_m)\\right|$";
    let expanded = "Let p denote the probability assigned to event A.\nExpected calibration error (ECE) uses confidence bins $B_m$.\n$p=\\operatorname{E C E}=\\sum_{m=1}^{M}\\frac{|B_m|}{n}\\left|\\operatorname{acc}(B_m)-\\operatorname{conf}(B_m)\\right|$";

    let named_stats = SemathEngine::default()
        .reset(snapshot(named))
        .unwrap()
        .stats;
    let expanded_stats = SemathEngine::default()
        .reset(snapshot(expanded))
        .unwrap()
        .stats;

    assert!(
        named_stats.semantic_entities < expanded_stats.semantic_entities,
        "a Roman named operator is one entity rather than three adjacent factors"
    );
    assert_eq!(
        named_stats.semantic_entities + 4,
        expanded_stats.semantic_entities,
        "the relation remains placeholder-free and the sum index is one scoped binder entity"
    );
}

fn query(query: Query, inventory_version: u64, document_version: u64) -> QueryEnvelope {
    QueryEnvelope {
        protocol_version: PROTOCOL_VERSION,
        epoch: "project:1".into(),
        inventory_version,
        document_version,
        analysis_generation: inventory_version,
        query,
    }
}

fn range(start_offset: u32, end_offset: u32) -> SourceRange {
    SourceRange {
        start_offset,
        end_offset,
    }
}

#[test]
fn relation_focus_selects_the_exact_system_child_without_edge_guessing() {
    fn symbol(name: &str, start: u32) -> SemanticExpr {
        SemanticExpr {
            kind: SemanticExprKind::Symbol(name.into()),
            range: range(start, start + 1),
            provenance: Vec::new(),
        }
    }
    fn relation(left: &str, right: &str, start: u32) -> SemanticExpr {
        SemanticExpr {
            kind: SemanticExprKind::Relation {
                operator: SemanticReference::new("equals", range(start + 1, start + 2), Vec::new()),
                left: Box::new(symbol(left, start)),
                right: Box::new(symbol(right, start + 2)),
            },
            range: range(start, start + 3),
            provenance: Vec::new(),
        }
    }
    let root = SemanticExpr {
        kind: SemanticExprKind::System(vec![relation("a", "b", 10), relation("y", "x", 20)]),
        range: range(10, 23),
        provenance: Vec::new(),
    };
    let math_range = root.range.clone();
    let y_start = 20;
    let y_range = range(y_start, y_start + 1);

    let selected = relation_expression_at_cursor(
        std::slice::from_ref(&root),
        &document("main", "main.tex", "          a=b       y=x", 1),
        &math_range,
        Some(&y_range),
        y_start,
    )
    .expect("the focused relation");
    assert_eq!(
        relation_head(selected).map(|(name, _)| name),
        Some("y".into())
    );

    let trailing = relation_expression_at_cursor(
        std::slice::from_ref(&root),
        &document("main", "main.tex", "          a=b       y=x", 1),
        &math_range,
        None,
        math_range.end_offset,
    )
    .expect("the trailing relation");
    assert_eq!(
        relation_head(trailing).map(|(name, _)| name),
        Some("y".into())
    );
}

#[test]
fn relation_focus_refuses_unowned_gaps_in_a_math_region() {
    let relation = SemanticExpr {
        kind: SemanticExprKind::Relation {
            operator: SemanticReference::new("equals", range(11, 12), Vec::new()),
            left: Box::new(SemanticExpr {
                kind: SemanticExprKind::Symbol("a".into()),
                range: range(10, 11),
                provenance: Vec::new(),
            }),
            right: Box::new(SemanticExpr {
                kind: SemanticExprKind::Symbol("b".into()),
                range: range(12, 13),
                provenance: Vec::new(),
            }),
        },
        range: range(10, 13),
        provenance: Vec::new(),
    };
    let math_range = range(0, 30);
    let source = document("main", "main.tex", "                              ", 1);

    assert!(
        relation_expression_at_cursor(
            std::slice::from_ref(&relation),
            &source,
            &math_range,
            None,
            5,
        )
        .is_none()
    );
    assert!(
        relation_expression_at_cursor(
            std::slice::from_ref(&relation),
            &source,
            &math_range,
            None,
            20,
        )
        .is_none()
    );
    assert!(
        relation_expression_at_cursor(
            std::slice::from_ref(&relation),
            &source,
            &math_range,
            None,
            29,
        )
        .is_none()
    );

    let punctuation = document("main", "main.tex", "          a=b .               ", 1);
    assert_eq!(
        relation_expression_at_cursor(
            std::slice::from_ref(&relation),
            &punctuation,
            &math_range,
            None,
            15,
        ),
        Some(&relation),
    );
}

#[test]
fn expands_a_style_body_to_its_exact_source_notation() {
    let mut input = document("main", "main.tex", "$\\mathbf{y}$", 1);
    input.nodes.push(NotationNode {
        kind: NotationNodeKind::Style,
        parent: None,
        children: Vec::new(),
        ranges: NotationNodeRanges {
            full: range(1, 11),
            command: Some(range(1, 8)),
            name: None,
            nucleus: None,
            editable: Some(range(9, 10)),
        },
        state: SyntaxState::Complete,
        name: Some("mathbf".into()),
        text: None,
        arguments: vec![NotationArgument {
            node: 0,
            role: "body".into(),
            syntax: "required".into(),
            range: range(9, 10),
        }],
        lexical_class: None,
        math_class: None,
        provenance: None,
    });
    assert_eq!(
        notation_occurrence_range(&input, &range(9, 10)),
        range(1, 11)
    );
}

#[test]
fn resolves_definition_on_both_edges_of_a_symbol() {
    let content = "Let $A$ denote an event. Let $B$ denote an event. $p=\\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}$";
    let occurrence = content.find("A \\cap").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    for offset in [occurrence, occurrence + 1] {
        let result = engine
            .query(query(
                Query::Definition {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::Locations { locations, .. } = result.value else {
            panic!("expected locations")
        };
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start_offset, 5);
    }
}

#[test]
fn differential_variable_owns_both_cursor_edges_inside_the_composite() {
    let content = "Let $x$ denote position. In $\\frac{dx}{dt}$, inspect $dx$.";
    let start = content.find("dx}{dt}").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    for offset in [start, start + 1] {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        let symbol = view.symbol.expect("differential variable focus");
        assert_eq!(symbol.symbol, "x");
        assert_eq!(symbol.source_notation, "x");
    }
}

#[test]
fn navigation_distinguishes_an_authorized_self_definition_from_references() {
    let content = "Let $x$ denote the input.";
    let offset = content.find('x').unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    for (query_index, query_kind) in [
        Query::Definition {
            file_id: "main".into(),
            offset,
        },
        Query::References {
            file_id: "main".into(),
            offset,
            include_declaration: true,
        },
        Query::References {
            file_id: "main".into(),
            offset,
            include_declaration: false,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let result = engine.query(query(query_kind, 1, 1)).unwrap();
        let QueryValue::Locations {
            authorization,
            locations,
        } = result.value
        else {
            panic!("expected locations")
        };
        assert!(matches!(
            authorization,
            crate::EntitySurfaceAuthorization::Authorized { .. }
        ));
        assert_eq!(locations.len(), usize::from(query_index == 1));
    }
}

#[test]
fn indexed_relation_head_is_not_offered_as_a_partial_base_rename() {
    for (content, notation) in [
        ("$U_b=q_bV_b$", "U_b"),
        (
            "Let $x^\\star$ be a minimizer. $\\nabla f(x^\\star)=0$",
            "x^\\star",
        ),
    ] {
        let offset = content.rfind(notation).unwrap() as u32 + notation.len() as u32;
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();

        let result = engine
            .query(query(
                Query::PrepareRename {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::RenamePreparation {
            authorization,
            range,
            placeholder,
        } = result.value
        else {
            panic!("expected rename preparation")
        };
        assert!(range.is_none(), "{notation}");
        assert!(placeholder.is_none(), "{notation}");
        assert!(matches!(
            authorization,
            crate::EntitySurfaceAuthorization::Refused { .. }
        ));
    }
}

#[test]
fn proven_binder_component_can_be_renamed_inside_indexed_notation() {
    let content = "External $i$.\n\n$$\n\\sum_{i=1}^n x_i\n$$\n\nExternal again $i$.";
    let use_offset = content.find("x_i").unwrap() as u32 + 2;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let preparation = engine
        .query(query(
            Query::PrepareRename {
                file_id: "main".into(),
                offset: use_offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::RenamePreparation {
        authorization,
        range: preparation_range,
        placeholder,
    } = preparation.value
    else {
        panic!("expected rename preparation")
    };
    assert!(matches!(
        authorization,
        crate::EntitySurfaceAuthorization::Authorized { .. }
    ));
    assert_eq!(placeholder.as_deref(), Some("i"));
    assert_eq!(preparation_range, Some(range(use_offset, use_offset + 1)));

    let rename = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "j".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::EditProposal {
        authorization,
        proposal: Some(proposal),
    } = rename.value
    else {
        panic!("expected rename proposal")
    };
    assert!(matches!(
        authorization,
        crate::EntitySurfaceAuthorization::Authorized { .. }
    ));
    assert_eq!(proposal.files.len(), 1);
    assert_eq!(proposal.files[0].edits.len(), 2);
    assert!(
        proposal.files[0]
            .edits
            .iter()
            .all(|edit| edit.expected_text == "i" && edit.replacement_text == "j")
    );
}

#[test]
fn navigation_and_rename_share_one_established_entity() {
    let content = "Let $A$ denote an event. Let $B$ denote an event. $p=\\frac{\\mathbb{P}(A \\cap B)}{\\mathbb{P}(B)}$";
    let use_offset = content.find("A \\cap").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let references = engine
        .query(query(
            Query::References {
                file_id: "main".into(),
                offset: use_offset,
                include_declaration: true,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Locations { locations, .. } = references.value else {
        panic!("expected locations")
    };
    assert_eq!(locations.len(), 2);

    let preparation = engine
        .query(query(
            Query::PrepareRename {
                file_id: "main".into(),
                offset: use_offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::RenamePreparation {
        authorization,
        range: preparation_range,
        placeholder,
    } = preparation.value
    else {
        panic!("expected rename preparation")
    };
    assert_eq!(preparation_range, Some(range(use_offset, use_offset + 1)));
    assert_eq!(placeholder.as_deref(), Some("A"));
    assert!(matches!(
        authorization,
        crate::EntitySurfaceAuthorization::Authorized { .. }
    ));

    let rename = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "E".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::EditProposal {
        authorization,
        proposal: Some(proposal),
    } = rename.value
    else {
        panic!("expected rename proposal")
    };
    assert!(matches!(
        authorization,
        crate::EntitySurfaceAuthorization::Authorized { .. }
    ));
    assert_eq!(proposal.files.len(), 1);
    assert_eq!(proposal.files[0].edits.len(), locations.len());
    assert!(
        proposal.files[0]
            .edits
            .iter()
            .all(|edit| { edit.expected_text == "A" && edit.replacement_text == "E" })
    );
}

#[test]
fn rename_refuses_to_merge_two_entities_in_the_same_scope() {
    let content = "Let $A$ denote an event. Let $B$ denote another event. $p=A$";
    let use_offset = content.rfind('A').unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "B".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::EditProposal {
        authorization,
        proposal: None,
    } = result.value
    else {
        panic!("expected rename rejection")
    };
    let crate::EntitySurfaceAuthorization::Refused { reason } = authorization else {
        panic!("expected typed refusal")
    };
    assert_eq!(reason.kind, crate::EntitySurfaceRefusalKind::Capture);
}

#[test]
fn rename_refuses_to_capture_a_visible_outer_entity() {
    let content =
        "Let $B$ denote the outer quantity.\n# Inner\nLet $A$ denote the inner quantity. Use $A$.";
    let section = content.find("# Inner").unwrap() as u32;
    let mut input = document("main", "main.md", content, 1);
    input.language = DocumentLanguage::Markdown;
    input.math_regions = test_math_regions(content, DocumentLanguage::Markdown);
    input.scopes = vec![
        SyntaxScope {
            kind: "document".into(),
            parent: None,
            range: range(0, content.len() as u32),
            state: MathRootState::Complete,
            name: None,
            level: None,
            source: None,
        },
        SyntaxScope {
            kind: "section".into(),
            parent: Some(0),
            range: range(section, content.len() as u32),
            state: MathRootState::Complete,
            name: Some("Inner".into()),
            level: None,
            source: None,
        },
    ];
    let use_offset = content.rfind("$A$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    let mut project = snapshot(content);
    project.documents = vec![input];
    engine.reset(project).unwrap();

    let captured = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "B".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::EditProposal {
        authorization: crate::EntitySurfaceAuthorization::Refused { reason },
        proposal: None,
    } = captured.value
    else {
        panic!("expected capture refusal")
    };
    assert_eq!(reason.kind, crate::EntitySurfaceRefusalKind::Capture);

    let safe = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset: use_offset,
                new_name: "C".into(),
            },
            1,
            1,
        ))
        .unwrap();
    assert!(matches!(
        safe.value,
        QueryValue::EditProposal {
            authorization: crate::EntitySurfaceAuthorization::Authorized { .. },
            proposal: Some(_),
        }
    ));
}

#[test]
fn rename_refuses_to_capture_an_unresolved_visible_occurrence() {
    let content = "Let $A$ denote the input. Observe free $B$ and then use $A$.";
    let offset = content.rfind("$A$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Rename {
                file_id: "main".into(),
                offset,
                new_name: "B".into(),
            },
            1,
            1,
        ))
        .unwrap();
    assert!(matches!(
        result.value,
        QueryValue::EditProposal {
            authorization: crate::EntitySurfaceAuthorization::Refused {
                reason: crate::EntitySurfaceRefusal {
                    kind: crate::EntitySurfaceRefusalKind::Capture,
                    ..
                },
            },
            proposal: None,
        }
    ));
}

#[test]
fn prose_acronym_cursor_is_addressable_by_the_shared_surface_policy() {
    let content = "Expected calibration error (ECE) is the metric. Use $\\operatorname{ECE}$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let offset = content.find("(ECE)").unwrap() as u32 + 2;
    let result = engine
        .query(query(
            Query::References {
                file_id: "main".into(),
                offset,
                include_declaration: true,
            },
            1,
            1,
        ))
        .unwrap();
    assert!(matches!(
        result.value,
        QueryValue::Locations {
            authorization: crate::EntitySurfaceAuthorization::Authorized { .. },
            locations,
        } if !locations.is_empty()
    ));
}

#[test]
fn exact_occurrence_range_outranks_a_structural_selection_alias() {
    let exact_id = SourceOccurrenceId {
        file_id: "main".into(),
        document_version: 1,
        local_id: 0,
    };
    let container_id = SourceOccurrenceId {
        file_id: "main".into(),
        document_version: 1,
        local_id: 1,
    };
    let exact_range = range(1, 2);
    let occurrences = vec![
        SourceOccurrence {
            id: exact_id.clone(),
            component_id: "main".into(),
            kind: OccurrenceKind::Notation,
            range: exact_range.clone(),
            selection_range: exact_range.clone(),
            scope_path: Vec::new(),
            structural_path: Vec::new(),
            availability_order: 1,
            surface: "P".into(),
            source_text: "P".into(),
            selection_text: "P".into(),
            notation: Vec::new(),
        },
        SourceOccurrence {
            id: container_id.clone(),
            component_id: "main".into(),
            kind: OccurrenceKind::Notation,
            range: range(1, 4),
            selection_range: exact_range.clone(),
            scope_path: Vec::new(),
            structural_path: Vec::new(),
            availability_order: 1,
            surface: "P_s".into(),
            source_text: "P_s".into(),
            selection_text: "P".into(),
            notation: Vec::new(),
        },
    ];
    let mut index = HashMap::new();
    index_occurrence_range(&mut index, ("main".into(), 1, 2), exact_id.clone());
    index_occurrence_range(&mut index, ("main".into(), 1, 2), container_id);

    assert_eq!(
        occurrence_id_at_range(&index, &occurrences, "main", &exact_range),
        Some(exact_id),
    );
}

#[test]
fn complete_indexed_notation_owns_its_shared_right_edge() {
    let content = "$P_s$";
    let mut input = document("main", "main.tex", content, 1);
    input.nodes = vec![
        NotationNode {
            kind: NotationNodeKind::Token,
            parent: Some(2),
            children: Vec::new(),
            ranges: NotationNodeRanges {
                full: range(1, 2),
                command: None,
                name: None,
                nucleus: None,
                editable: Some(range(1, 2)),
            },
            state: SyntaxState::Complete,
            name: None,
            text: Some("P".into()),
            arguments: Vec::new(),
            lexical_class: Some(LexicalClass::Identifier),
            math_class: None,
            provenance: None,
        },
        NotationNode {
            kind: NotationNodeKind::Token,
            parent: Some(2),
            children: Vec::new(),
            ranges: NotationNodeRanges {
                full: range(3, 4),
                command: None,
                name: None,
                nucleus: None,
                editable: Some(range(3, 4)),
            },
            state: SyntaxState::Complete,
            name: None,
            text: Some("s".into()),
            arguments: Vec::new(),
            lexical_class: Some(LexicalClass::Identifier),
            math_class: None,
            provenance: None,
        },
        NotationNode {
            kind: NotationNodeKind::Script,
            parent: None,
            children: vec![0, 1],
            ranges: NotationNodeRanges {
                full: range(1, 4),
                command: None,
                name: None,
                nucleus: Some(range(1, 2)),
                editable: Some(range(1, 4)),
            },
            state: SyntaxState::Complete,
            name: Some("subscript".into()),
            text: None,
            arguments: Vec::new(),
            lexical_class: None,
            math_class: None,
            provenance: None,
        },
    ];
    input.math_roots = vec![MathRoot {
        node: 2,
        delimiter: "inline-dollar".into(),
        full_range: range(0, 5),
        content_range: range(1, 4),
        state: MathRootState::Complete,
    }];
    input.scopes = vec![SyntaxScope {
        kind: "document".into(),
        parent: None,
        range: range(0, 5),
        state: MathRootState::Complete,
        name: None,
        level: None,
        source: None,
    }];
    let mut engine = SemathEngine::default();
    let mut project = snapshot(content);
    project.documents = vec![input];
    engine.reset(project).unwrap();
    let mut occurrence_ids = Vec::new();
    for offset in [1, 4] {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert_eq!(
            view.symbol.as_ref().map(|symbol| symbol.symbol.as_str()),
            Some("P_s")
        );
        let symbol = view.symbol.expect("expected indexed symbol focus");
        assert_eq!(symbol.location.range, range(1, 4));
        occurrence_ids.push(symbol.occurrence_id);
    }
    assert_eq!(occurrence_ids[0], occurrence_ids[1]);
}

#[test]
fn projects_vector_shape_through_a_trajectory_derivative() {
    let content =
        "Let $x(t)$ be an n-dimensional state vector. Inspect its derivative $\\dot{x}(t)$.";
    let offset = content.find("{x}").unwrap() as u32 + 2;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let symbol = view.symbol.expect("expected derivative symbol information");
    assert!(
        symbol.shapes.iter().any(|shape| shape.kind == "vector"),
        "expected a propagated vector shape; symbol={symbol:?}",
    );
}

#[test]
fn coalesces_overlapping_prose_definitions_into_one_entity() {
    let content = "The declarations $x_r\\in\\mathbb R^n$, $u_r\\in\\mathbb R^m$, $A_r\\in\\mathbb R^{n\\times n}$, and $B_r\\in\\mathbb R^{n\\times m}$ apply throughout.\nLet $x_r$, $A_r$, $B_r$, and $u_r$ denote state vector, state matrix, input matrix, and control input vector, respectively.\n\\[\\dot{x_r} = A_r{x_r}+B_r{u_r}\\]";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
}

#[test]
fn semantic_view_explains_a_typed_law_without_exposing_an_ast() {
    let content =
        "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\mathbf{F}\\cdot\\mathbf{v}$";
    let offset = content.rfind("P=").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(matches!(
        &view.decision,
        MeaningDecision::Established { reasons, .. } if !reasons.is_empty()
    ));
    assert_eq!(
        view.context.relations[0].relation_id,
        "classical-mechanics:mechanical-power"
    );
    assert_eq!(
        view.context.relations[0]
            .roles
            .iter()
            .map(|role| role.role.as_str())
            .collect::<Vec<_>>(),
        ["power", "force", "velocity"],
    );
    let authoring = &view.authoring_context;
    assert_eq!(
        authoring.disposition,
        crate::MathAuthoringDisposition::Established
    );
    let anchor = authoring.formula.as_ref().expect("exact formula anchor");
    assert_eq!(anchor.document_version, 1);
    assert_eq!(anchor.source_notation, "P=\\mathbf{F}\\cdot\\mathbf{v}");
    assert_eq!(anchor.location.path, "main.tex");
    assert_eq!(
        authoring.lifecycle.generation,
        crate::MathSourceGeneration::Authored
    );
    assert_eq!(
        authoring.lifecycle.freshness,
        crate::MathSourceFreshness::Current
    );
    assert!(authoring.lifecycle.editable);
    assert!(!authoring.truncated);
    assert_eq!(
        authoring.interpretations.exhaustiveness,
        crate::MathInterpretationExhaustiveness::BoundedOpenWorld
    );
    let hypothesis = authoring
        .interpretations
        .hypotheses
        .first()
        .expect("source-grounded law hypothesis");
    assert_eq!(
        hypothesis.hypothesis_id,
        "classical-mechanics:mechanical-power"
    );
    assert_eq!(
        hypothesis.support,
        crate::MathInterpretationSupportTier::Derived
    );
    assert_eq!(hypothesis.formula.as_ref(), authoring.formula.as_ref());
    assert!(hypothesis.evidence.iter().all(|item| {
        !item.evidence.source_ranges.is_empty()
            && item.role == crate::MathInterpretationEvidenceRole::Supporting
    }));
}

#[test]
fn semantic_view_exposes_conventional_notation_as_a_bounded_non_authoritative_candidate() {
    let content = "For a periodic signal, the asserted relation is $f=1/T$.";
    let offset = content.rfind("f=").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert!(matches!(view.decision, MeaningDecision::Partial { .. }));
    assert!(view.diagnostics.is_empty());
    let candidate = view
        .authoring_context
        .conventional_candidates
        .first()
        .expect("period-frequency convention candidate");
    assert_eq!(candidate.law_id, "period-frequency-reciprocity");
    assert_eq!(
        candidate.disposition,
        crate::ConventionalCandidateDisposition::ConventionalCandidate
    );
    assert!(candidate.requirements.iter().any(|requirement| matches!(
        requirement,
        crate::ConventionalRequirementInfo::RoleDeclaration { parameter, constraint, .. }
            if parameter == "frequency"
                && constraint.concepts == ["signals-systems:cyclic-frequency"]
    )));
    assert!(candidate.requirements.iter().any(|requirement| matches!(
        requirement,
        crate::ConventionalRequirementInfo::Condition { condition, .. }
            if condition.condition_id == "positive-period"
    )));
    assert!(candidate.evidence.iter().any(|evidence| {
        evidence.kind == "prose-domain-prior" && !evidence.source_ranges.is_empty()
    }));
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conventional
    );
    assert!(
        view.authoring_context
            .requirements
            .iter()
            .any(|requirement| {
                matches!(
                    requirement,
                    crate::MathInterpretationRequirementInfo::RoleDeclaration { parameter, .. }
                        if parameter == "frequency"
                )
            })
    );
    assert!(view.authoring_context.conditions.iter().any(|condition| {
        condition.condition_id == "positive-period"
            && condition.status == crate::ConstraintStatus::Required
    }));
    let hypothesis = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .find(|hypothesis| hypothesis.kind == crate::MathInterpretationKind::ReviewedConvention)
        .expect("reviewed convention remains a distinct hypothesis");
    assert_eq!(
        hypothesis.support,
        crate::MathInterpretationSupportTier::Tentative
    );
    assert!(
        hypothesis
            .missing_discriminator_ids
            .iter()
            .any(|id| id == "period-frequency-reciprocity/binding/frequency")
    );
    assert!(hypothesis.evidence.iter().any(|item| {
        item.provenance == crate::MathInterpretationEvidenceProvenance::DomainContext
    }));

    let definition = engine
        .query(query(
            Query::Definition {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Locations {
        authorization,
        locations,
    } = definition.value
    else {
        panic!("expected definition result")
    };
    assert!(matches!(
        authorization,
        crate::EntitySurfaceAuthorization::Refused { .. }
    ));
    assert!(locations.is_empty());
}

#[test]
fn removing_domain_context_retracts_the_conventional_candidate() {
    let content = "The asserted relation is $f=1/T$.";
    let offset = content.rfind("f=").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.authoring_context.conventional_candidates.is_empty(),
        "{view:#?}"
    );
}

#[test]
fn math_authoring_context_preserves_approximation_without_equality_authority() {
    let content = "The numerical approximation is $u_h\\approx u$.";
    let offset = content.find("u_h").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let approximation = view
        .authoring_context
        .approximation
        .expect("approximation disposition");
    assert_eq!(approximation.exactness, crate::MathExactness::Approximate);
    assert!(approximation.evidence.iter().any(|evidence| {
        evidence.rule_id == "semath/canonical-approximation" || !evidence.source_ranges.is_empty()
    }));
    assert!(!matches!(
        view.decision,
        MeaningDecision::Established { .. }
    ));
}

#[test]
fn conventional_candidates_cover_representative_stem_notation_families() {
    for (source, needle, law_id) in [
        (
            "The asserted linear algebra relation is $A x=b$.",
            "A x=b",
            "matrix-vector-product",
        ),
        (
            "The asserted probability relation is $\\mu=E(X)$.",
            "\\mu=E(X)",
            "expected-value-definition",
        ),
        (
            "The asserted classical mechanics relation is $F=ma$.",
            "F=ma",
            "newton-second-law",
        ),
        (
            "The asserted electromagnetism relation is $F=qE$.",
            "F=qE",
            "electric-force-law",
        ),
        (
            "The asserted thermodynamics relation is $PV=nRT$.",
            "PV=nRT",
            "ideal-gas",
        ),
    ] {
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(source)).unwrap();
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: source.find(needle).unwrap() as u32,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert!(
            view.authoring_context
                .conventional_candidates
                .iter()
                .any(|candidate| candidate.law_id == law_id),
            "missing {law_id} for {source}: {view:#?}"
        );
        assert!(view.diagnostics.is_empty(), "{source}: {view:#?}");
        assert!(!matches!(
            view.decision,
            MeaningDecision::Established { .. }
        ));
    }
}

#[test]
fn field_context_alone_does_not_authorize_a_complete_looking_law() {
    let source = "The altered expression does not establish the reviewed probability relation. $c=\\operatorname{Cov}(X,Z)$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(source)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: source.find("c=").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert!(!matches!(
        view.decision,
        MeaningDecision::Established { .. }
    ));
    assert!(
        view.context
            .relations
            .iter()
            .all(|relation| { relation.relation_id != "probability:covariance-value-definition" })
    );
}

#[test]
fn rejected_or_undeclared_formulae_never_export_authoritative_relations() {
    for (content, needle, excluded_relation) in [
        (
            "The worksheet lists $F=ma$ only as a rejected candidate; this analysis does not use that equation.",
            "F=ma",
            "classical-mechanics:newton-second-law",
        ),
        (
            "Do not apply Ohm's law in this nonlinear device. The archived comparison is $V=RI$.",
            "V=RI",
            "circuits:ohm-law",
        ),
        (
            "For comparison only, the report mentions $K=\\frac12mv^2$ but does not adopt the kinetic-energy model.",
            "K=\\frac12mv^2",
            "classical-mechanics:kinetic-energy-definition",
        ),
        (
            "This analysis does not use the ideal-gas model. The archived comparison is $PV=nRT$.",
            "PV=nRT",
            "thermodynamics:ideal-gas",
        ),
        (
            "The note does not adopt a heat-work sign convention and does not use the closed-system balance $\\Delta U=Q-W$.",
            "\\Delta U=Q-W",
            "thermodynamics:closed-system-first-law",
        ),
        (
            "The update is unusable because the state and gradient shapes conflict; do not apply $x_{k+1}=x_k-\\eta g_k$.",
            "x_{k+1}=x_k-\\eta g_k",
            "optimization-ml:gradient-descent-update",
        ),
        (
            "Without a consistent electrical reference convention, this analysis does not use $P=VI$.",
            "P=VI",
            "electromagnetism:electric-power-law",
        ),
        (
            "The archived ideal-gas relation $PV=nRT$ is invalid for this analysis.",
            "PV=nRT",
            "thermodynamics:ideal-gas",
        ),
        (
            "The model forbids the electric-power formula $P=VI$.",
            "P=VI",
            "electromagnetism:electric-power-law",
        ),
        (
            "The closed-system balance $\\Delta U=Q-W$ is unavailable for this open system.",
            "\\Delta U=Q-W",
            "thermodynamics:closed-system-first-law",
        ),
        (
            "The gradient update $x_{k+1}=x_k-\\eta g_k$ must be discarded.",
            "x_{k+1}=x_k-\\eta g_k",
            "optimization-ml:gradient-descent-update",
        ),
        (
            "The Newton equation $F=ma$ is excluded from this model.",
            "F=ma",
            "classical-mechanics:newton-second-law",
        ),
    ] {
        let view = semantic_view_at(
            content,
            (content.find(needle).unwrap() + needle.len()) as u32,
        );
        assert!(
            !matches!(
                view.decision,
                MeaningDecision::Established { .. } | MeaningDecision::Conflicting { .. }
            ),
            "{content}: {:?}",
            view.decision
        );
        assert!(
            view.context
                .relations
                .iter()
                .all(|relation| relation.relation_id != excluded_relation),
            "{content}: {:#?}",
            view.context.relations
        );
        assert!(view.diagnostics.is_empty(), "{content}: {view:#?}");
        assert!(
            view.authoring_context.equation_links.is_empty(),
            "{content}: {view:#?}"
        );
    }
}

#[test]
fn a_rejected_formula_cannot_authorize_navigation_or_editing() {
    let content = "The model forbids the electric-power formula $P=VI$.";
    let offset = content.find("P=VI").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    for query_kind in [
        Query::Definition {
            file_id: "main".into(),
            offset,
        },
        Query::References {
            file_id: "main".into(),
            offset,
            include_declaration: true,
        },
        Query::PrepareRename {
            file_id: "main".into(),
            offset,
        },
        Query::Rename {
            file_id: "main".into(),
            offset,
            new_name: "S".into(),
        },
    ] {
        let result = engine.query(query(query_kind, 1, 1)).unwrap();
        let authorization = match result.value {
            QueryValue::Locations { authorization, .. }
            | QueryValue::RenamePreparation { authorization, .. }
            | QueryValue::EditProposal { authorization, .. } => authorization,
            _ => panic!("expected an entity surface result"),
        };
        assert!(matches!(
            authorization,
            crate::EntitySurfaceAuthorization::Refused { .. }
        ));
    }
}

#[test]
fn an_outstanding_declaration_cannot_establish_a_bare_symbol() {
    for (content, needle) in [
        (
            "The undecided notation $T$ still requires a declaration; do not assign it a physical role.",
            "T",
        ),
        (
            "The audit records the undeclared expression $w=q^3$ without assigning it a meaning.",
            "w=q^3",
        ),
    ] {
        let view = semantic_view_at(
            content,
            (content.find(needle).unwrap() + needle.len()) as u32,
        );
        assert!(
            !matches!(view.decision, MeaningDecision::Established { .. }),
            "{content}: {:?}",
            view.decision
        );
        assert!(
            view.context.relations.is_empty(),
            "{content}: {:#?}",
            view.context.relations
        );
    }
}

#[test]
fn an_established_equation_routes_only_later_formulas_in_the_same_scope() {
    let source = "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\mathbf{F}\\cdot\\mathbf{v}$ Then inspect $z$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(source)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: source.rfind('z').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let mechanics = view
        .domains
        .first()
        .expect("the established equation routes its field forward");
    assert_eq!(mechanics.pack_id, "classical-mechanics");
    assert_eq!(mechanics.support, crate::DomainSupportTier::Supported);
    assert!(mechanics.evidence.iter().any(|evidence| {
        evidence.kind == "canonical-math"
            && evidence
                .source_ranges
                .iter()
                .all(|range| range.end_offset <= source.rfind('z').unwrap() as u32)
    }));

    let without_equation =
        "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. Then inspect $z$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(without_equation)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: without_equation.rfind('z').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.domains
            .iter()
            .all(|domain| domain.support != crate::DomainSupportTier::Supported)
    );
}

#[test]
fn an_established_equation_routes_a_later_relation_in_the_same_math_root() {
    let source = "A 20 Hz crossover is converted as $\\omega_c=2\\pi(20\\,\\mathrm{Hz}), f=1/T$.";
    let offset = source.rfind("f=").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(source)).unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    assert!(
        view.authoring_context
            .conventional_candidates
            .iter()
            .any(|candidate| candidate.law_id == "period-frequency-reciprocity")
    );
}

#[test]
fn an_established_final_equation_still_observes_its_domain_in_later_prose() {
    let source = "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\mathbf{F}\\cdot\\mathbf{v}$ Then inspect the conclusion.";
    let offset = source.rfind("conclusion").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(source)).unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    let mechanics = view
        .domains
        .first()
        .expect("the final domain observation retains established equation evidence");
    assert_eq!(mechanics.pack_id, "classical-mechanics");
    assert_eq!(mechanics.support, crate::DomainSupportTier::Supported);
}

#[test]
fn established_equation_does_not_activate_its_own_conventional_notation() {
    let source = "A 20 Hz crossover is converted as $\\omega_c=2\\pi(20\\,\\mathrm{Hz})$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(source)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: source.find("\\omega_c").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established
    );
    assert!(view.authoring_context.conventional_candidates.is_empty());
}

#[test]
fn established_equation_routes_a_later_formula_identically_after_incremental_upsert() {
    let original = "A 20 Hz crossover is converted as $\\omega_c=2\\pi(20\\,\\mathrm{Hz})$.";
    let changed = "A 20 Hz crossover is converted as $\\omega_c=2\\pi(20\\,\\mathrm{Hz})$. The asserted relation is $f=1/T$.";
    let offset = changed.rfind("f=").unwrap() as u32;

    let mut incremental = SemathEngine::default();
    incremental.reset(snapshot(original)).unwrap();
    incremental
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", changed, 2)),
            }],
        })
        .unwrap();
    let QueryValue::SemanticView { view: incremental } = incremental
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            2,
            2,
        ))
        .unwrap()
        .value
    else {
        panic!("expected incremental semantic view")
    };

    assert!(
        incremental
            .authoring_context
            .conventional_candidates
            .iter()
            .any(|candidate| candidate.law_id == "period-frequency-reciprocity")
    );

    let mut clean_snapshot = snapshot(changed);
    clean_snapshot.inventory_version = 2;
    clean_snapshot.documents[0].document_version = 2;
    let mut clean = SemathEngine::default();
    clean.reset(clean_snapshot).unwrap();
    let QueryValue::SemanticView { view: clean } = clean
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            2,
            2,
        ))
        .unwrap()
        .value
    else {
        panic!("expected clean semantic view")
    };

    assert_eq!(incremental, clean);
}

#[test]
fn formula_metadescription_establishes_only_the_attached_relation() {
    let content = "The selected constitutive model is\n\\[J=-D\\nabla c-q\\nabla\\phi.\\]";
    let offset = content.rfind("\\phi").unwrap() as u32 + "\\phi".len() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
    assert!(view.symbol.is_some_and(|symbol| symbol.entity_id.is_none()));
    assert!(view.context.entity_id.is_some());

    let head = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.find("J=").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = head.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(view.decision, MeaningDecision::Partial { .. }),
        "{view:#?}"
    );
    assert!(
        view.symbol
            .is_some_and(|symbol| symbol.definitions.is_empty())
    );
    assert!(view.context.entity_id.is_some());

    let inner = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.rfind("\\phi").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = inner.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(view.decision, MeaningDecision::Partial { .. }),
        "{view:#?}"
    );
    assert!(view.context.entity_id.is_none());
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.hypothesis_id == "source-meaning")
    );
}

#[test]
fn formula_metadescription_retains_its_meaning_after_trailing_punctuation() {
    let content = "The selected continuum identity is\n\\[J=-D\\nabla c.\\]";
    let relation_end = content.rfind("c.").unwrap() as u32 + 1;
    let punctuation_end = relation_end + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    for offset in [relation_end, punctuation_end] {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert!(matches!(view.decision, MeaningDecision::Established { .. }));
        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        );
        let formula_range = &view
            .authoring_context
            .formula
            .as_ref()
            .expect("selected formula")
            .location
            .range;
        assert!(
            view.authoring_context
                .interpretations
                .hypotheses
                .iter()
                .any(|hypothesis| {
                    hypothesis.hypothesis_id == "source-meaning"
                        && hypothesis.evidence.iter().any(|evidence| {
                            evidence.evidence.rule_id == "english-equation-flow-meaning"
                                && evidence.evidence.source_ranges.iter().any(|range| {
                                    range.start_offset <= formula_range.start_offset
                                        && formula_range.end_offset <= range.end_offset
                                })
                        })
                }),
            "offset {offset}: {view:#?}"
        );
    }
}

#[test]
fn formula_metadescription_does_not_escalate_an_existing_source_meaning() {
    let content = "Let $r$ be rectangular area, $u$ side length, and $v$ side width. The selected relation is\n\\[r=uv.\\]";
    let offset = content.rfind("v.").unwrap() as u32 + 2;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| {
                hypothesis.label == "rectangular area"
                    && hypothesis.support == crate::MathInterpretationSupportTier::Supported
            })
    );
    assert!(
        !view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .flat_map(|hypothesis| &hypothesis.evidence)
            .any(|evidence| evidence.evidence.rule_id == "semath/asserted-formula-meaning"),
        "{view:#?}"
    );
}

#[test]
fn descriptive_formula_labels_do_not_establish_an_unrecognized_root() {
    for content in [
        "Under the recorded model, the governing equation is $a=b$.",
        "The previous assertion was removed. The remaining untyped expression is $a=b$. No current source adopts a meaning for either symbol.",
    ] {
        let view = semantic_view_at(content, content.find("a=b").unwrap() as u32);

        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Unsupported,
            "{content}: {view:#?}"
        );
        assert!(view.context.relations.is_empty(), "{content}: {view:#?}");
    }
}

#[test]
fn attributed_formula_metadescription_cannot_establish_at_trailing_punctuation() {
    let content = "The attributed firmware formula is\n\\[J=-D\\nabla c.\\]";
    let offset = content.rfind("c.").unwrap() as u32 + 2;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        !matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
}

#[test]
fn semantic_view_does_not_project_a_nested_law_onto_the_formula_head() {
    let content = "Let $A$ and $B$ be events. The reported value is $p=P(A\\cap B)/P(B)$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let head = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.find("p=").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view: head } = head.value else {
        panic!("expected semantic view")
    };
    assert!(
        !matches!(
            head.decision,
            MeaningDecision::Established { ref meaning, .. }
                | MeaningDecision::Partial { ref meaning, .. }
                if meaning.relation_id.as_deref() == Some("probability:event-intersection")
        ),
        "a nested relation must not own the outer formula head: {:?}",
        head.decision
    );

    let nested = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.find("A\\cap B").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view: nested } = nested.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(nested.decision, MeaningDecision::Partial { .. }),
        "{nested:#?}"
    );
    assert_eq!(
        nested.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial
    );
    assert!(
        nested
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "probability:event-intersection"
                })
            })
    );
}

#[test]
fn semantic_view_projects_bounded_index_constraints_without_formula_reparsing() {
    let content = "Let $A$ be an $m$ by $n$ matrix. Let $x$ be an $n$-dimensional vector. Let $y$ denote the output. $y=Ax$. Inspect $y$.";
    let offset = content.rfind("$y$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    let update = engine.reset(snapshot(content)).unwrap();
    assert!(update.stats.semantic_derived_claims > 0);
    assert!(update.stats.semantic_constraint_work > 0);
    assert!(!update.stats.semantic_constraint_truncated);

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.context.claims.iter().any(|claim| {
            claim.predicate == "shape"
                && claim.value == "Vector[m]"
                && claim
                    .evidence
                    .iter()
                    .any(|evidence| evidence.rule_id == "semath/constraint/equality")
        }),
        "{:?}",
        view.context.claims
    );
    let without_relation = "Let $A$ be an $m$ by $n$ matrix. Let $x$ be an $n$-dimensional vector. Let $y$ denote the output. Inspect $y$.";
    let retracted = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", without_relation, 2)),
            }],
        })
        .unwrap();
    assert_eq!(retracted.stats.semantic_derived_claims, 0);
}

#[test]
fn a_role_first_derivative_keeps_structural_identity_without_a_false_conflict() {
    let content =
        "Let $x(t)$ be an n-dimensional state vector. Inspect its derivative $\\dot{x}(t)$.";
    let view = semantic_view_at(content, content.find("dot{x}").unwrap() as u32);

    assert!(matches!(view.decision, MeaningDecision::Partial { .. }));
    assert!(view.diagnostics.is_empty(), "{:?}", view.diagnostics);

    let asserted = "Let $x$ denote the state vector. Its derivative $\\dot{x}$ drives the model.";
    let view = semantic_view_at(asserted, asserted.find("x}$ drives").unwrap() as u32);
    assert!(matches!(view.decision, MeaningDecision::Established { .. }));
    assert!(view.diagnostics.is_empty(), "{:?}", view.diagnostics);
}

#[test]
fn law_roles_are_retained_as_retractable_derived_index_claims() {
    let content = "Let $A$ be an n by n matrix and $x$ an n-dimensional vector. Define $y=Ax$. Let $B$ be an n by n matrix and $z$ an n-dimensional vector. Then $z=By$. Inspect $y$.";
    let offset = content.rfind("$y$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let before_formula = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.find("$y=Ax$").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = before_formula.value else {
        panic!("expected semantic view")
    };
    assert!(!view.context.claims.iter().any(|claim| {
        claim
            .evidence
            .iter()
            .any(|evidence| evidence.rule_id.starts_with("law-chain/2/"))
    }));

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.context.claims.iter().any(|claim| {
            claim.predicate == "concept"
                && claim.value == "linear-algebra:vector"
                && claim.evidence.iter().any(|evidence| {
                    evidence.kind == "derived-claim"
                        && evidence.rule_id.starts_with("law-chain/2/linear-algebra:")
                })
        }),
        "{:?}",
        view.context.claims
    );

    let without_relation = "Let $A$ be an n by n matrix and $x$ an n-dimensional vector. Let $y$ denote the output. Let $B$ be an n by n matrix and $z$ an n-dimensional vector. Then $z=By$. Inspect $y$.";
    engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", without_relation, 2)),
            }],
        })
        .unwrap();
    let offset = without_relation.rfind("$y$").unwrap() as u32 + 1;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            2,
            2,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(!view.context.claims.iter().any(|claim| {
        claim
            .evidence
            .iter()
            .any(|evidence| evidence.rule_id.starts_with("law-chain/2/"))
    }));
}

#[test]
fn a_typed_law_role_can_support_one_later_law_without_backward_flow() {
    let content = "Let $A$ be an n by n matrix and $x$ an n-dimensional vector. Define $y=Ax$. Let $B$ be an n by n matrix and $z$ an n-dimensional vector. Then $z=By$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let laws = engine.index.observations("main").laws.all();
    let products = laws
        .iter()
        .filter(|law| law.title == "Matrix-vector product")
        .collect::<Vec<_>>();
    assert_eq!(products.len(), 2, "{laws:#?}");
    let later = products
        .iter()
        .find(|law| law.range.start_offset > content.find("Let $B$").unwrap() as u32)
        .expect("later product");
    assert!(later.bindings.iter().any(|binding| {
        binding.parameter == "vector"
            && binding.symbol == "y"
            && binding.proof == LawBindingProof::Derived
    }));

    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.rfind("z=By").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };
    let chain_evidence = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .flat_map(|hypothesis| &hypothesis.evidence)
        .find(|evidence| evidence.evidence.kind == "law-chain-binding")
        .expect("later interpretation retains the earlier law source");
    let first_formula = content.find("y=Ax").unwrap() as u32;
    assert!(chain_evidence.source_anchors.iter().any(|anchor| {
        anchor.location.file_id == "main"
            && anchor.location.range.start_offset <= first_formula
            && first_formula < anchor.location.range.end_offset
    }));
}

#[test]
fn probability_formula_does_not_gain_cross_field_forward_law_authority() {
    let content = r#"### Historical inputs

Let \(A\) be the event that the canary exceeds its latency budget and \(B\) the event that the error-rate alert fires. Historical rollouts give
\[
P(A)=0.18,\qquad P(B)=0.11,\qquad P(A\cap B)=0.04.
\]

### Calculation proposed

The draft go/no-go calculation added the two marginal probabilities:
\[
P(A\cup B)=P(A)+P(B)=0.29.
\]

### Accepted go/no-go value

Review rejected that value because the simultaneous event was counted twice. The accepted calculation is
\[
P_{\mathrm{any}}=P(A\cup B)=P(A)+P(B)-P(A\cap B)=0.25.
\]
The checklist therefore uses a 25 percent chance that at least one monitored risk appears. No independence assumption is needed or supplied.
"#;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let offset = (content.rfind("A\\cup B").unwrap() + "A\\cup ".len()) as u32;
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    let unrelated = [
        "circuits:ohm-law",
        "electromagnetism:electric-potential-energy",
        "electromagnetism:electric-power-law",
        "fluid-mechanics:steady-momentum-flux",
        "fluid-mechanics:volumetric-flow-rate",
        "signals-systems:wave-speed-relation",
        "thermodynamics-heat-transfer:thermal-resistance-rate",
    ];
    for hypothesis in &view.authoring_context.interpretations.hypotheses {
        if unrelated.contains(&hypothesis.hypothesis_id.as_str()) {
            assert_eq!(
                hypothesis.support,
                crate::MathInterpretationSupportTier::Tentative,
                "{} acquired cross-field support: {hypothesis:#?}",
                hypothesis.hypothesis_id
            );
            assert!(
                hypothesis.evidence.iter().all(|item| !matches!(
                    item.evidence.kind.as_str(),
                    "derived-binding" | "law-chain-binding"
                )),
                "{} acquired cross-field derived roles: {hypothesis:#?}",
                hypothesis.hypothesis_id
            );
        }
    }
    assert!(view.domains.iter().all(|domain| {
        domain.pack_id == "probability" || domain.support == crate::DomainSupportTier::Tentative
    }));
}

#[test]
fn semantic_view_projects_claim_status_only_from_typed_index_evidence() {
    let content = "Let $A$ denote an event. Inspect $A$.";
    let offset = content.rfind("$A$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let concept = view
        .context
        .claims
        .iter()
        .find(|claim| claim.predicate == "concept")
        .expect("typed concept claim");
    assert_eq!(concept.status, crate::SemanticClaimStatus::Certain);
    assert!(
        concept
            .evidence
            .iter()
            .all(|evidence| evidence.kind == "source-claim" && evidence.strength == "hard")
    );
    assert!(
        view.context
            .concepts
            .iter()
            .any(|item| item.concept_id == concept.value)
    );
    assert!(!view.authoring_context.claim_evidence.is_empty());
    assert!(view.authoring_context.claim_evidence.iter().all(|claim| {
        claim.claim.file_id == "main"
            && claim.claim.range.end_offset <= offset
            && claim
                .evidence
                .iter()
                .all(|evidence| evidence.rule_id != "semath/canonical-symbol-identity")
    }));
    assert!(view.authoring_context.notation_occurrences.len() >= 2);
}

#[test]
fn authoring_claim_evidence_emits_one_link_for_one_authored_claim() {
    let content = "The draft calls $x$ the unique estimate. Inspect $x$.";
    let offset = content.rfind("$x$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    let internal_facts = view
        .context
        .claims
        .iter()
        .filter(|claim| {
            claim
                .evidence
                .iter()
                .any(|evidence| evidence.rule_id == "english-relational-definition")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        internal_facts
            .iter()
            .map(|claim| claim.predicate.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        ["definition", "type"].into_iter().collect()
    );
    assert_eq!(view.authoring_context.claim_evidence.len(), 1);
    assert_eq!(
        view.authoring_context.claim_evidence[0].claim_id,
        "main:1:definition-claim:0"
    );
}

#[test]
fn formula_claim_projection_follows_the_selected_root_entity() {
    let content = "The source calls $x$ the unique sample. Let $g$ be a function. Inspect\n\\[\ng=x,\\qquad x\\in I.\n\\]";
    let trailing_formula_gap = content.rfind("\\]").unwrap() as u32 - 1;
    let view = semantic_view_at(content, trailing_formula_gap);

    assert_eq!(view.authoring_context.claim_evidence.len(), 1, "{view:#?}");
    assert_eq!(
        view.authoring_context.claim_evidence[0].claim_id,
        "main:1:definition-claim:0"
    );
}

#[test]
fn formula_claim_projection_does_not_substitute_a_declared_relation_head() {
    let content = "Let $i$ be electric current. Inspect $i=Ct$.";
    let selected_t = content.rfind('t').unwrap() as u32;
    let view = semantic_view_at(content, selected_t);

    assert!(
        view.authoring_context.claim_evidence.is_empty(),
        "{view:#?}"
    );
}

#[test]
fn exact_formula_selector_anchor_does_not_increase_semantic_certainty() {
    let content = "Inspect\n\\[\ng=x,\\qquad x\\in I.\n\\]";
    let trailing_formula_gap = content.rfind("\\]").unwrap() as u32 - 1;
    let view = semantic_view_at(content, trailing_formula_gap);

    assert_eq!(view.authoring_context.claim_evidence.len(), 1, "{view:#?}");
    assert!(
        view.authoring_context.claim_evidence[0]
            .evidence
            .iter()
            .all(|evidence| evidence.rule_id == "semath/canonical-symbol-identity")
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported,
        "{view:#?}"
    );
}

#[test]
fn formula_disposition_is_invariant_across_sibling_cursor_claims() {
    let content = "The source calls $x$ the unique sample. Inspect $g=x$.";
    let at_g = semantic_view_at(content, content.rfind("g=x").unwrap() as u32);
    let at_x = semantic_view_at(content, content.rfind("g=x").unwrap() as u32 + 2);

    assert_eq!(
        at_g.authoring_context.disposition, at_x.authoring_context.disposition,
        "g={at_g:#?}\nx={at_x:#?}"
    );
    assert_eq!(
        at_g.authoring_context.lifecycle, at_x.authoring_context.lifecycle,
        "g={at_g:#?}\nx={at_x:#?}"
    );
    assert_eq!(
        at_g.authoring_context.interpretations.analysis_limits,
        at_x.authoring_context.interpretations.analysis_limits,
        "g={at_g:#?}\nx={at_x:#?}"
    );
    assert_eq!(
        at_g.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported,
        "{at_g:#?}"
    );
    assert_ne!(
        at_g.authoring_context.claim_evidence, at_x.authoring_context.claim_evidence,
        "cursor claim links should remain independently selected"
    );
}

#[test]
fn cursor_projection_caps_do_not_change_formula_lifecycle() {
    let uses = std::iter::repeat_n("Inspect $x$.", 20)
        .collect::<Vec<_>>()
        .join(" ");
    let content = format!("The source calls $x$ the unique sample. {uses} Compare $g=x$.");
    let formula = content.rfind("g=x").unwrap() as u32;
    let at_g = semantic_view_at(&content, formula);
    let at_x = semantic_view_at(&content, formula + 2);

    assert_ne!(
        at_g.authoring_context.notation_occurrences,
        at_x.authoring_context.notation_occurrences
    );
    assert_eq!(at_x.authoring_context.notation_occurrences.len(), 16);
    assert!(!at_x.authoring_context.truncated, "{at_x:#?}");
    assert_eq!(
        at_g.authoring_context.lifecycle, at_x.authoring_context.lifecycle,
        "g={at_g:#?}\nx={at_x:#?}"
    );
    assert_eq!(
        at_g.authoring_context.truncated, at_x.authoring_context.truncated,
        "g={at_g:#?}\nx={at_x:#?}"
    );
    assert_eq!(
        at_g.authoring_context.interpretations.analysis_limits,
        at_x.authoring_context.interpretations.analysis_limits,
        "g={at_g:#?}\nx={at_x:#?}"
    );
}

#[test]
fn root_claim_search_finds_late_substantive_entities_before_output_capping() {
    let weak = [
        r"\alpha",
        r"\beta",
        r"\gamma",
        r"\delta",
        r"\epsilon",
        r"\varepsilon",
        r"\zeta",
        r"\eta",
        r"\theta",
        r"\vartheta",
        r"\iota",
        r"\kappa",
        r"\lambda",
        r"\mu",
        r"\nu",
        r"\xi",
        r"\omicron",
        r"\pi",
        r"\varpi",
        r"\rho",
        r"\varrho",
        r"\sigma",
        r"\varsigma",
        r"\tau",
        r"\upsilon",
        r"\phi",
        r"\varphi",
        r"\chi",
        r"\psi",
        r"\omega",
        r"\Gamma",
        r"\Delta",
        r"\Theta",
        r"\Lambda",
        r"\Xi",
        r"\Pi",
        r"\Sigma",
        r"\Upsilon",
        r"\Phi",
        r"\Psi",
        r"\Omega",
    ];
    let formula_view = |defined_first: bool| {
        let assumptions = weak
            .iter()
            .map(|symbol| format!("Assume ${symbol}>0$."))
            .collect::<Vec<_>>()
            .join(" ");
        let operands = if defined_first {
            std::iter::once("Q".to_string())
                .chain(weak.iter().map(ToString::to_string))
                .collect::<Vec<_>>()
                .join("+")
        } else {
            weak.iter()
                .map(ToString::to_string)
                .chain(std::iter::once("Q".to_string()))
                .collect::<Vec<_>>()
                .join("+")
        };
        let content = format!(
            "{assumptions} The source calls $Q$ the unique aggregate. Inspect ${operands}=R$."
        );
        let offset = content.rfind("=R").unwrap() as u32 + 1;
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(&content)).unwrap();
        let (owner_entities, root_claims, root_claims_truncated) = {
            let document = engine.index.documents.get("main").unwrap();
            let root = document.canonical_expressions.last().unwrap();
            let owner_range = match &root.kind {
                SemanticExprKind::Relation { left, .. } => &left.range,
                _ => &root.range,
            };
            let entities = document
                .semantic_occurrences
                .iter()
                .filter(|occurrence| {
                    owner_range.start_offset <= occurrence.selection_range.start_offset
                        && occurrence.selection_range.end_offset <= owner_range.end_offset
                })
                .filter_map(|occurrence| {
                    engine
                        .index
                        .cursor_focus(&document.document.file_id, occurrence)
                })
                .filter_map(|focus| engine.resolved_entity(&focus.occurrence_id))
                .collect::<BTreeSet<_>>()
                .len();
            let (claims, truncated) = engine.formula_root_index_claims(document, root);
            (entities, claims, truncated)
        };
        let QueryValue::SemanticView { view } = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap()
            .value
        else {
            panic!("expected semantic view")
        };
        (*view, owner_entities, root_claims, root_claims_truncated)
    };
    let first = formula_view(true);
    let last = formula_view(false);

    assert!(
        first.1 > 32 && last.1 > 32,
        "first={first:#?}\nlast={last:#?}"
    );
    assert!(!first.3 && !last.3, "first={first:#?}\nlast={last:#?}");
    assert_eq!(first.2, last.2, "first={first:#?}\nlast={last:#?}");
    assert!(!first.2.is_empty(), "first={first:#?}");
    assert_eq!(
        first.0.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial,
        "{first:#?}"
    );
    assert_eq!(
        first.0.authoring_context.disposition, last.0.authoring_context.disposition,
        "first={first:#?}\nlast={last:#?}"
    );
    assert_eq!(
        first.0.authoring_context.lifecycle, last.0.authoring_context.lifecycle,
        "first={first:#?}\nlast={last:#?}"
    );
    assert_eq!(
        first.0.authoring_context.interpretations.analysis_limits,
        last.0.authoring_context.interpretations.analysis_limits,
        "first={first:#?}\nlast={last:#?}"
    );
}

#[test]
fn authoring_claim_anchor_uses_the_claims_own_included_document() {
    let main = "\\input{definitions}\nInspect $A$.";
    let definitions = "Let $A$ denote an event.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: range(0, "\\input{definitions}".len() as u32),
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.rfind('A').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let claim = view
        .context
        .claims
        .iter()
        .flat_map(|claim| &claim.evidence)
        .flat_map(|evidence| &evidence.source_anchors)
        .find(|anchor| anchor.location.file_id == "definitions")
        .expect("included source claim");
    assert_eq!(claim.location.path, "definitions.tex");
    assert!(claim.location.range.end_offset <= definitions.len() as u32);
    assert!(view.authoring_context.claim_evidence.is_empty());
}

#[test]
fn formula_claim_filter_is_file_aware_and_does_not_leak_limits_or_concepts() {
    let main = "\\input{definitions}\n$x$";
    let definitions = (0..40)
        .map(|index| format!("Let $x$ be a length quantity for record {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: 0,
                end_offset: 19,
            },
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", &definitions, 1),
            ],
        })
        .unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.rfind('x').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    assert!(
        view.authoring_context.claim_evidence.is_empty(),
        "{view:#?}"
    );
    assert!(view.context.concepts.is_empty(), "{view:#?}");
    assert!(!view.authoring_context.truncated, "{view:#?}");
}

#[test]
fn public_claim_projection_does_not_join_same_spelling_across_scopes() {
    let content = "# First\nLet $x$ denote an event. Inspect $x$.\n# Second\nLet $x$ denote a function. Inspect $x$.";
    let first = content.find("Inspect $x$").unwrap() as u32 + "Inspect $".len() as u32;
    let second = content.rfind("Inspect $x$").unwrap() as u32 + "Inspect $".len() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let concepts = [first, second].map(|offset| {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        view.context
            .claims
            .iter()
            .filter(|claim| claim.predicate == "concept")
            .map(|claim| claim.value.clone())
            .collect::<Vec<_>>()
    });
    assert_eq!(concepts[0].len(), 1, "{:?}", concepts[0]);
    assert_eq!(concepts[1].len(), 1, "{:?}", concepts[1]);
    assert_ne!(concepts[0], concepts[1]);
}

#[test]
fn equality_lhs_establishes_source_ordered_symbol_identity_for_later_uses() {
    let content = "Let $d$ be length and $t$ duration. $v=d/t$. The derived value is $v$.";
    let later = content.rfind("$v$").unwrap() as u32 + 1;
    let earlier = content.find("$v=").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: later,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.context.claims.iter().any(|claim| {
            claim.predicate == "dimension"
                && claim.value == "length^1 · time^-1"
                && claim
                    .evidence
                    .iter()
                    .any(|evidence| evidence.rule_id == "semath/constraint/equality")
        }),
        "{:?}",
        view.context.claims
    );
    assert!(
        view.context
            .quantities
            .iter()
            .any(|quantity| quantity.dimension.display == "length · time^-1"),
        "{:?}",
        view.context.quantities
    );

    let definition = engine
        .query(query(
            Query::Definition {
                file_id: "main".into(),
                offset: earlier,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Locations { locations, .. } = definition.value else {
        panic!("expected locations")
    };
    assert!(
        locations.is_empty(),
        "an assignment is identity, not a prose definition"
    );
}

#[test]
fn implicit_assignment_identity_cannot_authorize_navigation_or_editing() {
    let content = "Let $d$ be length and $t$ duration. $v=d/t$. The derived value is $v$.";
    let offset = content.rfind("$v$").unwrap() as u32 + 1;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    for query_kind in [
        Query::Definition {
            file_id: "main".into(),
            offset,
        },
        Query::References {
            file_id: "main".into(),
            offset,
            include_declaration: true,
        },
        Query::PrepareRename {
            file_id: "main".into(),
            offset,
        },
        Query::Rename {
            file_id: "main".into(),
            offset,
            new_name: "w".into(),
        },
    ] {
        let result = engine.query(query(query_kind, 1, 1)).unwrap();
        let authorization = match result.value {
            QueryValue::Locations { authorization, .. }
            | QueryValue::RenamePreparation { authorization, .. }
            | QueryValue::EditProposal { authorization, .. } => authorization,
            _ => panic!("expected an entity surface result"),
        };
        assert!(matches!(
            authorization,
            crate::EntitySurfaceAuthorization::Refused {
                reason: crate::EntitySurfaceRefusal {
                    kind: crate::EntitySurfaceRefusalKind::Unsupported,
                    ..
                }
            }
        ));
    }
}

#[test]
fn diagnostics_report_only_a_demonstrable_typed_constraint_conflict() {
    let content = "Let $A$ be a $2$ by $3$ matrix. Let $x$ be a $4$-dimensional vector. Let $y$ denote the output. $y=Ax$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = result.value else {
        panic!("expected diagnostics")
    };
    let conflict = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "constraint-product-shape-conflict")
        .expect("numeric inner dimensions prove a product conflict");
    assert!(conflict.evidence.len() >= 3, "{:?}", conflict.evidence);

    let symbolic = "Let $A$ be an $m$ by $n$ matrix. Let $x$ be a $k$-dimensional vector. Let $y$ denote the output. $y=Ax$.";
    let mut symbolic_engine = SemathEngine::default();
    symbolic_engine.reset(snapshot(symbolic)).unwrap();
    let symbolic_result = symbolic_engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = symbolic_result.value else {
        panic!("expected diagnostics")
    };
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "constraint-product-shape-conflict" })
    );

    let proven_symbolic =
        "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}, k \\ne n$\n$y=Ax$";
    let mut proven_symbolic_engine = SemathEngine::default();
    proven_symbolic_engine
        .reset(snapshot(proven_symbolic))
        .unwrap();
    let proven_symbolic_result = proven_symbolic_engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = proven_symbolic_result.value else {
        panic!("expected diagnostics")
    };
    let conflict = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "constraint-product-shape-conflict")
        .expect("an explicit symbolic inequality proves the product conflict");
    assert_eq!(
        conflict.message,
        "Cannot multiply Matrix[m × n] by Vector[k]."
    );
    assert!(
        conflict
            .explanation
            .contains("Matrix multiplication requires the left inner dimension")
    );

    let without_comparison = "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}$\n$y=Ax$";
    proven_symbolic_engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", without_comparison, 2)),
            }],
        })
        .unwrap();
    let retracted = proven_symbolic_engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            2,
            2,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = retracted.value else {
        panic!("expected diagnostics")
    };
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "constraint-product-shape-conflict")
    );
}

#[test]
fn incompatible_redeclarations_share_one_typed_public_conflict() {
    let content = "Let $p$ denote a probability distribution.\n$p$ is a random variable.\n$p $";
    let offset = (content.rfind("$p ").unwrap() + 1) as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(view.decision, MeaningDecision::Conflicting { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .all(|hypothesis| {
                hypothesis.support != crate::MathInterpretationSupportTier::Contradicted
            })
    );
    assert_eq!(
        view.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "notation-role-conflict")
            .count(),
        1
    );
}

#[test]
fn incompatible_acceleration_shape_conflicts_and_blocks_newton_relation() {
    let content = "Let $F$ denote net force. Let $m$ denote scalar mass. Let $a$ denote an acceleration matrix. The proposed model is $F=ma$.";
    let offset = content.rfind("a$").unwrap() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert!(
        matches!(view.decision, MeaningDecision::Conflicting { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conflicting
    );
    assert!(
        view.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "notation-role-type-conflict")
    );
    assert!(
        view.context
            .relations
            .iter()
            .all(|relation| { relation.relation_id != "classical-mechanics:newton-second-law" })
    );
}

#[test]
fn scalar_and_vector_acceleration_shapes_preserve_the_newton_candidate() {
    for declaration in ["a scalar acceleration", "an acceleration vector"] {
        let content = format!(
            "Let $F$ denote net force. Let $m$ denote scalar mass. Let $a$ denote {declaration}. The quantities refer to one body in a common inertial frame. The model is $F=ma$."
        );
        let view = semantic_view_at(&content, content.rfind("F=").unwrap() as u32);

        assert!(
            !matches!(view.decision, MeaningDecision::Conflicting { .. }),
            "{declaration}: {view:#?}"
        );
        assert!(
            view.diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.code != "notation-role-type-conflict" })
        );
        assert!(
            view.authoring_context
                .interpretations
                .hypotheses
                .iter()
                .any(|hypothesis| {
                    hypothesis.relation.as_ref().is_some_and(|relation| {
                        relation.relation_id == "classical-mechanics:newton-second-law"
                    }) && hypothesis.support != crate::MathInterpretationSupportTier::Contradicted
                })
        );
    }
}

#[test]
fn included_acceleration_shape_conflict_blocks_the_newton_relation() {
    let main = "\\input{definitions}\nNewton's second law is $F=ma$.";
    for (shape, relation_expected) in [("scalar", true), ("matrix", false)] {
        let definitions = format!(
            "Let $F$ denote net force. Let $m$ denote scalar mass. Let $a$ denote an acceleration {shape}."
        );
        let mut main_document = document("main", "main.tex", main, 1);
        main_document.includes.push(ProjectInclude {
            path: "definitions".into(),
            kind: "input".into(),
            source: ProjectSourceRef {
                file_id: "main".into(),
                path: "main.tex".into(),
                range: SourceRange {
                    start_offset: 0,
                    end_offset: 19,
                },
            },
        });
        let mut engine = SemathEngine::default();
        engine
            .reset(ProjectSnapshot {
                protocol_version: PROTOCOL_VERSION,
                epoch: "project:1".into(),
                inventory_version: 1,
                project_id: "project".into(),
                main_file_id: Some("main".into()),
                documents: vec![
                    main_document,
                    document("definitions", "definitions.tex", &definitions, 1),
                ],
            })
            .unwrap();
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: main.rfind("F=").unwrap() as u32,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };

        let has_relation = view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter_map(|hypothesis| hypothesis.relation.as_ref())
            .any(|relation| relation.relation_id == "classical-mechanics:newton-second-law");
        assert_eq!(has_relation, relation_expected, "{shape}: {view:#?}");
        if !relation_expected {
            assert!(!matches!(
                view.decision,
                MeaningDecision::Established { .. }
            ));
        }
    }
}

#[test]
fn explicit_sign_convention_refutation_is_a_source_conflict() {
    let content = "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Without adopting the passive sign convention, consider $i=C\\frac{dv}{dt}$.";
    let view = semantic_view_at(content, content.rfind("i=").unwrap() as u32);

    assert!(matches!(view.decision, MeaningDecision::Established { .. }));
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conflicting
    );
    assert!(
        view.context
            .relations
            .iter()
            .all(|relation| { relation.relation_id != "circuits:capacitor-current-law" })
    );
    let hypothesis =
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .find(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                })
            })
            .expect("refuted law hypothesis");
    assert_eq!(
        hypothesis.support,
        crate::MathInterpretationSupportTier::Contradicted
    );
    assert!(hypothesis.conditions.iter().any(|condition| {
        condition.condition_id == "passive-sign-convention"
            && condition.status == crate::ConstraintStatus::Conflicting
    }));
    let contradicting = hypothesis
        .evidence
        .iter()
        .filter(|evidence| evidence.role == crate::MathInterpretationEvidenceRole::Contradicting)
        .collect::<Vec<_>>();
    assert!(!contradicting.is_empty());
    assert!(contradicting.iter().all(|evidence| {
        evidence.evidence.rule_id == "english-scientific-assumption"
            && matches!(
                evidence.evidence.kind.as_str(),
                "explicit-prose" | "attached-prose"
            )
    }));
    assert!(view.authoring_context.equation_links.is_empty());
}

#[test]
fn a_sign_convention_descriptor_only_authorizes_its_target_law() {
    for descriptor in ["ohm", "kirchhoff", "electric", "closed"] {
        let content = format!(
            "Let $C>0$ be capacitance, let $v(t)$ be voltage, let $i(t)$ be electric current, and let $t$ be time. Under this passive sign convention, the {descriptor} law is $i(t)=C\\frac{{dv}}{{dt}}(t)$."
        );
        let view = semantic_view_at(&content, content.rfind("i(t)=").unwrap() as u32);
        assert_ne!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established,
            "{descriptor}"
        );
        let capacitor = view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .find(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                })
            })
            .expect("bounded capacitor candidate");
        assert!(capacitor.conditions.iter().any(|condition| {
            condition.condition_id == "passive-sign-convention"
                && condition.status == crate::ConstraintStatus::Required
        }));
    }

    for (content, formula, relation_id, condition_id) in [
        (
            "Let $V$ be voltage, let $R$ be resistance, and let $I$ be electric current. Under this passive sign convention, the capacitor law is. $V=RI$.",
            "V=RI",
            "circuits:ohm-law",
            "consistent-references",
        ),
        (
            "Let $C>0$ be capacitance, let $v(t)$ be voltage, let $i(t)$ be electric current, and let $t$ be time. Under this passive sign convention, the capacitor law is. $i(t)=C\\frac{dv}{dt}(t)$.",
            "i(t)=",
            "circuits:capacitor-current-law",
            "passive-sign-convention",
        ),
    ] {
        let view = semantic_view_at(content, content.rfind(formula).unwrap() as u32);
        assert_ne!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        );
        let hypothesis = view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .find(|hypothesis| {
                hypothesis
                    .relation
                    .as_ref()
                    .is_some_and(|relation| relation.relation_id == relation_id)
            })
            .expect("bounded relation candidate");
        assert!(hypothesis.conditions.iter().any(|condition| {
            condition.condition_id == condition_id
                && condition.status == crate::ConstraintStatus::Required
        }));
    }
}

#[test]
fn a_refuted_sign_convention_does_not_authorize_equation_links() {
    let linked = "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Under the passive sign convention, the accepted reference is $i=C\\frac{dv}{dt}$. Let $j$ be electric current, $D$ capacitance, and $u$ voltage. Without adopting the passive sign convention, consider $j=D\\frac{du}{dt}$.";
    let linked_view = semantic_view_at(linked, linked.rfind("j=").unwrap() as u32);
    assert_eq!(
        linked_view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conflicting
    );
    assert!(linked_view.authoring_context.equation_links.is_empty());
}

#[test]
fn sign_convention_evidence_is_scoped_to_its_formula() {
    for adoption in [
        "Under the passive sign convention, use",
        "We explicitly use the passive sign convention, and consider",
        "We currently use the passive sign convention, and consider",
        "In this derivation we use the passive sign convention, and consider",
        "For this calculation we use the passive sign convention, and consider",
        "The calculation uses the passive sign convention, and consider",
    ] {
        let accepted = format!(
            "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. {adoption} $i=C\\frac{{dv}}{{dt}}$."
        );
        let accepted_view = semantic_view_at(&accepted, accepted.rfind("i=").unwrap() as u32);
        assert!(
            matches!(accepted_view.decision, MeaningDecision::Established { .. }),
            "{adoption}: {:?}",
            accepted_view.decision
        );
        assert!(
            accepted_view
                .context
                .relations
                .iter()
                .any(|relation| { relation.relation_id == "circuits:capacitor-current-law" })
        );
    }

    let scoped = "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time.\n\\section{Rejected}\nWithout adopting the passive sign convention, consider $i=C\\frac{dv}{dt}$.\n\\section{Accepted}\nUnder the passive sign convention, use $i=C\\frac{dv}{dt}$.";
    let scoped_view = semantic_view_at(scoped, scoped.rfind("i=").unwrap() as u32);
    assert!(matches!(
        scoped_view.decision,
        MeaningDecision::Established { .. }
    ));
    assert!(scoped_view.authoring_context.equation_links.is_empty());
}

#[test]
fn rejecting_a_refutation_is_not_a_sign_convention_conflict() {
    for double_negative in [
        "Without rejecting the passive sign convention",
        "Without ever rejecting the passive sign convention",
        "Without not adopting the passive sign convention",
        "Without ever repeatedly explicitly continuing to firmly and deliberately reject the passive sign convention",
        "Without adopting, even provisionally, the passive sign convention",
        "Without adopting the research and development passive sign convention",
        "Without adopting the ideal source assumption and the passive sign convention",
        "Without rejecting the active convention and the passive sign convention",
        "With no adoption of the passive sign convention",
        "We refuse to adopt the passive sign convention and",
        "Declining to adopt the passive sign convention",
        "Avoiding adoption of the passive sign convention",
        "The passive sign convention is declined;",
        "Rather than use the passive sign convention",
        "Instead of adopting the passive sign convention",
        "Prior to adopting the passive sign convention",
        "The passive sign convention ceased to be used;",
        "The passive sign convention is never used;",
        "We use an auxiliary meter to describe the passive sign convention, then",
        "The passive sign convention is described while we use an auxiliary meter, then",
        "We use an alternative to the passive sign convention and",
        "We adopt an alternative to the passive sign convention and",
        "An alternative to the passive sign convention is used;",
        "In lieu of using the passive sign convention",
        "In place of adopting the passive sign convention",
        "The passive sign convention is used only in a separate example, whereas here we",
        "The auxiliary meter is never fully used under the passive sign convention, while considering",
        "In a separate example, we use the passive sign convention, and here we",
        "The cited note uses the passive sign convention, but here we",
        "Smith uses the passive sign convention, but here we",
        "For the inductor, we use the passive sign convention, and for the capacitor we",
        "The passive sign convention applies to another circuit, but here we",
        "The passive sign convention is used for the inductor, but for the capacitor we",
        "The instructions say use the passive sign convention, but here we",
        "We intend to use the passive sign convention later, but currently we",
        "The passive sign convention applies if the reference terminal is positive, then we",
        "We use the passive sign convention if needed, then we",
        "One option is to use the passive sign convention, then we",
        "The measured current $i$ is recorded, and Smith uses the passive sign convention, then we",
        "The cited note reports current $i$ and uses the passive sign convention, then we",
        "For the other circuit, current $i$ is recorded and Smith uses the passive sign convention, then we",
        "The passive sign convention was used previously, and currently we",
        "The passive sign convention applies whenever the reference terminal is positive, and we",
        "The passive sign convention applies assuming the terminal is positive, and we",
        "We explicitly use the passive sign convention conditionally, then we",
        "In the cited example, $i$ uses the passive sign convention, then we",
        "In Smiths cited model, $i$ uses the passive sign convention, then we",
        "For the other circuit, current $i$ uses the passive sign convention, then we",
        "For a separate circuit, $i$ uses the passive sign convention, then we",
        "In another model, $i$ uses the passive sign convention, then we",
        "For this inductor we use the passive sign convention, and we",
        "For this appendix we use the passive sign convention, and we",
        "For this example we use the passive sign convention, and in the current derivation we",
        "Under the passive sign convention whenever the reference terminal is positive, we",
        "Under the passive sign convention assuming the terminal is positive, we",
        "Under the passive sign convention while analyzing the inductor, we",
        "Under the passive sign convention subject to a positive terminal, we",
        "Under the passive sign convention during the auxiliary analysis, we",
    ] {
        let content = format!(
            "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. {double_negative}, consider $i=C\\frac{{dv}}{{dt}}$."
        );
        let view = semantic_view_at(&content, content.rfind("i=").unwrap() as u32);
        assert!(
            !matches!(
                view.authoring_context.disposition,
                crate::MathAuthoringDisposition::Established
                    | crate::MathAuthoringDisposition::Conflicting
            ),
            "{double_negative}: {:?}",
            view.authoring_context.disposition
        );
        let capacitor = view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .find(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                })
            })
            .unwrap_or_else(|| {
                panic!("bounded capacitor hypothesis: {double_negative}: {view:#?}")
            });
        assert!(capacitor.conditions.iter().any(|condition| {
            condition.condition_id == "passive-sign-convention"
                && condition.status == crate::ConstraintStatus::Required
        }));
    }
}

#[test]
fn unknown_unicode_prose_cannot_authorize_a_sign_convention() {
    for qualified in [
        "조건부로",
        "만약 단자가 양수라면",
        "εάν ισχύει",
        "若条件成立",
    ] {
        let content = format!(
            "Let $i$ be electric current, $C$ capacitance, $v$ voltage, and $t$ time. Under the passive sign convention {qualified}, $i=C\\frac{{dv}}{{dt}}$."
        );
        let view = semantic_view_at(&content, content.rfind("i=").unwrap() as u32);
        assert!(
            !matches!(
                view.authoring_context.disposition,
                crate::MathAuthoringDisposition::Established
                    | crate::MathAuthoringDisposition::Conflicting
            ),
            "{qualified}: {:?}",
            view.authoring_context.disposition
        );
    }
}

#[test]
fn attached_sign_evidence_does_not_leak_to_a_preceding_formula() {
    for competing in [
        "Let $i$ be electric current, $C$ capacitance, $v$ voltage, $t$ time, and $L$ inductance. Consider $i=C\\frac{dv}{dt}$. Under the passive sign convention, use $v=L\\frac{di}{dt}$.",
        "Let $i$ be electric current, $C$ capacitance, $v$ voltage, $t$ time, and $L$ inductance. Compare $i=C\\frac{dv}{dt}$, but under the passive sign convention use $v=L\\frac{di}{dt}$.",
    ] {
        let competing_view = semantic_view_at(competing, competing.find("i=").unwrap() as u32);
        assert_ne!(
            competing_view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        );
        let competing_capacitor = competing_view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .find(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                })
            })
            .expect("bounded capacitor hypothesis");
        assert!(competing_capacitor.conditions.iter().any(|condition| {
            condition.condition_id == "passive-sign-convention"
                && condition.status == crate::ConstraintStatus::Required
        }));
    }
}

#[test]
fn a_typed_conflict_follows_every_participating_binding_to_later_uses() {
    for content in [
        "In this model let $u$ be a scalar temperature and let $u$ be a three-dimensional velocity vector. Use $u$ now.",
        "In one lifetime let $t$ be duration in seconds and let $t$ be temperature in kelvin. Inspect $t$.",
    ] {
        let offset = (content.rfind('$').unwrap() - 1) as u32;
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(content)).unwrap();
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert!(
            matches!(view.decision, MeaningDecision::Conflicting { .. }),
            "{content}: {:?}",
            view.decision
        );
    }
}

#[test]
fn non_asserting_formula_mentions_do_not_create_constraint_problems() {
    let declarations =
        "$A \\in \\mathbb{R}^{m \\times n}, B \\in \\mathbb{R}^{n \\times p}, p \\ne m$.\n";
    for mention in [
        "The reverse product $BA$ is not shape-compatible.",
        "If the operands were reversed, $BA$ would be considered.",
        "As reported in the reference, $BA$ is the implementation order.",
        "Alternatively, use $BA$.",
    ] {
        let content = format!("{declarations}{mention}");
        let mut engine = SemathEngine::default();
        engine.reset(snapshot(&content)).unwrap();
        let result = engine
            .query(query(
                Query::Diagnostics {
                    file_id: "main".into(),
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::Diagnostics { diagnostics } = result.value else {
            panic!("expected diagnostics")
        };
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "constraint-product-shape-conflict"),
            "{mention}: {diagnostics:?}"
        );
    }
}

#[test]
fn symbolic_comparisons_do_not_cross_sibling_document_scopes() {
    let content = [
        "# North",
        "$A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}$",
        "$y=Ax$",
        "# South",
        "$k \\ne n$",
    ]
    .join("\n");
    let south = content.find("# South").unwrap() as u32;
    let mut input = document("main", "main.md", &content, 1);
    input.language = DocumentLanguage::Markdown;
    input.scopes = vec![
        SyntaxScope {
            kind: "document".into(),
            parent: None,
            range: range(0, content.len() as u32),
            state: MathRootState::Complete,
            name: None,
            level: None,
            source: None,
        },
        SyntaxScope {
            kind: "section".into(),
            parent: Some(0),
            range: range(0, south),
            state: MathRootState::Complete,
            name: Some("North".into()),
            level: None,
            source: None,
        },
        SyntaxScope {
            kind: "section".into(),
            parent: Some(0),
            range: range(south, content.len() as u32),
            state: MathRootState::Complete,
            name: Some("South".into()),
            level: None,
            source: None,
        },
    ];
    input.math_regions = test_math_regions(&content, DocumentLanguage::Markdown);

    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![input],
        })
        .unwrap();
    let result = engine
        .query(query(
            Query::Diagnostics {
                file_id: "main".into(),
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::Diagnostics { diagnostics } = result.value else {
        panic!("expected diagnostics")
    };
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "constraint-product-shape-conflict"),
        "{diagnostics:?}"
    );
}

#[test]
fn semantic_view_follows_a_law_across_its_rhs_and_boundary() {
    let content = "Let $P$ be power.\nLet $F$ be force.\nLet $v$ be velocity.\nInstantaneous power is $P=\\mathbf{F}\\cdot\\mathbf{v}\\quad$.";
    let offsets = [
        content.find("$P=").unwrap() as u32,
        content.find("P=").unwrap() as u32,
        content.find("\\mathbf{F}").unwrap() as u32,
        content.rfind("\\mathbf{v}").unwrap() as u32 + "\\mathbf{v}".len() as u32,
        content.rfind("\\quad").unwrap() as u32 + "\\quad".len() as u32,
        content.rfind('$').unwrap() as u32 + 1,
    ];
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    for offset in offsets {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        assert!(
            matches!(&view.decision, MeaningDecision::Established { .. }),
            "offset {offset}"
        );
        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established,
            "offset {offset}"
        );
    }
}

#[test]
fn semantic_view_projects_a_chained_relation_at_its_trailing_boundary() {
    let content = r"This example states a first derivative. Let $f$ be a function of $x$, $x$ the differentiation variable, and $g$ its first derivative.
\[
g=\frac{d f}{d x}=\lim_{h\to0}\frac{f(x+h)-f(x)}{h}.
\]";
    let offset = content.find("}{h}.").unwrap() as u32 + "}{h}.".len() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.context.relations.iter().any(|relation| {
            relation.relation_id == "calculus-analysis:first-derivative-relation"
        })
    );
}

#[test]
fn semantic_view_uses_the_relation_head_for_display_metadata_boundaries_only() {
    let content = "\\[\nQ=Av.\n\\label{eq:flow}\n\\]";
    let period_end = content.find("Q=Av.").unwrap() as u32 + "Q=Av.".len() as u32;
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: period_end,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert_eq!(
        view.symbol.as_ref().map(|symbol| symbol.symbol.as_str()),
        Some("Q")
    );
    assert!(view.context.entity_id.is_some());

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.len() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(view.symbol.is_none());

    let result = engine
        .query(query(
            Query::PrepareRename {
                file_id: "main".into(),
                offset: period_end,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::RenamePreparation { range, .. } = result.value else {
        panic!("expected rename preparation")
    };
    assert!(range.is_none());
}

#[test]
fn composite_formula_ownership_is_exact_and_does_not_guess_internal_symbols() {
    let relation = SemanticExpr {
        kind: SemanticExprKind::Relation {
            operator: SemanticReference::new("equals", range(1, 2), Vec::new()),
            left: Box::new(SemanticExpr {
                kind: SemanticExprKind::Symbol("y".into()),
                range: range(0, 1),
                provenance: Vec::new(),
            }),
            right: Box::new(SemanticExpr {
                kind: SemanticExprKind::Power(
                    Box::new(SemanticExpr {
                        kind: SemanticExprKind::Symbol("x".into()),
                        range: range(2, 3),
                        provenance: Vec::new(),
                    }),
                    Box::new(SemanticExpr {
                        kind: SemanticExprKind::Number("2".into()),
                        range: range(4, 5),
                        provenance: Vec::new(),
                    }),
                ),
                range: range(2, 5),
                provenance: Vec::new(),
            }),
        },
        range: range(0, 5),
        provenance: Vec::new(),
    };

    assert!(matches!(
        canonical_expression_owner(&relation, &range(2, 5), true, None)
            .map(|expression| &expression.kind),
        Some(SemanticExprKind::Power(_, _))
    ));
    assert!(expression_carries_formula_fact(
        canonical_expression_owner(&relation, &range(2, 5), true, None).unwrap()
    ));
    assert_eq!(
        canonical_expression_owner(&relation, &range(0, 1), false, Some(5))
            .map(|expression| &expression.range),
        Some(&range(0, 5))
    );
    assert!(canonical_expression_owner(&relation, &range(2, 3), false, None).is_none());
    assert!(canonical_expression_owner(&relation, &range(3, 4), false, None).is_none());
}

#[test]
fn a_unique_later_negative_formula_retracts_the_earlier_relation() {
    let base = "The preliminary station model treats the suction pipe as a uniform section. Here \\(A\\) is internal area and \\(v\\) is section-averaged speed.\n\\[\nQ=Av.\n\\]\nThis is the initial definition of reported volume flow.\n\\input{revision}\n";
    let revision = "The operations review no longer publishes the preliminary volume-flow estimate \\(Q=Av\\): the installed meter reports mass flow directly, and the area-average velocity is not retained as a certified output. The reviewed calculation still uses\n\\[\n\\dot m=\\rho A v_{\\mathrm{bulk}}.\n\\]\n";
    let mut base_document = document("base", "base.tex", base, 1);
    let include_start = base.find("\\input{revision}").unwrap() as u32;
    base_document.includes.push(ProjectInclude {
        path: "revision".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "base".into(),
            path: "base.tex".into(),
            range: SourceRange {
                start_offset: include_start,
                end_offset: include_start + "\\input{revision}".len() as u32,
            },
        },
    });
    let expression = lower_document_region(
        &base_document,
        &base_document.math_regions.last().unwrap().content_range,
    );
    let digest = stable_text_digest(&render_canonical(&expression));
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("base".into()),
            documents: vec![
                base_document,
                document("revision", "revision.tex", revision, 1),
            ],
        })
        .unwrap();
    let relation_start = base.find("Q=Av").unwrap() as u32;
    let occurrence = engine
        .index
        .occurrence_id_for_range("base", &range(relation_start, relation_start + 1))
        .expect("expected relation head occurrence");
    assert!(
        engine
            .index
            .semantic
            .relation_is_retracted(&digest, &occurrence)
    );

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "base".into(),
                offset: base.find("Q=Av").unwrap() as u32 + 4,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(view.context.relations.is_empty());
    assert!(view.authoring_context.lifecycle.retracted);
    assert!(!view.authoring_context.lifecycle.editable);
    assert!(
        view.authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .any(|limit| limit.kind == crate::MathInterpretationAnalysisLimitKind::RetractedSource)
    );
    assert!(
        view.authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .all(|limit| !matches!(
                limit.kind,
                crate::MathInterpretationAnalysisLimitKind::CandidateSetCapped
                    | crate::MathInterpretationAnalysisLimitKind::EvidenceTruncated
                    | crate::MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped
            ))
    );
    assert!(
        view.authoring_context.interpretations.hypotheses.is_empty(),
        "retracted source must not retain supporting interpretations: {view:#?}"
    );
}

#[test]
fn a_negative_formula_in_a_disconnected_document_does_not_retract_a_relation() {
    let base = "The preliminary station model treats the suction pipe as a uniform section. Here \\(A\\) is internal area and \\(v\\) is section-averaged speed.\n\\[\nQ=Av.\n\\]\n";
    let revision = "The operations review no longer publishes the preliminary volume-flow estimate \\(Q=Av\\).";
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("base".into()),
            documents: vec![
                document("base", "base.tex", base, 1),
                document("revision", "revision.tex", revision, 1),
            ],
        })
        .unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "base".into(),
                offset: base.find("Q=Av").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert_eq!(
        view.context.relations[0].relation_id,
        "fluid-mechanics:volumetric-flow-rate"
    );
}

#[test]
fn formula_meaning_includes_a_source_ordered_relation_linked_by_entity_identity() {
    let content = "Let $P$ be power. Let $R$ be power. Let $F$ be force. Let $v$ be velocity. The first measured balance is\n\\[P=F\\cdot v.\\] The corresponding measured balance is\n\\[R=F\\cdot v.\\]";
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![document("main", "main.tex", content, 1)],
        })
        .unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.rfind("R=F\\cdot v").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let relation_ids = view
        .context
        .relations
        .iter()
        .map(|relation| relation.relation_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        relation_ids,
        BTreeSet::from(["classical-mechanics:mechanical-power"]),
        "{view:#?}"
    );
    let link = view
        .authoring_context
        .equation_links
        .first()
        .expect("source-backed prior equation link");
    assert_eq!(link.kind, crate::MathEquationLinkKind::SharedEntity);
    assert!(link.source.source_notation.contains("P=F\\cdot v"));
    assert!(link.target.source_notation.contains("R=F\\cdot v"));
    assert!(!link.shared_entities.is_empty());
    assert!(!link.evidence.is_empty());
}

#[test]
fn transparent_project_macro_has_the_same_meaning_and_invocation_provenance() {
    let content = "Let $P$ be power. Let $F$ be force. Let $v$ be velocity. $P=\\power{F}{v}$";
    let invocation_start = content.find("\\power").unwrap() as u32;
    let invocation_end = invocation_start + "\\power{F}{v}".len() as u32;
    let mut input = document("main", "main.tex", content, 1);
    input.macros.push(ProjectMacro {
        kind: ProjectMacroKind::Call,
        name: "power".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: invocation_start,
                end_offset: invocation_start + "\\power".len() as u32,
            },
        },
        definitions: Vec::new(),
        expansion: ProjectMacroExpansion {
            status: ProjectMacroExpansionStatus::Expanded,
            depth: 0,
            editable: false,
            surface: Some("\\mathbf{F}\\cdot\\mathbf{v}".into()),
            input_range: Some(SourceRange {
                start_offset: invocation_start,
                end_offset: invocation_end,
            }),
            notation: None,
        },
    });
    let mut project = snapshot(content);
    project.documents = vec![input];
    let mut engine = SemathEngine::default();
    engine.reset(project).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: invocation_start + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established
    );
    let relation = &view.context.relations[0];
    assert!(relation.evidence[0].source_ranges[0].contains(invocation_start));
    assert_eq!(
        view.authoring_context.lifecycle.generation,
        crate::MathSourceGeneration::Generated
    );
    assert!(!view.authoring_context.lifecycle.editable);
    assert!(
        view.authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .any(|limit| limit.kind == crate::MathInterpretationAnalysisLimitKind::GeneratedSource)
    );
    assert!(
        view.authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .all(|limit| !matches!(
                limit.kind,
                crate::MathInterpretationAnalysisLimitKind::CandidateSetCapped
                    | crate::MathInterpretationAnalysisLimitKind::EvidenceTruncated
                    | crate::MathInterpretationAnalysisLimitKind::DiscriminatorSetCapped
            ))
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .flat_map(|hypothesis| &hypothesis.evidence)
            .flat_map(|evidence| &evidence.source_anchors)
            .any(|anchor| anchor.generation == crate::MathSourceGeneration::Generated)
    );
    assert!(
        !view
            .authoring_context
            .formula
            .as_ref()
            .expect("macro formula anchor")
            .provenance
            .is_empty()
    );
}

#[test]
fn unsupported_formula_refuses_instead_of_guessing() {
    let content = "$a \\star b = c$";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.find('=').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(matches!(view.decision, MeaningDecision::Unsupported { .. }));
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported
    );
    assert!(view.context.relations.is_empty());
}

#[test]
fn explicitly_unasserted_candidate_formula_is_unsupported() {
    let content = "Consider $y(x)=|x|$ near the origin. Although the report asks for both $y'(0)$ and $dy/dx(0)$, neither expression has a value here. Consequently neither derivative notation is defined at zero, and the candidate equality $y'(0)=dy/dx(0)$ is not asserted.";
    let needle = "y'(0)=dy/dx(0)";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: (content.rfind(needle).unwrap() + needle.len()) as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(matches!(view.decision, MeaningDecision::Unsupported { .. }));
}

#[test]
fn cursor_entity_and_selected_formula_have_independent_decisions() {
    let conflicting = "Let $i$ denote electric current, $C$ capacitance, $v$ voltage, and $t$ time. Without adopting the passive sign convention, consider $i=C\\frac{dv}{dt}$.";
    let view = semantic_view_at(conflicting, conflicting.rfind("i=").unwrap() as u32);
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conflicting,
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                }) && hypothesis.support == crate::MathInterpretationSupportTier::Contradicted
            })
    );

    let unsupported = "Let $P$ denote the archived power token. The displayed template is rejected for this model: $P=VI$.";
    let view = semantic_view_at(unsupported, unsupported.rfind("P=VI").unwrap() as u32);
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported,
        "{view:#?}"
    );

    let established_formula = "The token $P_\\triangle$ is rejected. Let $F$ be force and $v$ be velocity. Mechanical power is $P_\\triangle=F\\cdot v$.";
    let view = semantic_view_at(
        established_formula,
        established_formula.rfind("P_\\triangle=").unwrap() as u32,
    );
    assert!(
        matches!(view.decision, MeaningDecision::Unsupported { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{view:#?}"
    );

    let ambiguous = "Let $x$ denote the recorded result. Let $A$ and $B$ be sets in one universe and events in one probability space. Both set intersection and event intersection apply to $x=A\\cap B$.";
    let view = semantic_view_at(ambiguous, ambiguous.rfind("x=A").unwrap() as u32);
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Ambiguous,
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .requirements
            .iter()
            .any(|requirement| {
                matches!(
                    requirement,
                    crate::MathInterpretationRequirementInfo::Disambiguation { alternatives, .. }
                        if alternatives.iter().any(|alternative| {
                            alternative.alternative_id == "probability:event-intersection"
                        }) && alternatives.iter().any(|alternative| {
                            alternative.alternative_id == "discrete-math:set-intersection"
                        })
                )
            }),
        "{view:#?}"
    );
    assert!(
        [
            "probability:event-intersection",
            "discrete-math:set-intersection",
        ]
        .iter()
        .all(|relation_id| view
            .authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis
                .relation
                .as_ref()
                .is_some_and(|relation| { relation.relation_id == *relation_id }))),
        "{view:#?}"
    );

    let conventional =
        "Let $f$ denote alpha. For a periodic signal, the asserted relation is $f=1/T$.";
    let view = semantic_view_at(conventional, conventional.rfind("f=").unwrap() as u32);
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conventional,
        "{view:#?}"
    );
    assert!(view.context.relations.iter().all(|relation| {
        relation.relation_id != "signals-systems:period-frequency-reciprocity"
    }));
}

#[test]
fn competing_unselected_conventions_cannot_authorize_a_relation() {
    let content = "For a closed system, the note presents either the heat in and work out convention or the heat out and work in convention. Neither alternative is selected. The candidate display is $\\Delta U=Q-W$.";
    let view = semantic_view_at(content, content.rfind("\\Delta U=Q-W").unwrap() as u32);

    assert!(
        !matches!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        ),
        "{view:#?}"
    );
    let withheld =
        view.authoring_context
            .conditions
            .iter()
            .find(|condition| {
                condition.kind == crate::ScientificConstraintKind::SignConvention
                    && condition.evidence.iter().any(|evidence| {
                        evidence.rule_id == "scientific-prose/alternative-selection"
                    })
            })
            .unwrap_or_else(|| panic!("typed nonselection remains attached: {view:#?}"));
    assert_eq!(withheld.status, crate::ConstraintStatus::Required);
    assert!(
        view.authoring_context
            .requirements
            .iter()
            .any(|requirement| {
                matches!(
                    requirement,
                    crate::MathInterpretationRequirementInfo::Condition { condition, .. }
                        if condition.condition_id == withheld.condition_id
                )
            })
    );
    assert!(
        view.context.relations.iter().all(|relation| {
            relation.relation_id != "thermodynamics-heat-transfer:closed-system-first-law"
        }),
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .all(|hypothesis| {
                hypothesis.relation.as_ref().is_none_or(|relation| {
                    relation.relation_id != "thermodynamics-heat-transfer:closed-system-first-law"
                        || !matches!(
                            hypothesis.support,
                            crate::MathInterpretationSupportTier::Explicit
                                | crate::MathInterpretationSupportTier::Derived
                                | crate::MathInterpretationSupportTier::Supported
                        )
                })
            }),
        "{view:#?}"
    );
}

#[test]
fn unrelated_alternatives_cannot_withhold_an_earlier_sign_convention() {
    let content = "Under the heat in and work out convention, the model is fixed. The appendix lists either a circle or a square. Neither alternative is selected. For a closed system, the asserted relation is $\\Delta U=Q-W$.";
    let view = semantic_view_at(content, content.rfind("\\Delta U=Q-W").unwrap() as u32);

    assert!(
        view.authoring_context.conditions.iter().all(|condition| {
            condition
                .evidence
                .iter()
                .all(|evidence| evidence.rule_id != "scientific-prose/alternative-selection")
        }),
        "{view:#?}"
    );
    assert!(
        view.authoring_context.conditions.iter().any(|condition| {
            condition.kind == crate::ScientificConstraintKind::SignConvention
                && condition.status == crate::ConstraintStatus::Verified
        }),
        "{view:#?}"
    );
}

#[test]
fn rejected_decorated_formula_is_non_positive_at_root_and_nested_cursor() {
    let content = "The candidate equality $J_\\triangle=D_\\triangle+\\lambda_\\triangle P_\\triangle$ is not asserted.";
    for offset in [
        content.rfind("J_\\triangle").unwrap() as u32,
        (content.rfind("P_\\triangle").unwrap() + "P_\\triangle".len()) as u32,
    ] {
        let view = semantic_view_at(content, offset);
        assert!(
            matches!(view.decision, MeaningDecision::Unsupported { .. }),
            "{view:#?}"
        );
        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Unsupported,
            "{view:#?}"
        );
        assert!(
            view.authoring_context
                .interpretations
                .hypotheses
                .iter()
                .any(|hypothesis| {
                    hypothesis.support == crate::MathInterpretationSupportTier::Contradicted
                        && hypothesis.evidence.iter().any(|evidence| {
                            evidence.role == crate::MathInterpretationEvidenceRole::Contradicting
                                && !evidence.source_anchors.is_empty()
                        })
                }),
            "{view:#?}"
        );
    }
}

#[test]
fn unspecified_formula_context_is_not_hard_contradiction_evidence() {
    let content = "The intended interpretation is not specified here. Consider $J_\\triangle=D_\\triangle+\\lambda_\\triangle P_\\triangle$.";
    let view = semantic_view_at(content, content.rfind("J_\\triangle").unwrap() as u32);

    assert!(
        !matches!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        ),
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .all(|hypothesis| {
                hypothesis.hypothesis_id != "source-meaning"
                    || hypothesis.support != crate::MathInterpretationSupportTier::Contradicted
            }),
        "{view:#?}"
    );
}

#[test]
fn standalone_math_root_uses_formula_adjudication_without_cursor_entity_proof() {
    let content = "Let $x$ denote the established state. Inspect $x$.";
    let view = semantic_view_at(content, content.rfind("$x$").unwrap() as u32 + 1);

    assert!(matches!(view.decision, MeaningDecision::Established { .. }));
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial
    );
    assert!(view.authoring_context.requirements.is_empty());
}

#[test]
fn compacted_snapshot_keeps_the_analyzed_selected_root() {
    let content = "The selected relation is $a=b$.";
    let start = content.find("a=b").unwrap() as u32;
    let mut input = document("main", "main.tex", content, 1);
    input.nodes = [
        ("a", LexicalClass::Identifier),
        ("=", LexicalClass::Operator),
        ("b", LexicalClass::Identifier),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (text, lexical_class))| NotationNode {
        kind: NotationNodeKind::Token,
        parent: Some(3),
        children: Vec::new(),
        ranges: NotationNodeRanges {
            full: range(start + index as u32, start + index as u32 + 1),
            command: None,
            name: None,
            nucleus: None,
            editable: Some(range(start + index as u32, start + index as u32 + 1)),
        },
        state: SyntaxState::Complete,
        name: None,
        text: Some(text.into()),
        arguments: Vec::new(),
        lexical_class: Some(lexical_class),
        math_class: None,
        provenance: None,
    })
    .chain(std::iter::once(NotationNode {
        kind: NotationNodeKind::Sequence,
        parent: None,
        children: vec![0, 1, 2],
        ranges: NotationNodeRanges {
            full: range(start, start + 3),
            command: None,
            name: None,
            nucleus: None,
            editable: Some(range(start, start + 3)),
        },
        state: SyntaxState::Complete,
        name: None,
        text: None,
        arguments: Vec::new(),
        lexical_class: None,
        math_class: None,
        provenance: None,
    }))
    .collect();
    input.math_roots = vec![MathRoot {
        node: 3,
        delimiter: "inline-dollar".into(),
        full_range: range(start - 1, start + 4),
        content_range: range(start, start + 3),
        state: MathRootState::Complete,
    }];
    input.scopes = vec![SyntaxScope {
        kind: "document".into(),
        parent: None,
        range: range(0, content.len() as u32),
        state: MathRootState::Complete,
        name: None,
        level: None,
        source: None,
    }];

    let mut engine = SemathEngine::default();
    let mut project = snapshot(content);
    project.documents = vec![input];
    engine.reset(project).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: start,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{view:#?}"
    );
}

#[test]
fn subrelation_source_meaning_cannot_establish_a_mixed_sibling_root() {
    let content = "The selected relation is $a=b\\land c=d$.";
    let view = semantic_view_at(content, content.rfind("c=d").unwrap() as u32);

    assert!(
        !matches!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established
        ),
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter(|hypothesis| hypothesis.hypothesis_id == "source-meaning")
            .all(|hypothesis| {
                !matches!(
                    hypothesis.support,
                    crate::MathInterpretationSupportTier::Explicit
                        | crate::MathInterpretationSupportTier::Derived
                        | crate::MathInterpretationSupportTier::Supported
                )
            })
    );
}

#[test]
fn selected_root_formula_adjudication_is_invariant_across_sibling_cursors() {
    let content =
        "Let $A$ and $B$ be events. Conditional probability is $P(A\\mid B)=P(A\\cap B)/P(B)$.";
    let first = semantic_view_at(content, content.rfind("A\\mid B").unwrap() as u32);
    let second = semantic_view_at(content, content.rfind("A\\cap B").unwrap() as u32);
    let relation_ids = |view: &crate::SemanticViewInfo| {
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter_map(|hypothesis| {
                hypothesis
                    .relation
                    .as_ref()
                    .map(|relation| relation.relation_id.clone())
            })
            .collect::<BTreeSet<_>>()
    };

    let first_ids = relation_ids(&first);
    let second_ids = relation_ids(&second);
    assert_eq!(
        first_ids, second_ids,
        "first={first:#?}\nsecond={second:#?}"
    );
    assert!(
        first_ids.contains("probability:conditional-probability"),
        "{first_ids:?}"
    );
    assert!(
        first_ids.contains("probability:event-intersection"),
        "{first_ids:?}"
    );
    assert_eq!(
        first.authoring_context.disposition,
        second.authoring_context.disposition
    );
    assert_eq!(
        first.authoring_context.requirements,
        second.authoring_context.requirements
    );
    assert_eq!(
        first.authoring_context.conditions,
        second.authoring_context.conditions
    );
    assert_eq!(
        first.authoring_context.interpretations,
        second.authoring_context.interpretations
    );
    assert_eq!(
        first.authoring_context.lifecycle.engine_limited,
        second.authoring_context.lifecycle.engine_limited
    );
    assert_eq!(
        first.authoring_context.truncated,
        second.authoring_context.truncated
    );
}

#[test]
fn condition_missing_and_rejected_roots_cannot_export_relations() {
    let condition_missing = "The bore determines area $A$ and the meter reports cross-section mean speed $v$. Density $\\rho$ was sampled at the same temperature. The proposed mass rate is $\\dot m=\\rho A v$.";
    let view = semantic_view_at(
        condition_missing,
        condition_missing.rfind("\\dot m=").unwrap() as u32,
    );
    assert!(
        view.authoring_context
            .conditions
            .iter()
            .any(|condition| { condition.status == crate::ConstraintStatus::Required })
    );
    assert!(view.context.relations.is_empty(), "{view:#?}");
    assert!(view.authoring_context.equation_links.is_empty());

    let rejected = "Let $P$ be power, $F$ force, and $v$ velocity. The mechanical-power equation $P=F\\cdot v$ is rejected.";
    let view = semantic_view_at(rejected, rejected.rfind("P=").unwrap() as u32);
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported
    );
    assert!(view.context.relations.is_empty(), "{view:#?}");
    assert!(view.authoring_context.equation_links.is_empty());
}

#[test]
fn rejected_preceding_formula_cannot_form_an_equation_link() {
    let content = "Let $P$ and $R$ be power, $F$ force, and $v$ velocity. The reference equation $P=F\\cdot v$ is rejected. The accepted equation is $R=F\\cdot v$.";
    let view = semantic_view_at(content, content.rfind("R=").unwrap() as u32);

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{view:#?}"
    );
    assert!(
        view.authoring_context.equation_links.is_empty(),
        "{view:#?}"
    );
}

#[test]
fn condition_missing_preceding_formula_cannot_form_an_equation_link() {
    let content = "Let $K$ be kinetic energy, $m$ mass, $v$ velocity, $P$ power, and $F$ force. The candidate relation is $K=\\frac12mv^2$. The accepted relation is $P=F\\cdot v$.";
    let prior = semantic_view_at(content, content.find("K=\\frac12").unwrap() as u32);
    assert!(
        prior
            .authoring_context
            .conditions
            .iter()
            .any(|condition| { condition.status == crate::ConstraintStatus::Required })
    );

    let current = semantic_view_at(content, content.rfind("P=F\\cdot v").unwrap() as u32);
    assert_eq!(
        current.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{current:#?}"
    );
    assert!(
        current.authoring_context.equation_links.is_empty(),
        "{current:#?}"
    );
}

#[test]
fn conventional_preceding_formula_cannot_form_an_equation_link() {
    let content = "For a periodic signal, the asserted relation is $f=1/T$. Later, for same-phase propagation, let $f$ be cyclic frequency, $c$ wave propagation speed, and $\\lambda$ wavelength. The accepted wave relation is $c=f\\lambda$.";
    let prior = semantic_view_at(content, content.find("f=1/T").unwrap() as u32);
    assert_eq!(
        prior.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conventional,
        "{prior:#?}"
    );

    let current = semantic_view_at(content, content.rfind("c=f\\lambda").unwrap() as u32);
    assert_eq!(
        current.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{current:#?}"
    );
    assert!(
        current.authoring_context.equation_links.is_empty(),
        "{current:#?}"
    );
}

#[test]
fn retracted_preceding_formula_cannot_form_an_equation_link() {
    let content = "Let $P$ and $R$ be power, $F$ force, and $v$ velocity. The relation displayed next is withdrawn and retained only as an archival quotation: $P=F\\cdot v$. The accepted relation is $R=F\\cdot v$.";
    let current = semantic_view_at(content, content.rfind("R=").unwrap() as u32);

    assert_eq!(
        current.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{current:#?}"
    );
    assert!(
        current.authoring_context.equation_links.is_empty(),
        "{current:#?}"
    );
}

#[test]
fn asserted_only_recognition_is_tentative_and_cannot_export_a_relation() {
    let content = "For a Newtonian fluid, the Newtonian shear relation is $x=y\\dot z$, but the report does not identify the roles of x, y, or z.";
    let view = semantic_view_at(content, content.find("x=y\\dot z").unwrap() as u32);

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial,
        "{view:#?}"
    );
    assert!(view.context.relations.is_empty(), "{view:#?}");
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter(|hypothesis| hypothesis.hypothesis_id.contains("newtonian-shear"))
            .all(|hypothesis| {
                hypothesis.support == crate::MathInterpretationSupportTier::Tentative
            }),
        "{view:#?}"
    );
}

#[test]
fn strictly_exported_selected_root_is_established_and_explicit() {
    let content = "Let $M$ be a p by q matrix, $x$ a q-dimensional vector, and $w$ a p-dimensional vector. The mapped vector is $w=Mx$.";
    let view = semantic_view_at(content, content.rfind("w=Mx").unwrap() as u32);

    assert!(
        view.context
            .relations
            .iter()
            .any(|relation| { relation.relation_id == "linear-algebra:matrix-vector-product" }),
        "{view:#?}"
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{view:#?}"
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .any(|hypothesis| {
                hypothesis.hypothesis_id == "linear-algebra:matrix-vector-product"
                    && hypothesis.support == crate::MathInterpretationSupportTier::Explicit
            }),
        "{view:#?}"
    );
}

#[test]
fn complete_root_law_outranks_its_nested_explanatory_relation() {
    let content = "Let $A$ and $B$ be events with positive $\\mathbb{P}(B)$. Conditional probability is $\\mathbb{P}(A\\mid B)=\\frac{\\mathbb{P}(A\\cap B)}{\\mathbb{P}(B)}$.";
    let view = semantic_view_at(
        content,
        content.rfind("\\mathbb{P}(A\\mid B)").unwrap() as u32,
    );

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{view:#?}"
    );
    let relation_ids = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .filter_map(|hypothesis| {
            hypothesis
                .relation
                .as_ref()
                .map(|relation| relation.relation_id.as_str())
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(relation_ids.contains("probability:conditional-probability"));
    assert!(relation_ids.contains("probability:event-intersection"));
}

#[test]
fn asserted_only_preceding_recognition_cannot_form_an_equation_link() {
    let content = "For a Newtonian fluid, the Newtonian shear relation is $x=y\\dot z$, but the report does not identify the roles of x, y, or z. Let $P$ be power, $F$ force, and $x$ velocity. The accepted power relation is $P=F\\cdot x$.";
    let prior = semantic_view_at(content, content.find("x=y\\dot z").unwrap() as u32);
    assert!(prior.context.relations.is_empty(), "{prior:#?}");

    let current = semantic_view_at(content, content.rfind("P=F\\cdot x").unwrap() as u32);
    assert_eq!(
        current.authoring_context.disposition,
        crate::MathAuthoringDisposition::Established,
        "{current:#?}"
    );
    assert!(
        current.authoring_context.equation_links.is_empty(),
        "{current:#?}"
    );
}

#[test]
fn source_grounded_capacitor_roles_support_the_conventional_formula() {
    let content = "\\section{Capacitor current}\nLet $C>0$ be a constant capacitance, let $v_C(t)$ be the voltage from the marked positive terminal to the marked negative terminal, and let $i_C(t)$ enter the positive terminal. Under this passive sign convention, the capacitor law is\n\\[\ni_C(t)=C\\frac{dv_C}{dt}(t).\n\\]\nThe relation applies where $v_C$ is differentiable; reversing the current reference would reverse the sign.\n";
    let view = semantic_view_at(content, content.rfind("i_C(t)=").unwrap() as u32);

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conventional,
        "{view:#?}"
    );
    let capacitor_hypotheses =
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter(|hypothesis| {
                hypothesis.relation.as_ref().is_some_and(|relation| {
                    relation.relation_id == "circuits:capacitor-current-law"
                })
            })
            .collect::<Vec<_>>();
    assert!(
        capacitor_hypotheses.iter().any(|hypothesis| {
            hypothesis.support == crate::MathInterpretationSupportTier::Supported
        }),
        "{capacitor_hypotheses:#?}"
    );
    assert!(view.context.relations.is_empty(), "{view:#?}");
}

#[test]
fn proposed_source_described_formula_is_unsupported_without_exporting_authority() {
    let content = "The proposed estimator is $\\widehat x\\in\\operatorname*{argmin}_{x\\in\\mathbb R^n}\\lVert Ax-b\\rVert_2^2$.";
    let view = semantic_view_at(content, content.find("\\widehat x").unwrap() as u32);

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported,
        "{view:#?}"
    );
    assert!(view.context.relations.is_empty(), "{view:#?}");
    assert!(view.authoring_context.equation_links.is_empty());
}

#[test]
fn incremental_upsert_matches_the_new_document_version() {
    let original = "Let $x$ denote the input. $y=x$";
    let changed = "Let $x$ denote the state. $y=x$";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(original)).unwrap();
    engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", changed, 2)),
            }],
        })
        .unwrap();
    let offset = changed.rfind('x').unwrap() as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset,
            },
            2,
            2,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert_eq!(view.authoring_context.lifecycle.document_version, 2);
    assert!(
        view.authoring_context
            .notation_occurrences
            .iter()
            .all(|occurrence| occurrence.occurrence_id.document_version == 2)
    );
    assert_eq!(view.symbol.unwrap().definitions[0].description, "the state");
    let stale = engine.query(query(
        Query::SemanticView {
            file_id: "main".into(),
            offset,
        },
        2,
        1,
    ));
    assert!(matches!(stale, Err(EngineError::DocumentVersionMismatch)));
}

#[test]
fn same_revision_opaque_relink_reanalyzes_without_accepting_stale_text() {
    let content = "Let $A$ and $B$ be events. $\\joint{A}{B}$";
    let start = content.find("\\joint").unwrap() as u32;
    let source = ProjectSourceRef {
        file_id: "main".into(),
        path: "main.tex".into(),
        range: range(start, start + "\\joint".len() as u32),
    };
    let mut expanded = document("main", "main.tex", content, 1);
    expanded.macros.push(ProjectMacro {
        kind: ProjectMacroKind::Call,
        name: "joint".into(),
        source: source.clone(),
        definitions: Vec::new(),
        expansion: ProjectMacroExpansion {
            status: ProjectMacroExpansionStatus::Expanded,
            depth: 1,
            editable: false,
            surface: Some("A \\cap B".into()),
            input_range: Some(range(start, start + "\\joint{A}{B}".len() as u32)),
            notation: None,
        },
    });
    let mut opaque = document("main", "main.tex", content, 1);
    opaque.macros.push(ProjectMacro {
        kind: ProjectMacroKind::Call,
        name: "joint".into(),
        source,
        definitions: Vec::new(),
        expansion: ProjectMacroExpansion {
            status: ProjectMacroExpansionStatus::Expanded,
            depth: 1,
            editable: false,
            surface: Some("\\csname A\\endcsname".into()),
            input_range: Some(range(start, start + "\\joint{A}{B}".len() as u32)),
            notation: Some(GeneratedNotationTree {
                nodes: vec![GeneratedNotationNode {
                    kind: NotationNodeKind::Command,
                    children: Vec::new(),
                    state: SyntaxState::Opaque,
                    name: Some("csname".into()),
                    text: None,
                    arguments: Vec::new(),
                    lexical_class: None,
                    math_class: None,
                }],
                root: 0,
            }),
        },
    });

    let mut project = snapshot(content);
    project.documents = vec![expanded];
    let mut engine = SemathEngine::default();
    engine.reset(project).unwrap();
    let update = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(opaque),
            }],
        })
        .unwrap();

    assert_eq!(update.changed_file_ids, ["main"]);
    assert_eq!(update.analyzed_file_ids, ["main"]);
    assert!(engine.index.documents["main"].engine_limited_ranges[0].contains(start + 1));
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: start,
            },
            2,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(
            &view.decision,
            MeaningDecision::Unsupported { reasons }
                if reasons.iter().any(|reason| reason.kind == crate::DecisionReasonKind::EngineLimit)
        ),
        "{:#?}",
        view.decision
    );
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::EngineLimited
    );
    assert!(view.authoring_context.lifecycle.engine_limited);
    assert!(!view.authoring_context.lifecycle.editable);
    assert!(
        view.authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .any(|limit| { limit.kind == crate::MathInterpretationAnalysisLimitKind::EngineLimit })
    );
}

#[test]
fn formula_authoring_limits_are_invariant_across_opaque_and_ordinary_siblings() {
    let content = "Let $x$ denote the state. The mixed expression is $x+\\joint{A}{B}$.";
    let macro_start = content.find("\\joint").unwrap() as u32;
    let macro_end = macro_start + "\\joint{A}{B}".len() as u32;
    let mut opaque = document("main", "main.tex", content, 1);
    opaque.macros.push(ProjectMacro {
        kind: ProjectMacroKind::Call,
        name: "joint".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: range(macro_start, macro_start + "\\joint".len() as u32),
        },
        definitions: Vec::new(),
        expansion: ProjectMacroExpansion {
            status: ProjectMacroExpansionStatus::Expanded,
            depth: 1,
            editable: false,
            surface: Some("\\csname A\\endcsname".into()),
            input_range: Some(range(macro_start, macro_end)),
            notation: Some(GeneratedNotationTree {
                nodes: vec![GeneratedNotationNode {
                    kind: NotationNodeKind::Command,
                    children: Vec::new(),
                    state: SyntaxState::Opaque,
                    name: Some("csname".into()),
                    text: None,
                    arguments: Vec::new(),
                    lexical_class: None,
                    math_class: None,
                }],
                root: 0,
            }),
        },
    });
    let mut project = snapshot(content);
    project.documents = vec![opaque];
    let mut engine = SemathEngine::default();
    engine.reset(project).unwrap();
    let query_view = |offset| {
        let QueryValue::SemanticView { view } = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset,
                },
                1,
                1,
            ))
            .unwrap()
            .value
        else {
            panic!("expected semantic view")
        };
        view
    };
    let ordinary_offset = content.find("$x+").unwrap() as u32 + 1;
    let ordinary = query_view(ordinary_offset);
    let opaque = query_view(macro_start + 1);

    assert_eq!(
        ordinary.authoring_context.disposition,
        opaque.authoring_context.disposition
    );
    assert_eq!(
        ordinary.authoring_context.lifecycle.engine_limited,
        opaque.authoring_context.lifecycle.engine_limited
    );
    assert!(ordinary.authoring_context.lifecycle.engine_limited);
    assert_eq!(
        ordinary.authoring_context.interpretations.analysis_limits,
        opaque.authoring_context.interpretations.analysis_limits
    );
    assert!(
        ordinary
            .authoring_context
            .interpretations
            .analysis_limits
            .iter()
            .any(|limit| { limit.kind == crate::MathInterpretationAnalysisLimitKind::EngineLimit })
    );
}

#[test]
fn append_only_comments_advance_the_version_without_semantic_reanalysis() {
    let original = "Let $x$ denote the input. $y=x$";
    let changed = format!("{original}\n% editor note\n  % another note");
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(original)).unwrap();

    let update = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", &changed, 2)),
            }],
        })
        .unwrap();

    assert_eq!(update.changed_file_ids, ["main"]);
    assert!(update.analyzed_file_ids.is_empty());
    let result = engine
        .query(query(
            Query::Definition {
                file_id: "main".into(),
                offset: changed.rfind('x').unwrap() as u32,
            },
            2,
            2,
        ))
        .unwrap();
    let QueryValue::Locations { locations, .. } = result.value else {
        panic!("expected locations")
    };
    assert_eq!(locations.len(), 1);
}

#[test]
fn non_comment_suffixes_still_trigger_semantic_reanalysis() {
    let original = "Let $x$ denote the input. $y=x$";
    let changed = format!("{original}\nLet $z$ denote the output.");
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(original)).unwrap();

    let update = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", &changed, 2)),
            }],
        })
        .unwrap();

    assert_eq!(update.analyzed_file_ids, ["main"]);
}

#[test]
fn protocol_requires_the_structural_frontend_contract() {
    let payload = serde_json::json!({
        "protocolVersion": PROTOCOL_VERSION,
        "epoch": "project:1",
        "inventoryVersion": 1,
        "projectId": "project",
        "documents": [{
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "$x$",
            "documentVersion": 1
        }]
    });
    let mut engine = SemathEngine::default();
    assert!(
        engine
            .reset_json(&serde_json::to_vec(&payload).unwrap())
            .is_err()
    );
}

#[test]
fn included_type_declarations_drive_project_law_inference() {
    let main = "\\input{definitions}\nUnder a consistent sign convention, $V=RI$";
    let definitions = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: 0,
                end_offset: 19,
            },
        },
    });
    let mut engine = SemathEngine::default();
    let update = engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();
    assert_eq!(update.stats.analyzed_documents, 2);
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.find('=').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(matches!(view.decision, MeaningDecision::Partial { .. }));
    assert_eq!(view.context.relations[0].relation_id, "circuits:ohm-law");
    let included_anchor = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .flat_map(|hypothesis| &hypothesis.evidence)
        .flat_map(|evidence| &evidence.source_anchors)
        .find(|anchor| anchor.location.file_id == "definitions")
        .expect("included evidence keeps its own document anchor");
    assert_eq!(included_anchor.location.path, "definitions.tex");
    assert_eq!(included_anchor.document_version, 1);
    assert_eq!(
        included_anchor.lifecycle,
        crate::MathInterpretationSourceLifecycle::Current
    );
    assert_eq!(
        included_anchor.generation,
        crate::MathSourceGeneration::Authored
    );

    let symbol_result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.find("V=").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = symbol_result.value else {
        panic!("expected semantic view")
    };
    let symbol = view.symbol.expect("expected V symbol information");
    assert_eq!(symbol.roles[0].concept_id, "quantities-units:voltage");
    assert_eq!(symbol.roles[0].evidence.source_ranges[0].start_offset, 0);
}

#[test]
fn display_led_relational_claims_are_format_neutral_and_remain_partial() {
    let roles = "The design matrix satisfies $A\\in\\mathbb R^{m\\times n}$, the observation vector satisfies $b\\in\\mathbb R^m$, and the parameter vector satisfies $x\\in\\mathbb R^n$, with $m\\ge n$.\n";
    let cases = [
        (
            DocumentLanguage::Latex,
            "least-squares-method.tex",
            "least-squares-roles.tex",
            "\\section{Least-squares estimate}\nThe role declarations are provided by the exact project path \\texttt{least-squares-roles.tex}.\n\\input{least-squares-roles.tex}\nThe draft calls $\\widehat x$ the unique estimate and defines it by\n\\[\n\\widehat x\\in\\operatorname*{argmin}_{x\\in\\mathbb R^n}\\lVert Ax-b\\rVert_2^2.\n\\]\nNo full-column-rank assumption for $A$ is stated, so uniqueness still requires justification.\n",
            "\\input{least-squares-roles.tex}",
            &[
                (9, 31),
                (33, 93),
                (102, 125),
                (126, 127),
                (160, 175),
                (189, 226),
                (309, 343),
                (348, 402),
            ][..],
        ),
        (
            DocumentLanguage::Markdown,
            "least-squares-method.md",
            "least-squares-roles.md",
            "# Least-squares estimate\n\nThe role declarations are provided by the exact project path [least-squares-roles.md](least-squares-roles.md). The draft calls $\\widehat x$ the unique estimate and defines it by\n\n$$\n\\widehat x\\in\\operatorname*{argmin}_{x\\in\\mathbb R^n}\\lVert Ax-b\\rVert_2^2.\n$$\n\nNo full-column-rank assumption for $A$ is stated, so uniqueness still requires justification.\n",
            "The role declarations are provided by the exact project path [least-squares-roles.md](least-squares-roles.md).",
            &[(0, 152), (166, 203), (288, 322), (327, 381)][..],
        ),
    ];

    for (language, main_path, roles_path, source, include_surface, visible_ranges) in cases {
        let mut main = document_with_language("main", main_path, source, 1, language);
        main.math_roots = main
            .math_regions
            .iter()
            .enumerate()
            .map(|(node, region)| MathRoot {
                node: node as u32,
                delimiter: region.delimiter.clone(),
                full_range: region.full_range.clone(),
                content_range: region.content_range.clone(),
                state: MathRootState::Complete,
            })
            .collect();
        main.visible_prose = visible_ranges
            .iter()
            .map(|(start, end)| VisibleProseSpan {
                range: range(*start, *end),
                state: CompleteSyntaxState::Complete,
            })
            .collect();
        let include_start = source.find(include_surface).unwrap() as u32;
        main.includes.push(ProjectInclude {
            path: roles_path.into(),
            kind: "input".into(),
            source: ProjectSourceRef {
                file_id: "main".into(),
                path: main_path.into(),
                range: range(include_start, include_start + include_surface.len() as u32),
            },
        });
        let roles_document = document_with_language("roles", roles_path, roles, 1, language);
        let formula_start = source.rfind("\\widehat x\\in").unwrap() as u32;
        let mut engine = SemathEngine::default();
        engine
            .reset(ProjectSnapshot {
                protocol_version: PROTOCOL_VERSION,
                epoch: "project:1".into(),
                inventory_version: 1,
                project_id: "project".into(),
                main_file_id: Some("main".into()),
                documents: vec![main, roles_document],
            })
            .unwrap();
        let QueryValue::SemanticView { view } = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: formula_start,
                },
                1,
                1,
            ))
            .unwrap()
            .value
        else {
            panic!("expected semantic view")
        };
        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Partial,
            "{language:?}: {:#?}",
            view.authoring_context
        );
        let claim_start = source.find("The draft calls").unwrap() as u32;
        let claim_end = source
            .find("\n\\[")
            .or_else(|| source.find("\n\n$$"))
            .unwrap() as u32;
        let matching_claims = view
            .authoring_context
            .claim_evidence
            .iter()
            .filter(|claim| {
                claim.claim.range == range(claim_start, claim_end)
                    && claim.modality == crate::MathClaimModality::Asserted
                    && claim.polarity == crate::MathClaimPolarity::Positive
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching_claims.len(),
            1,
            "{language:?}: {:#?}",
            view.authoring_context.claim_evidence
        );
    }
}

#[test]
fn identical_cross_document_evidence_ranges_keep_exact_source_identities() {
    let main = "\\input{definitions-a}\n\\input{definitions-b}\n$V=RI$";
    let definitions = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let mut main_document = document("main", "main.tex", main, 1);
    let second_include_start = main.find("\\input{definitions-b}").unwrap() as u32;
    main_document.includes.extend([
        ProjectInclude {
            path: "definitions-a".into(),
            kind: "input".into(),
            source: ProjectSourceRef {
                file_id: "main".into(),
                path: "main.tex".into(),
                range: SourceRange {
                    start_offset: 0,
                    end_offset: second_include_start - 1,
                },
            },
        },
        ProjectInclude {
            path: "definitions-b".into(),
            kind: "input".into(),
            source: ProjectSourceRef {
                file_id: "main".into(),
                path: "main.tex".into(),
                range: SourceRange {
                    start_offset: second_include_start,
                    end_offset: main.rfind('\n').unwrap() as u32,
                },
            },
        },
    ]);
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions-a", "definitions-a.tex", definitions, 1),
                document("definitions-b", "definitions-b.tex", definitions, 1),
            ],
        })
        .unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.find('=').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    let binding_evidence = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .flat_map(|hypothesis| &hypothesis.evidence)
        .filter(|item| item.evidence.kind == "canonical-binding")
        .collect::<Vec<_>>();
    assert!(!binding_evidence.is_empty());
    let anchored_files = binding_evidence
        .iter()
        .flat_map(|item| &item.source_anchors)
        .map(|anchor| anchor.location.file_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(anchored_files.contains("definitions-a"));
    assert!(anchored_files.contains("definitions-b"));
    assert!(!anchored_files.contains("main"));
    for item in binding_evidence {
        assert!(!item.source_anchors.is_empty());
        assert_eq!(item.evidence.source_ranges.len(), item.source_anchors.len());
        assert!(
            item.evidence
                .source_ranges
                .iter()
                .zip(&item.source_anchors)
                .all(|(range, anchor)| anchor.location.range == *range)
        );
        assert!(
            item.source_anchors
                .iter()
                .all(|anchor| { item.evidence.source_ranges.contains(&anchor.location.range) })
        );
        assert!(item.evidence.source_ranges.iter().all(|range| {
            item.source_anchors
                .iter()
                .any(|anchor| anchor.location.range == *range)
        }));
        assert!(item.source_anchors.windows(2).all(|pair| {
            pair[0]
                .location
                .file_id
                .cmp(&pair[1].location.file_id)
                .then(pair[0].document_version.cmp(&pair[1].document_version))
                .then(
                    pair[0]
                        .location
                        .range
                        .start_offset
                        .cmp(&pair[1].location.range.start_offset),
                )
                .then(
                    pair[0]
                        .location
                        .range
                        .end_offset
                        .cmp(&pair[1].location.range.end_offset),
                )
                .is_le()
        }));
        assert_eq!(item.evidence.source_anchors, item.source_anchors);
    }
}

#[test]
fn included_ordered_definition_evidence_keeps_its_source_document_anchor() {
    let main = "\\input{roles.tex}\nThe least squares approximation remains directional: $v\\approx D\\beta$.";
    let roles = "Let $v$, $D$, and $\\beta$ denote vector, linear operator matrix, and vector, respectively.";
    let mut main_document = document("methods", "methods.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "roles.tex".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "methods".into(),
            path: "methods.tex".into(),
            range: range(0, "\\input{roles.tex}".len() as u32),
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("methods".into()),
            documents: vec![main_document, document("roles", "roles.tex", roles, 1)],
        })
        .unwrap();
    let QueryValue::SemanticView { view } = engine
        .query(query(
            Query::SemanticView {
                file_id: "methods".into(),
                offset: main.find("D\\beta").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap()
        .value
    else {
        panic!("expected semantic view")
    };

    assert_eq!(
        view.authoring_context.requirements,
        view.authoring_context
            .interpretations
            .missing_discriminators
    );
    let projected_requirement_evidence = view
        .authoring_context
        .requirements
        .iter()
        .flat_map(|requirement| match requirement {
            crate::MathInterpretationRequirementInfo::Declaration { evidence, .. }
            | crate::MathInterpretationRequirementInfo::RoleDeclaration { evidence, .. }
            | crate::MathInterpretationRequirementInfo::Disambiguation { evidence, .. } => {
                evidence.as_slice()
            }
            crate::MathInterpretationRequirementInfo::Condition { condition, .. } => {
                condition.evidence.as_slice()
            }
        })
        .filter(|item| {
            matches!(
                item.evidence.rule_id.as_str(),
                "english-respectively-definition" | "english-clause-ordered-definition"
            )
        })
        .collect::<Vec<_>>();
    assert!(!projected_requirement_evidence.is_empty());
    for item in projected_requirement_evidence {
        assert_eq!(item.evidence.source_ranges.len(), item.source_anchors.len());
        assert!(
            item.evidence
                .source_ranges
                .iter()
                .zip(&item.source_anchors)
                .all(|(range, anchor)| {
                    anchor.location.file_id == "roles"
                        && anchor.location.path == "roles.tex"
                        && anchor.location.range == *range
                })
        );
    }

    let ordered_definition_evidence = view
        .authoring_context
        .interpretations
        .hypotheses
        .iter()
        .flat_map(|hypothesis| &hypothesis.evidence)
        .filter(|item| {
            matches!(
                item.evidence.rule_id.as_str(),
                "english-respectively-definition" | "english-clause-ordered-definition"
            )
        })
        .collect::<Vec<_>>();
    assert!(ordered_definition_evidence.is_empty());
}

#[test]
fn conventional_notation_does_not_downgrade_included_type_proof() {
    let main = "\\input{definitions}\nThe linear system is $Ax=b$.";
    let definitions = "Let $A$ denote a matrix, $x$ a vector, and $b$ a vector.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: 0,
                end_offset: 19,
            },
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();

    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.find("Ax=b").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(matches!(view.decision, MeaningDecision::Established { .. }));
    assert!(view.authoring_context.conventional_candidates.is_empty());
}

#[test]
fn asserted_project_reference_drives_and_retracts_source_ordered_law_inference() {
    let referenced = "Using the definitions in \\texttt{definitions.tex}. Under a consistent sign convention, $V=RI$.";
    let detached = "Under a consistent sign convention, $V=RI$.";
    let definitions = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                document("main", "main.tex", referenced, 1),
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();

    let relation_ids = |engine: &SemathEngine, content: &str, version| {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: content.find('=').unwrap() as u32,
                },
                version,
                version,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        view.context
            .relations
            .into_iter()
            .map(|relation| relation.relation_id)
            .collect::<BTreeSet<_>>()
    };
    assert!(relation_ids(&engine, referenced, 1).contains("circuits:ohm-law"));

    let update = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", detached, 2)),
            }],
        })
        .unwrap();
    assert_eq!(update.analyzed_file_ids, ["main"]);
    assert!(!relation_ids(&engine, detached, 2).contains("circuits:ohm-law"));
}

#[test]
fn referenced_document_changes_reanalyze_dependents() {
    let main = "Following the declarations in `shared/definitions.md`. Under a consistent sign convention, $V=RI$.";
    let definitions = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let withdrawn = "This document contains no symbol declarations.";
    let relation_ids = |engine: &SemathEngine, inventory_version| {
        let result = engine
            .query(query(
                Query::SemanticView {
                    file_id: "main".into(),
                    offset: main.find('=').unwrap() as u32,
                },
                inventory_version,
                1,
            ))
            .unwrap();
        let QueryValue::SemanticView { view } = result.value else {
            panic!("expected semantic view")
        };
        view.context
            .relations
            .into_iter()
            .map(|relation| relation.relation_id)
            .collect::<BTreeSet<_>>()
    };
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                document("main", "main.tex", main, 1),
                document("definitions", "shared/definitions.md", definitions, 1),
            ],
        })
        .unwrap();
    assert!(relation_ids(&engine, 1).contains("circuits:ohm-law"));

    let update = engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document(
                    "definitions",
                    "shared/definitions.md",
                    withdrawn,
                    2,
                )),
            }],
        })
        .unwrap();
    assert_eq!(
        update
            .analyzed_file_ids
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["definitions".into(), "main".into()])
    );
    let incremental_relations = relation_ids(&engine, 2);

    let mut clean = SemathEngine::default();
    clean
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                document("main", "main.tex", main, 1),
                document("definitions", "shared/definitions.md", withdrawn, 2),
            ],
        })
        .unwrap();
    let clean_relations = relation_ids(&clean, 2);
    assert_eq!(incremental_relations, clean_relations);
    assert!(!incremental_relations.contains("circuits:ohm-law"));
}

#[test]
fn included_assumptions_verify_conditions_without_cross_component_leakage() {
    let main = "\\input{definitions}\n$A \\cap B$";
    let definitions = "Let $A$ and $B$ be events in the same probability space.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: range(0, "\\input{definitions}".len() as u32),
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();
    let formula_offset = main.find("A \\cap B").unwrap() as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: formula_offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        matches!(view.decision, MeaningDecision::Established { .. }),
        "{view:#?}"
    );

    let local = "Let $A$ be an event. Let $B$ be an event.\n$A \\cap B$";
    let mut disconnected_engine = SemathEngine::default();
    disconnected_engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                document("main", "main.tex", local, 1),
                document(
                    "disconnected",
                    "disconnected.tex",
                    "Events $A$ and $B$ belong to the same probability space.",
                    1,
                ),
            ],
        })
        .unwrap();
    let result = disconnected_engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: local.find("A \\cap B").unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Partial
    );
}

#[test]
fn included_law_name_activates_the_typed_relation() {
    let main = "\\input{roles}\n$y=cx+tz$";
    let roles = "For Discrete state equation, let $y$ denote n-dimensional system state vector, $c$ denote n by n state matrix, $x$ denote n-dimensional system state vector, $t$ denote n by n input matrix, and $z$ denote n-dimensional control input vector.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "roles".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: range(0, 13),
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![main_document, document("roles", "roles.tex", roles, 1)],
        })
        .unwrap();
    let formula_offset = main.find("y=").unwrap() as u32;
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: formula_offset,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.context
            .relations
            .iter()
            .any(|relation| relation.relation_id == "control-systems:discrete-state-equation")
    );
}

#[test]
fn declarations_in_a_later_include_do_not_flow_backwards() {
    let main = "$V=RI$\n\\input{definitions}";
    let definitions = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let include_start = main.find("\\input").unwrap() as u32;
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: include_start,
                end_offset: main.len() as u32,
            },
        },
    });
    let mut engine = SemathEngine::default();
    engine
        .reset(ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 1,
            project_id: "project".into(),
            main_file_id: Some("main".into()),
            documents: vec![
                main_document,
                document("definitions", "definitions.tex", definitions, 1),
            ],
        })
        .unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: main.find('=').unwrap() as u32,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.symbol
            .expect("expected V symbol information")
            .roles
            .is_empty(),
        "a declaration included after the formula must not be visible at the formula"
    );
}

#[test]
fn incremental_project_type_refresh_matches_a_clean_rebuild() {
    let main = "\\input{definitions}\n$V=RI$";
    let original = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be a function.";
    let changed = "Let $V$ be voltage. Let $R$ be resistance. Let $I$ be electric current.";
    let mut main_document = document("main", "main.tex", main, 1);
    main_document.includes.push(ProjectInclude {
        path: "definitions".into(),
        kind: "input".into(),
        source: ProjectSourceRef {
            file_id: "main".into(),
            path: "main.tex".into(),
            range: SourceRange {
                start_offset: 0,
                end_offset: 19,
            },
        },
    });
    let base_snapshot = ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION,
        epoch: "project:1".into(),
        inventory_version: 1,
        project_id: "project".into(),
        main_file_id: Some("main".into()),
        documents: vec![
            main_document.clone(),
            document("definitions", "definitions.tex", original, 1),
        ],
    };
    let mut incremental = SemathEngine::default();
    incremental.reset(base_snapshot).unwrap();
    incremental
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("definitions", "definitions.tex", changed, 2)),
            }],
        })
        .unwrap();

    let clean_snapshot = ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION,
        epoch: "project:1".into(),
        inventory_version: 2,
        project_id: "project".into(),
        main_file_id: Some("main".into()),
        documents: vec![
            main_document,
            document("definitions", "definitions.tex", changed, 2),
        ],
    };
    let mut clean = SemathEngine::default();
    clean.reset(clean_snapshot).unwrap();
    let semantic_query = Query::SemanticView {
        file_id: "main".into(),
        offset: main.find('=').unwrap() as u32,
    };
    let incremental_value = incremental
        .query(query(semantic_query.clone(), 2, 1))
        .unwrap()
        .value;
    let clean_value = clean.query(query(semantic_query, 2, 1)).unwrap().value;
    assert_eq!(
        serde_json::to_value(incremental_value).unwrap(),
        serde_json::to_value(clean_value).unwrap()
    );
}

#[test]
fn equality_rhs_identity_projects_the_transferred_shape_at_later_uses() {
    let content =
        "Let $x$ be a three-dimensional vector and let $x=y$. Inspect $y$ after the equality.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.rfind("$y$").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    let symbol = view.symbol.expect("expected y symbol information");
    assert!(
        symbol
            .shapes
            .iter()
            .any(|shape| { shape.kind == "vector" && shape.dimensions == ["3".to_owned()] }),
        "{:?}",
        symbol.shapes
    );
}

#[test]
fn equality_chain_transfers_and_retracts_typed_facts_as_one_dependency_closure() {
    let content = "Let $x$ be a three-dimensional vector. $y=x$. $z=y$. Inspect $z$.";
    let mut engine = SemathEngine::default();
    engine.reset(snapshot(content)).unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: content.rfind("$z$").unwrap() as u32 + 1,
            },
            1,
            1,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.symbol.as_ref().is_some_and(|symbol| symbol
            .shapes
            .iter()
            .any(|shape| { shape.kind == "vector" && shape.dimensions == ["3".to_owned()] })),
        "{:?}",
        view.symbol
    );
    assert!(view.context.claims.iter().any(|claim| {
        claim.predicate == "shape"
            && claim.value == "Vector[3]"
            && claim
                .evidence
                .iter()
                .any(|evidence| evidence.rule_id == "semath/constraint/equality")
    }));

    let retracted = "Let $x$ be a three-dimensional vector. $z=y$. Inspect $z$.";
    engine
        .apply(ChangeEnvelope {
            protocol_version: PROTOCOL_VERSION,
            epoch: "project:1".into(),
            inventory_version: 2,
            analysis_generation: 2,
            changes: vec![ProjectChange::Upsert {
                document: Box::new(document("main", "main.tex", retracted, 2)),
            }],
        })
        .unwrap();
    let result = engine
        .query(query(
            Query::SemanticView {
                file_id: "main".into(),
                offset: retracted.rfind("$z$").unwrap() as u32 + 1,
            },
            2,
            2,
        ))
        .unwrap();
    let QueryValue::SemanticView { view } = result.value else {
        panic!("expected semantic view")
    };
    assert!(
        view.symbol
            .as_ref()
            .is_some_and(|symbol| symbol.shapes.is_empty()),
        "{:?}",
        view.symbol
    );
    assert!(
        view.context
            .claims
            .iter()
            .all(|claim| claim.value != "Vector[3]"),
        "{:?}",
        view.context.claims
    );
}

#[test]
fn explicit_role_shapes_do_not_conflict_with_neutral_formula_structure() {
    let cases = [
        (
            "For Bernoulli head decomposition, suppose $r$ is total head scalar, $a$ is pressure head scalar, $b$ is velocity head scalar, and $j$ is elevation head scalar. $r = a + b + j$",
            "r = a + b + j",
            "fluid-mechanics:bernoulli-head-decomposition",
        ),
        (
            "For Continuous Lyapunov equation, suppose $r$ is n by n state matrix, $a$ is n by n lyapunov certificate matrix, and $b$ is n by n lyapunov forcing matrix. $0 = r^T a + a r + b$",
            "0 = r^T a + a r + b",
            "control-systems:continuous-lyapunov-equation",
        ),
        (
            "For Thermal resistance relation, here $o$ denotes temperature difference scalar, $m$ denotes heat-transfer rate scalar, and $i$ denotes thermal resistance scalar. $o = m i$",
            "o = m i",
            "thermodynamics-heat-transfer:thermal-resistance-rate",
        ),
    ];

    for (content, formula, relation_id) in cases {
        let view = semantic_view_at(content, content.rfind(formula).unwrap() as u32 + 1);
        assert_eq!(
            view.authoring_context.disposition,
            crate::MathAuthoringDisposition::Established,
            "{relation_id}: diagnostics={:#?}",
            view.diagnostics,
        );
        assert!(
            view.authoring_context
                .interpretations
                .hypotheses
                .iter()
                .any(|hypothesis| hypothesis.hypothesis_id == relation_id),
            "{relation_id}: hypotheses={:#?}",
            view.authoring_context.interpretations.hypotheses,
        );
    }
}

#[test]
fn explicit_nonestablishment_refuses_the_selected_formula_relation() {
    let content = "The altered expression does not establish the reviewed probability-statistics relation. $-\\mu=E(X)$";
    let view = semantic_view_at(content, content.find("-\\mu=E(X)").unwrap() as u32);

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Unsupported,
        "diagnostics={:#?}",
        view.diagnostics,
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .all(|hypothesis| hypothesis.hypothesis_id != "probability:expected-value-definition"),
        "hypotheses={:#?}",
        view.authoring_context.interpretations.hypotheses,
    );
}

#[test]
fn selected_formula_conflicts_retain_the_exact_formula_owner() {
    let content = "Let $x$ and $y$ denote scalar calibration values. The first normative claim is $x=y$. The second normative claim is $x\\ne y$. Both claims are simultaneously binding.";
    let view = semantic_view_at(content, content.find("x\\ne y").unwrap() as u32);
    let formula = view.authoring_context.formula.as_ref().expect("formula");

    assert_eq!(
        view.authoring_context.disposition,
        crate::MathAuthoringDisposition::Conflicting,
        "{view:#?}",
    );
    assert!(
        view.authoring_context
            .interpretations
            .hypotheses
            .iter()
            .filter(|hypothesis| {
                hypothesis.support == crate::MathInterpretationSupportTier::Contradicted
            })
            .any(|hypothesis| hypothesis.formula.as_ref() == Some(formula)),
        "{view:#?}",
    );
}
