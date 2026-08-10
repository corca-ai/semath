use std::collections::BTreeSet;

use crate::semantic_index::{
    CandidateFamily, CandidateId, SemanticCandidateClaim, SourceOccurrence,
};
use crate::{NotationNodeKind, ProjectDocument};

const MAX_CANDIDATES_PER_OCCURRENCE: usize = 16;
const MAX_DOCUMENT_CANDIDATES: usize = 50_000;

#[derive(Clone, Debug)]
pub(crate) struct StructuralCandidateOption {
    pub(crate) family: CandidateFamily,
    pub(crate) interpretation: String,
}

/// Produces structural possibilities without promoting conventions to meaning.
#[cfg(test)]
pub(crate) fn generate_structural_candidates(
    document: &ProjectDocument,
    occurrences: &[SourceOccurrence],
) -> Vec<SemanticCandidateClaim> {
    let mut output = Vec::new();
    for occurrence in occurrences {
        let options = structural_candidate_options(
            document,
            &occurrence.structural_path,
            &occurrence.range,
            &occurrence.surface,
        );
        append_semantic_candidates(document, occurrence, &options, &mut output);
        if output.len() == MAX_DOCUMENT_CANDIDATES {
            return output;
        }
    }
    output
}

pub(crate) fn structural_candidate_options(
    document: &ProjectDocument,
    structural_path: &[u32],
    occurrence_range: &crate::SourceRange,
    surface: &str,
) -> Vec<StructuralCandidateOption> {
    let mut options = BTreeSet::<(CandidateFamily, String)>::new();
    for node_id in structural_path {
        let Some(node) = document.nodes.get(*node_id as usize) else {
            continue;
        };
        if node.ranges.full.start_offset < occurrence_range.start_offset
            || node.ranges.full.end_offset > occurrence_range.end_offset
        {
            continue;
        }
        match node.kind {
            NotationNodeKind::NamedOperator | NotationNodeKind::Token
                if followed_by_argument_structure(document, *node_id) =>
            {
                add(&mut options, CandidateFamily::Application, "application");
                add(
                    &mut options,
                    CandidateFamily::Juxtaposition,
                    "multiplication",
                );
            }
            NotationNodeKind::Modifier => {
                modifier_options(node.name.as_deref().unwrap_or_default(), &mut options)
            }
            NotationNodeKind::Style => {
                style_options(node.name.as_deref().unwrap_or_default(), &mut options)
            }
            NotationNodeKind::Script => script_options(document, node, &mut options),
            NotationNodeKind::Delimiter => {
                delimiter_options(node.name.as_deref().unwrap_or_default(), &mut options)
            }
            NotationNodeKind::Command => {
                command_options(node.name.as_deref().unwrap_or_default(), &mut options)
            }
            _ => {}
        }
    }
    if matches!(surface, "d" | "∂" | "δ")
        && structural_path
            .iter()
            .filter(|node| {
                document.nodes.get(**node as usize).is_some_and(|node| {
                    occurrence_range.start_offset <= node.ranges.full.start_offset
                        && node.ranges.full.end_offset <= occurrence_range.end_offset
                })
            })
            .any(|node| followed_by_notation_atom(document, *node))
    {
        add(&mut options, CandidateFamily::Differential, "differential");
        add(
            &mut options,
            CandidateFamily::Juxtaposition,
            "multiplication",
        );
    }
    surface_operator_options(surface, &mut options);
    options
        .into_iter()
        .take(MAX_CANDIDATES_PER_OCCURRENCE)
        .map(|(family, interpretation)| StructuralCandidateOption {
            family,
            interpretation,
        })
        .collect()
}

pub(crate) fn append_semantic_candidates(
    document: &ProjectDocument,
    occurrence: &SourceOccurrence,
    options: &[StructuralCandidateOption],
    output: &mut Vec<SemanticCandidateClaim>,
) {
    for option in options {
        if output.len() == MAX_DOCUMENT_CANDIDATES {
            return;
        }
        let local_id = output.len() as u32;
        output.push(SemanticCandidateClaim {
            id: CandidateId(format!(
                "{}:{}:candidate:{local_id}",
                document.file_id, document.document_version
            )),
            occurrence_id: occurrence.id.clone(),
            family: option.family,
            interpretation: option.interpretation.clone(),
            range: occurrence.range.clone(),
            supporting_claims: Vec::new(),
            rejecting_claims: Vec::new(),
        });
    }
}

