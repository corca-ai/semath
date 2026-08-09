use crate::parser::ParsedMath;
use crate::{EquationNode, SourceRange};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MathBinder {
    pub kind: String,
    pub symbol: String,
    pub declaration: SourceRange,
    pub scope: SourceRange,
}

pub(crate) fn binders(parsed: &ParsedMath) -> Vec<MathBinder> {
    let mut output = Vec::new();
    collect_binders(parsed, &parsed.root, &parsed.root.range, &mut output);
    output.sort_by_key(|binder| binder.declaration.start_offset);
    output
}

pub(crate) fn binder_at<'a>(
    parsed: &ParsedMath,
    binders: &'a [MathBinder],
    offset: u32,
) -> Option<&'a MathBinder> {
    let (symbol, occurrence) = parsed
        .symbols
        .iter()
        .find(|(_, range)| range.contains(offset))?;
    resolve_binder(binders, symbol, occurrence)
}

pub(crate) fn bound_occurrences(
    parsed: &ParsedMath,
    binders: &[MathBinder],
    target: &MathBinder,
) -> Vec<SourceRange> {
    parsed
        .symbols
        .iter()
        .filter(|(symbol, range)| {
            symbol == &target.symbol
                && resolve_binder(binders, symbol, range)
                    .is_some_and(|binder| binder.declaration == target.declaration)
        })
        .map(|(_, range)| range.clone())
        .collect()
}

pub(crate) fn rename_rejection(
    parsed: &ParsedMath,
    binders: &[MathBinder],
    target: &MathBinder,
    new_name: &str,
) -> Option<String> {
    if new_name == target.symbol {
        return Some("The new name is unchanged.".into());
    }
    if !valid_binder_name(new_name) {
        return Some("Bound variables can currently be renamed to one letter.".into());
    }

    let target_occurrences = bound_occurrences(parsed, binders, target);
    if binders.iter().any(|binder| {
        binder.symbol == new_name
            && nested_in(binder, target)
            && target_occurrences
                .iter()
                .any(|occurrence| binder.scope.contains(occurrence.start_offset))
    }) {
        return Some(format!(
            "Renaming `{}` to `{new_name}` would make an occurrence belong to a nested `{new_name}` binder.",
            target.symbol
        ));
    }

    let captures_existing = parsed.symbols.iter().any(|(symbol, occurrence)| {
        if symbol != new_name || !target.scope.contains(occurrence.start_offset) {
            return false;
        }
        match resolve_binder(binders, symbol, occurrence) {
            Some(existing) => !nested_in(existing, target),
            None => true,
        }
    });
    if captures_existing {
        return Some(format!(
            "Renaming `{}` to `{new_name}` would capture an existing `{new_name}` occurrence.",
            target.symbol
        ));
    }

    None
}

fn valid_binder_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(char::is_alphabetic) && characters.next().is_none()
}

fn nested_in(candidate: &MathBinder, target: &MathBinder) -> bool {
    candidate.declaration.start_offset > target.declaration.start_offset
        && candidate.scope.end_offset <= target.scope.end_offset
}

fn resolve_binder<'a>(
    binders: &'a [MathBinder],
    symbol: &str,
    occurrence: &SourceRange,
) -> Option<&'a MathBinder> {
    binders
        .iter()
        .filter(|binder| {
            binder.symbol == symbol
                && (binder.declaration == *occurrence
                    || (binder.declaration.start_offset <= occurrence.start_offset
                        && binder.scope.contains(occurrence.start_offset)))
        })
        .max_by_key(|binder| {
            (
                binder.declaration.start_offset,
                u32::MAX - (binder.scope.end_offset - binder.scope.start_offset),
            )
        })
}

