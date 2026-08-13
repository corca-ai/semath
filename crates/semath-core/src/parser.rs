#[cfg(test)]
use crate::{DocumentLanguage, SourceIndex};
use crate::{
    EquationNode, GeneratedNotationTree, MathRegion, MathRootState, NotationNode, NotationNodeKind,
    ProjectDocument, ProjectSourceRef, SourceRange, StructuralDeclaration,
    WASMTEX_SYNTAX_SCHEMA_VERSION,
};

#[derive(Clone, Debug)]
pub(crate) struct ParsedMath {
    pub region: MathRegion,
    pub root: EquationNode,
    pub symbols: Vec<(String, SourceRange)>,
}

pub(crate) fn parse_snapshot(document: &ProjectDocument) -> Result<Vec<ParsedMath>, String> {
    if document.schema_version != WASMTEX_SYNTAX_SCHEMA_VERSION {
        return Err(format!(
            "unsupported wasmtex syntax schema {}; expected {}",
            document.schema_version, WASMTEX_SYNTAX_SCHEMA_VERSION
        ));
    }
    let source_length = document.content.encode_utf16().count() as u32;
    validate_snapshot(document, source_length)?;
    document
        .math_roots
        .iter()
        .map(|root| {
            let mut symbols = Vec::new();
            let mut visiting = vec![false; document.nodes.len()];
            let equation =
                lower_snapshot_node(root.node, &document.nodes, &mut visiting, &mut symbols)?;
            Ok(ParsedMath {
                region: MathRegion {
                    full_range: root.full_range.clone(),
                    content_range: root.content_range.clone(),
                    delimiter: root.delimiter.clone(),
                    closed: root.state == MathRootState::Complete,
                },
                root: equation,
                symbols,
            })
        })
        .collect()
}

