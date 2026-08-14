use std::collections::{BTreeSet, HashMap};

use super::{
    SemathEngine, canonical_expression_owner, index_occurrence_range, notation_occurrence_range,
    occurrence_id_at_range, relation_expression_at_cursor, stable_text_digest,
};
use crate::canonical::{
    SemanticExpr, SemanticExprKind, SemanticReference, lower_document_region, relation_head,
    render_canonical,
};
use crate::parser::test_math_regions;
use crate::semantic_index::{OccurrenceKind, SourceOccurrence, SourceOccurrenceId};
use crate::{
    ChangeEnvelope, DocumentLanguage, GeneratedNotationNode, GeneratedNotationTree, LexicalClass,
    MathRoot, MathRootState, MeaningDecision, NotationArgument, NotationNode, NotationNodeKind,
    NotationNodeRanges, PROTOCOL_VERSION, ProjectChange, ProjectDocument, ProjectInclude,
    ProjectMacro, ProjectMacroExpansion, ProjectMacroExpansionStatus, ProjectMacroKind,
    ProjectSnapshot, ProjectSourceRef, Query, QueryEnvelope, QueryValue, SourceRange, SyntaxScope,
    SyntaxState,
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

#[test]
fn chained_equality_stores_operands_and_source_linked_relations_without_a_system_placeholder() {
    let mut engine = SemathEngine::default();
    let update = engine.reset(snapshot("$a=b=c$")).unwrap();

    assert_eq!(update.stats.semantic_entities, 5);
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
    assert!(matches!(
        nested.decision,
        MeaningDecision::Established { ref meaning, .. }
            | MeaningDecision::Partial { ref meaning, .. }
            if meaning.relation_id.as_deref() == Some("probability:event-intersection")
    ));
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
    assert!(matches!(view.decision, MeaningDecision::Conflicting { .. }));
    assert_eq!(
        view.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "notation-role-conflict")
            .count(),
        1
    );
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
            matches!(&view.decision, MeaningDecision::Partial { meaning, .. } if meaning.label == "Mechanical power"),
            "offset {offset}"
        );
    }
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
    let content = "The bore determines area $A$ and the meter reports cross-section mean speed $v$. The preliminary volume rate is\n\\[Q=A v.\\]\nDensity $\\rho$ was sampled at the same temperature, allowing the corresponding mass rate to be written as\n\\[\\dot m=\\rho Q=\\rho A v.\\]";
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
                offset: content.find("\\dot m").unwrap() as u32 + 1,
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
        BTreeSet::from([
            "fluid-mechanics:mass-flow-rate",
            "fluid-mechanics:volumetric-flow-rate",
        ])
    );
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
    assert!(matches!(view.decision, MeaningDecision::Partial { .. }));
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
