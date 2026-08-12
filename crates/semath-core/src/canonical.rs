use crate::{
    GeneratedNotationNode, GeneratedNotationTree, LexicalClass, NotationNodeKind, ProjectDocument,
    ProjectMacroKind, SourceRange,
};
#[cfg(test)]
use crate::{ProjectMacroExpansionStatus, SourceIndex};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticExpr {
    pub kind: SemanticExprKind,
    pub range: SourceRange,
    pub provenance: Vec<SourceRange>,
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticReference {
    pub value: String,
    pub range: SourceRange,
    pub provenance: Vec<SourceRange>,
}

impl PartialEq for SemanticReference {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for SemanticReference {}

impl SemanticReference {
    pub(crate) fn new(
        value: impl Into<String>,
        range: SourceRange,
        provenance: Vec<SourceRange>,
    ) -> Self {
        Self {
            value: value.into(),
            range,
            provenance,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn from_expression(value: impl Into<String>, source: &SemanticExpr) -> Self {
        Self::new(value, source.range.clone(), source.provenance.clone())
    }
}

impl fmt::Display for SemanticReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

impl PartialEq<str> for SemanticReference {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl PartialEq<&str> for SemanticReference {
    fn eq(&self, other: &&str) -> bool {
        self.value == *other
    }
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
        variable: SemanticReference,
        order: u8,
    },
    Relation {
        operator: SemanticReference,
        left: Box<SemanticExpr>,
        right: Box<SemanticExpr>,
    },
    Apply {
        operator: SemanticReference,
        arguments: Vec<SemanticExpr>,
    },
    Index {
        base: Box<SemanticExpr>,
        indices: Vec<SemanticExpr>,
    },
    Condition {
        value: Box<SemanticExpr>,
        predicate: Box<SemanticExpr>,
    },
    Binder {
        operator: SemanticReference,
        variables: Vec<SemanticExpr>,
        lower: Option<Box<SemanticExpr>>,
        upper: Option<Box<SemanticExpr>>,
        body: Box<SemanticExpr>,
    },
    System(Vec<SemanticExpr>),
    Piecewise(Vec<PiecewiseBranch>),
    Unknown(String),
}

pub(crate) fn expression_children(expression: &SemanticExpr) -> Vec<&SemanticExpr> {
    match &expression.kind {
        SemanticExprKind::Sum(items) | SemanticExprKind::Product(items) => items.iter().collect(),
        SemanticExprKind::Dot(left, right)
        | SemanticExprKind::Cross(left, right)
        | SemanticExprKind::Fraction(left, right)
        | SemanticExprKind::Power(left, right)
        | SemanticExprKind::Relation { left, right, .. } => vec![left, right],
        SemanticExprKind::Negate(inner)
        | SemanticExprKind::Derivative {
            expression: inner, ..
        } => vec![inner],
        SemanticExprKind::Apply { arguments, .. } => arguments.iter().collect(),
        SemanticExprKind::Index { base, indices } => {
            std::iter::once(base.as_ref()).chain(indices).collect()
        }
        SemanticExprKind::Condition { value, predicate } => vec![value, predicate],
        SemanticExprKind::Binder {
            variables,
            lower,
            upper,
            body,
            ..
        } => variables
            .iter()
            .chain(lower.iter().map(Box::as_ref))
            .chain(upper.iter().map(Box::as_ref))
            .chain(std::iter::once(body.as_ref()))
            .collect(),
        SemanticExprKind::System(equations) => equations.iter().collect(),
        SemanticExprKind::Piecewise(branches) => branches
            .iter()
            .flat_map(|branch| std::iter::once(&branch.value).chain(branch.condition.as_ref()))
            .collect(),
        SemanticExprKind::Symbol(_)
        | SemanticExprKind::Number(_)
        | SemanticExprKind::Unknown(_) => Vec::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PiecewiseBranch {
    pub value: SemanticExpr,
    pub condition: Option<SemanticExpr>,
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
    Presentation,
    Open { delimiter: char, source_group: bool },
    Close(char),
    Structured(Box<SemanticExpr>),
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
        return Parser::new(canonical_tokens(tokenize(&chunks, false))).parse_document();
    }
    Parser::new(snapshot_tokens(document, range)).parse_document()
}

fn snapshot_tokens(document: &ProjectDocument, range: &SourceRange) -> Vec<Token> {
    let mut tokens = Vec::new();
    if let Some(root) = document.math_roots.iter().find(|root| {
        root.content_range.start_offset <= range.start_offset
            && range.end_offset <= root.content_range.end_offset
    }) {
        emit_notation_node(&NotationArena::Source(document), root.node, &mut tokens);
    }
    canonical_tokens(tokens)
}

enum NotationArena<'a> {
    Source(&'a ProjectDocument),
    Generated {
        tree: &'a GeneratedNotationTree,
        range: &'a SourceRange,
        provenance: &'a [SourceRange],
    },
}

enum ArenaNode<'a> {
    Source(&'a crate::NotationNode),
    Generated(&'a GeneratedNotationNode),
}

impl ArenaNode<'_> {
    fn kind(&self) -> NotationNodeKind {
        match self {
            Self::Source(node) => node.kind,
            Self::Generated(node) => node.kind,
        }
    }

    fn children(&self) -> &[u32] {
        match self {
            Self::Source(node) => &node.children,
            Self::Generated(node) => &node.children,
        }
    }

    fn name(&self) -> Option<&str> {
        match self {
            Self::Source(node) => node.name.as_deref(),
            Self::Generated(node) => node.name.as_deref(),
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Source(node) => node.text.as_deref(),
            Self::Generated(node) => node.text.as_deref(),
        }
    }

    fn lexical_class(&self) -> Option<LexicalClass> {
        match self {
            Self::Source(node) => node.lexical_class,
            Self::Generated(node) => node.lexical_class,
        }
    }

    fn math_class(&self) -> Option<&str> {
        match self {
            Self::Source(node) => node.math_class.as_deref(),
            Self::Generated(node) => node.math_class.as_deref(),
        }
    }

    fn argument_node(&self, role: &str) -> Option<u32> {
        match self {
            Self::Source(node) => node
                .arguments
                .iter()
                .find(|argument| argument.role == role)
                .map(|argument| argument.node),
            Self::Generated(node) => node
                .arguments
                .iter()
                .find(|argument| argument.role == role)
                .map(|argument| argument.node),
        }
    }
}

impl<'a> NotationArena<'a> {
    fn node(&self, node_id: u32) -> Option<ArenaNode<'a>> {
        match self {
            Self::Source(document) => document.nodes.get(node_id as usize).map(ArenaNode::Source),
            Self::Generated { tree, .. } => {
                tree.nodes.get(node_id as usize).map(ArenaNode::Generated)
            }
        }
    }

    fn token_context(&self, node: &ArenaNode<'_>) -> (SourceRange, Vec<SourceRange>) {
        match (self, node) {
            (Self::Source(_), ArenaNode::Source(node)) => {
                (node.ranges.full.clone(), syntax_provenance(node))
            }
            (
                Self::Generated {
                    range, provenance, ..
                },
                ArenaNode::Generated(_),
            ) => ((*range).clone(), provenance.to_vec()),
            _ => unreachable!("arena and node kinds always agree"),
        }
    }
}

fn emit_notation_node(arena: &NotationArena<'_>, node_id: u32, tokens: &mut Vec<Token>) {
    let Some(node) = arena.node(node_id) else {
        return;
    };
    let (range, provenance) = arena.token_context(&node);
    let push = |tokens: &mut Vec<Token>, kind: TokenKind| {
        tokens.push(Token {
            kind,
            range: range.clone(),
            provenance: provenance.clone(),
        });
    };
    let emit_children = |tokens: &mut Vec<Token>| {
        for child in node.children() {
            emit_notation_node(arena, *child, tokens);
        }
    };

    match node.kind() {
        NotationNodeKind::Token => {
            if let Some(kind) =
                semantic_token_kind(node.text().unwrap_or_default(), node.lexical_class())
            {
                push(tokens, kind);
            }
        }
        NotationNodeKind::NamedOperator => push(
            tokens,
            TokenKind::Identifier(node.name().unwrap_or_default().to_owned()),
        ),
        NotationNodeKind::Group => {
            push(
                tokens,
                TokenKind::Open {
                    delimiter: '{',
                    source_group: true,
                },
            );
            emit_children(tokens);
            push(tokens, TokenKind::Close('}'));
        }
        NotationNodeKind::Script => {
            if let (NotationArena::Source(document), ArenaNode::Source(source)) = (arena, &node)
                && source.name.as_deref() == Some("subscript")
                && source
                    .children
                    .first()
                    .and_then(|child| document.nodes.get(*child as usize))
                    .is_some_and(|node| is_evaluation_delimiter(document, node))
            {
                return;
            }
            if let Some(base) = node.children().first() {
                emit_notation_node(arena, *base, tokens);
            }
            if node.name() == Some("prime") {
                push(tokens, TokenKind::Operator('\''));
                return;
            }
            push(
                tokens,
                TokenKind::Operator(if node.name() == Some("superscript") {
                    '^'
                } else {
                    '_'
                }),
            );
            if let Some(script) = node.children().get(1) {
                emit_notation_node(arena, *script, tokens);
            }
        }
        NotationNodeKind::Modifier => {
            if matches!(node.name(), Some("dot" | "ddot")) {
                push(
                    tokens,
                    TokenKind::Command(node.name().unwrap_or_default().to_owned()),
                );
            }
            emit_children(tokens);
        }
        NotationNodeKind::Style => {
            if matches!(
                node.name(),
                Some(
                    "mathbf"
                        | "mathrm"
                        | "mathit"
                        | "mathbb"
                        | "mathcal"
                        | "mathsf"
                        | "boldsymbol"
                        | "operatorname"
                        | "vec"
                        | "tilde"
                        | "boxed"
                )
            ) {
                let command_range = match (&arena, &node) {
                    (NotationArena::Source(_), ArenaNode::Source(source)) => source
                        .ranges
                        .command
                        .clone()
                        .unwrap_or_else(|| range.clone()),
                    _ => range.clone(),
                };
                tokens.push(Token {
                    kind: TokenKind::Command(node.name().unwrap_or_default().to_owned()),
                    range: command_range,
                    provenance: provenance.clone(),
                });
            }
            emit_children(tokens);
        }
        NotationNodeKind::Command => {
            if let (NotationArena::Source(document), ArenaNode::Source(source)) = (arena, &node)
                && let Some((tree, call_range, provenance)) =
                    composite_macro_notation(document, source)
            {
                let generated = NotationArena::Generated {
                    tree,
                    range: call_range,
                    provenance: &provenance,
                };
                let mut expansion_tokens = Vec::new();
                emit_notation_node(&generated, tree.root, &mut expansion_tokens);
                let expression = Parser::new(canonical_tokens(expansion_tokens)).parse_document();
                push(tokens, TokenKind::Structured(Box::new(expression)));
                return;
            }
            if is_math_class_wrapper(node.name())
                && node.math_class().is_some()
                && let Some(nucleus) = node.argument_node("nucleus")
            {
                if let Some(argument) = arena.node(nucleus)
                    && argument.kind() == NotationNodeKind::Group
                {
                    for child in argument.children() {
                        emit_notation_node(arena, *child, tokens);
                    }
                } else {
                    emit_notation_node(arena, nucleus, tokens);
                }
                return;
            }
            if is_presentation_command(node.name()) {
                push(tokens, TokenKind::Presentation);
                return;
            }
            if is_ignorable_command(node.name()) {
                return;
            }
            push(
                tokens,
                TokenKind::Command(node.name().unwrap_or_default().to_owned()),
            );
            emit_children(tokens);
        }
        NotationNodeKind::Error
            if matches!(arena, NotationArena::Generated { .. })
                && matches!(node.name(), Some("superscript" | "subscript")) =>
        {
            push(
                tokens,
                TokenKind::Operator(if node.name() == Some("superscript") {
                    '^'
                } else {
                    '_'
                }),
            );
            emit_children(tokens);
        }
        NotationNodeKind::Opaque | NotationNodeKind::Error => {}
        NotationNodeKind::Delimiter => {
            if let (NotationArena::Source(document), ArenaNode::Source(source)) = (arena, &node)
                && matches!(source.name.as_deref(), Some("left" | "right"))
            {
                emit_sized_delimiter(document, source, tokens);
                return;
            }
            let delimiters = match node.name() {
                Some("()") => Some(('(', ')')),
                Some("[]") => Some(('[', ']')),
                Some("{}") => Some(('{', '}')),
                _ => None,
            };
            if let Some((open, _)) = delimiters {
                push(
                    tokens,
                    TokenKind::Open {
                        delimiter: open,
                        source_group: false,
                    },
                );
            }
            emit_children(tokens);
            if let Some((_, close)) = delimiters {
                push(tokens, TokenKind::Close(close));
            }
        }
        NotationNodeKind::Environment => {
            if let (NotationArena::Source(document), ArenaNode::Source(source)) = (arena, &node)
                && let Some(expression) = lower_structured_environment(document, source)
            {
                push(tokens, TokenKind::Structured(Box::new(expression)));
            } else {
                emit_children(tokens);
            }
        }
        NotationNodeKind::Sequence | NotationNodeKind::Alignment => emit_children(tokens),
    }
}

pub(crate) fn is_math_class_wrapper(name: Option<&str>) -> bool {
    matches!(
        name,
        Some("mathord" | "mathop" | "mathbin" | "mathrel" | "mathopen" | "mathclose" | "mathinner")
    )
}

fn lower_structured_environment(
    document: &ProjectDocument,
    environment: &crate::NotationNode,
) -> Option<SemanticExpr> {
    let rows = environment
        .children
        .iter()
        .copied()
        .filter(|node_id| {
            let Some(node) = document.nodes.get(*node_id as usize) else {
                return false;
            };
            node.kind == NotationNodeKind::Alignment && node.name.as_deref() == Some("row")
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    let kind = match environment.name.as_deref() {
        Some("cases") => {
            let branches = rows
                .iter()
                .filter_map(|row_id| {
                    let row = document.nodes.get(*row_id as usize)?;
                    let cells = row
                        .children
                        .iter()
                        .copied()
                        .filter(|node_id| {
                            let Some(node) = document.nodes.get(*node_id as usize) else {
                                return false;
                            };
                            node.kind == NotationNodeKind::Alignment
                                && node.name.as_deref() == Some("cell")
                        })
                        .collect::<Vec<_>>();
                    let value = lower_source_node(document, *cells.first()?)?;
                    let condition = cells
                        .get(1)
                        .and_then(|cell_id| lower_source_node(document, *cell_id));
                    Some(PiecewiseBranch { value, condition })
                })
                .collect::<Vec<_>>();
            (!branches.is_empty()).then_some(SemanticExprKind::Piecewise(branches))?
        }
        Some("aligned" | "align" | "align*" | "gathered" | "split") => {
            let mut equations = lower_aligned_rows(document, &rows);
            if equations.len() == 1 {
                return equations.pop();
            }
            (!equations.is_empty()).then_some(SemanticExprKind::System(equations))?
        }
        _ => return None,
    };
    Some(SemanticExpr {
        kind,
        range: environment.ranges.full.clone(),
        provenance: syntax_provenance(environment),
    })
}

fn lower_aligned_rows(document: &ProjectDocument, rows: &[u32]) -> Vec<SemanticExpr> {
    let mut equations = Vec::new();
    let mut pending = Vec::new();
    let mut pending_has_relation = false;
    for row_id in rows {
        let mut row = Vec::new();
        emit_notation_node(&NotationArena::Source(document), *row_id, &mut row);
        let row_has_relation = tokens_contain_relation(&row);
        if pending_has_relation && row_has_relation {
            equations
                .push(Parser::new(canonical_tokens(std::mem::take(&mut pending))).parse_document());
            pending_has_relation = false;
        }
        pending.extend(row);
        pending_has_relation |= row_has_relation;
    }
    if !pending.is_empty() {
        equations.push(Parser::new(canonical_tokens(pending)).parse_document());
    }
    equations
}

fn tokens_contain_relation(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| {
        matches!(token.kind, TokenKind::Operator('=' | '<' | '>'))
            || matches!(&token.kind, TokenKind::Command(command) if is_relation_command(command))
    })
}

fn lower_source_node(document: &ProjectDocument, node_id: u32) -> Option<SemanticExpr> {
    let mut tokens = Vec::new();
    emit_notation_node(&NotationArena::Source(document), node_id, &mut tokens);
    let tokens = canonical_tokens(tokens);
    (!tokens.is_empty()).then(|| Parser::new(tokens).parse_document())
}

fn semantic_token_kind(text: &str, lexical_class: Option<LexicalClass>) -> Option<TokenKind> {
    match lexical_class? {
        LexicalClass::Number => Some(TokenKind::Number(text.to_owned())),
        LexicalClass::Operator | LexicalClass::Punctuation => {
            text.chars().next().map(TokenKind::Operator)
        }
        LexicalClass::Identifier => Some(TokenKind::Identifier(text.to_owned())),
        LexicalClass::Other if matches!(text, "." | "," | ";" | ":") => {
            text.chars().next().map(TokenKind::Operator)
        }
        LexicalClass::Other => Some(TokenKind::Identifier(text.to_owned())),
    }
}

fn composite_macro_notation<'a>(
    document: &'a ProjectDocument,
    node: &crate::NotationNode,
) -> Option<(&'a GeneratedNotationTree, &'a SourceRange, Vec<SourceRange>)> {
    let command_start = node.ranges.command.as_ref()?.start_offset;
    let event = document.macros.iter().find(|event| {
        event.kind == ProjectMacroKind::Call
            && event
                .expansion
                .input_range
                .as_ref()
                .is_some_and(|range| range.start_offset == command_start)
    })?;
    let notation = event.expansion.notation.as_ref()?;
    let call_range = event.expansion.input_range.as_ref()?;
    let mut provenance = vec![event.source.range.clone()];
    provenance.extend(
        event
            .definitions
            .iter()
            .map(|definition| definition.range.clone()),
    );
    provenance.sort_by_key(|range| (range.start_offset, range.end_offset));
    provenance.dedup();
    Some((notation, call_range, provenance))
}

fn is_evaluation_delimiter(document: &ProjectDocument, node: &crate::NotationNode) -> bool {
    node.kind == NotationNodeKind::Delimiter
        && node.name.as_deref() == Some("right")
        && node.children.iter().any(|child| {
            document
                .nodes
                .get(*child as usize)
                .and_then(|child| child.text.as_deref())
                .is_some_and(|text| matches!(text, "." | "|"))
        })
}

fn emit_sized_delimiter(
    document: &ProjectDocument,
    node: &crate::NotationNode,
    tokens: &mut Vec<Token>,
) {
    let Some(text) = node.children.iter().find_map(|child| {
        document
            .nodes
            .get(*child as usize)
            .and_then(|child| child.text.as_deref())
    }) else {
        return;
    };
    let kind = match (node.name.as_deref(), text) {
        (_, "." | "|") => return,
        (Some("left"), "(") => TokenKind::Open {
            delimiter: '(',
            source_group: false,
        },
        (Some("left"), "[") => TokenKind::Open {
            delimiter: '[',
            source_group: false,
        },
        (Some("left"), "{") => TokenKind::Open {
            delimiter: '{',
            source_group: false,
        },
        (Some("right"), ")") => TokenKind::Close(')'),
        (Some("right"), "]") => TokenKind::Close(']'),
        (Some("right"), "}") => TokenKind::Close('}'),
        _ => return,
    };
    tokens.push(Token {
        kind,
        range: node.ranges.full.clone(),
        provenance: syntax_provenance(node),
    });
}

pub(crate) fn is_ignorable_command(name: Option<&str>) -> bool {
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
                | "big"
                | "Big"
                | "bigl"
                | "bigr"
                | "Bigl"
                | "Bigr"
                | "displaystyle"
                | "textstyle"
                | "scriptstyle"
                | "scriptscriptstyle"
                | "rm"
                | "label"
                | "tag"
                | "tag*"
                | "notag"
                | "nonumber"
        )
    )
}

fn is_presentation_command(name: Option<&str>) -> bool {
    matches!(
        name,
        Some("displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle")
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
    let mut input = tokens.into_iter().peekable();
    while let Some(mut token) = input.next() {
        if let TokenKind::Number(number) = &mut token.kind
            && let Some(decimal) = input.peek()
            && matches!(decimal.kind, TokenKind::Operator('.'))
            && token.range.end_offset == decimal.range.start_offset
            && token.provenance == decimal.provenance
        {
            let decimal = input.next().expect("peeked decimal token");
            if let Some(fraction) = input.peek()
                && let TokenKind::Number(fractional_digits) = &fraction.kind
                && decimal.range.end_offset == fraction.range.start_offset
                && decimal.provenance == fraction.provenance
            {
                number.push('.');
                number.push_str(fractional_digits);
                let fraction = input.next().expect("peeked fractional digits");
                token.range.end_offset = fraction.range.end_offset;
            } else {
                output.push(token);
                output.push(decimal);
                continue;
            }
        }
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

fn fold_vertical_bar_groups(tokens: Vec<Token>) -> Vec<Token> {
    let mut output = Vec::with_capacity(tokens.len());
    let mut cursor = 0;
    while cursor < tokens.len() {
        if !is_vertical_bar(&tokens[cursor].kind)
            || !vertical_bar_can_open(output.last().map(|token: &Token| &token.kind))
        {
            output.push(tokens[cursor].clone());
            cursor += 1;
            continue;
        }
        let mut depth = 0_u32;
        let close = (cursor + 1..tokens.len()).find(|index| {
            match tokens[*index].kind {
                TokenKind::Open { .. } => depth += 1,
                TokenKind::Close(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
            depth == 0 && is_vertical_bar(&tokens[*index].kind)
        });
        let Some(close) = close.filter(|close| cursor + 1 < *close) else {
            output.push(tokens[cursor].clone());
            cursor += 1;
            continue;
        };
        let inner_tokens = tokens[cursor + 1..close].to_vec();
        if inner_tokens.iter().any(|token| {
            matches!(token.kind, TokenKind::Operator('=' | '<' | '>'))
                || matches!(&token.kind, TokenKind::Command(command) if is_relation_command(command))
        }) {
            output.push(tokens[cursor].clone());
            cursor += 1;
            continue;
        }
        let inner = Parser::new(canonical_tokens(inner_tokens)).parse_document();
        let open = &tokens[cursor];
        let close_token = &tokens[close];
        let mut provenance = open.provenance.clone();
        provenance.extend(inner.provenance.iter().cloned());
        provenance.extend(close_token.provenance.iter().cloned());
        provenance.sort_by_key(|range| (range.start_offset, range.end_offset));
        provenance.dedup();
        output.push(Token {
            kind: TokenKind::Structured(Box::new(SemanticExpr {
                kind: SemanticExprKind::Apply {
                    operator: SemanticReference::new(
                        "vertical-bars",
                        open.range.clone(),
                        open.provenance.clone(),
                    ),
                    arguments: vec![inner],
                },
                range: SourceRange {
                    start_offset: open.range.start_offset,
                    end_offset: close_token.range.end_offset,
                },
                provenance,
            })),
            range: SourceRange {
                start_offset: open.range.start_offset,
                end_offset: close_token.range.end_offset,
            },
            provenance: Vec::new(),
        });
        cursor = close + 1;
    }
    output
}

fn is_vertical_bar(token: &TokenKind) -> bool {
    matches!(token, TokenKind::Identifier(value) if value == "|")
        || matches!(token, TokenKind::Operator('|'))
}

fn vertical_bar_can_open(previous: Option<&TokenKind>) -> bool {
    previous.is_none_or(|token| {
        matches!(
            token,
            TokenKind::Open { .. }
                | TokenKind::Operator('=' | '+' | '-' | '*' | '/' | ',' | '<' | '>')
        ) || matches!(token, TokenKind::Command(command) if is_relation_command(command))
    })
}

fn canonical_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let mut tokens = fold_vertical_bar_groups(coalesce_numbers(tokens));
    while tokens.last().is_some_and(|token| {
        matches!(token.kind, TokenKind::Operator(',' | '.'))
            || matches!(&token.kind, TokenKind::Number(value) if value == ".")
    }) {
        tokens.pop();
    }
    tokens
}

pub(crate) fn lower_template(source: &str) -> SemanticExpr {
    let range = SourceRange {
        start_offset: 0,
        end_offset: source.encode_utf16().count() as u32,
    };
    Parser::new(canonical_tokens(tokenize(
        &[SurfaceChunk {
            text: source.into(),
            range,
            provenance: Vec::new(),
        }],
        true,
    )))
    .parse_document()
}

pub(crate) fn canonical_template(source: &str) -> String {
    render_canonical(&lower_template(source))
}

pub(crate) fn render_canonical(expression: &SemanticExpr) -> String {
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
        SemanticExprKind::Index { base, indices } => format!(
            "index({},{})",
            render_canonical(base),
            render_items(indices)
        ),
        SemanticExprKind::Condition { value, predicate } => {
            render_pair("condition", value, predicate)
        }
        SemanticExprKind::Binder {
            operator,
            variables,
            lower,
            upper,
            body,
        } => format!(
            "binder({operator},vars({}),lower({}),upper({}),{})",
            render_items(variables),
            lower.as_deref().map(render_canonical).unwrap_or_default(),
            upper.as_deref().map(render_canonical).unwrap_or_default(),
            render_canonical(body)
        ),
        SemanticExprKind::System(equations) => render_list("system", equations),
        SemanticExprKind::Piecewise(branches) => format!(
            "piecewise({})",
            branches
                .iter()
                .map(|branch| format!(
                    "branch({},{})",
                    render_canonical(&branch.value),
                    branch
                        .condition
                        .as_ref()
                        .map(render_canonical)
                        .unwrap_or_default()
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
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
    if let Some(declaration) = declaration_head(&expression) {
        return vec![declaration];
    }
    if let SemanticExprKind::Relation { operator, left, .. } = &expression.kind
        && matches!(operator.as_str(), "member-of" | "not-member-of")
    {
        return match &left.kind {
            SemanticExprKind::Symbol(name) => vec![(name.clone(), left.range.clone())],
            SemanticExprKind::Index { .. } => expression_name(left)
                .map(|name| vec![(name, left.range.clone())])
                .unwrap_or_default(),
            _ => Vec::new(),
        };
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
            SemanticExprKind::Index { .. } => {
                let name = expression_name(&item)?;
                Some((name, item.range))
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn relation_head_symbol(
    document: &ProjectDocument,
    range: &SourceRange,
) -> Option<(String, SourceRange)> {
    let expression = lower_document_region(document, range);
    relation_head(&expression)
}

pub(crate) fn relation_head(expression: &SemanticExpr) -> Option<(String, SourceRange)> {
    match &expression.kind {
        SemanticExprKind::Relation { left, .. } => declaration_head(left),
        SemanticExprKind::System(relations) => relations.first().and_then(relation_head),
        _ => None,
    }
}

fn declaration_head(expression: &SemanticExpr) -> Option<(String, SourceRange)> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) => Some((name.clone(), expression.range.clone())),
        SemanticExprKind::Index { .. } => {
            Some((expression_name(expression)?, expression.range.clone()))
        }
        SemanticExprKind::Apply { operator, .. } => {
            Some((operator.value.clone(), operator.range.clone()))
        }
        SemanticExprKind::Derivative { expression, .. } => declaration_head(expression),
        _ => None,
    }
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
                if matches!(name, "label" | "tag") {
                    if name == "tag" && chunk.text[cursor..].starts_with('*') {
                        cursor += 1;
                    }
                    cursor = skip_braced_argument(&chunk.text, cursor);
                    continue;
                }
                if matches!(name, "notag" | "nonumber") {
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
                if is_presentation_command(Some(name)) {
                    tokens.push(Token {
                        kind: TokenKind::Presentation,
                        range: token_range(chunk, start, cursor),
                        provenance: chunk.provenance.clone(),
                    });
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
                    '{' | '(' | '[' => TokenKind::Open {
                        delimiter: character,
                        source_group: character == '{',
                    },
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

    fn parse_document(mut self) -> SemanticExpr {
        let leading_presentation = self
            .tokens
            .iter()
            .take_while(|token| matches!(token.kind, TokenKind::Presentation))
            .count();
        if leading_presentation > 0 {
            let prefix = self.tokens[0].clone();
            self.tokens.drain(..leading_presentation);
            let mut expression = self.parse_document();
            expression.range = merge_range(&prefix.range, &expression.range);
            expression.provenance.extend(prefix.provenance);
            expression
                .provenance
                .sort_by_key(|range| (range.start_offset, range.end_offset));
            expression.provenance.dedup();
            return expression;
        }
        let mut depth = 0_u32;
        let mut separators = Vec::new();
        for (index, token) in self.tokens.iter().enumerate() {
            match token.kind {
                TokenKind::Open { .. } => depth += 1,
                TokenKind::Close(_) => depth = depth.saturating_sub(1),
                TokenKind::Operator(',') if depth == 0 => separators.push(index),
                _ => {}
            }
        }
        let mut bounds = separators
            .iter()
            .copied()
            .chain(std::iter::once(self.tokens.len()));
        let mut start = 0_usize;
        let relation_segments = bounds
            .by_ref()
            .filter(|end| {
                let segment_start = start;
                start = *end + 1;
                self.tokens[segment_start..*end].iter().any(|token| {
                    matches!(token.kind, TokenKind::Operator('=' | '<' | '>'))
                        || matches!(&token.kind, TokenKind::Command(command) if is_relation_command(command))
                })
            })
            .count();
        if relation_segments < 2 {
            return self.parse_relation();
        }

        let mut expressions = Vec::new();
        let mut start = 0;
        for end in separators
            .into_iter()
            .chain(std::iter::once(self.tokens.len()))
        {
            if start < end {
                expressions.push(Parser::new(self.tokens[start..end].to_vec()).parse_relation());
            }
            start = end + 1;
        }
        let first = expressions.first().expect("multiple relation segments");
        let last = expressions.last().expect("multiple relation segments");
        SemanticExpr {
            range: merge_range(&first.range, &last.range),
            provenance: expressions
                .iter()
                .flat_map(|expression| expression.provenance.clone())
                .collect(),
            kind: SemanticExprKind::System(expressions),
        }
    }

    fn parse_relation(&mut self) -> SemanticExpr {
        let mut expression = self.parse_logical();
        while let Some(operator) = self.consume_statement_relation() {
            let right = self.parse_logical();
            expression = combined(
                &expression,
                &right,
                SemanticExprKind::Relation {
                    operator,
                    left: Box::new(expression.clone()),
                    right: Box::new(right.clone()),
                },
            );
        }
        expression
    }

    fn parse_logical(&mut self) -> SemanticExpr {
        let mut expression = self.parse_comparison();
        while let Some(operator) = self.consume_logical_connective() {
            let right = self.parse_comparison();
            expression = combined(
                &expression,
                &right,
                SemanticExprKind::Apply {
                    operator,
                    arguments: vec![expression.clone(), right.clone()],
                },
            );
        }
        expression
    }

    fn parse_comparison(&mut self) -> SemanticExpr {
        let mut operands = vec![self.parse_set_operation()];
        let mut operators = Vec::new();
        while let Some(operator) = self.consume_comparison() {
            operators.push(operator);
            operands.push(self.parse_set_operation());
        }
        if operators.is_empty() {
            return operands.pop().expect("one relation operand");
        }

        let common_equality_left = operators.iter().all(|operator| operator == "equals");
        let mut relations = operators
            .into_iter()
            .enumerate()
            .map(|(index, operator)| {
                let left = &operands[if common_equality_left { 0 } else { index }];
                let right = &operands[index + 1];
                combined(
                    left,
                    right,
                    SemanticExprKind::Relation {
                        operator,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    },
                )
            })
            .collect::<Vec<_>>();
        if relations.len() == 1 {
            return relations.pop().expect("one relation");
        }
        let first = relations.first().expect("chained relations");
        let last = relations.last().expect("chained relations");
        SemanticExpr {
            range: merge_range(&first.range, &last.range),
            provenance: relations
                .iter()
                .flat_map(|relation| relation.provenance.clone())
                .collect(),
            kind: SemanticExprKind::System(relations),
        }
    }

    fn parse_set_operation(&mut self) -> SemanticExpr {
        let mut expression = self.parse_sum();
        loop {
            let operator = if self.consume_command("cap") {
                self.previous_reference("intersection")
            } else if self.consume_command("cup") {
                self.previous_reference("union")
            } else if self.consume_command("circ") {
                self.previous_reference("compose")
            } else {
                break;
            };
            let right = self.parse_sum();
            expression = combined(
                &expression,
                &right,
                SemanticExprKind::Apply {
                    operator,
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
                terms.push(negate_term(term));
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
                if let (Some(differential), Some(variable)) = (
                    factors.last().and_then(split_compact_differential),
                    split_compact_differential(&denominator),
                ) {
                    let numerator = factors.pop().expect("compact differential numerator");
                    factors.push(SemanticExpr {
                        range: merge_range(&numerator.range, &denominator.range),
                        provenance: merge_provenance(&numerator, &denominator),
                        kind: SemanticExprKind::Derivative {
                            expression: Box::new(SemanticExpr {
                                kind: SemanticExprKind::Symbol(differential.0),
                                range: differential.1,
                                provenance: numerator.provenance,
                            }),
                            variable: SemanticReference::new(
                                variable.0,
                                variable.1,
                                denominator.provenance,
                            ),
                            order: 1,
                        },
                    });
                    continue;
                }
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
                                variable: SemanticReference::from_expression(
                                    variable,
                                    &variable_expression,
                                ),
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
            if self.consume_operator('*') {
                factors.push(self.parse_power());
                continue;
            }
            if is_argument_group(self.peek()) {
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
            if expression_name(&factors[index]).as_deref() == Some("D_t") {
                let operator = factors.remove(index);
                let expression = factors.remove(index);
                let variable = indexed_operator_variable(&operator, "t")
                    .unwrap_or_else(|| SemanticReference::from_expression("t", &operator));
                factors.insert(
                    index,
                    SemanticExpr {
                        range: merge_range(&operator.range, &expression.range),
                        provenance: merge_provenance(&operator, &expression),
                        kind: SemanticExprKind::Derivative {
                            expression: Box::new(expression),
                            variable,
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
                    variable: SemanticReference::from_expression(variable, &operator),
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
                expression = apply_subscript(expression, &subscript);
            } else if self.consume_operator('^') {
                self.split_unbraced_transpose_prefix();
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
                            operator: SemanticReference::from_expression("transpose", &exponent),
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
                let prime = &self.tokens[self.cursor - 1];
                let variable = SemanticReference::from_expression("t", &expression);
                let mut provenance = expression.provenance.clone();
                provenance.extend(prime.provenance.clone());
                provenance.sort_by_key(|range| (range.start_offset, range.end_offset));
                provenance.dedup();
                expression = SemanticExpr {
                    range: merge_range(&expression.range, &prime.range),
                    provenance,
                    kind: SemanticExprKind::Derivative {
                        expression: Box::new(expression),
                        variable,
                        order: 1,
                    },
                };
            } else if is_argument_group(self.peek()) && is_callable_expression(&expression) {
                let argument = self.parse_prefix();
                expression = apply_argument(expression.clone(), argument)
                    .expect("callable postfix was checked before consuming its argument");
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
                "mathbf" | "mathrm" | "mathit" | "mathbb" | "mathcal" | "mathsf" | "boldsymbol"
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
                        provenance: [token.provenance.clone(), expression.provenance.clone()]
                            .concat(),
                        kind: SemanticExprKind::Derivative {
                            expression: Box::new(expression),
                            variable: SemanticReference::new(
                                "t",
                                token.range.clone(),
                                token.provenance.clone(),
                            ),
                            order: u8::from(command == "ddot") + 1,
                        },
                    };
                }
                "sum" => {
                    let token = self.next();
                    return SemanticExpr {
                        kind: SemanticExprKind::Apply {
                            operator: SemanticReference::new(
                                "sum",
                                token.range.clone(),
                                token.provenance.clone(),
                            ),
                            arguments: Vec::new(),
                        },
                        range: token.range,
                        provenance: token.provenance,
                    };
                }
                "int" | "iint" | "iiint" => return self.parse_integral(),
                "nabla" => {
                    let token = self.next();
                    let operator = if self.consume_command("cdot") {
                        "divergence"
                    } else if self.consume_command("times") {
                        "curl"
                    } else if self.consume_operator('^') {
                        let order = self.parse_group_or_atom();
                        if matches!(&order.kind, SemanticExprKind::Number(value) if value == "2") {
                            "laplacian"
                        } else {
                            "nabla-power"
                        }
                    } else {
                        "gradient"
                    };
                    let mut argument = self.parse_power();
                    if matches!(self.peek(), TokenKind::Open { delimiter: '(', .. }) {
                        let application_argument = self.parse_power();
                        if let Some(applied) =
                            apply_argument(argument.clone(), application_argument)
                        {
                            argument = applied;
                        }
                    }
                    return SemanticExpr {
                        range: merge_range(&token.range, &argument.range),
                        provenance: [token.provenance.clone(), argument.provenance.clone()]
                            .concat(),
                        kind: SemanticExprKind::Apply {
                            operator: SemanticReference::new(
                                operator,
                                token.range.clone(),
                                token.provenance.clone(),
                            ),
                            arguments: vec![argument],
                        },
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
                        provenance: [command.provenance.clone(), expression.provenance.clone()]
                            .concat(),
                        kind: SemanticExprKind::Apply {
                            operator: SemanticReference::new(
                                "norm",
                                command.range.clone(),
                                command.provenance.clone(),
                            ),
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
        if let Some(derivative) = derivative_parts(&numerator, &denominator) {
            let range = merge_range(&command.range, &denominator.range);
            let provenance = [
                command.provenance,
                numerator.provenance.clone(),
                denominator.provenance.clone(),
            ]
            .concat();
            return match derivative {
                ParsedDerivative::Total {
                    expression,
                    variable,
                    order,
                } => SemanticExpr {
                    range,
                    provenance,
                    kind: SemanticExprKind::Derivative {
                        expression: Box::new(expression),
                        variable,
                        order,
                    },
                },
                ParsedDerivative::Partial {
                    expression,
                    variables,
                    order,
                } => {
                    let mut arguments = vec![expression];
                    arguments.extend(variables.into_iter().map(|variable| SemanticExpr {
                        kind: SemanticExprKind::Symbol(variable.value),
                        range: variable.range,
                        provenance: variable.provenance,
                    }));
                    arguments.push(SemanticExpr {
                        kind: SemanticExprKind::Number(order.to_string()),
                        range: numerator.range.clone(),
                        provenance: numerator.provenance.clone(),
                    });
                    SemanticExpr {
                        range,
                        provenance,
                        kind: SemanticExprKind::Apply {
                            operator: SemanticReference::from_expression(
                                "partial-derivative",
                                &numerator,
                            ),
                            arguments,
                        },
                    }
                }
            };
        }
        combined(
            &numerator,
            &denominator,
            SemanticExprKind::Fraction(Box::new(numerator.clone()), Box::new(denominator.clone())),
        )
    }

    fn parse_integral(&mut self) -> SemanticExpr {
        let command = self.next();
        let mut lower = None;
        let mut upper = None;
        loop {
            if self.consume_operator('_') {
                lower = Some(self.parse_group_or_atom());
            } else if self.consume_operator('^') {
                upper = Some(self.parse_group_or_atom());
            } else {
                break;
            }
        }
        let start = self.cursor;
        let differential = (start..self.tokens.len().saturating_sub(1))
            .rev()
            .find(|index| {
                token_name(&self.tokens[*index].kind) == Some("d")
                    && token_name(&self.tokens[*index + 1].kind).is_some()
                    && self.tokens.get(*index + 2).is_none_or(|token| {
                        !starts_atom(&token.kind)
                            || matches!(token.kind, TokenKind::Command(ref name) if is_relation_command(name))
                    })
            });
        let Some(differential) = differential else {
            return SemanticExpr {
                kind: SemanticExprKind::Unknown("incomplete-integral".into()),
                range: command.range,
                provenance: command.provenance,
            };
        };
        if differential == start {
            return SemanticExpr {
                kind: SemanticExprKind::Unknown("missing-integrand".into()),
                range: command.range,
                provenance: command.provenance,
            };
        }
        let integrand = Parser::new(self.tokens[start..differential].to_vec()).parse_relation();
        let variable_token = self.tokens[differential + 1].clone();
        let variable = SemanticExpr {
            kind: SemanticExprKind::Symbol(
                token_name(&variable_token.kind)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            range: variable_token.range.clone(),
            provenance: variable_token.provenance.clone(),
        };
        self.cursor = differential + 2;
        SemanticExpr {
            range: merge_range(&command.range, &variable_token.range),
            provenance: command.provenance.clone(),
            kind: SemanticExprKind::Binder {
                operator: SemanticReference::new(
                    "integral",
                    command.range.clone(),
                    command.provenance.clone(),
                ),
                variables: vec![variable],
                lower: lower.map(Box::new),
                upper: upper.map(Box::new),
                body: Box::new(integrand),
            },
        }
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
            TokenKind::Structured(expression) => *expression,
            TokenKind::Presentation => SemanticExpr {
                kind: SemanticExprKind::Unknown("presentation".into()),
                range: token.range,
                provenance: token.provenance,
            },
            TokenKind::Open {
                delimiter: open,
                source_group,
            } => {
                let mut expression = self.parse_relation();
                let expected = match open {
                    '{' => '}',
                    '(' => ')',
                    '[' => ']',
                    _ => unreachable!(),
                };
                if self.consume_close(expected) && source_group {
                    let close = &self.tokens[self.cursor - 1];
                    expression.range = merge_range(&token.range, &close.range);
                    expression.provenance.extend(token.provenance);
                    expression
                        .provenance
                        .extend(close.provenance.iter().cloned());
                    expression
                        .provenance
                        .sort_by_key(|range| (range.start_offset, range.end_offset));
                    expression.provenance.dedup();
                }
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

    fn consume_statement_relation(&mut self) -> Option<SemanticReference> {
        if self.consume_command("iff")
            || self.consume_command("Leftrightarrow")
            || self.consume_command("Longleftrightarrow")
        {
            Some(self.previous_reference("equivalent-to"))
        } else if self.consume_command("implies")
            || self.consume_command("Rightarrow")
            || self.consume_command("Longrightarrow")
        {
            Some(self.previous_reference("implies"))
        } else {
            None
        }
    }

    fn consume_logical_connective(&mut self) -> Option<SemanticReference> {
        if self.consume_command("land") || self.consume_command("wedge") {
            Some(self.previous_reference("and"))
        } else if self.consume_command("lor") || self.consume_command("vee") {
            Some(self.previous_reference("or"))
        } else {
            None
        }
    }

    fn consume_comparison(&mut self) -> Option<SemanticReference> {
        if self.consume_operator(':') {
            if self.consume_operator('=') {
                Some(self.previous_reference("equals"))
            } else {
                self.cursor -= 1;
                None
            }
        } else if self.consume_operator('=')
            || self.consume_command("coloneqq")
            || self.consume_command("triangleq")
        {
            Some(self.previous_reference("equals"))
        } else if self.consume_operator('<') {
            Some(self.previous_reference("less-than"))
        } else if self.consume_operator('>') {
            Some(self.previous_reference("greater-than"))
        } else if self.consume_command("le") || self.consume_command("leq") {
            Some(self.previous_reference("less-or-equal"))
        } else if self.consume_command("ge") || self.consume_command("geq") {
            Some(self.previous_reference("greater-or-equal"))
        } else if self.consume_command("ne") || self.consume_command("neq") {
            Some(self.previous_reference("not-equals"))
        } else if self.consume_command("in") {
            Some(self.previous_reference("member-of"))
        } else if self.consume_command("notin") {
            Some(self.previous_reference("not-member-of"))
        } else if self.consume_command("subset") {
            Some(self.previous_reference("proper-subset-of"))
        } else if self.consume_command("subseteq") {
            Some(self.previous_reference("subset-of"))
        } else if self.consume_command("supset") {
            Some(self.previous_reference("proper-superset-of"))
        } else if self.consume_command("supseteq") {
            Some(self.previous_reference("superset-of"))
        } else {
            None
        }
    }

    fn previous_reference(&self, value: &str) -> SemanticReference {
        let token = &self.tokens[self.cursor - 1];
        SemanticReference::new(value, token.range.clone(), token.provenance.clone())
    }

    fn consume_operator(&mut self, expected: char) -> bool {
        if matches!(self.peek(), TokenKind::Operator(actual) if *actual == expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn split_unbraced_transpose_prefix(&mut self) {
        let Some(token) = self.tokens.get(self.cursor).cloned() else {
            return;
        };
        let TokenKind::Identifier(value) = token.kind else {
            return;
        };
        let Some(rest) = value.strip_prefix('T').filter(|rest| !rest.is_empty()) else {
            return;
        };
        let split = token.range.start_offset + 1;
        self.tokens[self.cursor] = Token {
            kind: TokenKind::Identifier("T".into()),
            range: SourceRange {
                start_offset: token.range.start_offset,
                end_offset: split,
            },
            provenance: token.provenance.clone(),
        };
        self.tokens.insert(
            self.cursor + 1,
            Token {
                kind: TokenKind::Identifier(rest.into()),
                range: SourceRange {
                    start_offset: split,
                    end_offset: token.range.end_offset,
                },
                provenance: token.provenance,
            },
        );
    }

    fn consume_close(&mut self, expected: char) -> bool {
        if matches!(self.peek(), TokenKind::Close(actual) if *actual == expected) {
            self.cursor += 1;
            true
        } else {
            false
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

fn negate_term(mut expression: SemanticExpr) -> SemanticExpr {
    if let SemanticExprKind::Product(factors) = &mut expression.kind
        && let Some(first) = factors.first_mut()
    {
        let negated = SemanticExpr {
            range: first.range.clone(),
            provenance: first.provenance.clone(),
            kind: SemanticExprKind::Negate(Box::new(first.clone())),
        };
        *first = negated;
        return expression;
    }
    SemanticExpr {
        range: expression.range.clone(),
        provenance: expression.provenance.clone(),
        kind: SemanticExprKind::Negate(Box::new(expression)),
    }
}

fn apply_subscript(expression: SemanticExpr, subscript: &SemanticExpr) -> SemanticExpr {
    match expression.kind.clone() {
        SemanticExprKind::Symbol(_) => combined(
            &expression,
            subscript,
            SemanticExprKind::Index {
                base: Box::new(expression.clone()),
                indices: split_arguments(subscript.clone()),
            },
        ),
        SemanticExprKind::Index { base, mut indices } => {
            indices.extend(split_arguments(subscript.clone()));
            combined(
                &expression,
                subscript,
                SemanticExprKind::Index { base, indices },
            )
        }
        SemanticExprKind::Negate(inner) => {
            let inner = apply_subscript(*inner, subscript);
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
            let inner = apply_subscript(*inner, subscript);
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

fn indexed_operator_variable(
    expression: &SemanticExpr,
    expected: &str,
) -> Option<SemanticReference> {
    let SemanticExprKind::Index { indices, .. } = &expression.kind else {
        return None;
    };
    let index = indices
        .iter()
        .find(|index| expression_name(index).as_deref() == Some(expected))?;
    Some(SemanticReference::from_expression(expected, index))
}

fn apply_argument(expression: SemanticExpr, argument: SemanticExpr) -> Option<SemanticExpr> {
    match expression.kind.clone() {
        SemanticExprKind::Symbol(operator) => Some(SemanticExpr {
            range: merge_range(&expression.range, &argument.range),
            provenance: merge_provenance(&expression, &argument),
            kind: SemanticExprKind::Apply {
                operator: SemanticReference::from_expression(operator, &expression),
                arguments: split_arguments(argument),
            },
        }),
        SemanticExprKind::Index { .. } => Some(SemanticExpr {
            range: merge_range(&expression.range, &argument.range),
            provenance: merge_provenance(&expression, &argument),
            kind: SemanticExprKind::Apply {
                operator: SemanticReference::from_expression(
                    expression_name(&expression)?,
                    &expression,
                ),
                arguments: split_arguments(argument),
            },
        }),
        SemanticExprKind::Derivative {
            expression: inner,
            mut variable,
            order,
        } => {
            let implicit_variable = variable.range == inner.range;
            if implicit_variable
                && let [argument] = split_arguments(argument.clone()).as_slice()
                && let Some(name) = expression_name(argument)
            {
                variable = SemanticReference::from_expression(name, argument);
            }
            Some(SemanticExpr {
                range: merge_range(&expression.range, &argument.range),
                provenance: merge_provenance(&expression, &argument),
                kind: SemanticExprKind::Derivative {
                    expression: inner,
                    variable,
                    order,
                },
            })
        }
        _ => None,
    }
}

fn is_callable_expression(expression: &SemanticExpr) -> bool {
    match &expression.kind {
        SemanticExprKind::Symbol(_) => true,
        SemanticExprKind::Derivative { expression, .. } => matches!(
            expression.kind,
            SemanticExprKind::Symbol(_) | SemanticExprKind::Index { .. }
        ),
        _ => false,
    }
}

fn is_argument_group(token: &TokenKind) -> bool {
    matches!(
        token,
        TokenKind::Open {
            delimiter: '(' | '[',
            ..
        }
    )
}

fn split_arguments(argument: SemanticExpr) -> Vec<SemanticExpr> {
    let SemanticExprKind::Product(items) = &argument.kind else {
        return vec![argument];
    };
    if let Some(separator) = items
        .iter()
        .position(|item| matches!(symbol_name(item), Some("mid")))
        && separator > 0
        && separator + 1 < items.len()
        && !items[separator + 1..]
            .iter()
            .any(|item| matches!(symbol_name(item), Some("mid")))
    {
        let left = associative(items[..separator].to_vec(), SemanticExprKind::Product);
        let right = associative(items[separator + 1..].to_vec(), SemanticExprKind::Product);
        return vec![combined(
            &left,
            &right,
            SemanticExprKind::Condition {
                value: Box::new(left.clone()),
                predicate: Box::new(right.clone()),
            },
        )];
    }
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
        TokenKind::Command(command) => {
            !is_relation_command(command)
                && !matches!(
                    command.as_str(),
                    "cap"
                        | "cdot"
                        | "circ"
                        | "cup"
                        | "land"
                        | "lor"
                        | "right"
                        | "times"
                        | "vee"
                        | "wedge"
                )
        }
        TokenKind::Identifier(_)
        | TokenKind::Number(_)
        | TokenKind::Open { .. }
        | TokenKind::Structured(_) => true,
        TokenKind::Operator(',') => true,
        _ => false,
    }
}

fn token_name(token: &TokenKind) -> Option<&str> {
    match token {
        TokenKind::Identifier(name) | TokenKind::Command(name) => Some(name),
        _ => None,
    }
}

fn is_relation_command(name: &str) -> bool {
    matches!(
        name,
        "coloneqq"
            | "iff"
            | "implies"
            | "Leftrightarrow"
            | "Longleftrightarrow"
            | "Longrightarrow"
            | "Rightarrow"
            | "ge"
            | "geq"
            | "in"
            | "le"
            | "leq"
            | "ne"
            | "neq"
            | "notin"
            | "subset"
            | "subseteq"
            | "supset"
            | "supseteq"
            | "triangleq"
    )
}

enum ParsedDerivative {
    Total {
        expression: SemanticExpr,
        variable: SemanticReference,
        order: u8,
    },
    Partial {
        expression: SemanticExpr,
        variables: Vec<SemanticReference>,
        order: u8,
    },
}

fn derivative_parts(
    numerator: &SemanticExpr,
    denominator: &SemanticExpr,
) -> Option<ParsedDerivative> {
    let SemanticExprKind::Product(numerator_factors) = &numerator.kind else {
        return None;
    };
    let SemanticExprKind::Product(denominator_factors) = &denominator.kind else {
        return None;
    };
    let (operator, numerator_order) = differential_order(numerator_factors.first()?)?;
    let expression = numerator_factors.get(1)?.clone();
    if operator == "d" {
        let (denominator_operator, _) = differential_order(denominator_factors.first()?)?;
        if denominator_operator != "d" || denominator_factors.len() != 2 {
            return None;
        }
        let (variable, variable_order) = variable_order(&denominator_factors[1])?;
        if numerator_order != variable_order {
            return None;
        }
        return Some(ParsedDerivative::Total {
            expression,
            variable,
            order: numerator_order,
        });
    }
    if operator != "partial" {
        return None;
    }
    let mut variables = Vec::new();
    let mut denominator_order = 0_u8;
    let mut cursor = 0;
    while cursor + 1 < denominator_factors.len() {
        let (denominator_operator, operator_order) =
            differential_order(&denominator_factors[cursor])?;
        if denominator_operator != "partial" || operator_order != 1 {
            return None;
        }
        let (variable, order) = variable_order(&denominator_factors[cursor + 1])?;
        denominator_order = denominator_order.checked_add(order)?;
        variables.extend(std::iter::repeat_n(variable, order as usize));
        cursor += 2;
    }
    (cursor == denominator_factors.len() && numerator_order == denominator_order).then_some(
        ParsedDerivative::Partial {
            expression,
            variables,
            order: numerator_order,
        },
    )
}

fn split_compact_differential(expression: &SemanticExpr) -> Option<(String, SourceRange)> {
    let SemanticExprKind::Symbol(symbol) = &expression.kind else {
        return None;
    };
    let mut characters = symbol.chars();
    (characters.next()? == 'd')
        .then_some(())
        .filter(|_| characters.clone().count() == 1)?;
    let value = characters.next()?.to_string();
    Some((
        value,
        SourceRange {
            start_offset: expression.range.start_offset + 1,
            end_offset: expression.range.end_offset,
        },
    ))
}

fn differential_order(expression: &SemanticExpr) -> Option<(&str, u8)> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) if matches!(name.as_str(), "d" | "partial") => {
            Some((name, 1))
        }
        SemanticExprKind::Power(base, exponent) => {
            let name = symbol_name(base)?;
            matches!(name, "d" | "partial")
                .then(|| number_order(exponent).map(|order| (name, order)))?
        }
        _ => None,
    }
}

fn variable_order(expression: &SemanticExpr) -> Option<(SemanticReference, u8)> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) => Some((
            SemanticReference::from_expression(name.clone(), expression),
            1,
        )),
        SemanticExprKind::Power(base, exponent) => Some((
            SemanticReference::from_expression(symbol_name(base)?, base),
            number_order(exponent)?,
        )),
        _ => None,
    }
}

fn number_order(expression: &SemanticExpr) -> Option<u8> {
    let SemanticExprKind::Number(value) = &expression.kind else {
        return None;
    };
    value.parse::<u8>().ok().filter(|order| *order > 0)
}

fn symbol_name(expression: &SemanticExpr) -> Option<&str> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) => Some(name),
        _ => None,
    }
}

pub(crate) fn expression_name(expression: &SemanticExpr) -> Option<String> {
    match &expression.kind {
        SemanticExprKind::Symbol(name) | SemanticExprKind::Number(name) => Some(name.clone()),
        SemanticExprKind::Negate(inner) => Some(format!("-{}", expression_name(inner)?)),
        SemanticExprKind::Sum(items) => items
            .iter()
            .map(expression_name)
            .collect::<Option<Vec<_>>>()
            .map(|items| items.join("+").replace("+-", "-")),
        SemanticExprKind::Product(items) => items
            .iter()
            .map(expression_name)
            .collect::<Option<Vec<_>>>()
            .map(|items| items.concat()),
        SemanticExprKind::Power(base, exponent) => Some(format!(
            "{}^{}",
            expression_name(base)?,
            expression_name(exponent)?
        )),
        SemanticExprKind::Index { base, indices } => Some(format!(
            "{}_{}",
            expression_name(base)?,
            indices
                .iter()
                .map(expression_name)
                .collect::<Option<Vec<_>>>()?
                .join(",")
        )),
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
    use super::{
        SemanticExprKind, TokenKind, declared_symbols, lower_document_region, lower_template,
        render_canonical, semantic_token_kind,
    };
    use crate::{LexicalClass, ProjectDocument, SourceRange};

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
    fn lowers_comma_separated_relations_as_one_structural_system() {
        assert_eq!(
            render_canonical(&lower_template(
                "A \\in \\mathbb{R}^{m \\times n}, x \\in \\mathbb{R}^{k}, k \\ne n"
            )),
            "system(relation(member-of,symbol(A),power(symbol(R),cross(symbol(m),symbol(n)))),relation(member-of,symbol(x),power(symbol(R),symbol(k))),relation(not-equals,symbol(k),symbol(n)))"
        );
    }

    #[test]
    fn lowers_chained_relations_as_explicit_constraints() {
        assert_eq!(
            render_canonical(&lower_template("a=b=c")),
            "system(relation(equals,symbol(a),symbol(b)),relation(equals,symbol(a),symbol(c)))"
        );
        assert_eq!(
            render_canonical(&lower_template(r"a<b\leq c")),
            "system(relation(less-than,symbol(a),symbol(b)),relation(less-or-equal,symbol(b),symbol(c)))"
        );
    }

    #[test]
    fn pairs_expression_boundary_vertical_bars_without_consuming_set_builders() {
        assert_eq!(
            render_canonical(&lower_template("|A\\cup B|=|A|+|B|-|A\\cap B|")),
            "relation(equals,apply(vertical-bars,apply(union,symbol(A),symbol(B))),sum(apply(vertical-bars,symbol(A)),apply(vertical-bars,symbol(B)),negate(apply(vertical-bars,apply(intersection,symbol(A),symbol(B))))))"
        );
        assert!(!render_canonical(&lower_template("x|x>0")).contains("vertical-bars"));
        assert!(!render_canonical(&lower_template("|x>0")).contains("vertical-bars"));
    }

    #[test]
    fn lowers_definition_equality_to_the_shared_relation_ir() {
        let expected = render_canonical(&lower_template("g(x)=\\nabla f(x)"));
        assert_eq!(
            render_canonical(&lower_template("g(x):=\\nabla f(x)")),
            expected
        );
        assert_eq!(
            render_canonical(&lower_template("g(x)\\coloneqq\\nabla f(x)")),
            expected
        );
        assert_eq!(
            render_canonical(&lower_template("g(x)\\triangleq\\nabla f(x)")),
            expected
        );
    }

    #[test]
    fn source_ranges_preserve_tex_arguments_but_not_presentation_groups() {
        for source in ["m_{e10}", "A_c^{(1)}", r"\mathbf{F}_{p08}"] {
            let expression = lower_template(source);
            assert_eq!(expression.range.start_offset, 0, "{source}");
            assert_eq!(
                expression.range.end_offset,
                source.encode_utf16().count() as u32,
                "{source}"
            );
        }

        let grouped = lower_template("(v_{e10})");
        assert_eq!(grouped.range.start_offset, 1);
        assert_eq!(grouped.range.end_offset, 8);

        let styled = lower_template(r"\displaystyle x=y");
        assert!(matches!(styled.kind, SemanticExprKind::Relation { .. }));
        assert_eq!(styled.range.start_offset, 0);
        assert_eq!(styled.range.end_offset, 17);
    }

    #[test]
    fn lowers_engineering_operators_compositionally() {
        let state = lower_template("\\dot{x}=Ax+Bu");
        assert!(matches!(state.kind, SemanticExprKind::Relation { .. }));
        let lyapunov = lower_template("A^T P + P A = -Q");
        assert!(matches!(lyapunov.kind, SemanticExprKind::Relation { .. }));
        let capacitor = lower_template("i=C\\frac{d v}{d t}");
        assert!(matches!(capacitor.kind, SemanticExprKind::Relation { .. }));
        assert_eq!(
            render_canonical(&lower_template("F=m*a")),
            "relation(equals,symbol(F),product(symbol(m),symbol(a)))"
        );
    }

    #[test]
    fn normalizes_reviewed_lyapunov_notation() {
        assert_eq!(
            render_canonical(&lower_template(
                "(A_c^{(1)})^TP_c^{(1)}+P_c^{(1)}A_c^{(1)}=-Q_c^{(1)}"
            )),
            "relation(equals,sum(product(apply(transpose,index(symbol(A),symbol(c))),index(symbol(P),symbol(c))),product(index(symbol(P),symbol(c)),index(symbol(A),symbol(c)))),negate(index(symbol(Q),symbol(c))))",
        );
        assert_eq!(
            render_canonical(&lower_template(
                "\\underbrace{I_a+I_b}_{\\rm entering}=\\underbrace{I_c+I_d}_{\\rm leaving}"
            )),
            "relation(equals,sum(index(symbol(I),symbol(a)),index(symbol(I),symbol(b))),sum(index(symbol(I),symbol(c)),index(symbol(I),symbol(d))))",
        );
    }

    #[test]
    fn lowers_compact_total_differentials_in_quotients() {
        assert_eq!(
            render_canonical(&lower_template("i=-C dv/dt")),
            "relation(equals,symbol(i),product(negate(symbol(C)),derivative(symbol(v),t,1)))"
        );
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
    fn lowers_calculus_operators_with_explicit_variables_orders_and_bounds() {
        assert_eq!(
            render_canonical(&lower_template("\\int_0^1 g(t) \\, d t")),
            "binder(integral,vars(symbol(t)),lower(number(0)),upper(number(1)),apply(g,symbol(t)))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\frac{d^2 f}{d x^2}")),
            "derivative(symbol(f),x,2)"
        );
        assert_eq!(
            render_canonical(&lower_template(
                "\\frac{\\partial^2 f}{\\partial x \\partial y}"
            )),
            "apply(partial-derivative,symbol(f),symbol(x),symbol(y),number(2))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\nabla f(x)")),
            "apply(gradient,apply(f,symbol(x)))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\nabla \\cdot E")),
            "apply(divergence,symbol(E))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\nabla \\times E")),
            "apply(curl,symbol(E))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\nabla^2 u")),
            "apply(laplacian,symbol(u))"
        );
        assert_eq!(
            render_canonical(&lower_template("\\dot v_s(t)")),
            "derivative(index(symbol(v),symbol(s)),t,1)"
        );
        assert_eq!(
            render_canonical(&lower_template("y'(x)")),
            "derivative(symbol(y),x,1)"
        );
        assert_eq!(lower_template("v_m'").range.end_offset, 4);
        assert_eq!(
            render_canonical(&lower_template("\\frac{d y}{d x}(x)")),
            "derivative(symbol(y),x,1)"
        );
        let operator_derivative = lower_template("D_t v");
        assert_eq!(
            render_canonical(&operator_derivative),
            "derivative(symbol(v),t,1)"
        );
        let SemanticExprKind::Derivative { variable, .. } = operator_derivative.kind else {
            panic!("expected derivative")
        };
        assert_eq!(
            variable.range,
            SourceRange {
                start_offset: 2,
                end_offset: 3,
            }
        );
    }

    #[test]
    fn preserves_index_condition_and_reference_provenance_as_structure() {
        assert_eq!(
            render_canonical(&lower_template("x_{i,j}")),
            "index(symbol(x),symbol(i),symbol(j))"
        );
        assert_eq!(
            render_canonical(&lower_template("P(A \\mid B)")),
            "apply(P,condition(symbol(A),symbol(B)))"
        );
        let relation = lower_template("x=y");
        let SemanticExprKind::Relation { operator, .. } = relation.kind else {
            panic!("expected relation")
        };
        assert_eq!(
            operator.range,
            SourceRange {
                start_offset: 1,
                end_offset: 2
            }
        );
        assert!(operator.provenance.is_empty());
        assert!(matches!(
            lower_template("\\int").kind,
            SemanticExprKind::Unknown(ref reason) if reason == "incomplete-integral"
        ));
    }

    #[test]
    fn snapshot_declarations_keep_indexed_time_surfaces_distinct() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "$i_1(t)$", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":2,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":2}},"state":"complete","text":"i","lexicalClass":"identifier"},
                {"kind":"token","parent":2,"children":[],"ranges":{"full":{"startOffset":3,"endOffset":4}},"state":"complete","text":"1","lexicalClass":"number"},
                {"kind":"script","parent":5,"children":[0,1],"ranges":{"full":{"startOffset":1,"endOffset":4}},"state":"complete","name":"subscript"},
                {"kind":"token","parent":4,"children":[],"ranges":{"full":{"startOffset":5,"endOffset":6}},"state":"complete","text":"t","lexicalClass":"identifier"},
                {"kind":"delimiter","parent":5,"children":[3],"ranges":{"full":{"startOffset":4,"endOffset":7}},"state":"complete","name":"()"},
                {"kind":"sequence","parent":null,"children":[2,4],"ranges":{"full":{"startOffset":1,"endOffset":7}},"state":"complete"}
            ],
            "mathRoots": [{"node":5,"delimiter":"$","fullRange":{"startOffset":0,"endOffset":8},"contentRange":{"startOffset":1,"endOffset":7},"state":"complete"}],
            "visibleProse": [], "scopes": [], "blocks": [], "declarations": [], "mathRegions": [], "macros": [], "includes": []
        })).unwrap();

        assert_eq!(
            declared_symbols(
                &document,
                &SourceRange {
                    start_offset: 1,
                    end_offset: 7,
                },
            ),
            vec![(
                "i_1".into(),
                SourceRange {
                    start_offset: 1,
                    end_offset: 4,
                },
            )]
        );
    }

    #[test]
    fn snapshot_products_preserve_discrete_state_structure() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "r = a b + j p", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"r","lexicalClass":"identifier"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"a","lexicalClass":"identifier"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":6,"endOffset":7}},"state":"complete","text":"b","lexicalClass":"identifier"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":8,"endOffset":9}},"state":"complete","text":"+","lexicalClass":"operator"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":10,"endOffset":11}},"state":"complete","text":"j","lexicalClass":"identifier"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":12,"endOffset":13}},"state":"complete","text":"p","lexicalClass":"identifier"},
                {"kind":"sequence","parent":null,"children":[0,1,2,3,4,5,6],"ranges":{"full":{"startOffset":0,"endOffset":13}},"state":"complete"}
            ],
            "mathRoots": [{"node":7,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":13},"contentRange":{"startOffset":0,"endOffset":13},"state":"complete"}],
            "visibleProse": [], "scopes": [], "declarations": [], "macros": [], "includes": []
        })).unwrap();
        assert_eq!(
            render_canonical(&lower_document_region(
                &document,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 13,
                }
            )),
            "relation(equals,symbol(r),sum(product(symbol(a),symbol(b)),product(symbol(j),symbol(p))))"
        );
    }

    #[test]
    fn snapshot_lowering_treats_tex_math_class_wrappers_as_transparent() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "$P=F\\mathbin{\\cdot}v$", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":2}},"state":"complete","text":"P","lexicalClass":"identifier","mathClass":"ordinary"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"=","lexicalClass":"operator","mathClass":"relation"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":3,"endOffset":4}},"state":"complete","text":"F","lexicalClass":"identifier","mathClass":"ordinary"},
                {"kind":"command","parent":4,"children":[],"ranges":{"full":{"startOffset":13,"endOffset":18}},"state":"complete","name":"cdot","mathClass":"binary"},
                {"kind":"group","parent":5,"children":[3],"ranges":{"full":{"startOffset":12,"endOffset":19}},"state":"complete"},
                {"kind":"command","parent":7,"children":[4],"ranges":{"full":{"startOffset":4,"endOffset":19},"command":{"startOffset":4,"endOffset":12},"nucleus":{"startOffset":12,"endOffset":19}},"state":"complete","name":"mathbin","arguments":[{"node":4,"role":"nucleus","syntax":"required","range":{"startOffset":12,"endOffset":19}}],"mathClass":"binary"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":19,"endOffset":20}},"state":"complete","text":"v","lexicalClass":"identifier","mathClass":"ordinary"},
                {"kind":"sequence","parent":null,"children":[0,1,2,5,6],"ranges":{"full":{"startOffset":1,"endOffset":20}},"state":"complete"}
            ],
            "mathRoots": [{"node":7,"delimiter":"$","fullRange":{"startOffset":0,"endOffset":21},"contentRange":{"startOffset":1,"endOffset":20},"state":"complete"}],
            "visibleProse": [], "scopes": [], "declarations": [], "macros": [], "includes": []
        }))
        .unwrap();

        assert_eq!(
            render_canonical(&lower_document_region(
                &document,
                &SourceRange {
                    start_offset: 1,
                    end_offset: 20,
                }
            )),
            "relation(equals,symbol(P),dot(symbol(F),symbol(v)))"
        );
    }

    #[test]
    fn lowers_conditional_application_without_baking_in_probability() {
        assert_eq!(
            render_canonical(&lower_template("P(A \\mid B)")),
            "apply(P,condition(symbol(A),symbol(B)))"
        );
        assert_eq!(
            render_canonical(&lower_template("P(A) / P(B)")),
            "fraction(apply(P,symbol(A)),apply(P,symbol(B)))"
        );
        assert_eq!(
            render_canonical(&lower_template("G(s) = Y(s) / U(s)")),
            "relation(equals,apply(G,symbol(s)),fraction(apply(Y,symbol(s)),apply(U,symbol(s))))"
        );
    }

    #[test]
    fn lowers_square_bracket_function_application_without_naming_an_operator() {
        assert_eq!(
            render_canonical(&lower_template("\\mathbb E[X]")),
            "apply(E,symbol(X))"
        );
        assert_eq!(
            render_canonical(&lower_template("F[x,y]")),
            "apply(F,symbol(x),symbol(y))"
        );
    }

    #[test]
    fn logical_connectives_preserve_their_relation_operands() {
        assert_eq!(
            render_canonical(&lower_template(
                "x\\in A\\cup B\\iff (x\\in A)\\lor(x\\in B)"
            )),
            "relation(equivalent-to,relation(member-of,symbol(x),apply(union,symbol(A),symbol(B))),apply(or,relation(member-of,symbol(x),symbol(A)),relation(member-of,symbol(x),symbol(B))))"
        );
        assert_eq!(
            render_canonical(&lower_template("p\\Longrightarrow q\\land r")),
            "relation(implies,symbol(p),apply(and,symbol(q),symbol(r)))"
        );
    }

    #[test]
    fn equation_metadata_does_not_change_the_semantic_relation() {
        assert_eq!(
            semantic_token_kind(".", Some(LexicalClass::Other)),
            Some(TokenKind::Operator('.'))
        );
        assert_eq!(
            render_canonical(&lower_template("q=-k\\nabla T\\label{eq:flux}")),
            render_canonical(&lower_template("q=-k\\nabla T"))
        );
        assert_eq!(
            render_canonical(&lower_template("q=-k\\nabla T.\\label{eq:flux}")),
            render_canonical(&lower_template("q=-k\\nabla T"))
        );
        assert_eq!(
            render_canonical(&lower_template("x=0.5")),
            "relation(equals,symbol(x),number(0.5))"
        );
    }

    #[test]
    fn snapshot_lowering_preserves_delimiters_and_ignores_spacing_commands() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8,
            "proseAnnotations": [],
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "v(t)=R\\,i(t)",
            "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"v","lexicalClass":"identifier"},
                {"kind":"token","parent":2,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"t","lexicalClass":"identifier"},
                {"kind":"delimiter","parent":9,"children":[1],"ranges":{"full":{"startOffset":1,"endOffset":4}},"state":"complete","name":"()"},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":5,"endOffset":6}},"state":"complete","text":"R","lexicalClass":"identifier"},
                {"kind":"command","parent":9,"children":[],"ranges":{"full":{"startOffset":6,"endOffset":8}},"state":"complete","name":","},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":8,"endOffset":9}},"state":"complete","text":"i","lexicalClass":"identifier"},
                {"kind":"token","parent":8,"children":[],"ranges":{"full":{"startOffset":10,"endOffset":11}},"state":"complete","text":"t","lexicalClass":"identifier"},
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

    #[test]
    fn snapshot_lowering_keeps_decimal_relations_separate_across_spacing() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "P(A\\cap B)=0.012,\\qquad P(B)=0.08>0.", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"P","lexicalClass":"identifier"},
                {"kind":"token","parent":4,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"A","lexicalClass":"identifier"},
                {"kind":"command","parent":4,"children":[],"ranges":{"full":{"startOffset":3,"endOffset":7}},"state":"complete","name":"cap","mathClass":"binary"},
                {"kind":"token","parent":4,"children":[],"ranges":{"full":{"startOffset":8,"endOffset":9}},"state":"complete","text":"B","lexicalClass":"identifier"},
                {"kind":"delimiter","parent":24,"children":[1,2,3],"ranges":{"full":{"startOffset":1,"endOffset":10}},"state":"complete","name":"()"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":10,"endOffset":11}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":11,"endOffset":12}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":12,"endOffset":13}},"state":"complete","text":".","lexicalClass":"other"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":13,"endOffset":14}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":14,"endOffset":15}},"state":"complete","text":"1","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":15,"endOffset":16}},"state":"complete","text":"2","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":16,"endOffset":17}},"state":"complete","text":",","lexicalClass":"punctuation"},
                {"kind":"command","parent":24,"children":[],"ranges":{"full":{"startOffset":17,"endOffset":23}},"state":"complete","name":"qquad"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":24,"endOffset":25}},"state":"complete","text":"P","lexicalClass":"identifier"},
                {"kind":"token","parent":15,"children":[],"ranges":{"full":{"startOffset":26,"endOffset":27}},"state":"complete","text":"B","lexicalClass":"identifier"},
                {"kind":"delimiter","parent":24,"children":[14],"ranges":{"full":{"startOffset":25,"endOffset":28}},"state":"complete","name":"()"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":28,"endOffset":29}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":29,"endOffset":30}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":30,"endOffset":31}},"state":"complete","text":".","lexicalClass":"other"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":31,"endOffset":32}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":32,"endOffset":33}},"state":"complete","text":"8","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":33,"endOffset":34}},"state":"complete","text":">","lexicalClass":"operator"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":34,"endOffset":35}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"token","parent":24,"children":[],"ranges":{"full":{"startOffset":35,"endOffset":36}},"state":"complete","text":".","lexicalClass":"other"},
                {"kind":"sequence","parent":null,"children":[0,4,5,6,7,8,9,10,11,12,13,15,16,17,18,19,20,21,22,23],"ranges":{"full":{"startOffset":0,"endOffset":36}},"state":"complete"}
            ],
            "mathRoots": [{"node":24,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":36},"contentRange":{"startOffset":0,"endOffset":36},"state":"complete"}],
            "visibleProse": [], "scopes": [], "blocks": [], "declarations": [], "mathRegions": [], "macros": [], "includes": []
        }))
        .unwrap();

        assert_eq!(
            render_canonical(&lower_document_region(
                &document,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 36,
                },
            )),
            "system(relation(equals,apply(P,apply(intersection,symbol(A),symbol(B))),number(0.012)),system(relation(equals,apply(P,symbol(B)),number(0.08)),relation(greater-than,number(0.08),number(0))))"
        );
    }

    #[test]
    fn snapshot_lowering_preserves_piecewise_branches_and_aligned_systems() {
        let piecewise: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":1,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"x","lexicalClass":"identifier"},
                {"kind":"alignment","parent":6,"children":[0],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","name":"cell"},
                {"kind":"token","parent":5,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"x","lexicalClass":"identifier"},
                {"kind":"token","parent":5,"children":[],"ranges":{"full":{"startOffset":3,"endOffset":4}},"state":"complete","text":">","lexicalClass":"operator"},
                {"kind":"token","parent":5,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"alignment","parent":6,"children":[2,3,4],"ranges":{"full":{"startOffset":2,"endOffset":5}},"state":"complete","name":"cell"},
                {"kind":"alignment","parent":15,"children":[1,5],"ranges":{"full":{"startOffset":0,"endOffset":5}},"state":"complete","name":"row"},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":6,"endOffset":7}},"state":"complete","text":"-","lexicalClass":"operator"},
                {"kind":"token","parent":9,"children":[],"ranges":{"full":{"startOffset":7,"endOffset":8}},"state":"complete","text":"x","lexicalClass":"identifier"},
                {"kind":"alignment","parent":14,"children":[7,8],"ranges":{"full":{"startOffset":6,"endOffset":8}},"state":"complete","name":"cell"},
                {"kind":"token","parent":13,"children":[],"ranges":{"full":{"startOffset":8,"endOffset":9}},"state":"complete","text":"x","lexicalClass":"identifier"},
                {"kind":"token","parent":13,"children":[],"ranges":{"full":{"startOffset":9,"endOffset":10}},"state":"complete","text":"<","lexicalClass":"operator"},
                {"kind":"token","parent":13,"children":[],"ranges":{"full":{"startOffset":10,"endOffset":11}},"state":"complete","text":"0","lexicalClass":"number"},
                {"kind":"alignment","parent":14,"children":[10,11,12],"ranges":{"full":{"startOffset":8,"endOffset":11}},"state":"complete","name":"cell"},
                {"kind":"alignment","parent":15,"children":[9,13],"ranges":{"full":{"startOffset":6,"endOffset":11}},"state":"complete","name":"row"},
                {"kind":"environment","parent":16,"children":[6,14],"ranges":{"full":{"startOffset":0,"endOffset":11}},"state":"complete","name":"cases"},
                {"kind":"sequence","parent":null,"children":[15],"ranges":{"full":{"startOffset":0,"endOffset":11}},"state":"complete"}
            ],
            "mathRoots": [{"node":16,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":11},"contentRange":{"startOffset":0,"endOffset":11},"state":"complete"}],
            "visibleProse": [], "scopes": [], "declarations": [], "macros": [], "includes": []
        })).unwrap();
        assert_eq!(
            render_canonical(&lower_document_region(
                &piecewise,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 11
                }
            )),
            "piecewise(branch(symbol(x),relation(greater-than,symbol(x),number(0))),branch(negate(symbol(x)),relation(less-than,symbol(x),number(0))))"
        );

        let system: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"x","lexicalClass":"identifier"},
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":2}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"y","lexicalClass":"identifier"},
                {"kind":"alignment","parent":8,"children":[0,1,2],"ranges":{"full":{"startOffset":0,"endOffset":3}},"state":"complete","name":"row"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"y","lexicalClass":"identifier"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":5,"endOffset":6}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":7,"children":[],"ranges":{"full":{"startOffset":6,"endOffset":7}},"state":"complete","text":"z","lexicalClass":"identifier"},
                {"kind":"alignment","parent":8,"children":[4,5,6],"ranges":{"full":{"startOffset":4,"endOffset":7}},"state":"complete","name":"row"},
                {"kind":"environment","parent":9,"children":[3,7],"ranges":{"full":{"startOffset":0,"endOffset":7}},"state":"complete","name":"aligned"},
                {"kind":"sequence","parent":null,"children":[8],"ranges":{"full":{"startOffset":0,"endOffset":7}},"state":"complete"}
            ],
            "mathRoots": [{"node":9,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":7},"contentRange":{"startOffset":0,"endOffset":7},"state":"complete"}],
            "visibleProse": [], "scopes": [], "declarations": [], "macros": [], "includes": []
        })).unwrap();
        assert_eq!(
            render_canonical(&lower_document_region(
                &system,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 7
                }
            )),
            "system(relation(equals,symbol(x),symbol(y)),relation(equals,symbol(y),symbol(z)))"
        );

        let continued: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex",
            "language": "latex", "content": "", "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"a","lexicalClass":"identifier"},
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":2}},"state":"complete","text":"+","lexicalClass":"operator"},
                {"kind":"token","parent":3,"children":[],"ranges":{"full":{"startOffset":2,"endOffset":3}},"state":"complete","text":"b","lexicalClass":"identifier"},
                {"kind":"alignment","parent":7,"children":[0,1,2],"ranges":{"full":{"startOffset":0,"endOffset":3}},"state":"complete","name":"row"},
                {"kind":"token","parent":6,"children":[],"ranges":{"full":{"startOffset":4,"endOffset":5}},"state":"complete","text":"=","lexicalClass":"operator"},
                {"kind":"token","parent":6,"children":[],"ranges":{"full":{"startOffset":5,"endOffset":6}},"state":"complete","text":"c","lexicalClass":"identifier"},
                {"kind":"alignment","parent":7,"children":[4,5],"ranges":{"full":{"startOffset":4,"endOffset":6}},"state":"complete","name":"row"},
                {"kind":"environment","parent":8,"children":[3,6],"ranges":{"full":{"startOffset":0,"endOffset":6}},"state":"complete","name":"aligned"},
                {"kind":"sequence","parent":null,"children":[7],"ranges":{"full":{"startOffset":0,"endOffset":6}},"state":"complete"}
            ],
            "mathRoots": [{"node":8,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":6},"contentRange":{"startOffset":0,"endOffset":6},"state":"complete"}],
            "visibleProse": [], "scopes": [], "declarations": [], "macros": [], "includes": []
        })).unwrap();
        assert_eq!(
            render_canonical(&lower_document_region(
                &continued,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 6,
                }
            )),
            "relation(equals,sum(symbol(a),symbol(b)),symbol(c))"
        );
    }

    #[test]
    fn snapshot_lowering_consumes_composite_macro_notation_without_parsing_surface_tex() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8,
            "proseAnnotations": [],
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "\\law",
            "documentVersion": 1,
            "nodes": [
                {"kind":"command","parent":1,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":4},"command":{"startOffset":0,"endOffset":4}},"state":"opaque","name":"law"},
                {"kind":"sequence","parent":null,"children":[0],"ranges":{"full":{"startOffset":0,"endOffset":4}},"state":"complete"}
            ],
            "mathRoots": [{"node":1,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":4},"contentRange":{"startOffset":0,"endOffset":4},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":4},"state":"complete"}],
            "declarations": [],
            "macros": [{
                "kind":"call",
                "name":"law",
                "source":{"fileId":"main","path":"main.tex","range":{"startOffset":0,"endOffset":4}},
                "definitions":[],
                "expansion":{
                    "status":"expanded",
                    "depth":0,
                    "editable":false,
                    "surface":"ignored by Semath",
                    "inputRange":{"startOffset":0,"endOffset":4},
                    "notation":{
                        "nodes":[
                            {"kind":"token","children":[],"state":"complete","text":"K","lexicalClass":"identifier"},
                            {"kind":"token","children":[],"state":"complete","text":"=","lexicalClass":"operator"},
                            {"kind":"token","children":[],"state":"complete","text":"m","lexicalClass":"identifier"},
                            {"kind":"token","children":[],"state":"complete","text":"v","lexicalClass":"identifier"},
                            {"kind":"sequence","children":[0,1,2,3],"state":"complete"}
                        ],
                        "root":4
                    }
                }
            }],
            "includes": []
        }))
        .unwrap();
        let expression = lower_document_region(
            &document,
            &SourceRange {
                start_offset: 0,
                end_offset: 4,
            },
        );
        assert_eq!(
            render_canonical(&expression),
            "relation(equals,symbol(K),product(symbol(m),symbol(v)))"
        );
    }

    #[test]
    fn composite_macro_expansion_remains_one_operand_inside_a_formula() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8,
            "proseAnnotations": [],
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "m\\dtemp",
            "documentVersion": 1,
            "nodes": [
                {"kind":"token","parent":2,"children":[],"ranges":{"full":{"startOffset":0,"endOffset":1}},"state":"complete","text":"m","lexicalClass":"identifier"},
                {"kind":"command","parent":2,"children":[],"ranges":{"full":{"startOffset":1,"endOffset":7},"command":{"startOffset":1,"endOffset":7}},"state":"opaque","name":"dtemp"},
                {"kind":"sequence","parent":null,"children":[0,1],"ranges":{"full":{"startOffset":0,"endOffset":7}},"state":"complete"}
            ],
            "mathRoots": [{"node":2,"delimiter":"generated","fullRange":{"startOffset":0,"endOffset":7},"contentRange":{"startOffset":0,"endOffset":7},"state":"complete"}],
            "visibleProse": [],
            "scopes": [{"kind":"document","parent":null,"range":{"startOffset":0,"endOffset":7},"state":"complete"}],
            "declarations": [],
            "macros": [{
                "kind":"call",
                "name":"dtemp",
                "source":{"fileId":"main","path":"main.tex","range":{"startOffset":1,"endOffset":7}},
                "definitions":[],
                "expansion":{
                    "status":"expanded",
                    "depth":0,
                    "editable":false,
                    "surface":"ignored by Semath",
                    "inputRange":{"startOffset":1,"endOffset":7},
                    "notation":{
                        "nodes":[
                            {"kind":"token","children":[],"state":"complete","text":"Delta","lexicalClass":"identifier"},
                            {"kind":"token","children":[],"state":"complete","text":"T","lexicalClass":"identifier"},
                            {"kind":"sequence","children":[0,1],"state":"complete"}
                        ],
                        "root":2
                    }
                }
            }],
            "includes": []
        }))
        .unwrap();

        assert_eq!(
            render_canonical(&lower_document_region(
                &document,
                &SourceRange {
                    start_offset: 0,
                    end_offset: 7,
                },
            )),
            "product(symbol(m),symbol(DeltaT))"
        );
    }
}
