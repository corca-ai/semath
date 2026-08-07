use crate::{DocumentLanguage, EquationNode, MathRegion, SourceIndex, SourceRange};

#[derive(Clone, Debug)]
pub(crate) struct ParsedMath {
    pub region: MathRegion,
    pub root: EquationNode,
    pub symbols: Vec<(String, SourceRange)>,
}

pub(crate) fn math_regions(source: &str, language: DocumentLanguage) -> Vec<MathRegion> {
    let index = SourceIndex::new(source);
    let bytes = source.as_bytes();
    let mut regions = Vec::new();
    let mut cursor = 0usize;
    let mut fenced = false;

    while cursor < bytes.len() {
        if language == DocumentLanguage::Markdown && source[cursor..].starts_with("```") {
            fenced = !fenced;
            cursor += 3;
            continue;
        }
        if fenced {
            cursor += source[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            continue;
        }
        if language == DocumentLanguage::Latex && bytes[cursor] == b'%' && !escaped(bytes, cursor) {
            cursor = source[cursor..]
                .find('\n')
                .map_or(bytes.len(), |next| cursor + next + 1);
            continue;
        }

        let delimiter = if source[cursor..].starts_with("$$") {
            Some(("$$", "$$", 2usize))
        } else if source[cursor..].starts_with("\\[") {
            Some(("\\[", "\\]", 2usize))
        } else if source[cursor..].starts_with("\\(") {
            Some(("\\(", "\\)", 2usize))
        } else if bytes[cursor] == b'$' && !escaped(bytes, cursor) {
            Some(("$", "$", 1usize))
        } else {
            None
        };

        let Some((open, close, open_len)) = delimiter else {
            cursor += source[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            continue;
        };
        let content_start = cursor + open_len;
        let mut search = content_start;
        let mut close_start = None;
        while search < bytes.len() {
            if source[search..].starts_with(close) && !escaped(bytes, search) {
                close_start = Some(search);
                break;
            }
            search += source[search..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }
        let content_end = close_start.unwrap_or(bytes.len());
        let full_end = close_start.map_or(bytes.len(), |start| start + close.len());
        regions.push(MathRegion {
            full_range: range(&index, cursor, full_end),
            content_range: range(&index, content_start, content_end),
            delimiter: open.to_string(),
            closed: close_start.is_some(),
        });
        cursor = full_end.max(cursor + open_len);
    }

    regions
}

fn escaped(bytes: &[u8], offset: usize) -> bool {
    let mut slashes = 0;
    let mut cursor = offset;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slashes += 1;
        cursor -= 1;
    }
    slashes % 2 == 1
}

fn range(index: &SourceIndex, start: usize, end: usize) -> SourceRange {
    SourceRange {
        start_offset: index.utf16_for_byte(start),
        end_offset: index.utf16_for_byte(end),
    }
}

pub(crate) fn parse_regions(source: &str, regions: &[MathRegion]) -> Vec<ParsedMath> {
    let index = SourceIndex::new(source);
    regions
        .iter()
        .filter_map(|region| {
            let start = index.byte_for_utf16(region.content_range.start_offset);
            let end = index.byte_for_utf16(region.content_range.end_offset);
            (start <= end && end <= source.len()).then(|| {
                let mut parser = Parser::new(&source[start..end], start, &index);
                let root = parser.parse_sequence(None);
                ParsedMath {
                    region: region.clone(),
                    root,
                    symbols: parser.symbols,
                }
            })
        })
        .collect()
}

struct Parser<'a> {
    source: &'a str,
    base_byte: usize,
    cursor: usize,
    index: &'a SourceIndex,
    symbols: Vec<(String, SourceRange)>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, base_byte: usize, index: &'a SourceIndex) -> Self {
        Self {
            source,
            base_byte,
            cursor: 0,
            index,
            symbols: Vec::new(),
        }
    }

    fn parse_sequence(&mut self, terminator: Option<char>) -> EquationNode {
        let start = self.cursor;
        let mut children = Vec::new();
        while self.cursor < self.source.len() {
            self.skip_whitespace();
            if self.cursor >= self.source.len() {
                break;
            }
            if terminator.is_some_and(|end| self.peek() == Some(end)) {
                break;
            }
            children.push(self.parse_atom());
        }
        EquationNode {
            kind: "sequence".into(),
            label: None,
            range: self.absolute_range(start, self.cursor),
            children,
        }
    }

    fn parse_atom(&mut self) -> EquationNode {
        let start = self.cursor;
        let Some(character) = self.peek() else {
            return self.node("unknown", None, start, start, Vec::new());
        };
        let mut node = match character {
            '{' => self.parse_group(),
            '\\' => self.parse_command(),
            '(' | '[' => self.parse_delimited(character),
            '0'..='9' => self.parse_number(),
            '+' | '-' | '=' | '<' | '>' | '*' | '/' | '|' | ',' | ':' => {
                self.bump();
                self.node(
                    "operator",
                    Some(character.to_string()),
                    start,
                    self.cursor,
                    Vec::new(),
                )
            }
            '^' | '_' => {
                self.bump();
                self.node(
                    "script-marker",
                    Some(character.to_string()),
                    start,
                    self.cursor,
                    Vec::new(),
                )
            }
            _ => {
                self.bump();
                let label = character.to_string();
                let span = self.absolute_range(start, self.cursor);
                if character.is_alphabetic() {
                    self.symbols.push((label.clone(), span.clone()));
                }
                self.node(
                    if character.is_alphabetic() {
                        "symbol"
                    } else {
                        "text"
                    },
                    Some(label),
                    start,
                    self.cursor,
                    Vec::new(),
                )
            }
        };

        let mut scripts = Vec::new();
        while matches!(self.peek(), Some('^' | '_')) {
            let marker_start = self.cursor;
            let marker = self.bump().unwrap();
            let child = if self.peek() == Some('{') {
                self.parse_group()
            } else {
                self.parse_atom()
            };
            scripts.push(self.node(
                if marker == '^' {
                    "superscript"
                } else {
                    "subscript"
                },
                None,
                marker_start,
                self.cursor,
                vec![child],
            ));
        }
        if !scripts.is_empty() {
            let mut children = vec![node];
            children.extend(scripts);
            node = self.node("scripted", None, start, self.cursor, children);
        }
        node
    }

    fn parse_group(&mut self) -> EquationNode {
        let start = self.cursor;
        self.bump();
        let body = self.parse_sequence(Some('}'));
        if self.peek() == Some('}') {
            self.bump();
        }
        self.node("group", None, start, self.cursor, vec![body])
    }

    fn parse_delimited(&mut self, open: char) -> EquationNode {
        let start = self.cursor;
        let close = if open == '(' { ')' } else { ']' };
        self.bump();
        let body = self.parse_sequence(Some(close));
        if self.peek() == Some(close) {
            self.bump();
        }
        self.node(
            "delimited",
            Some(format!("{open}{close}")),
            start,
            self.cursor,
            vec![body],
        )
    }

    fn parse_number(&mut self) -> EquationNode {
        let start = self.cursor;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit() || character == '.')
        {
            self.bump();
        }
        self.node(
            "number",
            Some(self.source[start..self.cursor].to_string()),
            start,
            self.cursor,
            Vec::new(),
        )
    }

    fn parse_command(&mut self) -> EquationNode {
        let start = self.cursor;
        self.bump();
        let name_start = self.cursor;
        while self.peek().is_some_and(char::is_alphabetic) {
            self.bump();
        }
        if self.cursor == name_start {
            self.bump();
        }
        let name = self.source[name_start..self.cursor].to_string();
        let mut children = Vec::new();
        let kind = match name.as_str() {
            "frac" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    children.push(self.parse_group());
                }
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    children.push(self.parse_group());
                }
                "fraction"
            }
            "sqrt" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    children.push(self.parse_group());
                }
                "root"
            }
            "sum" => "sum",
            "int" => "integral",
            "lim" => "limit",
            "forall" | "exists" => "quantifier",
            "left" | "right" => "delimiter-command",
            "mathbb" | "mathbf" | "mathrm" | "operatorname" => {
                self.skip_whitespace();
                if self.peek() == Some('{') {
                    children.push(self.parse_group());
                }
                "styled"
            }
            _ => {
                if !name.is_empty() {
                    self.symbols
                        .push((format!("\\{name}"), self.absolute_range(start, self.cursor)));
                }
                "command"
            }
        };
        self.node(
            kind,
            Some(format!("\\{name}")),
            start,
            self.cursor,
            children,
        )
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.cursor += character.len_utf8();
        Some(character)
    }

    fn node(
        &self,
        kind: &str,
        label: Option<String>,
        start: usize,
        end: usize,
        children: Vec<EquationNode>,
    ) -> EquationNode {
        EquationNode {
            kind: kind.into(),
            label,
            range: self.absolute_range(start, end),
            children,
        }
    }

    fn absolute_range(&self, start: usize, end: usize) -> SourceRange {
        range(self.index, self.base_byte + start, self.base_byte + end)
    }
}