fn add(options: &mut BTreeSet<(CandidateFamily, String)>, family: CandidateFamily, value: &str) {
    options.insert((family, value.to_owned()));
}

fn followed_by_argument_structure(document: &ProjectDocument, node_id: u32) -> bool {
    next_meaningful_sibling(document, node_id).is_some_and(|node| {
        matches!(
            node.kind,
            NotationNodeKind::Delimiter | NotationNodeKind::Group
        )
    })
}

fn followed_by_notation_atom(document: &ProjectDocument, node_id: u32) -> bool {
    next_meaningful_sibling(document, node_id).is_some_and(|node| {
        !matches!(
            node.kind,
            NotationNodeKind::Opaque | NotationNodeKind::Error
        )
    })
}

fn next_meaningful_sibling(
    document: &ProjectDocument,
    node_id: u32,
) -> Option<&crate::NotationNode> {
    let parent_id = document
        .nodes
        .get(node_id as usize)
        .and_then(|node| node.parent)?;
    let parent = document.nodes.get(parent_id as usize)?;
    let position = parent.children.iter().position(|child| *child == node_id)?;
    parent.children[position + 1..]
        .iter()
        .filter_map(|sibling| document.nodes.get(*sibling as usize))
        .find(|node| {
            node.text
                .as_deref()
                .is_none_or(|text| !text.trim().is_empty())
        })
}

fn modifier_options(name: &str, options: &mut BTreeSet<(CandidateFamily, String)>) {
    let values: &[&str] = match name {
        "hat" | "widehat" => &["estimate", "transform", "unit-vector"],
        "bar" | "overline" => &["mean", "conjugate", "closure"],
        "tilde" | "widetilde" => &["transform", "equivalence-class"],
        "vec" | "overrightarrow" => &["vector", "directed-map"],
        "dot" | "ddot" => &["time-derivative", "decoration"],
        "underline" | "underbar" => &["vector", "emphasis"],
        _ => &["decoration"],
    };
    for value in values {
        add(options, CandidateFamily::Decoration, value);
    }
}

fn style_options(name: &str, options: &mut BTreeSet<(CandidateFamily, String)>) {
    let values: &[&str] = match name {
        "mathbf" | "bm" | "boldsymbol" => &["vector", "tensor"],
        "mathbb" => &["set", "number-system"],
        "mathcal" | "mathscr" => &["set", "operator", "space"],
        "mathrm" | "operatorname" | "text" => &["named-surface", "unit", "label"],
        _ => &["styled-surface"],
    };
    for value in values {
        add(options, CandidateFamily::Style, value);
    }
}

fn script_options(
    document: &ProjectDocument,
    node: &crate::NotationNode,
    options: &mut BTreeSet<(CandidateFamily, String)>,
) {
    match node.name.as_deref() {
        Some("subscript") => {
            add(options, CandidateFamily::Script, "index");
            add(options, CandidateFamily::Script, "restriction");
            if let Some(base) = node
                .children
                .first()
                .and_then(|child| document.nodes.get(*child as usize))
                && base.kind == NotationNodeKind::Command
            {
                command_options(base.name.as_deref().unwrap_or_default(), options);
            }
        }
        Some("prime") => {
            add(options, CandidateFamily::Script, "derivative");
            add(options, CandidateFamily::Script, "related-quantity");
        }
        Some("superscript") => {
            add(options, CandidateFamily::Script, "power");
            let source = node
                .children
                .get(1)
                .map(|child| bounded_node_text(document, *child, 0))
                .unwrap_or_default();
            match source.as_str() {
                "T" | "t" | "top" => add(options, CandidateFamily::Script, "transpose"),
                "-1" => add(options, CandidateFamily::Script, "inverse"),
                "*" | "dagger" => add(options, CandidateFamily::Script, "adjoint"),
                _ => {}
            }
        }
        _ => {}
    }
}

