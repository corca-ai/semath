use crate::SourceRange;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CursorOccurrence<'a> {
    pub occurrence: &'a SourceRange,
    pub selection: &'a SourceRange,
    pub application_end: Option<u32>,
}

/// Selects the single semantic occurrence that owns a UTF-16 cursor offset.
/// Exact containment always outranks a left-hand trailing edge, and narrower
/// real-source occurrences outrank their structural selection containers.
pub(crate) fn occurrence_at_cursor(
    occurrences: &[CursorOccurrence<'_>],
    offset: u32,
) -> Option<usize> {
    let eligible = occurrences
        .iter()
        .enumerate()
        .filter_map(|(index, occurrence)| {
            ownership_priority(*occurrence, offset).map(|priority| (index, priority))
        })
        .collect::<Vec<_>>();
    let shadowed_by_nested_trailing_edge = |index: usize, priority: u8| {
        priority == 0
            && eligible.iter().any(|(child_index, child_priority)| {
                *child_priority == 1
                    && *child_index != index
                    && strictly_contains(
                        occurrences[index].occurrence,
                        occurrences[*child_index].occurrence,
                    )
            })
    };
    let best_priority = eligible
        .iter()
        .filter(|(index, priority)| !shadowed_by_nested_trailing_edge(*index, *priority))
        .map(|(_, priority)| *priority)
        .min()?;
    let mut candidates = eligible
        .iter()
        .filter(|(index, priority)| {
            *priority == best_priority && !shadowed_by_nested_trailing_edge(*index, *priority)
        })
        .map(|(index, _)| (*index, &occurrences[*index]))
        .collect::<Vec<_>>();
    candidates.sort_by(|(_, left), (_, right)| {
        let left_occurrence_width = left.occurrence.end_offset - left.occurrence.start_offset;
        let right_occurrence_width = right.occurrence.end_offset - right.occurrence.start_offset;
        let occurrence_order = if best_priority == 1 {
            right_occurrence_width.cmp(&left_occurrence_width)
        } else {
            left_occurrence_width.cmp(&right_occurrence_width)
        };
        occurrence_order
            .then_with(|| {
                let left_selection_width = left.selection.end_offset - left.selection.start_offset;
                let right_selection_width =
                    right.selection.end_offset - right.selection.start_offset;
                left_selection_width.cmp(&right_selection_width)
            })
            .then_with(|| {
                left.selection
                    .start_offset
                    .cmp(&right.selection.start_offset)
            })
    });
    let (selected_index, selected) = *candidates.first()?;
    if candidates.get(1).is_some_and(|(_, next)| {
        next.occurrence == selected.occurrence && next.selection != selected.selection
    }) {
        return None;
    }
    Some(selected_index)
}

fn strictly_contains(container: &SourceRange, child: &SourceRange) -> bool {
    container.start_offset <= child.start_offset
        && child.end_offset.saturating_add(1) < container.end_offset
}

fn ownership_priority(occurrence: CursorOccurrence<'_>, offset: u32) -> Option<u8> {
    if occurrence.occurrence.contains(offset) {
        Some(0)
    } else if nonempty_trailing_edge(occurrence.occurrence, offset) {
        Some(1)
    } else if occurrence.selection.contains(offset) {
        Some(2)
    } else if nonempty_trailing_edge(occurrence.selection, offset) {
        Some(3)
    } else if occurrence.application_end == Some(offset) {
        Some(4)
    } else {
        None
    }
}

fn nonempty_trailing_edge(range: &SourceRange, offset: u32) -> bool {
    range.start_offset < range.end_offset && range.end_offset == offset
}

pub(crate) fn interior_offset(range: &SourceRange, cursor_offset: u32) -> u32 {
    if range.start_offset >= range.end_offset {
        return cursor_offset;
    }
    cursor_offset
        .max(range.start_offset)
        .min(range.end_offset - 1)
}

#[cfg(test)]
mod tests {
    use super::{CursorOccurrence, occurrence_at_cursor};
    use crate::SourceRange;

    fn range(start_offset: u32, end_offset: u32) -> SourceRange {
        SourceRange {
            start_offset,
            end_offset,
        }
    }

    #[test]
    fn occurrence_ownership_is_invariant_across_nucleus_and_structural_edges() {
        let nucleus = range(6, 7);
        let decorated = range(1, 8);
        let occurrences = [CursorOccurrence {
            occurrence: &nucleus,
            selection: &decorated,
            application_end: Some(11),
        }];
        for offset in [1, 6, 7, 8, 11] {
            assert_eq!(occurrence_at_cursor(&occurrences, offset), Some(0));
        }
    }

    #[test]
    fn exact_right_occurrence_beats_the_left_trailing_edge() {
        let left = range(1, 2);
        let right = range(2, 3);
        let occurrences = [
            CursorOccurrence {
                occurrence: &left,
                selection: &left,
                application_end: None,
            },
            CursorOccurrence {
                occurrence: &right,
                selection: &right,
                application_end: None,
            },
        ];
        assert_eq!(occurrence_at_cursor(&occurrences, 2), Some(1));
    }

    #[test]
    fn complete_notation_owns_a_shared_trailing_edge() {
        let base = range(1, 2);
        let index = range(3, 4);
        let indexed = range(1, 4);
        let occurrences = [
            CursorOccurrence {
                occurrence: &indexed,
                selection: &base,
                application_end: None,
            },
            CursorOccurrence {
                occurrence: &index,
                selection: &index,
                application_end: None,
            },
        ];
        assert_eq!(occurrence_at_cursor(&occurrences, 3), Some(1));
        assert_eq!(occurrence_at_cursor(&occurrences, 4), Some(0));
    }

    #[test]
    fn nested_atomic_trailing_edge_outranks_its_structural_container() {
        let composite = range(1, 10);
        let variable = range(4, 5);
        let occurrences = [
            CursorOccurrence {
                occurrence: &composite,
                selection: &composite,
                application_end: None,
            },
            CursorOccurrence {
                occurrence: &variable,
                selection: &variable,
                application_end: None,
            },
        ];
        assert_eq!(occurrence_at_cursor(&occurrences, 5), Some(1));

        let braced_container = range(1, 6);
        let final_index = range(4, 5);
        let braced = [
            CursorOccurrence {
                occurrence: &braced_container,
                selection: &braced_container,
                application_end: None,
            },
            CursorOccurrence {
                occurrence: &final_index,
                selection: &final_index,
                application_end: None,
            },
        ];
        assert_eq!(occurrence_at_cursor(&braced, 5), Some(0));
    }

    #[test]
    fn ownership_does_not_cross_whitespace_or_choose_ambiguous_containers() {
        let left = range(1, 2);
        let left_selection = range(0, 4);
        let duplicate_selection = range(0, 5);
        let occurrences = [
            CursorOccurrence {
                occurrence: &left,
                selection: &left_selection,
                application_end: None,
            },
            CursorOccurrence {
                occurrence: &left,
                selection: &duplicate_selection,
                application_end: None,
            },
        ];
        assert!(occurrence_at_cursor(&occurrences, 3).is_none());
        assert!(occurrence_at_cursor(&occurrences, 6).is_none());
    }
}
