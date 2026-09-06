//! Numeric bounds and live orderings for windows with a known collection extent.

use super::*;
use crate::checks::ranges::expressions::normalize_exclusive_end;

/// Prove the upper bound and ordering only. The caller independently requires
/// nonnegative endpoints through `lower_bounds::prove` before admitting a window.
pub(super) fn prove(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
    length: usize,
) -> bool {
    let Ok(length) = i64::try_from(length) else {
        return false;
    };
    let start = if range.start.is_valid() {
        expression_integer_value(program, facts, range.start)
    } else {
        Some(0)
    };
    let end = if !range.end.is_valid() {
        Some(length)
    } else if let Some(end) = expression_integer_value(program, facts, range.end) {
        let Some(end) = normalize_exclusive_end(end, range.end_inclusive) else {
            return false;
        };
        Some(end)
    } else {
        None
    };

    if let Some(end) = end {
        if end < 0 || end > length {
            return false;
        }
        return match start {
            Some(start) => start >= 0 && start <= end,
            None => at_most_constant(program, machine, state, facts, range.start, end),
        };
    }

    // A symbolic inclusive end must be strictly below the collection length.
    let end_limit = if range.end_inclusive {
        length - 1
    } else {
        length
    };
    if !at_most_constant(program, machine, state, facts, range.end, end_limit) {
        return false;
    }
    // Zero precedes a separately proven nonnegative end. Otherwise retain an
    // explicit live ordering; two independently bounded endpoints can reverse.
    start == Some(0)
        || facts.at_most_is_proven(
            &program.expression_table.display_name(range.start),
            &program.expression_table.display_name(range.end),
        )
}

fn at_most_constant(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    limit: i64,
) -> bool {
    if let Some(value) = expression_integer_value(program, facts, expression) {
        return value <= limit;
    }
    if expression_enforced_declared_range(program, machine, state, expression)
        .is_some_and(|(_, maximum)| maximum <= limit)
    {
        return true;
    }
    let Some(exclusive) = limit
        .checked_add(1)
        .and_then(|limit| usize::try_from(limit).ok())
    else {
        return false;
    };
    let label = program.expression_table.display_name(expression);
    facts.index_upper_bound_is_proven(&label, exclusive)
        || facts.index_upper_bound_is_proven_via_ordering(&label, exclusive)
}