fn bounded_node_text(document: &ProjectDocument, node_id: u32, depth: u8) -> String {
    if depth == 8 {
        return String::new();
    }
    let Some(node) = document.nodes.get(node_id as usize) else {
        return String::new();
    };
    if let Some(text) = &node.text {
        return text.clone();
    }
    if node.children.is_empty() {
        return node.name.clone().unwrap_or_default();
    }
    node.children
        .iter()
        .map(|child| bounded_node_text(document, *child, depth + 1))
        .collect()
}

fn surface_operator_options(surface: &str, options: &mut BTreeSet<(CandidateFamily, String)>) {
    let values: &[&str] = match surface {
        "|" => &["absolute-value", "conditional", "restriction", "evaluation"],
        ":" => &["type-ascription", "ratio", "such-that", "map-domain"],
        "*" | "⋆" => &["multiplication", "convolution", "adjoint"],
        "→" | "↦" => &["mapping", "limit", "transition"],
        _ => &[],
    };
    for value in values {
        add(options, CandidateFamily::Operator, value);
    }
}

fn delimiter_options(name: &str, options: &mut BTreeSet<(CandidateFamily, String)>) {
    let values: &[&str] = match name {
        "()" => &["grouping", "application", "tuple"],
        "[]" => &["list", "interval", "commutator", "evaluation"],
        "{}" => &["set", "grouping"],
        "||" => &["norm", "conditional", "parallel"],
        "|" => &["absolute-value", "conditional", "restriction", "evaluation"],
        _ => &["grouping"],
    };
    for value in values {
        add(options, CandidateFamily::Bracketed, value);
    }
}