pub(crate) fn selection_path(node: &EquationNode, offset: u32, output: &mut Vec<SourceRange>) {
    if !node.range.contains(offset) {
        return;
    }
    for child in &node.children {
        selection_path(child, offset, output);
    }
    if output.last() != Some(&node.range) && node.range.start_offset < node.range.end_offset {
        output.push(node.range.clone());
    }
}

pub(crate) fn deepest_node(node: &EquationNode, offset: u32) -> Option<&EquationNode> {
    if !node.range.contains(offset) {
        return None;
    }
    node.children
        .iter()
        .find_map(|child| deepest_node(child, offset))
        .or(Some(node))
}

#[cfg(test)]
mod tests {
    use super::{math_regions, parse_regions, selection_path};
    use crate::DocumentLanguage;

    #[test]
    fn finds_markdown_math_but_not_fenced_code() {
        let source = "before $x_i$\n```\n$ignored$\n```\nafter \\[\\frac{1}{N}\\]";
        let regions = math_regions(source, DocumentLanguage::Markdown);
        assert_eq!(regions.len(), 2);
        assert!(regions.iter().all(|region| region.closed));
    }

    #[test]
    fn builds_nested_selection_ranges() {
        let source = "$\\frac{1}{N}x_i$";
        let regions = math_regions(source, DocumentLanguage::Latex);
        let parsed = parse_regions(source, &regions);
        let x = parsed[0]
            .symbols
            .iter()
            .find(|(symbol, _)| symbol == "x")
            .unwrap();
        let mut ranges = Vec::new();
        selection_path(&parsed[0].root, x.1.start_offset, &mut ranges);
        assert!(ranges.len() >= 2);
        assert!(ranges.windows(2).all(|pair| {
            pair[0].start_offset >= pair[1].start_offset && pair[0].end_offset <= pair[1].end_offset
        }));
    }
}
