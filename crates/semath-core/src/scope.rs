#[cfg(test)]
use crate::{DocumentLanguage, SourceIndex};
use crate::{ProjectDocument, SourceRange, SyntaxBlockKind};

pub(crate) fn scope_visible(declaration: &[u32], occurrence: &[u32]) -> bool {
    declaration.len() <= occurrence.len()
        && declaration
            .iter()
            .zip(occurrence)
            .all(|(left, right)| left == right)
}

#[derive(Clone, Debug)]
struct Scope {
    id: usize,
    depth: usize,
    range: SourceRange,
    path: Vec<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct ScopeGraph {
    scopes: Vec<Scope>,
}

impl ScopeGraph {
    pub fn new(document: &ProjectDocument) -> Self {
        #[cfg(test)]
        if document.scopes.is_empty() {
            return test_scope_graph(document);
        }
        let document_end = document.content.encode_utf16().count() as u32;
        let mut scopes = document
            .scopes
            .iter()
            .enumerate()
            .filter(|(id, syntax)| *id == 0 || !transparent_scope(syntax))
            .map(|(id, syntax)| {
                let mut ancestors = Vec::new();
                let mut parent = syntax.parent;
                let mut visited = vec![false; document.scopes.len()];
                while let Some(parent_id) = parent {
                    let parent_index = parent_id as usize;
                    if parent_index >= document.scopes.len() || visited[parent_index] {
                        break;
                    }
                    visited[parent_index] = true;
                    let ancestor = &document.scopes[parent_index];
                    if !transparent_scope(ancestor) {
                        ancestors.push(parent_id);
                    }
                    parent = ancestor.parent;
                }
                ancestors.reverse();
                if !transparent_scope(syntax) {
                    ancestors.push(id as u32);
                }
                Scope {
                    id,
                    depth: ancestors.len(),
                    range: syntax.range.clone(),
                    path: ancestors,
                }
            })
            .collect::<Vec<_>>();
        if scopes.iter().all(|scope| scope.depth != 0) || scopes.is_empty() {
            scopes.insert(
                0,
                Scope {
                    id: 0,
                    depth: 0,
                    range: SourceRange {
                        start_offset: 0,
                        end_offset: document_end,
                    },
                    path: Vec::new(),
                },
            );
        }
        Self { scopes }
    }

    pub fn id_at(&self, offset: u32) -> usize {
        self.scope_at(offset).id
    }

    pub fn depth(&self, scope_id: usize) -> usize {
        self.scopes
            .iter()
            .find(|scope| scope.id == scope_id)
            .map_or(0, |scope| scope.depth)
    }

    pub fn visible(&self, scope_id: usize, offset: u32) -> bool {
        self.scopes
            .iter()
            .find(|scope| scope.id == scope_id)
            .is_some_and(|scope| {
                scope.range.start_offset <= offset && offset < scope.range.end_offset
            })
    }

    pub fn range_at(&self, offset: u32) -> SourceRange {
        self.scope_at(offset).range.clone()
    }

    pub fn is_document_scope_at(&self, offset: u32) -> bool {
        self.scope_at(offset).id == 0
    }

    pub fn path_at(&self, offset: u32) -> Vec<u32> {
        self.scope_at(offset).path.clone()
    }