fn command_options(name: &str, options: &mut BTreeSet<(CandidateFamily, String)>) {
    match name {
        "sum" | "prod" | "int" | "iint" | "iiint" | "lim" | "forall" | "exists" => {
            add(options, CandidateFamily::Binder, "binder");
        }
        "partial" => {
            add(options, CandidateFamily::Differential, "differential");
            add(options, CandidateFamily::Differential, "derivative");
        }
        "nabla" => {
            add(options, CandidateFamily::Differential, "gradient");
            add(options, CandidateFamily::Differential, "divergence");
            add(options, CandidateFamily::Differential, "curl");
        }
        "cdot" => {
            add(options, CandidateFamily::Operator, "multiplication");
            add(options, CandidateFamily::Operator, "inner-product");
        }
        "times" => {
            add(options, CandidateFamily::Operator, "multiplication");
            add(options, CandidateFamily::Operator, "cross-product");
            add(options, CandidateFamily::Operator, "cartesian-product");
        }
        "circ" => {
            add(options, CandidateFamily::Operator, "composition");
            add(options, CandidateFamily::Operator, "circle-product");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceRange;
    use crate::semantic_index::{OccurrenceKind, SourceOccurrenceId};

    fn occurrence(
        range: SourceRange,
        selection_range: SourceRange,
        structural_path: Vec<u32>,
        surface: &str,
    ) -> SourceOccurrence {
        SourceOccurrence {
            id: SourceOccurrenceId {
                file_id: "main".into(),
                document_version: 1,
                local_id: 0,
            },
            component_id: "main".into(),
            kind: OccurrenceKind::Notation,
            range,
            selection_range,
            scope_path: vec![0],
            structural_path,
            availability_order: 0,
            surface: surface.into(),
            source_text: surface.into(),
            notation: Vec::new(),
        }
    }

    #[test]
    fn named_calls_remain_two_unresolved_structural_possibilities() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 4,
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "\\operatorname{acc}(x)",
            "documentVersion": 1,
            "nodes": [
                {"kind":"named-operator","parent":2,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":18},"name":{"startOffset":14,"endOffset":17}},"state":"complete","name":"acc"},
                {"kind":"delimiter","parent":2,"children":[],"ranges":{"full":{"startOffset":18,"endOffset":21}},"state":"complete","name":"()"},
                {"kind":"sequence","parent":null,"children":[0,1],"ranges":{"full":{"startOffset":0,"endOffset":21}},"state":"complete"}
            ],
            "mathRoots": [{"node":2,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":21},"contentRange":{"startOffset":0,"endOffset":21},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":21},"state":"complete"}],
            "declarations": [],
            "macros": [],
            "includes": []
        }))
        .unwrap();
        let candidates = generate_structural_candidates(
            &document,
            &[occurrence(
                SourceRange {
                    start_offset: 0,
                    end_offset: 18,
                },
                SourceRange {
                    start_offset: 14,
                    end_offset: 17,
                },
                vec![2, 0],
                "acc",
            )],
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.family, candidate.interpretation.as_str()))
                .collect::<Vec<_>>(),
            [
                (CandidateFamily::Application, "application"),
                (CandidateFamily::Juxtaposition, "multiplication"),
            ]
        );
        assert!(candidates.iter().all(|candidate| {
            candidate.supporting_claims.is_empty() && candidate.rejecting_claims.is_empty()
        }));
    }

    #[test]
    fn decorations_are_bounded_and_do_not_merge_with_the_nucleus() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 4,
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "\\hat y",
            "documentVersion": 1,
            "nodes": [
                {"kind":"modifier","parent":null,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":6},"nucleus":{"startOffset":5,"endOffset":6}},"state":"complete","name":"hat"}
            ],
            "mathRoots": [{"node":0,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":6},"contentRange":{"startOffset":0,"endOffset":6},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":6},"state":"complete"}],
            "declarations": [],
            "macros": [],
            "includes": []
        }))
        .unwrap();
        let candidates = generate_structural_candidates(
            &document,
            &[occurrence(
                SourceRange {
                    start_offset: 0,
                    end_offset: 6,
                },
                SourceRange {
                    start_offset: 5,
                    end_offset: 6,
                },
                vec![0],
                "y",
            )],
        );
        assert_eq!(candidates.len(), 3);
        assert!(candidates.iter().all(|candidate| {
            candidate.family == CandidateFamily::Decoration && candidate.range.end_offset == 6
        }));
    }

    #[test]
    fn application_requires_the_next_meaningful_sibling_to_be_an_argument() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 4,
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "a+(x)",
            "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}} ,"state":"complete","text":"a"},
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":2}} ,"state":"complete","text":"+"},
                {"kind":"delimiter","parent":3,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":5}} ,"state":"complete","name":"()"},
                {"kind":"sequence","parent":null,"children":[0,1,2],"ranges":{"full":{"startOffset":0,"endOffset":5}} ,"state":"complete"}
            ],
            "mathRoots": [{"node":3,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":5},"contentRange":{"startOffset":0,"endOffset":5},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":5},"state":"complete"}],
            "declarations": [],
            "macros": [],
            "includes": []
        }))
        .unwrap();
        let candidates = generate_structural_candidates(
            &document,
            &[occurrence(
                SourceRange {
                    start_offset: 0,
                    end_offset: 1,
                },
                SourceRange {
                    start_offset: 0,
                    end_offset: 1,
                },
                vec![3, 0],
                "a",
            )],
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn generic_family_tables_are_deterministic_and_bounded() {
        let mut options = BTreeSet::new();
        modifier_options("bar", &mut options);
        style_options("mathbf", &mut options);
        delimiter_options("[]", &mut options);
        for command in ["sum", "partial", "cdot", "times", "circ"] {
            command_options(command, &mut options);
        }
        let families = options
            .iter()
            .map(|(family, _)| *family)
            .collect::<BTreeSet<_>>();
        assert!(families.contains(&CandidateFamily::Binder));
        assert!(families.contains(&CandidateFamily::Bracketed));
        assert!(families.contains(&CandidateFamily::Decoration));
        assert!(families.contains(&CandidateFamily::Differential));
        assert!(families.contains(&CandidateFamily::Operator));
        assert!(families.contains(&CandidateFamily::Style));
    }
}
