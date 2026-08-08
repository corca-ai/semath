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
        self.parse_atom_with_application(true)
    }

    fn parse_atom_with_application(&mut self, allow_application: bool) -> EquationNode {
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
                // An unbraced script consumes exactly one atom. In particular,
                // `\sum^n (x)` must not reinterpret `(x)` as an application of
                // the superscript `n`.
                self.parse_atom_with_application(false)
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
        while allow_application && application_head(&node) {
            self.skip_whitespace();
            let Some(open @ ('(' | '[')) = self.peek() else {
                break;
            };
            let arguments = self.parse_delimited(open);
            node = self.node(
                "application",
                None,
                start,
                self.cursor,
                vec![node, arguments],
            );
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
        if name == "left" {
            return self.parse_left_right(start);
        }
        if name == "begin" {
            return self.parse_environment(start);
        }
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

    fn parse_left_right(&mut self, start: usize) -> EquationNode {
        self.skip_whitespace();
        let open = self.consume_delimiter();
        let body_start = self.cursor;
        let right = self.source[body_start..]
            .find("\\right")
            .map(|offset| body_start + offset);
        let body_end = right.unwrap_or(self.source.len());
        let body = self.parse_slice(body_start, body_end);
        self.cursor = body_end;
        let close = if right.is_some() {
            self.cursor += "\\right".len();
            self.skip_whitespace();
            self.consume_delimiter()
        } else {
            String::new()
        };
        self.node(
            "delimited",
            Some(format!("{open}{close}")),
            start,
            self.cursor,
            vec![body],
        )
    }

    fn parse_environment(&mut self, start: usize) -> EquationNode {
        self.skip_whitespace();
        let Some((environment, _name_node)) = self.consume_literal_group() else {
            return self.node(
                "command",
                Some("\\begin".into()),
                start,
                self.cursor,
                Vec::new(),
            );
        };
        if !matches!(
            environment.as_str(),
            "matrix"
                | "pmatrix"
                | "bmatrix"
                | "Bmatrix"
                | "vmatrix"
                | "Vmatrix"
                | "smallmatrix"
                | "cases"
        ) {
            return self.node(
                "environment",
                Some(environment),
                start,
                self.cursor,
                Vec::new(),
            );
        }

        let body_start = self.cursor;
        let end_marker = format!("\\end{{{environment}}}");
        let end_start = self.source[body_start..]
            .find(&end_marker)
            .map_or(self.source.len(), |offset| body_start + offset);
        let rows = self.environment_rows(body_start, end_start);
        self.cursor = if end_start < self.source.len() {
            end_start + end_marker.len()
        } else {
            end_start
        };
        self.node(
            if environment == "cases" {
                "cases"
            } else {
                "matrix"
            },
            Some(environment),
            start,
            self.cursor,
            rows,
        )
    }

    fn environment_rows(&mut self, start: usize, end: usize) -> Vec<EquationNode> {
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        let mut cell_start = start;
        let mut row_start = start;
        let mut cursor = start;
        let mut depth = 0usize;

        while cursor < end {
            let rest = &self.source[cursor..end];
            if depth == 0 && rest.starts_with("\\\\") {
                cells.push(self.environment_cell(cell_start, cursor));
                rows.push(self.environment_row(row_start, cursor, cells));
                cells = Vec::new();
                cursor += 2;
                cell_start = cursor;
                row_start = cursor;
                continue;
            }
            let character = rest.chars().next().unwrap();
            if depth == 0 && character == '&' {
                cells.push(self.environment_cell(cell_start, cursor));
                cursor += 1;
                cell_start = cursor;
                continue;
            }
            match character {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            cursor += character.len_utf8();
        }
        if cell_start < end || !cells.is_empty() {
            cells.push(self.environment_cell(cell_start, end));
            rows.push(self.environment_row(row_start, end, cells));
        }
        rows
    }

    fn environment_cell(&mut self, start: usize, end: usize) -> EquationNode {
        let body = self.parse_slice(start, end);
        self.node("cell", None, start, end, vec![body])
    }

    fn environment_row(&self, start: usize, end: usize, cells: Vec<EquationNode>) -> EquationNode {
        self.node("row", None, start, end, cells)
    }

    fn parse_slice(&mut self, start: usize, end: usize) -> EquationNode {
        let mut parser = Parser::new(&self.source[start..end], self.base_byte + start, self.index);
        let root = parser.parse_sequence(None);
        self.symbols.extend(parser.symbols);
        root
    }

    fn consume_literal_group(&mut self) -> Option<(String, EquationNode)> {
        if self.peek() != Some('{') {
            return None;
        }
        let start = self.cursor;
        self.bump();
        let content_start = self.cursor;
        while self.peek().is_some_and(|character| character != '}') {
            self.bump();
        }
        let content_end = self.cursor;
        if self.peek() == Some('}') {
            self.bump();
        }
        Some((
            self.source[content_start..content_end].to_string(),
            self.node("group", None, start, self.cursor, Vec::new()),
        ))
    }

    fn consume_delimiter(&mut self) -> String {
        let start = self.cursor;
        if self.peek() == Some('\\') {
            self.bump();
            while self.peek().is_some_and(char::is_alphabetic) {
                self.bump();
            }
        } else {
            self.bump();
        }
        self.source[start..self.cursor].to_string()
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

fn application_head(node: &EquationNode) -> bool {
    matches!(
        node.kind.as_str(),
        "symbol" | "command" | "styled" | "scripted" | "application"
    )
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

    #[test]
    fn represents_applications_matrices_cases_and_paired_delimiters() {
        let source = concat!(
            "$f(x_i) + ",
            "\\begin{bmatrix}a & b \\\\ c & d\\end{bmatrix} + ",
            "\\begin{cases}x & x > 0 \\\\ -x & x \\le 0\\end{cases} + ",
            "\\left( y + 1 \\right)$",
        );
        let parsed = parse_regions(source, &math_regions(source, DocumentLanguage::Latex));
        let root = &parsed[0].root;
        let kinds = root
            .children
            .iter()
            .map(|node| node.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"application"));
        let matrix = root
            .children
            .iter()
            .find(|node| node.kind == "matrix")
            .unwrap();
        assert_eq!(matrix.children.len(), 2);
        assert!(matrix.children.iter().all(|row| row.children.len() == 2));
        let cases = root
            .children
            .iter()
            .find(|node| node.kind == "cases")
            .unwrap();
        assert_eq!(cases.children.len(), 2);
        assert!(
            root.children
                .iter()
                .any(|node| { node.kind == "delimited" && node.label.as_deref() == Some("()") })
        );
    }
}