fn validate_snapshot(document: &ProjectDocument, source_length: u32) -> Result<(), String> {
    if document.nodes.len() > 100_000 {
        return Err("notation arena exceeds the Semath ingestion cap".to_owned());
    }
    let valid_range = |range: &SourceRange| valid_source_range(range, source_length);
    for (id, node) in document.nodes.iter().enumerate() {
        if !valid_range(&node.ranges.full) {
            return Err(format!("notation node {id} has an invalid source range"));
        }
        for range in [
            node.ranges.command.as_ref(),
            node.ranges.name.as_ref(),
            node.ranges.nucleus.as_ref(),
            node.ranges.editable.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !valid_range(range) || !range_contains(&node.ranges.full, range) {
                return Err(format!("notation node {id} has an invalid optional range"));
            }
        }
        let mut unique_children = std::collections::BTreeSet::new();
        if node
            .children
            .iter()
            .any(|child| !unique_children.insert(*child))
        {
            return Err(format!("notation node {id} has a duplicate child"));
        }
        if node
            .children
            .iter()
            .any(|child| *child as usize >= document.nodes.len())
        {
            return Err(format!("notation node {id} has an invalid child"));
        }
        if node.kind == NotationNodeKind::Token && node.lexical_class.is_none() {
            return Err(format!(
                "notation token {id} is missing its syntax-v7 lexical class"
            ));
        }
        if node
            .math_class
            .as_deref()
            .is_some_and(|class| !valid_math_class(class))
        {
            return Err(format!("notation node {id} has an invalid math class"));
        }
        if node
            .parent
            .is_some_and(|parent| parent as usize >= document.nodes.len())
        {
            return Err(format!("notation node {id} has an invalid parent"));
        }
        for child in &node.children {
            let child = &document.nodes[*child as usize];
            if child.parent != Some(id as u32) {
                return Err(format!(
                    "notation node {id} has an inconsistent child parent"
                ));
            }
            if !range_contains(&node.ranges.full, &child.ranges.full) {
                return Err(format!("notation node {id} has a child outside its range"));
            }
        }
        if let Some(parent) = node.parent {
            let parent = &document.nodes[parent as usize];
            let linked = parent.children.contains(&(id as u32))
                || parent
                    .arguments
                    .iter()
                    .any(|argument| argument.node == id as u32);
            if !linked {
                return Err(format!("notation node {id} has no reciprocal parent edge"));
            }
        }
        let mut unique_arguments = std::collections::BTreeSet::new();
        for argument in &node.arguments {
            let Some(argument_node) = document.nodes.get(argument.node as usize) else {
                return Err(format!("notation node {id} has an invalid argument"));
            };
            if !unique_arguments.insert(argument.node)
                || !node.children.contains(&argument.node)
                || !matches!(argument.syntax.as_str(), "required" | "optional")
                || !valid_range(&argument.range)
                || !range_contains(&node.ranges.full, &argument.range)
                || !range_contains(&argument.range, &argument_node.ranges.full)
            {
                return Err(format!("notation node {id} has an invalid argument range"));
            }
        }
        if let Some(provenance) = &node.provenance
            && (!matches!(
                provenance.origin.as_str(),
                "source" | "call-site" | "definition" | "expansion" | "generated"
            ) || !valid_reference(document, source_length, &provenance.source)
                || provenance
                    .call_site
                    .as_ref()
                    .is_some_and(|source| !valid_reference(document, source_length, source))
                || provenance
                    .definitions
                    .iter()
                    .any(|source| !valid_reference(document, source_length, source)))
        {
            return Err(format!("notation node {id} has invalid provenance"));
        }
    }
    for root in &document.math_roots {
        if root.node as usize >= document.nodes.len()
            || document.nodes[root.node as usize].parent.is_some()
            || !valid_range(&root.full_range)
            || !valid_range(&root.content_range)
            || root.content_range.start_offset < root.full_range.start_offset
            || root.content_range.end_offset > root.full_range.end_offset
            || !range_contains(
                &root.content_range,
                &document.nodes[root.node as usize].ranges.full,
            )
        {
            return Err("math root is corrupt".to_owned());
        }
    }
    let mut roots = document.math_roots.iter().collect::<Vec<_>>();
    roots.sort_by_key(|root| (root.full_range.start_offset, root.full_range.end_offset));
    if roots.windows(2).any(|pair| {
        pair[0].full_range.start_offset < pair[1].full_range.end_offset
            && pair[1].full_range.start_offset < pair[0].full_range.end_offset
    }) {
        return Err("math roots overlap".to_owned());
    }
    let mut reachable = vec![false; document.nodes.len()];
    let mut stack = document
        .math_roots
        .iter()
        .map(|root| root.node)
        .collect::<Vec<_>>();
    while let Some(id) = stack.pop() {
        if reachable[id as usize] {
            continue;
        }
        reachable[id as usize] = true;
        stack.extend(document.nodes[id as usize].children.iter().copied());
    }
    if reachable.iter().any(|reachable| !reachable) {
        return Err("notation arena contains unreachable nodes".to_owned());
    }
    let document_scopes = document
        .scopes
        .iter()
        .filter(|scope| scope.kind == "document" && scope.parent.is_none())
        .collect::<Vec<_>>();
    if document_scopes.len() != 1
        || document_scopes[0].range
            != (SourceRange {
                start_offset: 0,
                end_offset: source_length,
            })
    {
        return Err("syntax snapshot must contain one full document scope".to_owned());
    }
    for (id, scope) in document.scopes.iter().enumerate() {
        if !valid_range(&scope.range) {
            return Err(format!("syntax scope {id} has an invalid range"));
        }
        if !matches!(scope.kind.as_str(), "document" | "section" | "environment")
            || scope.kind == "document" && scope.parent.is_some()
        {
            return Err(format!("syntax scope {id} has an invalid kind"));
        }
        if let Some(parent) = scope.parent {
            let Some(parent_scope) = document.scopes.get(parent as usize) else {
                return Err(format!("syntax scope {id} has an invalid parent"));
            };
            if parent_scope.range.start_offset > scope.range.start_offset
                || scope.range.end_offset > parent_scope.range.end_offset
            {
                return Err(format!("syntax scope {id} escapes its parent"));
            }
        }
        let mut cursor = scope.parent;
        let mut visited = vec![false; document.scopes.len()];
        while let Some(parent) = cursor {
            let parent = parent as usize;
            if parent >= document.scopes.len() || visited[parent] {
                return Err(format!("syntax scope {id} has a cyclic parent chain"));
            }
            visited[parent] = true;
            cursor = document.scopes[parent].parent;
        }
    }
    if !ordered_non_overlapping(
        document.visible_prose.iter().map(|span| &span.range),
        source_length,
    ) {
        return Err("visible prose span has an invalid range".to_owned());
    }
    for annotation in &document.prose_annotations {
        if !valid_prose_annotation(annotation)
            || !valid_range(&annotation.range)
            || annotation.value_range.as_ref().is_some_and(|range| {
                !valid_range(range) || !range_contains(&annotation.range, range)
            })
        {
            return Err("prose annotation has an invalid range".to_owned());
        }
    }
    for (id, block) in document.blocks.iter().enumerate() {
        let Some(parent) = document.scopes.get(block.parent_scope as usize) else {
            return Err(format!("syntax block {id} has an invalid parent scope"));
        };
        if !valid_range(&block.range)
            || !range_contains_offset(&parent.range, block.range.start_offset)
        {
            return Err(format!("syntax block {id} has an invalid range"));
        }
        if block
            .content_range
            .as_ref()
            .is_some_and(|range| !valid_range(range) || !range_contains(&block.range, range))
        {
            return Err(format!("syntax block {id} has an invalid content range"));
        }
    }
    if !ordered_non_overlapping(
        document.blocks.iter().map(|block| &block.range),
        source_length,
    ) {
        return Err("syntax blocks must be ordered and non-overlapping".to_owned());
    }
    for declaration in &document.declarations {
        if !valid_declaration_references(document, source_length, declaration) {
            return Err("structural declaration has invalid source provenance".to_owned());
        }
    }
    for event in &document.macros {
        if !valid_owned_reference(document, source_length, &event.source)
            || event
                .definitions
                .iter()
                .any(|source| !valid_reference(document, source_length, source))
            || event
                .expansion
                .input_range
                .as_ref()
                .is_some_and(|range| !valid_source_range(range, source_length))
        {
            return Err(format!(
                "macro {} has invalid source provenance",
                event.name
            ));
        }
    }
    if document
        .includes
        .iter()
        .any(|include| !valid_owned_reference(document, source_length, &include.source))
    {
        return Err("include has invalid source provenance".to_owned());
    }
    let mut generated_nodes = 0usize;
    for (event, tree) in document
        .macros
        .iter()
        .filter_map(|event| event.expansion.notation.as_ref().map(|tree| (event, tree)))
    {
        generated_nodes = generated_nodes.saturating_add(tree.nodes.len());
        if generated_nodes > 100_000 {
            return Err("generated notation exceeds the Semath ingestion cap".to_owned());
        }
        validate_generated_tree(tree).map_err(|reason| {
            format!(
                "macro {} has invalid generated notation: {reason}",
                event.name
            )
        })?;
    }
    Ok(())
}

