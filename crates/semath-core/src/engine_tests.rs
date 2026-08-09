use super::SemathEngine;
use crate::parser::test_math_regions;
use crate::{
    ChangeEnvelope, DocumentLanguage, PROTOCOL_VERSION, ProjectChange, ProjectDocument,
    ProjectInclude, ProjectMacro, ProjectMacroExpansion, ProjectMacroExpansionStatus,
    ProjectMacroKind, ProjectSnapshot, ProjectSourceRef, Query, QueryEnvelope, QueryValue,
    SourceRange,
};

fn document(file_id: &str, path: &str, content: &str, version: u64) -> ProjectDocument {
    ProjectDocument {
        file_id: file_id.into(),
        path: path.into(),
        language: DocumentLanguage::Latex,
        content: content.into(),
        document_version: version,
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
    assert_eq!(view.status, "established");
    assert_eq!(view.summary, "Mechanical power");
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
    assert!(view.refusal.is_none());
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
        assert_eq!(view.status, "established", "offset {offset}");
        assert_eq!(view.summary, "Mechanical power", "offset {offset}");
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
    assert_eq!(view.status, "established");
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
    assert_eq!(view.status, "unsupported");
    assert!(view.refusal.is_some());
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
                document: document("main", "main.tex", changed, 2),
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
        source_range: SourceRange {
            start_offset: 0,
            end_offset: 19,
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
    assert_eq!(view.status, "established");
    assert_eq!(view.context.relations[0].relation_id, "circuits:ohm-law");
}
