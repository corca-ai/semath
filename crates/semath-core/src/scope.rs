#[cfg(test)]
use crate::{DocumentLanguage, SourceIndex};
use crate::{ProjectDocument, SourceRange, SyntaxBlockKind};
use std::collections::BTreeMap;

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
                    if ancestor.kind != "document" {
                        ancestors.push(parent_id);
                    }
                    parent = ancestor.parent;
                }
                ancestors.reverse();
                if syntax.kind != "document" {
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
        append_equation_cluster_scopes(document, &mut scopes);
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

fn append_equation_cluster_scopes(document: &ProjectDocument, scopes: &mut Vec<Scope>) {
    let mut starts = document
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.kind == SyntaxBlockKind::Paragraph)
        .filter(|(_, block)| block_contains_relation(document, &block.range))
        .map(|(block_id, block)| {
            (
                block_id as u32,
                block.parent_scope,
                block.range.start_offset,
            )
        })
        .collect::<Vec<_>>();
    starts.sort_by_key(|(_, parent, start)| (*parent, *start));
    let counts = starts
        .iter()
        .fold(BTreeMap::new(), |mut counts, (_, parent, _)| {
            *counts.entry(*parent).or_insert(0usize) += 1;
            counts
        });
    for (position, (block_id, parent_id, start_offset)) in starts.iter().enumerate() {
        if counts.get(parent_id).copied().unwrap_or_default() < 2 {
            continue;
        }
        let Some((parent_range, mut path, parent_depth)) = scopes
            .iter()
            .find(|scope| scope.id == *parent_id as usize)
            .map(|scope| (scope.range.clone(), scope.path.clone(), scope.depth))
        else {
            continue;
        };
        let end_offset = starts[position + 1..]
            .iter()
            .find(|(_, next_parent, _)| next_parent == parent_id)
            .map_or(parent_range.end_offset, |(_, _, next_start)| *next_start);
        if *start_offset >= end_offset {
            continue;
        }
        path.push(0x8000_0000 | *block_id);
        scopes.push(Scope {
            id: scopes.len(),
            depth: parent_depth + 1,
            range: SourceRange {
                start_offset: *start_offset,
                end_offset,
            },
            path,
        });
    }
}

fn block_contains_relation(document: &ProjectDocument, block: &SourceRange) -> bool {
    document.math_roots.iter().any(|root| {
        block.start_offset <= root.full_range.start_offset
            && root.full_range.end_offset <= block.end_offset
            && document.nodes.iter().any(|node| {
                node.math_class.as_deref() == Some("relation")
                    && root.full_range.start_offset <= node.ranges.full.start_offset
                    && node.ranges.full.end_offset <= root.full_range.end_offset
            })
    })
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
                parent_scope: block.parent_scope,
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
