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
    let best_priority = occurrences
        .iter()
        .filter_map(|occurrence| ownership_priority(*occurrence, offset))
        .min()?;
    let mut candidates = occurrences
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| ownership_priority(**occurrence, offset) == Some(best_priority))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(_, occurrence)| {
        (
            occurrence.occurrence.end_offset - occurrence.occurrence.start_offset,
            occurrence.selection.end_offset - occurrence.selection.start_offset,
            occurrence.selection.start_offset,
        )
    });
    let (selected_index, selected) = *candidates.first()?;
    if candidates.get(1).is_some_and(|(_, next)| {
        next.occurrence == selected.occurrence && next.selection != selected.selection
    }) {
        return None;
    }
    Some(selected_index)
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

pub(crate) fn item_at_cursor<T>(
    items: &[(T, SourceRange)],
    offset: u32,
) -> Option<(&T, &SourceRange)> {
    let mut containing = items.iter().filter(|(_, range)| range.contains(offset));
    let selected = containing.next()?;
    if containing.next().is_some() {
        return None;
    }
    Some((&selected.0, &selected.1))
}

pub(crate) fn item_at_cursor_with_trailing_edge<T>(
    items: &[(T, SourceRange)],
    offset: u32,
) -> Option<(&T, &SourceRange)> {
    item_at_cursor(items, offset).or_else(|| {
        let mut trailing = items.iter().filter(|(_, range)| {
            range.start_offset < range.end_offset && range.end_offset == offset
        });
        let selected = trailing.next()?;
        if trailing.next().is_some() {
            return None;
        }
        Some((&selected.0, &selected.1))
    })
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
    use super::{
        CursorOccurrence, interior_offset, item_at_cursor_with_trailing_edge, occurrence_at_cursor,
    };
    use crate::SourceRange;

    fn range(start_offset: u32, end_offset: u32) -> SourceRange {
        SourceRange {
            start_offset,
            end_offset,
        }
    }

    #[test]
    fn accepts_start_interior_and_trailing_edge_for_nonempty_ranges() {
        for start in 0..32 {
            for width in 1..8 {
                let end = start + width;
                let items = [("target", range(start, end))];
                for offset in start..=end {
                    let (_, selected) = item_at_cursor_with_trailing_edge(&items, offset).unwrap();
                    assert_eq!(selected, &range(start, end));
                    assert!(selected.contains(interior_offset(selected, offset)));
                }
            }
        }
    }

    #[test]
    fn exact_start_wins_over_the_left_trailing_edge() {
        let items = [("left", range(2, 3)), ("right", range(3, 5))];
        let (item, selected) = item_at_cursor_with_trailing_edge(&items, 3).unwrap();
        assert_eq!(*item, "right");
        assert_eq!(selected, &range(3, 5));
    }

    #[test]
    fn refuses_ambiguous_overlaps_and_duplicate_trailing_edges() {
        assert!(
            item_at_cursor_with_trailing_edge(
                &[("wide", range(1, 5)), ("nested", range(2, 4))],
                3,
            )
            .is_none()
        );
        assert!(
            item_at_cursor_with_trailing_edge(
                &[("first", range(1, 4)), ("second", range(2, 4))],
                4,
            )
            .is_none()
        );
    }

    #[test]
    fn does_not_snap_across_gaps_or_to_empty_ranges() {
        let items = [("left", range(1, 2)), ("empty", range(4, 4))];
        assert!(item_at_cursor_with_trailing_edge(&items, 3).is_none());
        assert!(item_at_cursor_with_trailing_edge(&items, 4).is_none());
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
