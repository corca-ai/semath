use crate::SourceRange;

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
    use super::{interior_offset, item_at_cursor_with_trailing_edge};
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
}
