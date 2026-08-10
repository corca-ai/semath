use crate::{NotationNodeKind, ProjectDocument, SourceRange};
#[cfg(test)]
use crate::{ProjectMacroExpansionStatus, SourceIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticExpr {
    pub kind: SemanticExprKind,
    pub range: SourceRange,
    pub provenance: Vec<SourceRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SemanticExprKind {
    Symbol(String),
    Number(String),
    Sum(Vec<SemanticExpr>),
    Product(Vec<SemanticExpr>),
    Dot(Box<SemanticExpr>, Box<SemanticExpr>),
    Cross(Box<SemanticExpr>, Box<SemanticExpr>),
    Fraction(Box<SemanticExpr>, Box<SemanticExpr>),
    Power(Box<SemanticExpr>, Box<SemanticExpr>),
    Negate(Box<SemanticExpr>),
    Derivative {
        expression: Box<SemanticExpr>,
        variable: String,
        order: u8,
    },
    Relation {
        operator: String,
        left: Box<SemanticExpr>,
        right: Box<SemanticExpr>,
    },
    Apply {
        operator: String,
        arguments: Vec<SemanticExpr>,
    },
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SurfaceChunk {
    text: String,
    range: SourceRange,
    provenance: Vec<SourceRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TokenKind {
    Command(String),
    Identifier(String),
    Number(String),
    Operator(char),
    Open(char),
    Close(char),
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    range: SourceRange,
    provenance: Vec<SourceRange>,
}

pub(crate) fn lower_document_region(
    document: &ProjectDocument,
    range: &SourceRange,
) -> SemanticExpr {
    #[cfg(test)]
    if document.nodes.is_empty() {
        let chunks = expanded_surface(document, range);
        return Parser::new(tokenize(&chunks, false)).parse_relation();
    }
    Parser::new(snapshot_tokens(document, range)).parse_relation()
}

fn snapshot_tokens(document: &ProjectDocument, range: &SourceRange) -> Vec<Token> {
    let mut tokens = Vec::new();
    if let Some(root) = document.math_roots.iter().find(|root| {
        root.content_range.start_offset <= range.start_offset
            && range.end_offset <= root.content_range.end_offset
    }) {
        emit_snapshot_node(document, root.node, &mut tokens);
    }
    coalesce_numbers(tokens)
}

fn emit_snapshot_node(document: &ProjectDocument, node_id: u32, tokens: &mut Vec<Token>) {
    let Some(node) = document.nodes.get(node_id as usize) else {
        return;
    };
    let range = node.ranges.full.clone();
    let provenance = syntax_provenance(node);
    let push = |tokens: &mut Vec<Token>, kind: TokenKind| {
        tokens.push(Token {
            kind,
            range: range.clone(),
            provenance: provenance.clone(),
        });
    };
    match node.kind {
        NotationNodeKind::Token => {
            let text = node.text.as_deref().unwrap_or_default();
            if text.chars().all(|character| character.is_whitespace()) {
                return;
            }
            let kind = if text
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.')
                && text.chars().any(|character| character.is_ascii_digit())
            {
                TokenKind::Number(text.to_owned())
            } else if text.chars().count() == 1 {
                match text.chars().next().unwrap() {
                    '{' | '(' | '[' => TokenKind::Open(text.chars().next().unwrap()),
                    '}' | ')' | ']' => TokenKind::Close(text.chars().next().unwrap()),
                    character if "+-=<>*/|,:'&".contains(character) => {
                        TokenKind::Operator(character)
                    }
                    _ => TokenKind::Identifier(text.to_owned()),
                }
            } else {
                TokenKind::Identifier(text.to_owned())
            };
            push(tokens, kind);
        }
        NotationNodeKind::NamedOperator => push(
            tokens,
            TokenKind::Identifier(node.name.clone().unwrap_or_default()),
        ),
        NotationNodeKind::Group => {
            push(tokens, TokenKind::Open('{'));
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
            push(tokens, TokenKind::Close('}'));
        }
        NotationNodeKind::Script => {
            if let Some(base) = node.children.first() {
                emit_snapshot_node(document, *base, tokens);
            }
            push(
                tokens,
                TokenKind::Operator(if node.name.as_deref() == Some("superscript") {
                    '^'
                } else {
                    '_'
                }),
            );
            if let Some(script) = node.children.get(1) {
                emit_snapshot_node(document, *script, tokens);
            }
        }
        NotationNodeKind::Modifier => {
            if matches!(node.name.as_deref(), Some("dot" | "ddot")) {
                push(
                    tokens,
                    TokenKind::Command(node.name.clone().unwrap_or_default()),
                );
            }
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
        }
        NotationNodeKind::Style => {
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
        }
        NotationNodeKind::Command => {
            if is_spacing_command(node.name.as_deref()) {
                return;
            }
            push(
                tokens,
                TokenKind::Command(node.name.clone().unwrap_or_default()),
            );
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
        }
        NotationNodeKind::Opaque | NotationNodeKind::Error => {}
        NotationNodeKind::Delimiter => {
            let delimiters = match node.name.as_deref() {
                Some("()") => Some(('(', ')')),
                Some("[]") => Some(('[', ']')),
                Some("{}") => Some(('{', '}')),
                _ => None,
            };
            if let Some((open, _)) = delimiters {
                push(tokens, TokenKind::Open(open));
            }
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
            if let Some((_, close)) = delimiters {
                push(tokens, TokenKind::Close(close));
            }
        }
        NotationNodeKind::Sequence
        | NotationNodeKind::Alignment
        | NotationNodeKind::Environment => {
            for child in &node.children {
                emit_snapshot_node(document, *child, tokens);
            }
        }
    }
}

fn is_spacing_command(name: Option<&str>) -> bool {
    matches!(
        name,
        Some(
            " " | ","
                | ":"
                | ";"
                | "!"
                | "quad"
                | "qquad"
                | "enspace"
                | "thinspace"
                | "medspace"
                | "thickspace"
                | "negthinspace"
        )
    )
}

fn syntax_provenance(node: &crate::NotationNode) -> Vec<SourceRange> {
    let Some(provenance) = &node.provenance else {
        return Vec::new();
    };
    if provenance.origin == "source" {
        return Vec::new();
    }
    let mut ranges = vec![provenance.source.range.clone()];
    ranges.extend(
        provenance
            .call_site
            .iter()
            .map(|source| source.range.clone()),
    );
    ranges.extend(
        provenance
            .definitions
            .iter()
            .map(|source| source.range.clone()),
    );
    ranges.sort_by_key(|range| (range.start_offset, range.end_offset));
    ranges.dedup();
    ranges
}

fn coalesce_numbers(tokens: Vec<Token>) -> Vec<Token> {
    let mut output: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if let Some(previous) = output.last_mut()
            && let (TokenKind::Number(left), TokenKind::Number(right)) =
                (&mut previous.kind, &token.kind)
            && previous.range.end_offset == token.range.start_offset
            && previous.provenance == token.provenance
        {
            left.push_str(right);
            previous.range.end_offset = token.range.end_offset;
            continue;
        }
        output.push(token);
    }
    output
}

pub(crate) fn lower_template(source: &str) -> SemanticExpr {
    let range = SourceRange {
        start_offset: 0,
        end_offset: source.encode_utf16().count() as u32,
    };
    Parser::new(tokenize(
        &[SurfaceChunk {
            text: source.into(),
            range,
            provenance: Vec::new(),
        }],
        true,
    ))
    .parse_relation()
}

pub(crate) fn canonical_template(source: &str) -> String {
    render_canonical(&lower_template(source))
}

fn render_canonical(expression: &SemanticExpr) -> String {
    match &expression.kind {
        SemanticExprKind::Symbol(value) => format!("symbol({value})"),
        SemanticExprKind::Number(value) => format!("number({value})"),
        SemanticExprKind::Sum(items) => render_list("sum", items),
        SemanticExprKind::Product(items) => render_list("product", items),
        SemanticExprKind::Dot(left, right) => render_pair("dot", left, right),
        SemanticExprKind::Cross(left, right) => render_pair("cross", left, right),
        SemanticExprKind::Fraction(left, right) => render_pair("fraction", left, right),
        SemanticExprKind::Power(left, right) => render_pair("power", left, right),
        SemanticExprKind::Negate(inner) => format!("negate({})", render_canonical(inner)),
        SemanticExprKind::Derivative {
            expression,
            variable,
            order,
        } => format!(
            "derivative({},{variable},{order})",
            render_canonical(expression)
        ),
        SemanticExprKind::Relation {
            operator,
            left,
            right,
        } => format!(
            "relation({operator},{},{})",
            render_canonical(left),
            render_canonical(right)
        ),
        SemanticExprKind::Apply {
            operator,
            arguments,
        } => format!("apply({operator},{})", render_items(arguments)),
        SemanticExprKind::Unknown(value) => format!("unknown({value})"),
    }
}

fn render_list(name: &str, items: &[SemanticExpr]) -> String {
    format!("{name}({})", render_items(items))
}

fn render_items(items: &[SemanticExpr]) -> String {
    items
        .iter()
        .map(render_canonical)
        .collect::<Vec<_>>()
        .join(",")
}

fn render_pair(name: &str, left: &SemanticExpr, right: &SemanticExpr) -> String {
    format!(
        "{name}({},{})",
        render_canonical(left),
        render_canonical(right)
    )
}

pub(crate) fn declared_symbols(
    document: &ProjectDocument,
    range: &SourceRange,
) -> Vec<(String, SourceRange)> {
    let expression = lower_document_region(document, range);
    if let SemanticExprKind::Symbol(name) = expression.kind {
        return vec![(name, expression.range)];
    }
    if let SemanticExprKind::Relation { operator, left, .. } = &expression.kind
        && matches!(operator.as_str(), "member-of" | "not-member-of")
        && let SemanticExprKind::Symbol(name) = &left.kind
    {
        return vec![(name.clone(), left.range.clone())];
    }
    let SemanticExprKind::Product(items) = expression.kind else {
        return Vec::new();
    };
    items
        .into_iter()
        .take_while(|item| !matches!(symbol_name(item), Some("in")))
        .filter_map(|item| match item.kind {
            SemanticExprKind::Symbol(name)
                if name.chars().any(char::is_alphabetic) && name != "mathbb" =>
            {
                Some((name, item.range))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
fn expanded_surface(document: &ProjectDocument, range: &SourceRange) -> Vec<SurfaceChunk> {
    let index = SourceIndex::new(&document.content);
    let start = index.byte_for_utf16(range.start_offset);
    let end = index.byte_for_utf16(range.end_offset);
    let mut calls = document
        .macros
        .iter()
        .filter(|event| {
            event.kind == crate::ProjectMacroKind::Call
                && event.expansion.status == ProjectMacroExpansionStatus::Expanded
        })
        .filter_map(|event| {
            Some((
                event.expansion.input_range.as_ref()?,
                event.expansion.surface.as_ref()?,
                &event.source.range,
            ))
        })
        .filter(|(call, _, _)| {
            range.start_offset <= call.start_offset && call.end_offset <= range.end_offset
        })
        .collect::<Vec<_>>();
    calls.sort_by_key(|(call, _, _)| call.start_offset);

    let mut chunks = Vec::new();
    let mut cursor = start;
    for (call, surface, source) in calls {
        let call_start = index.byte_for_utf16(call.start_offset);
        let call_end = index.byte_for_utf16(call.end_offset);
        if call_start < cursor || call_end > end {
            continue;
        }
        push_source_chunk(&mut chunks, &document.content, &index, cursor, call_start);
        chunks.push(SurfaceChunk {
            text: surface.clone(),
            range: call.clone(),
            provenance: vec![source.clone()],
        });
        cursor = call_end;
    }
    push_source_chunk(&mut chunks, &document.content, &index, cursor, end);
    chunks
}

#[cfg(test)]
fn push_source_chunk(
    chunks: &mut Vec<SurfaceChunk>,
    source: &str,
    index: &SourceIndex,
    start: usize,
    end: usize,
) {
    if start >= end {
        return;
    }
    chunks.push(SurfaceChunk {
        text: source[start..end].into(),
        range: SourceRange {
            start_offset: index.utf16_for_byte(start),
            end_offset: index.utf16_for_byte(end),
        },
        provenance: Vec::new(),
    });
}

fn tokenize(chunks: &[SurfaceChunk], word_identifiers: bool) -> Vec<Token> {
    let mut tokens = Vec::new();
    for chunk in chunks {
        let mut cursor = 0;
        while cursor < chunk.text.len() {
            let character = chunk.text[cursor..].chars().next().unwrap();
            if character.is_whitespace() {
                cursor += character.len_utf8();
                continue;
            }
            let start = cursor;
            let kind = if character == '\\' {
                cursor += 1;
                let name_start = cursor;
                while cursor < chunk.text.len()
                    && chunk.text[cursor..]
                        .chars()
                        .next()
                        .is_some_and(|next| next.is_alphabetic())
                {
                    cursor += chunk.text[cursor..].chars().next().unwrap().len_utf8();
                }
                if name_start == cursor {
                    cursor += chunk.text[cursor..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8);
                }
                let name = &chunk.text[name_start..cursor];
                if matches!(name, "begin" | "end") {
                    cursor = skip_braced_argument(&chunk.text, cursor);
                    continue;
                }
                if matches!(
                    name,
                    "left"
                        | "right"
                        | "bigl"
                        | "bigr"
                        | "Bigl"
                        | "Bigr"
                        | "big"
                        | "Big"
                        | "displaystyle"
                        | "textstyle"
                        | "scriptstyle"
                        | ","
                        | ";"
                        | "!"
                        | "quad"
                        | "qquad"
                        | "rm"
                        | "\\"
                ) {
                    if matches!(name, "left" | "right") && chunk.text[cursor..].starts_with('.') {
                        cursor += 1;
                    }
                    continue;
                }
                TokenKind::Command(name.into())
            } else if character.is_alphabetic() {
                cursor += character.len_utf8();
                while word_identifiers
                    && cursor < chunk.text.len()
                    && chunk.text[cursor..]
                        .chars()
                        .next()
                        .is_some_and(|next| next.is_alphanumeric() || next == '-')
                {
                    cursor += chunk.text[cursor..].chars().next().unwrap().len_utf8();
                }
                TokenKind::Identifier(chunk.text[start..cursor].into())
            } else if character.is_ascii_digit() || character == '.' {
                cursor += character.len_utf8();
                while cursor < chunk.text.len()
                    && chunk.text[cursor..]
                        .chars()
                        .next()
                        .is_some_and(|next| next.is_ascii_digit() || next == '.')
                {
                    cursor += chunk.text[cursor..].chars().next().unwrap().len_utf8();
                }
                TokenKind::Number(chunk.text[start..cursor].into())
            } else if character == '&' {
                cursor += character.len_utf8();
                continue;
            } else {
                cursor += character.len_utf8();
                match character {
                    '{' | '(' | '[' => TokenKind::Open(character),
                    '}' | ')' | ']' => TokenKind::Close(character),
                    other => TokenKind::Operator(other),
                }
            };
            tokens.push(Token {
                kind,
                range: token_range(chunk, start, cursor),
                provenance: chunk.provenance.clone(),
            });
        }
    }
    tokens
}

fn skip_braced_argument(source: &str, mut cursor: usize) -> usize {
    while cursor < source.len()
        && source[cursor..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
    {
        cursor += source[cursor..].chars().next().unwrap().len_utf8();
    }
    if source[cursor..].starts_with('{')
        && let Some(end) = source[cursor + 1..].find('}')
    {
        return cursor + end + 2;
    }
    cursor
}

fn token_range(chunk: &SurfaceChunk, start: usize, end: usize) -> SourceRange {
    if !chunk.provenance.is_empty() {
        return chunk.range.clone();
    }
    SourceRange {
        start_offset: chunk.range.start_offset + chunk.text[..start].encode_utf16().count() as u32,
        end_offset: chunk.range.start_offset + chunk.text[..end].encode_utf16().count() as u32,
    }
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn parse_relation(&mut self) -> SemanticExpr {
        let left = self.parse_set_operation();
        if let Some(operator) = self.consume_relation() {
            let right = self.parse_set_operation();
            return combined(
                &left,
                &right,
                SemanticExprKind::Relation {
                    operator,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                },
            );
        }
        left
    }

    fn parse_set_operation(&mut self) -> SemanticExpr {
        let mut expression = self.parse_sum();
        loop {
            let operator = if self.consume_command("cap") {
                "intersection"
            } else if self.consume_command("cup") {
                "union"
            } else if self.consume_command("circ") {
                "compose"
            } else {
                break;
            };
            let right = self.parse_sum();
            expression = combined(
                &expression,
                &right,
                SemanticExprKind::Apply {
                    operator: operator.into(),
                    arguments: vec![expression.clone(), right.clone()],
                },
            );
        }
        expression
    }

    fn parse_sum(&mut self) -> SemanticExpr {
        let mut terms = vec![self.parse_product()];
        loop {
            if self.consume_operator('+') {
                terms.push(self.parse_product());
            } else if self.consume_operator('-') {
                let term = self.parse_product();
                terms.push(SemanticExpr {
                    range: term.range.clone(),
                    provenance: term.provenance.clone(),
                    kind: SemanticExprKind::Negate(Box::new(term)),
                });
            } else {
                break;
            }
        }
        associative(terms, SemanticExprKind::Sum)
    }

    fn parse_product(&mut self) -> SemanticExpr {
        let mut factors = vec![self.parse_power()];
        loop {
            if self.consume_operator('/') {
                let denominator = self.parse_power();
                if factors.len() >= 2
                    && matches!(symbol_name(&factors[factors.len() - 2]), Some("d"))
                    && matches!(symbol_name(&denominator), Some("d"))
                    && starts_atom(self.peek())
                {
                    let variable_expression = self.parse_power();
                    if let Some(variable) = expression_name(&variable_expression) {
                        let differential = factors.pop().unwrap();
                        let _ = factors.pop();
                        factors.push(SemanticExpr {
                            range: merge_range(&differential.range, &variable_expression.range),
                            provenance: merge_provenance(&differential, &variable_expression),
                            kind: SemanticExprKind::Derivative {
                                expression: Box::new(differential),
                                variable,
                                order: 1,
                            },
                        });
                        continue;
                    }
                }
                let numerator = associative(factors, SemanticExprKind::Product);
                return combined(
                    &numerator,
                    &denominator,
                    SemanticExprKind::Fraction(
                        Box::new(numerator.clone()),
                        Box::new(denominator.clone()),
                    ),
                );
            }
            if self.consume_command("cdot") {
                let left = associative(factors, SemanticExprKind::Product);
                let right = self.parse_power();
                factors = vec![combined(
                    &left,
                    &right,
                    SemanticExprKind::Dot(Box::new(left.clone()), Box::new(right.clone())),
                )];
                continue;
            }
            if self.consume_command("times") {
                let left = associative(factors, SemanticExprKind::Product);
                let right = self.parse_power();
                factors = vec![combined(
                    &left,
                    &right,
                    SemanticExprKind::Cross(Box::new(left.clone()), Box::new(right.clone())),
                )];
                continue;
            }
            if matches!(self.peek(), TokenKind::Open('(')) {
                let argument = self.parse_power();
                if let Some(previous) = factors.last().cloned()
                    && let Some(applied) = apply_argument(previous, argument.clone())
                {
                    factors.pop();
                    factors.push(applied);
                    continue;
                }
                factors.push(argument);
                continue;
            }
            if starts_atom(self.peek()) {
                factors.push(self.parse_power());
                continue;
            }
            break;
        }
        if factors.len() == 2
            && matches!(symbol_name(&factors[0]), Some("Delta"))
            && let Some(name) = expression_name(&factors[1])
        {
            let first = factors.remove(0);
            let second = factors.remove(0);
            return combined(
                &first,
                &second,
                SemanticExprKind::Symbol(format!("Delta{name}")),
            );
        }
        let mut index = 0;
        while index + 1 < factors.len() {
            if matches!(symbol_name(&factors[index]), Some("D_t")) {
                let operator = factors.remove(index);
                let expression = factors.remove(index);
                factors.insert(
                    index,
                    SemanticExpr {
                        range: merge_range(&operator.range, &expression.range),
                        provenance: merge_provenance(&operator, &expression),
                        kind: SemanticExprKind::Derivative {
                            expression: Box::new(expression),
                            variable: "t".into(),
                            order: 1,
                        },
                    },
                );
            }
            index += 1;
        }
        if factors.len() == 2
            && let SemanticExprKind::Fraction(numerator, denominator) = &factors[0].kind
            && matches!(symbol_name(numerator), Some("d"))
            && let SemanticExprKind::Product(parts) = &denominator.kind
            && parts.len() == 2
            && matches!(symbol_name(&parts[0]), Some("d"))
            && let Some(variable) = expression_name(&parts[1])
        {
            let operator = factors.remove(0);
            let expression = factors.remove(0);
            return SemanticExpr {
                range: merge_range(&operator.range, &expression.range),
                provenance: merge_provenance(&operator, &expression),
                kind: SemanticExprKind::Derivative {
                    expression: Box::new(expression),
                    variable,
                    order: 1,
                },
            };
        }
        associative(factors, SemanticExprKind::Product)
    }

    fn parse_power(&mut self) -> SemanticExpr {
        let mut expression = self.parse_prefix();
        loop {
            if self.consume_operator('_') {
                let subscript = self.parse_group_or_atom();
                if let Some(subscript_name) = expression_name(&subscript) {
                    expression = apply_subscript(expression, &subscript, &subscript_name);
                }
            } else if self.consume_operator('^') {
                let exponent = self.parse_group_or_atom();
                expression = if matches!(&exponent.kind, SemanticExprKind::Number(value) if value == "1")
                {
                    SemanticExpr {
                        range: merge_range(&expression.range, &exponent.range),
                        ..expression
                    }
                } else if matches!(symbol_name(&exponent), Some("T" | "top" | "intercal")) {
                    SemanticExpr {
                        range: merge_range(&expression.range, &exponent.range),
                        provenance: merge_provenance(&expression, &exponent),
                        kind: SemanticExprKind::Apply {
                            operator: "transpose".into(),
                            arguments: vec![expression],
                        },
                    }
                } else {
                    combined(
                        &expression,
                        &exponent,
                        SemanticExprKind::Power(
                            Box::new(expression.clone()),
                            Box::new(exponent.clone()),
                        ),
                    )
                };
            } else if self.consume_operator('\'') {
                expression = SemanticExpr {
                    range: expression.range.clone(),
                    provenance: expression.provenance.clone(),
                    kind: SemanticExprKind::Derivative {
                        expression: Box::new(expression),
                        variable: "t".into(),
                        order: 1,
                    },
                };
            } else {
                break;
            }
        }
        expression
    }

    fn parse_prefix(&mut self) -> SemanticExpr {
        if self.consume_operator('-') {
            let expression = self.parse_prefix();
            return SemanticExpr {
                range: expression.range.clone(),
                provenance: expression.provenance.clone(),
                kind: SemanticExprKind::Negate(Box::new(expression)),
            };
        }
        if self.peek_command("frac") || self.peek_command("tfrac") || self.peek_command("dfrac") {
            return self.parse_fraction();
        }
        if let Some(command) = self.peek_command_name().map(str::to_owned) {
            match command.as_str() {
                "mathbf" | "mathrm" | "mathit" | "mathcal" | "mathsf" | "boldsymbol"
                | "operatorname" | "vec" | "tilde" | "boxed" => {
                    let command = self.next();
                    let mut expression = self.parse_group_or_atom();
                    expression.range = merge_range(&command.range, &expression.range);
                    expression.provenance.extend(command.provenance);
                    return expression;
                }
                "dot" | "ddot" => {
                    let token = self.next();
                    let expression = self.parse_group_or_atom();
                    return SemanticExpr {
                        range: merge_range(&token.range, &expression.range),
                        provenance: [token.provenance, expression.provenance.clone()].concat(),
                        kind: SemanticExprKind::Derivative {
                            expression: Box::new(expression),
                            variable: "t".into(),
                            order: u8::from(command == "ddot") + 1,
                        },
                    };
                }
                "sum" => {
                    let token = self.next();
                    return SemanticExpr {
                        kind: SemanticExprKind::Apply {
                            operator: "sum".into(),
                            arguments: Vec::new(),
                        },
                        range: token.range,
                        provenance: token.provenance,
                    };
                }
                "underbrace" => {
                    let command = self.next();
                    let mut expression = self.parse_group_or_atom();
                    expression.range = merge_range(&command.range, &expression.range);
                    if self.consume_operator('_') {
                        let _ = self.parse_group_or_atom();
                    }
                    return expression;
                }
                "left" => {
                    let command = self.next();
                    let mut expression = self.parse_group_or_atom();
                    let end = if self.consume_command("right") {
                        let end = self
                            .tokens
                            .get(self.cursor)
                            .map(|token| token.range.clone());
                        if matches!(self.peek(), TokenKind::Close(_)) {
                            self.cursor += 1;
                        }
                        end
                    } else {
                        None
                    };
                    expression.range = end.as_ref().map_or_else(
                        || merge_range(&command.range, &expression.range),
                        |end| merge_range(&command.range, end),
                    );
                    expression.provenance.extend(command.provenance);
                    return expression;
                }
                "lVert" => {
                    let command = self.next();
                    let expression = self.parse_power();
                    let end = if self.consume_command("rVert") {
                        self.tokens[self.cursor - 1].range.clone()
                    } else {
                        expression.range.clone()
                    };
                    return SemanticExpr {
                        range: merge_range(&command.range, &end),
                        provenance: [command.provenance, expression.provenance.clone()].concat(),
                        kind: SemanticExprKind::Apply {
                            operator: "norm".into(),
                            arguments: vec![expression],
                        },
                    };
                }
                _ => {}
            }
        }
        self.parse_atom()
    }

    fn parse_fraction(&mut self) -> SemanticExpr {
        let command = self.next();
        let compact_digits = match self.peek() {
            TokenKind::Number(value) if value.len() == 2 => Some(value.clone()),
            _ => None,
        };
        let (numerator, denominator) = match compact_digits {
            Some(value) => {
                let token = self.next();
                let midpoint = token.range.start_offset + 1;
                (
                    SemanticExpr {
                        kind: SemanticExprKind::Number(value[..1].into()),
                        range: SourceRange {
                            start_offset: token.range.start_offset,
                            end_offset: midpoint,
                        },
                        provenance: token.provenance.clone(),
                    },
                    SemanticExpr {
                        kind: SemanticExprKind::Number(value[1..].into()),
                        range: SourceRange {
                            start_offset: midpoint,
                            end_offset: token.range.end_offset,
                        },
                        provenance: token.provenance,
                    },
                )
            }
            _ => (self.parse_group_or_atom(), self.parse_group_or_atom()),
        };
        if let Some((expression, variable, order)) = derivative_parts(&numerator, &denominator) {
            return SemanticExpr {
                range: merge_range(&command.range, &denominator.range),
                provenance: [
                    command.provenance,
                    numerator.provenance,
                    denominator.provenance,
                ]
                .concat(),
                kind: SemanticExprKind::Derivative {
                    expression: Box::new(expression),
                    variable,
                    order,
                },
            };
        }
        combined(
            &numerator,
            &denominator,
            SemanticExprKind::Fraction(Box::new(numerator.clone()), Box::new(denominator.clone())),
        )
    }

    fn parse_atom(&mut self) -> SemanticExpr {
        let token = self.next();
        match token.kind {
            TokenKind::Identifier(name) | TokenKind::Command(name) => SemanticExpr {
                kind: SemanticExprKind::Symbol(name),
                range: token.range,
                provenance: token.provenance,
            },
            TokenKind::Number(number) => SemanticExpr {
                kind: SemanticExprKind::Number(number),
                range: token.range,
                provenance: token.provenance,
            },
            TokenKind::Operator(operator) => SemanticExpr {
                kind: SemanticExprKind::Symbol(operator.to_string()),
                range: token.range,
                provenance: token.provenance,
            },
            TokenKind::Open(open) => {
                let expression = self.parse_relation();
                let expected = match open {
                    '{' => '}',
                    '(' => ')',
                    '[' => ']',
                    _ => unreachable!(),
                };
                self.consume_close(expected);
                expression
            }
            other => SemanticExpr {
                kind: SemanticExprKind::Unknown(format!("{other:?}")),
                range: token.range,
                provenance: token.provenance,
            },
        }
    }

    fn parse_group_or_atom(&mut self) -> SemanticExpr {
        self.parse_prefix()
    }

    fn consume_relation(&mut self) -> Option<String> {
        if self.consume_operator('=') {
            Some("equals".into())
        } else if self.consume_command("le") || self.consume_command("leq") {
            Some("less-or-equal".into())
        } else if self.consume_command("ge") || self.consume_command("geq") {
            Some("greater-or-equal".into())
        } else if self.consume_command("in") {
            Some("member-of".into())
        } else if self.consume_command("notin") {
            Some("not-member-of".into())
        } else if self.consume_command("subset") {
            Some("proper-subset-of".into())
        } else if self.consume_command("subseteq") {
            Some("subset-of".into())
        } else if self.consume_command("supset") {
            Some("proper-superset-of".into())
        } else if self.consume_command("supseteq") {
            Some("superset-of".into())
        } else {
            None
        }
    }

    fn consume_operator(&mut self, expected: char) -> bool {
        if matches!(self.peek(), TokenKind::Operator(actual) if *actual == expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn consume_close(&mut self, expected: char) {
        if matches!(self.peek(), TokenKind::Close(actual) if *actual == expected) {
            self.cursor += 1;
        }
    }

    fn consume_command(&mut self, expected: &str) -> bool {
        if self.peek_command(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek_command(&self, expected: &str) -> bool {
        matches!(self.peek(), TokenKind::Command(name) if name == expected)
    }

    fn peek_command_name(&self) -> Option<&str> {
        match self.peek() {
            TokenKind::Command(name) => Some(name),
            _ => None,
        }
    }

    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.cursor)
            .map_or(&TokenKind::End, |token| &token.kind)
    }

    fn next(&mut self) -> Token {
        let token = self.tokens.get(self.cursor).cloned().unwrap_or(Token {
            kind: TokenKind::End,
            range: SourceRange {
                start_offset: 0,
                end_offset: 0,
            },
            provenance: Vec::new(),
        });
        self.cursor += usize::from(self.cursor < self.tokens.len());
        token
    }
}

fn apply_subscript(
    expression: SemanticExpr,
    subscript: &SemanticExpr,
    subscript_name: &str,
) -> SemanticExpr {
    match expression.kind.clone() {
        SemanticExprKind::Symbol(base_name) => combined(
            &expression,
            subscript,
            SemanticExprKind::Symbol(format!("{base_name}_{subscript_name}")),
        ),
        SemanticExprKind::Negate(inner) => {
            let inner = apply_subscript(*inner, subscript, subscript_name);
            SemanticExpr {
                range: merge_range(&expression.range, &subscript.range),
                provenance: merge_provenance(&expression, subscript),
                kind: SemanticExprKind::Negate(Box::new(inner)),
            }
        }
        SemanticExprKind::Derivative {
            expression: inner,
            variable,
            order,
        } => {
            let inner = apply_subscript(*inner, subscript, subscript_name);
            SemanticExpr {
                range: merge_range(&expression.range, &subscript.range),
                provenance: merge_provenance(&expression, subscript),
                kind: SemanticExprKind::Derivative {
                    expression: Box::new(inner),
                    variable,
                    order,
                },
            }
        }
        _ => expression,
    }
}

fn apply_argument(expression: SemanticExpr, argument: SemanticExpr) -> Option<SemanticExpr> {
    match expression.kind.clone() {
        SemanticExprKind::Symbol(operator) => Some(SemanticExpr {
            range: merge_range(&expression.range, &argument.range),
            provenance: merge_provenance(&expression, &argument),
            kind: SemanticExprKind::Apply {
                operator,
                arguments: split_arguments(argument),
            },
        }),
        SemanticExprKind::Derivative {
            expression: inner,
            variable,
            order,
        } => {
            let applied = apply_argument(*inner, argument)?;
            Some(SemanticExpr {
                range: applied.range.clone(),
                provenance: applied.provenance.clone(),
                kind: SemanticExprKind::Derivative {
                    expression: Box::new(applied),
                    variable,
                    order,
                },
            })
        }
        _ => None,
    }
}

fn split_arguments(argument: SemanticExpr) -> Vec<SemanticExpr> {
    let SemanticExprKind::Product(items) = &argument.kind else {
        return vec![argument];
    };
    if !items
        .iter()
        .any(|item| matches!(symbol_name(item), Some(",")))
    {
        return vec![argument];
    }
    let mut arguments = Vec::new();
    let mut current = Vec::new();
    for item in items {
        if matches!(symbol_name(item), Some(",")) {
            if !current.is_empty() {
                arguments.push(associative(
                    std::mem::take(&mut current),
                    SemanticExprKind::Product,
                ));
            }
        } else {
            current.push(item.clone());
        }
    }
    if !current.is_empty() {
        arguments.push(associative(current, SemanticExprKind::Product));
    }
    arguments
}

fn starts_atom(token: &TokenKind) -> bool {
    match token {
        TokenKind::Command(command) => !matches!(
            command.as_str(),
            "cap"
                | "cdot"
                | "circ"
                | "cup"
                | "ge"
                | "geq"
                | "in"
                | "le"
                | "leq"
                | "notin"
                | "right"
                | "subset"
                | "subseteq"
                | "supset"
                | "supseteq"
                | "times"
        ),
        TokenKind::Identifier(_) | TokenKind::Number(_) | TokenKind::Open(_) => true,
        TokenKind::Operator(',') => true,
        _ => false,
    }
}

fn derivative_parts(
    numerator: &SemanticExpr,
    denominator: &SemanticExpr,
) -> Option<(SemanticExpr, String, u8)> {
    let SemanticExprKind::Product(numerator_factors) = &numerator.kind else {
        return None;
    };
    let SemanticExprKind::Product(denominator_factors) = &denominator.kind else {
        return None;
    };
    let (Some("d"), Some(expression), Some("d"), Some(variable)) = (
        numerator_factors.first().and_then(symbol_name),
        numerator_factors.get(1),
        denominator_factors.first().and_then(symbol_name),
        denominator_factors.get(1).and_then(symbol_name),
    ) else {
        return None;
    };
    Some((expression.clone(), variable.into(), 1))
}

fn symbol_name(expression: &SemanticExpr) -> Option<&str> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) => Some(name),
        _ => None,
    }
}

fn expression_name(expression: &SemanticExpr) -> Option<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) | SemanticExprKind::Number(name) => Some(name.clone()),
        SemanticExprKind::Product(items) => items
            .iter()
            .map(expression_name)
            .collect::<Option<Vec<_>>>()
            .map(|items| items.concat()),
        _ => None,
    }
}

pub(crate) fn associative(
    mut expressions: Vec<SemanticExpr>,
    constructor: impl FnOnce(Vec<SemanticExpr>) -> SemanticExprKind,
) -> SemanticExpr {
    if expressions.len() == 1 {
        return expressions.remove(0);
    }
    let range = merge_range(
        &expressions.first().unwrap().range,
        &expressions.last().unwrap().range,
    );
    let provenance = expressions
        .iter()
        .flat_map(|expression| expression.provenance.clone())
        .collect();
    SemanticExpr {
        kind: constructor(expressions),
        range,
        provenance,
    }
}

fn combined(left: &SemanticExpr, right: &SemanticExpr, kind: SemanticExprKind) -> SemanticExpr {
    SemanticExpr {
        kind,
        range: merge_range(&left.range, &right.range),
        provenance: merge_provenance(left, right),
    }
}

fn merge_range(left: &SourceRange, right: &SourceRange) -> SourceRange {
    SourceRange {
        start_offset: left.start_offset.min(right.start_offset),
        end_offset: left.end_offset.max(right.end_offset),
    }
}

fn merge_provenance(left: &SemanticExpr, right: &SemanticExpr) -> Vec<SourceRange> {
    let mut provenance = left.provenance.clone();
    provenance.extend(right.provenance.iter().cloned());
    provenance.sort_by_key(|range| (range.start_offset, range.end_offset));
    provenance.dedup();
    provenance
}

#[cfg(test)]
mod tests {
    use super::{SemanticExprKind, lower_document_region, lower_template, render_canonical};
    use crate::{ProjectDocument, SourceRange};

    #[test]
    fn presentation_forms_share_the_same_semantic_symbol() {
        for source in ["x", "{x}", "\\mathbf{x}", "\\boldsymbol{x}", "\\vec{x}"] {
            assert!(matches!(
                lower_template(source).kind,
                SemanticExprKind::Symbol(ref name) if name == "x"
            ));
        }
    }

    #[test]
    fn lowers_engineering_operators_compositionally() {
        let state = lower_template("\\dot{x}=Ax+Bu");
        assert!(matches!(state.kind, SemanticExprKind::Relation { .. }));
        let lyapunov = lower_template("A^T P + P A = -Q");
        assert!(matches!(lyapunov.kind, SemanticExprKind::Relation { .. }));
        let capacitor = lower_template("i=C\\frac{d v}{d t}");
        assert!(matches!(capacitor.kind, SemanticExprKind::Relation { .. }));
    }

    #[test]
    fn lowers_scientific_relations_and_applications_as_explicit_operators() {
        assert!(matches!(
            lower_template("A \\cup B").kind,
            SemanticExprKind::Apply { ref operator, ref arguments }
                if operator == "union" && arguments.len() == 2
        ));
        assert!(matches!(
            lower_template("x \\in A").kind,
            SemanticExprKind::Relation { ref operator, .. } if operator == "member-of"
        ));
        assert!(matches!(
            lower_template("f(x,y)").kind,
            SemanticExprKind::Apply { ref operator, ref arguments }
                if operator == "f" && arguments.len() == 2
        ));
        assert!(matches!(
            lower_template("f \\circ g").kind,
            SemanticExprKind::Apply { ref operator, ref arguments }
                if operator == "compose" && arguments.len() == 2
        ));
    }

    #[test]
    fn snapshot_lowering_preserves_delimiters_and_ignores_spacing_commands() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 4,
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "v(t)=R\\,i(t)",
            "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"v"},
                {"kind":"token","parent":2,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"t"},
                {"kind":"delimiter","parent":9,"children":[1],"ranges":{"full":{"startOffset":1,"endOffset":4}},"state":"complete","name":"()"},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"="},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":5,"endOffset":6}},"state":"complete","text":"R"},
                {"kind":"command","parent":9,"children":[],"ranges":{"full":{"startOffset":6,"endOffset":8}},"state":"complete","name":","},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":8,"endOffset":9}},"state":"complete","text":"i"},
                {"kind":"token","parent":8,"children":[],"ranges":{"full":{"startOffset":10,"endOffset":11}},"state":"complete","text":"t"},
                {"kind":"delimiter","parent":9,"children":[7],"ranges":{"full":{"startOffset":9,"endOffset":12}},"state":"complete","name":"()"},
                {"kind":"sequence","parent":null,"children":[0,2,3,4,5,6,8],"ranges":{"full":{"startOffset":0,"endOffset":12}},"state":"complete"}
            ],
            "mathRoots": [{"node":9,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":12},"contentRange":{"startOffset":0,"endOffset":12},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":12},"state":"complete"}],
            "declarations": [],
            "macros": [],
            "includes": []
        }))
        .unwrap();
        let expression = lower_document_region(
            &document,
            &SourceRange {
                start_offset: 0,
                end_offset: 12,
            },
        );
        assert_eq!(
            render_canonical(&expression),
            "relation(equals,apply(v,symbol(t)),product(symbol(R),apply(i,symbol(t))))"
        );
    }
}
