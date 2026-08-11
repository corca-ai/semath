use super::{SemathEngine, notation_occurrence_range};
use crate::parser::test_math_regions;
use crate::{
    ChangeEnvelope, DocumentLanguage, GeneratedNotationNode, GeneratedNotationTree,
    MeaningDecision, NotationArgument, NotationNode, NotationNodeKind, NotationNodeRanges,
    PROTOCOL_VERSION, ProjectChange, ProjectDocument, ProjectInclude, ProjectMacro,
    ProjectMacroExpansion, ProjectMacroExpansionStatus, ProjectMacroKind, ProjectSnapshot,
    ProjectSourceRef, Query, QueryEnvelope, QueryValue, SourceRange, SyntaxState,
};

fn document(file_id: &str, path: &str, content: &str, version: u64) -> ProjectDocument {
    ProjectDocument {
        prose_annotations: vec![],
        file_id: file_id.into(),
        path: path.into(),
        language: DocumentLanguage::Latex,
        content: content.into(),
        document_version: version,
        schema_version: 8,
        nodes: Vec::new(),
        math_roots: Vec::new(),
        visible_prose: Vec::new(),
        scopes: Vec::new(),
        blocks: Vec::new(),
        declarations: Vec::new(),
        math_regions: test_math_regions(content, DocumentLanguage::Latex),
        macros: Vec::new(),
        includes: Vec::new(),
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
        let QueryValue::Locations { locations } = result.value else {
            panic!("expected locations")
        };
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start_offset, 5);
    }
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
        MeaningDecision::Partial { meaning, requirements, .. }
            if meaning.label == "Mechanical power" && !requirements.is_empty()
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
}

#[test]
fn semantic_view_follows_a_law_across_its_rhs_and_boundary() {
    let content = "Let $P$ be power.\nLet $F$ be force.\nLet $v$ be velocity.\nInstantaneous power is $P=\\mathbf{F}\\cdot\\mathbf{v}$.";
    let offsets = [
        content.find("$P=").unwrap() as u32,
        content.find("P=").unwrap() as u32,
        content.find("\\mathbf{F}").unwrap() as u32,
        content.rfind('v').unwrap() as u32 + 1,
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
            matches!(&view.decision, MeaningDecision::Partial { meaning, .. } if meaning.label == "Mechanical power"),
            "offset {offset}"
        );
    }
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
    assert!(matches!(view.decision, MeaningDecision::Partial { .. }));
    let relation = &view.context.relations[0];
    assert!(relation.evidence[0].source_ranges[0].contains(invocation_start));
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
    assert!(matches!(
        view.decision,
        MeaningDecision::Unsupported { ref reasons } if !reasons.is_empty()
    ));
    assert!(view.context.relations.is_empty());
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
    assert_eq!(view.symbol.unwrap().definitions[0].description, "the state");
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
    let QueryValue::Locations { locations } = result.value else {
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
    let main = "\\input{definitions}\n$V=RI$";
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