fn range_contains(container: &SourceRange, nested: &SourceRange) -> bool {
    container.start_offset <= nested.start_offset && nested.end_offset <= container.end_offset
}

fn range_contains_offset(range: &SourceRange, offset: u32) -> bool {
    range.start_offset <= offset && offset < range.end_offset
}

fn valid_source_range(range: &SourceRange, source_length: u32) -> bool {
    range.start_offset <= range.end_offset && range.end_offset <= source_length
}

fn valid_math_class(class: &str) -> bool {
    matches!(
        class,
        "ordinary"
            | "operator"
            | "binary"
            | "relation"
            | "opening"
            | "closing"
            | "punctuation"
            | "inner"
    )
}

fn valid_reference(
    document: &ProjectDocument,
    source_length: u32,
    source: &ProjectSourceRef,
) -> bool {
    source.file_id != document.file_id
        || source.path == document.path && valid_source_range(&source.range, source_length)
}

fn valid_owned_reference(
    document: &ProjectDocument,
    source_length: u32,
    source: &ProjectSourceRef,
) -> bool {
    source.file_id == document.file_id
        && source.path == document.path
        && valid_source_range(&source.range, source_length)
}

fn ordered_non_overlapping<'a>(
    ranges: impl IntoIterator<Item = &'a SourceRange>,
    source_length: u32,
) -> bool {
    let mut previous_end = 0;
    for range in ranges {
        if !valid_source_range(range, source_length) || range.start_offset < previous_end {
            return false;
        }
        previous_end = range.end_offset;
    }
    true
}

fn valid_prose_annotation(annotation: &crate::ProseAnnotation) -> bool {
    match annotation.kind.as_str() {
        "citation" => !annotation.name.is_empty() && annotation.value_range.is_none(),
        "document-field" => matches!(annotation.name.as_str(), "title" | "author" | "keywords"),
        _ => false,
    }
}

fn valid_declaration_references(
    document: &ProjectDocument,
    source_length: u32,
    declaration: &StructuralDeclaration,
) -> bool {
    let valid = |source| valid_owned_reference(document, source_length, source);
    match declaration {
        StructuralDeclaration::Class { source, .. }
        | StructuralDeclaration::Package { source, .. }
        | StructuralDeclaration::Environment { source, .. } => valid(source),
        StructuralDeclaration::Macro {
            source,
            body_source,
            ..
        } => valid(source) && body_source.as_ref().is_none_or(&valid),
        StructuralDeclaration::Operator {
            source,
            name_source,
            surface_source,
            ..
        } => valid(source) && valid(name_source) && valid(surface_source),
        StructuralDeclaration::PairedDelimiter {
            source,
            name_source,
            ..
        } => valid(source) && valid(name_source),
        StructuralDeclaration::Glossary {
            source,
            key_source,
            options,
            fields,
            ..
        } => {
            valid(source)
                && valid(key_source)
                && options.iter().all(|field| valid(&field.source))
                && fields.iter().all(|field| valid(&field.source))
        }
        StructuralDeclaration::Acronym {
            source,
            key_source,
            short_source,
            long_source,
            options,
            ..
        } => {
            valid(source)
                && valid(key_source)
                && valid(short_source)
                && valid(long_source)
                && options.iter().all(|field| valid(&field.source))
        }
    }
}

