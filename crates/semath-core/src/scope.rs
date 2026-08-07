use crate::{DocumentLanguage, ProjectDocument, SourceIndex, SourceRange};

#[derive(Clone, Debug)]
struct Scope {
    id: usize,
    depth: usize,
    range: SourceRange,
}

#[derive(Clone, Debug)]
pub(crate) struct ScopeGraph {
    scopes: Vec<Scope>,
}

impl ScopeGraph {
    pub fn new(document: &ProjectDocument) -> Self {
        let index = SourceIndex::new(&document.content);
        let document_end = index.utf16_for_byte(document.content.len());
        let headings = headings(document, &index);
        let mut scopes = vec![Scope {
            id: 0,
            depth: 0,
            range: SourceRange {
                start_offset: 0,
                end_offset: document_end,
            },
        }];
        for (position, (depth, start_offset)) in headings.iter().enumerate() {
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
            });
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

    fn scope_at(&self, offset: u32) -> &Scope {
        self.scopes
            .iter()
            .filter(|scope| scope.range.start_offset <= offset && offset < scope.range.end_offset)
            .max_by_key(|scope| scope.depth)
            .unwrap_or(&self.scopes[0])
    }
}

fn headings(document: &ProjectDocument, index: &SourceIndex) -> Vec<(usize, u32)> {
    match document.language {
        DocumentLanguage::Markdown => markdown_headings(&document.content, index),
        DocumentLanguage::Latex => latex_headings(&document.content, index),
        DocumentLanguage::Bibtex => Vec::new(),
    }
}

fn markdown_headings(source: &str, index: &SourceIndex) -> Vec<(usize, u32)> {
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

fn latex_headings(source: &str, index: &SourceIndex) -> Vec<(usize, u32)> {
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
            math_regions: Vec::new(),
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
