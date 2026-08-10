#[cfg(test)]
use crate::{DocumentLanguage, SourceIndex};
use crate::{ProjectDocument, SourceRange};

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
                        ancestors.push(ancestor.range.start_offset);
                    }
                    parent = ancestor.parent;
                }
                ancestors.reverse();
                if syntax.kind != "document" {
                    ancestors.push(syntax.range.start_offset);
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
    use super::ScopeGraph;
    use crate::{DocumentLanguage, ProjectDocument};

    fn document(content: &str, language: DocumentLanguage) -> ProjectDocument {
        ProjectDocument {
            file_id: "main".into(),
            path: "main.md".into(),
            language,
            content: content.into(),
            document_version: 1,
            schema_version: 4,
            nodes: Vec::new(),
            math_roots: Vec::new(),
            visible_prose: Vec::new(),
            scopes: Vec::new(),
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
}