fn validate_generated_tree(tree: &GeneratedNotationTree) -> Result<(), &'static str> {
    if tree.nodes.is_empty() || tree.nodes.len() > 10_000 {
        return Err("node count is outside the per-expansion cap");
    }
    if tree.root as usize >= tree.nodes.len() {
        return Err("root does not exist");
    }
    let mut incoming = vec![0usize; tree.nodes.len()];
    for node in &tree.nodes {
        let mut unique_children = std::collections::BTreeSet::new();
        if node
            .children
            .iter()
            .any(|child| !unique_children.insert(*child))
        {
            return Err("node contains a duplicate child");
        }
        if node
            .children
            .iter()
            .chain(node.arguments.iter().map(|argument| &argument.node))
            .any(|child| *child as usize >= tree.nodes.len())
        {
            return Err("child does not exist");
        }
        for child in &node.children {
            incoming[*child as usize] += 1;
        }
    }
    let mut active = vec![false; tree.nodes.len()];
    let mut visited = vec![false; tree.nodes.len()];
    let mut stack = vec![(tree.root, false, 0usize)];
    while let Some((node_id, leaving, depth)) = stack.pop() {
        let index = node_id as usize;
        if leaving {
            active[index] = false;
            visited[index] = true;
            continue;
        }
        if active[index] {
            return Err("tree contains a cycle");
        }
        if visited[index] {
            continue;
        }
        if depth > 128 {
            return Err("tree exceeds the nesting cap");
        }
        active[index] = true;
        stack.push((node_id, true, depth));
        for child in tree.nodes[index]
            .children
            .iter()
            .chain(
                tree.nodes[index]
                    .arguments
                    .iter()
                    .map(|argument| &argument.node),
            )
            .rev()
        {
            stack.push((*child, false, depth + 1));
        }
    }
    if visited.iter().any(|visited| !visited) {
        return Err("tree contains unreachable nodes");
    }
    if incoming[tree.root as usize] != 0
        || incoming
            .iter()
            .enumerate()
            .any(|(index, count)| index != tree.root as usize && *count != 1)
    {
        return Err("nodes do not form one rooted tree");
    }
    Ok(())
}

fn lower_snapshot_node(
    id: u32,
    nodes: &[NotationNode],
    visiting: &mut [bool],
    symbols: &mut Vec<(String, SourceRange)>,
) -> Result<EquationNode, String> {
    let index = id as usize;
    let node = nodes
        .get(index)
        .ok_or_else(|| format!("notation node {id} does not exist"))?;
    if visiting[index] {
        return Err(format!("notation node {id} contains a cycle"));
    }
    visiting[index] = true;
    let mut child_symbols = Vec::new();
    let mut children = node
        .children
        .iter()
        .map(|child| lower_snapshot_node(*child, nodes, visiting, &mut child_symbols))
        .collect::<Result<Vec<_>, _>>()?;
    visiting[index] = false;

    let range = node.ranges.full.clone();
    let (kind, label) = match node.kind {
        NotationNodeKind::Token => {
            let text = node.text.clone().unwrap_or_default();
            let kind = match node.lexical_class {
                Some(crate::LexicalClass::Number) => "number",
                Some(crate::LexicalClass::Operator | crate::LexicalClass::Punctuation) => {
                    "operator"
                }
                Some(crate::LexicalClass::Identifier) => "symbol",
                Some(crate::LexicalClass::Other) => "text",
                None => return Err(format!("notation token {id} has no lexical class")),
            };
            if kind == "symbol" && !text.is_empty() {
                symbols.push((text.clone(), range.clone()));
            }
            (kind, Some(text))
        }
        NotationNodeKind::NamedOperator => {
            let name = node.name.clone().unwrap_or_default();
            let symbol_range = node.ranges.name.clone().unwrap_or_else(|| range.clone());
            if !name.is_empty() {
                symbols.push((name.clone(), symbol_range));
            }
            children.clear();
            ("symbol", Some(name))
        }
        NotationNodeKind::Sequence => ("sequence", None),
        NotationNodeKind::Group => ("group", None),
        NotationNodeKind::Delimiter => ("delimited", node.name.clone()),
        NotationNodeKind::Modifier | NotationNodeKind::Style => {
            ("styled", node.name.as_ref().map(|name| format!("\\{name}")))
        }
        NotationNodeKind::Script => {
            let script_kind = node.name.as_deref().unwrap_or("subscript");
            if children.len() >= 2 {
                let script = children.remove(1);
                let script_range = script.range.clone();
                children.insert(
                    1,
                    EquationNode {
                        kind: script_kind.to_owned(),
                        label: None,
                        range: script_range,
                        children: vec![script],
                    },
                );
            }
            ("scripted", None)
        }
        NotationNodeKind::Command => {
            let name = node.name.clone().unwrap_or_default();
            let kind = match name.as_str() {
                "frac" | "dfrac" | "tfrac" => "fraction",
                "sqrt" => "root",
                "sum" => "sum",
                "int" => "integral",
                "lim" => "limit",
                "forall" | "exists" => "quantifier",
                _ => "command",
            };
            if kind == "command" && !name.is_empty() {
                symbols.push((
                    format!("\\{name}"),
                    node.ranges.command.clone().unwrap_or_else(|| range.clone()),
                ));
            }
            (kind, Some(format!("\\{name}")))
        }
        NotationNodeKind::Alignment => ("alignment", node.name.clone()),
        NotationNodeKind::Environment => ("environment", node.name.clone()),
        NotationNodeKind::Opaque => ("opaque", node.name.clone().or_else(|| node.text.clone())),
        NotationNodeKind::Error => ("error", node.name.clone().or_else(|| node.text.clone())),
    };
    if node.kind != NotationNodeKind::NamedOperator {
        symbols.extend(child_symbols);
    }
    Ok(EquationNode {
        kind: kind.to_owned(),
        label,
        range,
        children,
    })
}

