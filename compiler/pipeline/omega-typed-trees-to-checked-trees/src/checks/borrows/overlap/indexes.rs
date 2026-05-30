use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};

pub(super) fn index_expressions_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Integer(left_value), ExpressionNode::Integer(right_value)) => {
            left_value == right_value
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Integer(right_value)) => {
            range_may_contain_integer(program, left_range, *right_value)
        }
        (ExpressionNode::Integer(left_value), ExpressionNode::Range(right_range)) => {
            range_may_contain_integer(program, right_range, *left_value)
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Range(right_range)) => {
            ranges_may_overlap(program, left_range, right_range)
        }
        _ => true,
    }
}

fn range_may_contain_integer(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
    value: i64,
) -> bool {
    let (start, end) = range_integer_bounds(program, range);
    // An empty half-open window `[a, a)` contains nothing, so it is disjoint
    // from every index even when the index itself is unknown.
    if range_is_provably_empty(start, end) {
        return false;
    }
    if start.is_some_and(|start| value < start) {
        return false;
    }
    if end.is_some_and(|end| value >= end) {
        return false;
    }
    true
}

fn ranges_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &TableRangeExpression,
    right: &TableRangeExpression,
) -> bool {
    let (left_start, left_end) = range_integer_bounds(program, left);
    let (right_start, right_end) = range_integer_bounds(program, right);

    // Either window being provably empty makes the pair disjoint regardless of
    // the other window's bounds.
    if range_is_provably_empty(left_start, left_end)
        || range_is_provably_empty(right_start, right_end)
    {
        return false;
    }

    // Two half-open windows `[ls, le)` and `[rs, re)` are disjoint when one ends
    // at or before the other starts.
    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && left_end <= right_start
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && right_end <= left_start
    {
        return false;
    }
    true
}

/// A half-open window `[start, end)` with `end <= start` is empty and therefore
/// overlaps nothing. Bounds that are not compile-time integers cannot prove
/// emptiness.
fn range_is_provably_empty(start: Option<i64>, end: Option<i64>) -> bool {
    matches!((start, end), (Some(start), Some(end)) if end <= start)
}

fn range_integer_bounds(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
) -> (Option<i64>, Option<i64>) {
    (
        integer_expression_value(program, range.start),
        exclusive_end_bound(program, range),
    )
}

/// The half-open (exclusive) upper bound of a range window.
///
/// All overlap reasoning here is in terms of half-open windows `[start, end)`.
/// An inclusive range `a..=b` covers index `b`, so its exclusive end is `b + 1`.
/// Normalizing here keeps `range_may_contain_integer`/`ranges_may_overlap` sound:
/// without it, `view[0..=3]` would be read as `[0, 3)` and a borrow of element 3
/// (or window `3..5`) would be mis-classified as disjoint.
///
/// A `b + 1` that overflows `i64` (the `..=i64::MAX` edge) cannot be represented
/// as an exclusive bound, so the end is reported as unknown (`None`), which the
/// overlap checks treat conservatively as possibly-overlapping.
fn exclusive_end_bound(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
) -> Option<i64> {
    let end = integer_expression_value(program, range.end)?;
    if range.end_inclusive {
        end.checked_add(1)
    } else {
        Some(end)
    }
}

fn integer_expression_value(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(program: &mut omega_typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Integer(value))
    }

    fn range(
        program: &mut omega_typed_trees::TypedTrees,
        start: i64,
        end: i64,
        end_inclusive: bool,
    ) -> ExpressionHandle {
        let start = integer(program, start);
        let end = integer(program, end);
        program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            }))
    }

    #[test]
    fn exclusive_range_disjoint_from_index_at_end() {
        let mut program = omega_typed_trees::TypedTrees::default();
        // `[0, 3)` does not contain index 3.
        let window = range(&mut program, 0, 3, false);
        let index = integer(&mut program, 3);
        assert!(!index_expressions_may_overlap(&program, window, index));
    }

    #[test]
    fn inclusive_range_overlaps_index_at_end() {
        let mut program = omega_typed_trees::TypedTrees::default();
        // `0..=3` covers index 3 -- must overlap (soundness).
        let window = range(&mut program, 0, 3, true);
        let index = integer(&mut program, 3);
        assert!(index_expressions_may_overlap(&program, window, index));
    }

    #[test]
    fn inclusive_range_overlaps_adjacent_window() {
        let mut program = omega_typed_trees::TypedTrees::default();
        // `0..=3` = `[0, 4)` overlaps `3..5` = `[3, 5)` at index 3.
        let left = range(&mut program, 0, 3, true);
        let right = range(&mut program, 3, 5, false);
        assert!(index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn exclusive_adjacent_windows_are_disjoint() {
        let mut program = omega_typed_trees::TypedTrees::default();
        // `[0, 3)` and `[3, 5)` share no index.
        let left = range(&mut program, 0, 3, false);
        let right = range(&mut program, 3, 5, false);
        assert!(!index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn disjoint_windows_stay_disjoint() {
        let mut program = omega_typed_trees::TypedTrees::default();
        let left = range(&mut program, 0, 2, false);
        let right = range(&mut program, 4, 8, false);
        assert!(!index_expressions_may_overlap(&program, left, right));
    }

    #[test]
    fn empty_inclusive_window_overlaps_nothing() {
        let mut program = omega_typed_trees::TypedTrees::default();
        // `2..=1` normalizes to `[2, 2)` -- empty, disjoint from index 2.
        let window = range(&mut program, 2, 1, true);
        let index = integer(&mut program, 2);
        assert!(!index_expressions_may_overlap(&program, window, index));
    }
}