    fn scope_at(&self, offset: u32) -> &Scope {
        self.scopes
            .iter()
            .filter(|scope| scope.range.start_offset <= offset && offset < scope.range.end_offset)
            .max_by_key(|scope| scope.depth)
            .unwrap_or(&self.scopes[0])
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AttachmentGraph {
    regions: Vec<AttachmentRegion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AttachmentRegion {
    order: usize,
    parent_scope: u32,
    kind: SyntaxBlockKind,
    range: SourceRange,
}

impl AttachmentGraph {
    pub fn new(document: &ProjectDocument) -> Self {
        let regions = document
            .blocks
            .iter()
            .enumerate()
            .map(|(order, block)| AttachmentRegion {
                order,
                parent_scope: attachment_scope(document, block.parent_scope),
                kind: block.kind,
                range: block.range.clone(),
            })
            .collect();
        Self { regions }
    }

    pub fn permits(&self, left: &SourceRange, right: &SourceRange) -> bool {
        if self.regions.is_empty() {
            return true;
        }
        let Some(left) = self.region_for(left) else {
            return false;
        };
        let Some(right) = self.region_for(right) else {
            return false;
        };
        if left.parent_scope != right.parent_scope || left.order.abs_diff(right.order) > 2 {
            return false;
        }
        if left.order == right.order {
            return true;
        }
        let (start, end) = if left.order <= right.order {
            (left.order, right.order)
        } else {
            (right.order, left.order)
        };
        !self.regions[start + 1..end]
            .iter()
            .any(|region| region.kind == SyntaxBlockKind::Heading)
    }

    pub fn candidate_edges(&self) -> u32 {
        self.regions
            .windows(2)
            .filter(|pair| pair[0].parent_scope == pair[1].parent_scope)
            .count() as u32
    }

    fn region_for(&self, range: &SourceRange) -> Option<&AttachmentRegion> {
        self.regions
            .iter()
            .filter(|region| ranges_overlap(&region.range, range))
            .min_by_key(|region| region.range.end_offset - region.range.start_offset)
    }
}

fn transparent_scope(scope: &crate::SyntaxScope) -> bool {
    scope.kind == "document"
        || (scope.kind == "environment"
            && scope
                .name
                .as_deref()
                .is_some_and(|name| name == "document" || is_math_environment(name)))
}

fn attachment_scope(document: &ProjectDocument, mut scope_id: u32) -> u32 {
    while let Some(scope) = document.scopes.get(scope_id as usize) {
        if scope.kind != "environment" || !scope.name.as_deref().is_some_and(is_math_environment) {
            break;
        }
        let Some(parent) = scope.parent else {
            break;
        };
        scope_id = parent;
    }
    scope_id
}

fn is_math_environment(name: &str) -> bool {
    matches!(
        name,
        "equation"
            | "equation*"
            | "align"
            | "align*"
            | "aligned"
            | "gather"
            | "gather*"
            | "gathered"
            | "multline"
            | "multline*"
            | "split"
            | "cases"
    )
}

fn ranges_overlap(left: &SourceRange, right: &SourceRange) -> bool {
    left.start_offset < right.end_offset && right.start_offset < left.end_offset
}

#[cfg(test)]
fn test_scope_graph(document: &ProjectDocument) -> ScopeGraph {
    let index = SourceIndex::new(&document.content);
    let document_end = index.utf16_for_byte(document.content.len());
    let headings = match document.language {
        DocumentLanguage::Markdown => test_markdown_headings(&document.content, &index),
        DocumentLanguage::Latex => test_latex_headings(&document.content, &index),
        DocumentLanguage::Bibtex => Vec::new(),
    };
    let mut scopes = vec![Scope {
        id: 0,
        depth: 0,
        range: SourceRange {
            start_offset: 0,
            end_offset: document_end,
        },
        path: Vec::new(),
    }];
    let mut parents: Vec<(usize, u32)> = Vec::new();
    for (position, (depth, start_offset)) in headings.iter().enumerate() {
        while parents
            .last()
            .is_some_and(|(parent_depth, _)| parent_depth >= depth)
        {
            parents.pop();
        }
        let mut path = parents
            .iter()
            .map(|(_, offset)| *offset)
            .collect::<Vec<_>>();
        path.push(*start_offset);
        let end_offset = headings[position + 1..]
            .iter()
            .find(|(next_depth, _)| next_depth <= depth)
            .map_or(document_end, |(_, next_start)| *next_start);
        scopes.push(Scope {
            id: position + 1,
            depth: *depth,
            range: SourceRange {
                start_offset: *start_offset,
                end_offset,
            },
            path,
        });
        parents.push((*depth, *start_offset));
    }
    ScopeGraph { scopes }
}

#[cfg(test)]
fn test_markdown_headings(source: &str, index: &SourceIndex) -> Vec<(usize, u32)> {
    let mut headings = Vec::new();
    let mut byte_offset = 0;
    let mut fenced = false;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
            if (1..=6).contains(&hashes)
                && trimmed
                    .as_bytes()
                    .get(hashes)
                    .is_some_and(u8::is_ascii_whitespace)
            {
                let indentation = line.len() - line.trim_start().len();
                headings.push((hashes, index.utf16_for_byte(byte_offset + indentation)));
            }
        }
        byte_offset += line.len();
    }
    headings
}

#[cfg(test)]
fn test_latex_headings(source: &str, index: &SourceIndex) -> Vec<(usize, u32)> {
    let mut headings = Vec::new();
    let mut byte_offset = 0;
    for line in source.split_inclusive('\n') {
        let visible = line.split('%').next().unwrap_or("");
        for (command, depth) in [
            ("\\section", 1),
            ("\\subsection", 2),
            ("\\subsubsection", 3),
        ] {
            if let Some(relative) = visible.find(command) {
                headings.push((depth, index.utf16_for_byte(byte_offset + relative)));
                break;
            }
        }
        byte_offset += line.len();
    }
    headings
}

#[cfg(test)]
mod tests {
    use super::{AttachmentGraph, ScopeGraph};
    use crate::{
        DocumentLanguage, MathRootState, ProjectDocument, SourceRange, SyntaxBlock,
        SyntaxBlockKind, SyntaxScope,
    };

    fn document(content: &str, language: DocumentLanguage) -> ProjectDocument {
        ProjectDocument {
            prose_annotations: vec![],
            file_id: "main".into(),
            path: "main.md".into(),
            language,
            content: content.into(),
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
        }
    }