#[cfg(test)]
pub(crate) fn test_math_regions(source: &str, language: DocumentLanguage) -> Vec<MathRegion> {
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

#[cfg(test)]
fn escaped(bytes: &[u8], offset: usize) -> bool {
    let mut slashes = 0;
    let mut cursor = offset;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        slashes += 1;
        cursor -= 1;
    }
    slashes % 2 == 1
}

#[cfg(test)]
fn range(index: &SourceIndex, start: usize, end: usize) -> SourceRange {
    SourceRange {
        start_offset: index.utf16_for_byte(start),
        end_offset: index.utf16_for_byte(end),
    }
}

#[cfg(test)]
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

#[cfg(test)]
struct Parser<'a> {
    source: &'a str,
    base_byte: usize,
    cursor: usize,
    index: &'a SourceIndex,
    symbols: Vec<(String, SourceRange)>,
}

#[cfg(test)]
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
            "mathbb" | "mathbf" | "mathrm" | "mathcal" | "mathsf" | "mathtt" | "mathit"
            | "operatorname" => {
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

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::{
        parse_regions, parse_snapshot, selection_path, test_math_regions, validate_generated_tree,
    };
    use crate::{
        DocumentLanguage, GeneratedNotationNode, GeneratedNotationTree, NotationNodeKind,
        ProjectDocument, SyntaxState,
    };

    fn valid_snapshot_document() -> ProjectDocument {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 8,
            "proseAnnotations": [],
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "$x$",
            "documentVersion": 1,
            "nodes": [
                {
                    "kind": "token", "parent": 1, "children": [],
                    "ranges": {"full": {"startOffset": 1, "endOffset": 2}, "editable": {"startOffset": 1, "endOffset": 2}},
                    "state": "complete", "text": "x", "lexicalClass": "identifier"
                },
                {
                    "kind": "sequence", "parent": null, "children": [0],
                    "ranges": {"full": {"startOffset": 1, "endOffset": 2}},
                    "state": "complete"
                }
            ],
            "mathRoots": [
                {"node": 1, "delimiter": "$", "fullRange": {"startOffset": 0, "endOffset": 3}, "contentRange": {"startOffset": 1, "endOffset": 2}, "state": "complete"}
            ],
            "visibleProse": [],
            "scopes": [{"kind": "document", "parent": null, "range": {"startOffset": 0, "endOffset": 3}, "state": "complete"}],
            "blocks": [], "declarations": [], "macros": [], "includes": []
        }))
        .unwrap()
    }

    #[test]
    fn rejects_out_of_bounds_optional_node_ranges() {
        let mut document = valid_snapshot_document();
        document.nodes[0].ranges.editable = Some(crate::SourceRange {
            start_offset: 1,
            end_offset: 99,
        });
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_missing_notation_argument_nodes() {
        let mut document = valid_snapshot_document();
        document.nodes[1].arguments.push(crate::NotationArgument {
            node: 99,
            role: "nucleus".into(),
            syntax: "required".into(),
            range: crate::SourceRange {
                start_offset: 1,
                end_offset: 2,
            },
        });
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_notation_argument_ranges() {
        let mut document = valid_snapshot_document();
        document.nodes[1].arguments.push(crate::NotationArgument {
            node: 0,
            role: "nucleus".into(),
            syntax: "required".into(),
            range: crate::SourceRange {
                start_offset: 1,
                end_offset: 99,
            },
        });
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_children_that_escape_their_parent_range() {
        let mut document = valid_snapshot_document();
        document.nodes[0].ranges.full = crate::SourceRange {
            start_offset: 0,
            end_offset: 3,
        };
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_parent_links_missing_the_reciprocal_child() {
        let mut document = valid_snapshot_document();
        document.nodes[1].children.clear();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_duplicate_child_links() {
        let mut document = valid_snapshot_document();
        document.nodes[1].children.push(0);
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_root_nodes_that_escape_math_content() {
        let mut document = valid_snapshot_document();
        document.nodes[1].ranges.full = crate::SourceRange {
            start_offset: 0,
            end_offset: 3,
        };
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_duplicate_overlapping_math_roots() {
        let mut document = valid_snapshot_document();
        document.math_roots.push(document.math_roots[0].clone());
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_out_of_bounds_prose_annotations() {
        let mut document = valid_snapshot_document();
        document.prose_annotations = serde_json::from_value(serde_json::json!([{
            "kind": "citation", "name": "cite", "range": {"startOffset": 2, "endOffset": 99}, "state": "complete"
        }]))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_blocks_with_missing_parent_scopes() {
        let mut document = valid_snapshot_document();
        document.blocks = serde_json::from_value(serde_json::json!([{
            "kind": "paragraph", "parentScope": 99, "range": {"startOffset": 0, "endOffset": 3}, "state": "complete"
        }]))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_block_content_that_escapes_its_block() {
        let mut document = valid_snapshot_document();
        document.blocks = serde_json::from_value(serde_json::json!([{
            "kind": "paragraph", "parentScope": 0, "range": {"startOffset": 1, "endOffset": 2},
            "contentRange": {"startOffset": 0, "endOffset": 3}, "state": "complete"
        }]))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn accepts_blocks_that_cross_their_starting_scope_boundary() {
        let mut document = valid_snapshot_document();
        document.scopes = serde_json::from_value(serde_json::json!([
            {"kind": "document", "parent": null, "range": {"startOffset": 0, "endOffset": 3}, "state": "complete"},
            {"kind": "environment", "name": "lemma", "parent": 0, "range": {"startOffset": 0, "endOffset": 1}, "state": "complete"}
        ]))
        .unwrap();
        document.blocks = serde_json::from_value(serde_json::json!([{
            "kind": "paragraph", "parentScope": 1, "range": {"startOffset": 0, "endOffset": 3}, "state": "complete"
        }]))
        .unwrap();

        assert!(parse_snapshot(&document).is_ok());
    }

    #[test]
    fn rejects_cycles_through_generated_argument_edges() {
        let mut document = valid_snapshot_document();
        document.macros = serde_json::from_value(serde_json::json!([{
            "kind": "call", "name": "bad", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 1, "endOffset": 2}},
            "definitions": [],
            "expansion": {
                "status": "expanded", "depth": 1, "editable": true, "inputRange": {"startOffset": 1, "endOffset": 2},
                "notation": {"root": 0, "nodes": [{
                    "kind": "command", "children": [], "state": "complete", "name": "bad",
                    "arguments": [{"node": 0, "role": "nucleus", "syntax": "required"}]
                }]}
            }
        }]))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unreachable_source_notation_nodes() {
        let mut document = valid_snapshot_document();
        document.nodes.push(
            serde_json::from_value(serde_json::json!({
                "kind": "token", "parent": null, "children": [],
                "ranges": {"full": {"startOffset": 1, "endOffset": 2}},
                "state": "complete", "text": "orphan", "lexicalClass": "identifier"
            }))
            .unwrap(),
        );
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_duplicate_source_notation_arguments() {
        let mut document = valid_snapshot_document();
        let argument = crate::NotationArgument {
            node: 0,
            role: "body".into(),
            syntax: "required".into(),
            range: crate::SourceRange {
                start_offset: 1,
                end_offset: 2,
            },
        };
        document.nodes[1].arguments = vec![argument.clone(), argument];
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_arguments_that_are_not_structural_children() {
        let mut document = valid_snapshot_document();
        document.nodes[1].children.clear();
        document.nodes[1].arguments.push(crate::NotationArgument {
            node: 0,
            role: "body".into(),
            syntax: "required".into(),
            range: crate::SourceRange {
                start_offset: 1,
                end_offset: 2,
            },
        });
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unknown_source_argument_syntax() {
        let mut document = valid_snapshot_document();
        document.nodes[1].arguments.push(crate::NotationArgument {
            node: 0,
            role: "body".into(),
            syntax: "invented".into(),
            range: crate::SourceRange {
                start_offset: 1,
                end_offset: 2,
            },
        });
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unknown_math_classes() {
        let mut document = valid_snapshot_document();
        document.nodes[0].math_class = Some("invented".into());
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unknown_provenance_origins() {
        let mut document = valid_snapshot_document();
        document.nodes[0].provenance = serde_json::from_value(serde_json::json!({
            "origin": "invented", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 1, "endOffset": 2}},
            "editable": true
        })).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_invalid_local_provenance_references() {
        let mut document = valid_snapshot_document();
        document.nodes[0].provenance = serde_json::from_value(serde_json::json!({
            "origin": "source", "source": {"fileId": "main", "path": "wrong.tex", "range": {"startOffset": 1, "endOffset": 2}},
            "editable": true
        })).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_overlapping_visible_prose_spans() {
        let mut document = valid_snapshot_document();
        document.visible_prose = serde_json::from_value(serde_json::json!([
            {"range": {"startOffset": 0, "endOffset": 2}, "state": "complete"},
            {"range": {"startOffset": 1, "endOffset": 3}, "state": "complete"}
        ]))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unknown_prose_annotation_kinds() {
        let mut document = valid_snapshot_document();
        document.prose_annotations = serde_json::from_value(serde_json::json!([{
            "kind": "invented", "name": "value", "range": {"startOffset": 0, "endOffset": 1}, "state": "complete"
        }])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_nested_document_scopes() {
        let mut document = valid_snapshot_document();
        document.scopes.push(serde_json::from_value(serde_json::json!({
            "kind": "document", "parent": 0, "range": {"startOffset": 1, "endOffset": 2}, "state": "complete"
        })).unwrap());
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unknown_scope_kinds() {
        let mut document = valid_snapshot_document();
        document.scopes.push(serde_json::from_value(serde_json::json!({
            "kind": "invented", "parent": 0, "range": {"startOffset": 1, "endOffset": 2}, "state": "complete"
        })).unwrap());
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_overlapping_syntax_blocks() {
        let mut document = valid_snapshot_document();
        document.blocks = serde_json::from_value(serde_json::json!([
            {"kind": "paragraph", "parentScope": 0, "range": {"startOffset": 0, "endOffset": 2}, "state": "complete"},
            {"kind": "paragraph", "parentScope": 0, "range": {"startOffset": 1, "endOffset": 3}, "state": "complete"}
        ])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_out_of_order_syntax_blocks() {
        let mut document = valid_snapshot_document();
        document.blocks = serde_json::from_value(serde_json::json!([
            {"kind": "paragraph", "parentScope": 0, "range": {"startOffset": 2, "endOffset": 3}, "state": "complete"},
            {"kind": "paragraph", "parentScope": 0, "range": {"startOffset": 0, "endOffset": 1}, "state": "complete"}
        ])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_invalid_local_declaration_references() {
        let mut document = valid_snapshot_document();
        document.declarations = serde_json::from_value(serde_json::json!([{
            "kind": "environment", "name": "lemma",
            "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 0, "endOffset": 99}}
        }])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_invalid_macro_input_ranges() {
        let mut document = valid_snapshot_document();
        document.macros = serde_json::from_value(serde_json::json!([{
            "kind": "call", "name": "m", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 1, "endOffset": 2}},
            "definitions": [], "expansion": {"status": "expanded", "depth": 0, "editable": true, "inputRange": {"startOffset": 1, "endOffset": 99}}
        }])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_invalid_local_include_references() {
        let mut document = valid_snapshot_document();
        document.includes = serde_json::from_value(serde_json::json!([{
            "path": "other.tex", "type": "input",
            "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 0, "endOffset": 99}}
        }])).unwrap();
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_duplicate_generated_notation_edges() {
        let mut document = valid_snapshot_document();
        document.macros = generated_macro(serde_json::json!({"root": 0, "nodes": [
            {"kind": "sequence", "children": [1, 1], "state": "complete"},
            {"kind": "token", "children": [], "state": "complete", "text": "x", "lexicalClass": "identifier"}
        ]}));
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_shared_generated_notation_nodes() {
        let mut document = valid_snapshot_document();
        document.macros = generated_macro(serde_json::json!({"root": 0, "nodes": [
            {"kind": "sequence", "children": [1, 2], "state": "complete"},
            {"kind": "group", "children": [3], "state": "complete"},
            {"kind": "group", "children": [3], "state": "complete"},
            {"kind": "token", "children": [], "state": "complete", "text": "x", "lexicalClass": "identifier"}
        ]}));
        assert!(parse_snapshot(&document).is_err());
    }

    #[test]
    fn rejects_unreachable_generated_notation_nodes() {
        let mut document = valid_snapshot_document();
        document.macros = generated_macro(serde_json::json!({"root": 0, "nodes": [
            {"kind": "token", "children": [], "state": "complete", "text": "x", "lexicalClass": "identifier"},
            {"kind": "token", "children": [], "state": "complete", "text": "orphan", "lexicalClass": "identifier"}
        ]}));
        assert!(parse_snapshot(&document).is_err());
    }

    fn generated_macro(notation: serde_json::Value) -> Vec<crate::ProjectMacro> {
        serde_json::from_value(serde_json::json!([{
            "kind": "call", "name": "m", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 1, "endOffset": 2}},
            "definitions": [], "expansion": {
                "status": "expanded", "depth": 0, "editable": true,
                "inputRange": {"startOffset": 1, "endOffset": 2}, "notation": notation
            }
        }])).unwrap()
    }
    #[test]
    fn rejects_cyclic_generated_macro_notation() {
        let tree = GeneratedNotationTree {
            root: 0,
            nodes: vec![GeneratedNotationNode {
                kind: NotationNodeKind::Sequence,
                children: vec![0],
                state: SyntaxState::Complete,
                name: None,
                text: None,
                arguments: Vec::new(),
                lexical_class: None,
                math_class: None,
            }],
        };
        assert_eq!(validate_generated_tree(&tree), Err("tree contains a cycle"));
    }

    #[test]
    fn finds_markdown_math_but_not_fenced_code() {
        let source = "before $x_i$\n```\n$ignored$\n```\nafter \\[\\frac{1}{N}\\]";
        let regions = test_math_regions(source, DocumentLanguage::Markdown);
        assert_eq!(regions.len(), 2);
        assert!(regions.iter().all(|region| region.closed));
    }

    #[test]
    fn builds_nested_selection_ranges() {
        let source = "$\\frac{1}{N}x_i$";
        let regions = test_math_regions(source, DocumentLanguage::Latex);
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
        let parsed = parse_regions(source, &test_math_regions(source, DocumentLanguage::Latex));
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

    #[test]
    fn v7_named_operator_is_one_occurrence_and_incomplete_nodes_degrade_locally() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8,
            "proseAnnotations": [],
            "fileId": "main",
            "path": "main.tex",
            "language": "latex",
            "content": "$\\operatorname{ECE}$ $x+{$",
            "documentVersion": 1,
            "nodes": [
                {
                    "kind": "token", "parent": 1, "children": [],
                    "ranges": {"full": {"startOffset": 15, "endOffset": 18}, "editable": {"startOffset": 15, "endOffset": 18}},
                    "state": "complete", "text": "ECE", "lexicalClass": "identifier",
                    "provenance": {"origin": "source", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 15, "endOffset": 18}}, "editable": true}
                },
                {
                    "kind": "named-operator", "parent": null, "children": [0],
                    "ranges": {"full": {"startOffset": 1, "endOffset": 19}, "name": {"startOffset": 15, "endOffset": 18}, "editable": {"startOffset": 1, "endOffset": 19}},
                    "state": "complete", "name": "ECE",
                    "provenance": {"origin": "source", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 1, "endOffset": 19}}, "editable": true}
                },
                {
                    "kind": "opaque", "parent": null, "children": [],
                    "ranges": {"full": {"startOffset": 22, "endOffset": 25}, "editable": {"startOffset": 22, "endOffset": 25}},
                    "state": "truncated",
                    "provenance": {"origin": "source", "source": {"fileId": "main", "path": "main.tex", "range": {"startOffset": 22, "endOffset": 25}}, "editable": true}
                }
            ],
            "mathRoots": [
                {"node": 1, "delimiter": "$", "fullRange": {"startOffset": 0, "endOffset": 20}, "contentRange": {"startOffset": 1, "endOffset": 19}, "state": "complete"},
                {"node": 2, "delimiter": "$", "fullRange": {"startOffset": 21, "endOffset": 26}, "contentRange": {"startOffset": 22, "endOffset": 25}, "state": "incomplete"}
            ],
            "visibleProse": [{"range": {"startOffset": 20, "endOffset": 21}, "state": "complete"}],
            "scopes": [{"kind": "document", "parent": null, "range": {"startOffset": 0, "endOffset": 26}, "state": "complete"}],
            "declarations": [], "macros": [], "includes": []
        }))
        .unwrap();

        let parsed = parse_snapshot(&document).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0].symbols,
            [(
                "ECE".to_owned(),
                crate::SourceRange {
                    start_offset: 15,
                    end_offset: 18
                }
            )]
        );
        assert!(parsed[1].symbols.is_empty());
        assert!(!parsed[1].region.closed);
    }

    #[test]
    fn v5_rejects_a_corrupt_arena_root() {
        let document: ProjectDocument = serde_json::from_value(serde_json::json!({
            "schemaVersion": 8, "proseAnnotations": [], "fileId": "main", "path": "main.tex", "language": "latex",
            "content": "$x$", "documentVersion": 1, "nodes": [],
            "mathRoots": [{"node": 4, "delimiter": "$", "fullRange": {"startOffset": 0, "endOffset": 3}, "contentRange": {"startOffset": 1, "endOffset": 2}, "state": "complete"}],
            "visibleProse": [],
            "scopes": [{"kind": "document", "parent": null, "range": {"startOffset": 0, "endOffset": 3}, "state": "complete"}],
            "declarations": [], "macros": [], "includes": []
        }))
        .unwrap();
        assert!(parse_snapshot(&document).is_err());
    }
}