fn collect_binders(
    parsed: &ParsedMath,
    node: &EquationNode,
    parent_scope: &SourceRange,
    output: &mut Vec<MathBinder>,
) {
    let scope = if node.kind == "sequence" {
        &node.range
    } else {
        parent_scope
    };

    if node.kind == "sequence" {
        for (index, child) in node.children.iter().enumerate() {
            let child_scope = if scripted_binder_operator(child) {
                SourceRange {
                    start_offset: node.range.start_offset,
                    end_offset: node
                        .children
                        .get(index + 1)
                        .map_or(child.range.end_offset, |body| body.range.end_offset),
                }
            } else {
                node.range.clone()
            };
            collect_binders(parsed, child, &child_scope, output);
        }
        return;
    }

    if node.kind == "scripted"
        && parent_scope.end_offset > node.range.end_offset
        && let Some(operator) = node.children.first()
        && matches!(operator.kind.as_str(), "sum" | "limit")
        && let Some(subscript) = node.children.iter().find(|child| child.kind == "subscript")
        && let Some((symbol, declaration)) = first_symbol_in(parsed, &subscript.range)
    {
        output.push(MathBinder {
            kind: operator.kind.clone(),
            symbol: symbol.clone(),
            declaration: declaration.clone(),
            scope: SourceRange {
                start_offset: declaration.start_offset,
                end_offset: scope.end_offset,
            },
        });
    } else if node.kind == "quantifier"
        && let Some((symbol, declaration)) = parsed.symbols.iter().find(|(symbol, range)| {
            range.start_offset >= node.range.end_offset
                && range.end_offset <= scope.end_offset
                && valid_binder_name(symbol)
        })
    {
        output.push(MathBinder {
            kind: node.label.clone().unwrap_or_else(|| "quantifier".into()),
            symbol: symbol.clone(),
            declaration: declaration.clone(),
            scope: SourceRange {
                start_offset: declaration.start_offset,
                end_offset: scope.end_offset,
            },
        });
    }

    for child in &node.children {
        collect_binders(parsed, child, scope, output);
    }
}

fn scripted_binder_operator(node: &EquationNode) -> bool {
    if node.kind == "application" {
        return node.children.first().is_some_and(scripted_binder_operator);
    }
    node.kind == "scripted"
        && node
            .children
            .first()
            .is_some_and(|operator| matches!(operator.kind.as_str(), "sum" | "limit"))
}

fn first_symbol_in<'a>(
    parsed: &'a ParsedMath,
    range: &SourceRange,
) -> Option<&'a (String, SourceRange)> {
    parsed.symbols.iter().find(|(symbol, symbol_range)| {
        symbol_range.start_offset >= range.start_offset
            && symbol_range.end_offset <= range.end_offset
            && valid_binder_name(symbol)
    })
}

#[cfg(test)]
mod tests {
    use super::{binder_at, binders, bound_occurrences, rename_rejection};
    use crate::DocumentLanguage;
    use crate::parser::{parse_regions, test_math_regions};

    fn parsed(source: &str) -> crate::parser::ParsedMath {
        parse_regions(source, &test_math_regions(source, DocumentLanguage::Latex)).remove(0)
    }

    #[test]
    fn resolves_sum_limit_and_quantifier_binders() {
        for source in [
            "$\\sum_{i=1}^n x_i$",
            "$\\lim_{n\\to\\infty} a_n$",
            "$\\forall x \\in X, P(x)$",
            "$\\exists y \\in Y, Q(y)$",
        ] {
            let parsed = parsed(source);
            let found = binders(&parsed);
            assert_eq!(found.len(), 1, "{source}");
            assert!(bound_occurrences(&parsed, &found, &found[0]).len() >= 2);
        }
    }

    #[test]
    fn keeps_shadowed_occurrences_with_the_innermost_binder() {
        let source = "$\\sum_{i=1}^n (x_i + \\sum_{i=1}^m y_i) + z_i$";
        let parsed = parsed(source);
        let found = binders(&parsed);
        assert_eq!(found.len(), 2);
        assert_eq!(bound_occurrences(&parsed, &found, &found[0]).len(), 2);
        assert_eq!(bound_occurrences(&parsed, &found, &found[1]).len(), 2);
        let inner_use = source.find("y_i").unwrap() as u32 + 2;
        assert_eq!(
            binder_at(&parsed, &found, inner_use).unwrap().declaration,
            found[1].declaration
        );
    }

    #[test]
    fn rejects_capture_and_nested_collisions() {
        let capture = parsed("$\\sum_{i=1}^n (x_i + j)$");
        let found = binders(&capture);
        assert!(rename_rejection(&capture, &found, &found[0], "j").is_some());

        let nested = parsed("$\\sum_{i=1}^n (x_i + \\sum_{j=1}^m a_i)$");
        let found = binders(&nested);
        assert!(rename_rejection(&nested, &found, &found[0], "j").is_some());
    }

    #[test]
    fn bounds_a_sum_to_its_next_structural_atom() {
        let source = "$\\sum_{i=1}^n x_i + z_i$";
        let expression = parsed(source);
        let found = binders(&expression);
        let occurrences = bound_occurrences(&expression, &found, &found[0]);
        assert_eq!(occurrences.len(), 2);
        assert!(
            occurrences
                .iter()
                .all(|range| { range.start_offset < source.find("z_i").unwrap() as u32 })
        );

        let no_body = parsed("$\\sum_{i=1}^n$");
        assert!(binders(&no_body).is_empty());
    }
}