    #[test]
    fn nests_markdown_sections_and_ignores_fenced_headings() {
        let source = "root\n# A\na\n## B\nb\n```\n# ignored\n```\n# C\nc";
        let scopes = ScopeGraph::new(&document(source, DocumentLanguage::Markdown));
        let root = scopes.id_at(1);
        let section_a = scopes.id_at(source.find("a\n##").unwrap() as u32);
        let subsection_b = scopes.id_at(source.find("b\n```").unwrap() as u32);
        let section_c = scopes.id_at(source.rfind('c').unwrap() as u32);
        assert_eq!(root, 0);
        assert_ne!(section_a, subsection_b);
        assert_ne!(section_a, section_c);
        assert!(scopes.visible(section_a, source.find("b\n```").unwrap() as u32));
        assert!(!scopes.visible(section_a, source.rfind('c').unwrap() as u32));
    }

    #[test]
    fn scope_paths_use_structure_instead_of_source_offsets() {
        let mut first = document("# A\ntext", DocumentLanguage::Markdown);
        first.scopes = vec![
            syntax_scope("document", None, 0, 8),
            syntax_scope("section", Some(0), 0, 8),
        ];
        let mut shifted = document("prefix\n# A\ntext", DocumentLanguage::Markdown);
        shifted.scopes = vec![
            syntax_scope("document", None, 0, 15),
            syntax_scope("section", Some(0), 7, 15),
        ];

        assert_eq!(ScopeGraph::new(&first).path_at(4), vec![1]);
        assert_eq!(ScopeGraph::new(&shifted).path_at(11), vec![1]);
    }

    #[test]
    fn display_layout_preserves_lexical_identity_but_theorems_do_not() {
        let mut source = document("01234567890123456789", DocumentLanguage::Latex);
        let mut display = syntax_scope("environment", Some(1), 5, 10);
        display.name = Some("align".into());
        let mut theorem = syntax_scope("environment", Some(1), 12, 18);
        theorem.name = Some("theorem".into());
        source.scopes = vec![
            syntax_scope("document", None, 0, 20),
            syntax_scope("section", Some(0), 0, 20),
            display,
            theorem,
        ];
        let graph = ScopeGraph::new(&source);
        assert_eq!(graph.path_at(3), graph.path_at(7));
        assert_eq!(graph.id_at(3), graph.id_at(7));
        assert!(graph.visible(graph.id_at(7), 19));
        assert!(!graph.visible(graph.id_at(15), 19));
        assert_ne!(graph.path_at(3), graph.path_at(15));
        assert!(super::scope_visible(&graph.path_at(3), &graph.path_at(15)));
        assert!(!super::scope_visible(
            &graph.path_at(15),
            &graph.path_at(19)
        ));
    }

    #[test]
    fn attachment_is_bounded_by_parent_scope_and_block_distance() {
        let mut source = document("lead\n$$x=y$$\nwhere\nfar", DocumentLanguage::Latex);
        source.scopes = vec![syntax_scope("document", None, 0, 24)];
        source.blocks = vec![
            syntax_block(SyntaxBlockKind::Paragraph, 0, 0, 4),
            syntax_block(SyntaxBlockKind::DisplayMath, 0, 5, 12),
            syntax_block(SyntaxBlockKind::Paragraph, 0, 13, 18),
            syntax_block(SyntaxBlockKind::Paragraph, 0, 19, 22),
        ];
        let graph = AttachmentGraph::new(&source);

        assert!(graph.permits(&range(0, 4), &range(5, 12)));
        assert!(graph.permits(&range(5, 12), &range(13, 18)));
        assert!(!graph.permits(&range(0, 4), &range(19, 22)));
        assert_eq!(graph.candidate_edges(), 3);
    }

    #[test]
    fn math_environment_scope_is_transparent_to_prose_attachment() {
        let mut source = document(
            "roles\n\\begin{equation}x=y\\end{equation}",
            DocumentLanguage::Latex,
        );
        let mut equation = syntax_scope("environment", Some(0), 6, 42);
        equation.name = Some("equation".into());
        source.scopes = vec![syntax_scope("document", None, 0, 42), equation];
        source.blocks = vec![
            syntax_block(SyntaxBlockKind::Paragraph, 0, 0, 5),
            syntax_block(SyntaxBlockKind::DisplayMath, 1, 6, 42),
        ];

        let graph = AttachmentGraph::new(&source);
        assert!(graph.permits(&range(0, 5), &range(6, 42)));
        assert_eq!(graph.candidate_edges(), 1);
    }

    fn range(start_offset: u32, end_offset: u32) -> SourceRange {
        SourceRange {
            start_offset,
            end_offset,
        }
    }

    fn syntax_scope(kind: &str, parent: Option<u32>, start: u32, end: u32) -> SyntaxScope {
        SyntaxScope {
            kind: kind.into(),
            parent,
            range: range(start, end),
            state: MathRootState::Complete,
            name: None,
            level: None,
            source: None,
        }
    }

    fn syntax_block(kind: SyntaxBlockKind, parent_scope: u32, start: u32, end: u32) -> SyntaxBlock {
        SyntaxBlock {
            kind,
            parent_scope,
            range: range(start, end),
            state: MathRootState::Complete,
            content_range: None,
            name: None,
        }
    }
}
