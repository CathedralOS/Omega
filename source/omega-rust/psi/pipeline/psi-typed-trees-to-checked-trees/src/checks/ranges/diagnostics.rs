use psi_typed_trees::expression::{ExpressionHandle, TableRangeExpression};

use super::expressions::{expression_integer_value, normalize_exclusive_end};
use super::facts::RangeFacts;

#[derive(Clone, Copy)]
pub(super) enum SubsliceRangeFailure {
    StartBound,
    EndBound,
    Ordering,
    Bounds,
    /// An inclusive range `a..=b` whose normalized exclusive end `b + 1`
    /// overflows (the `..=` overflow edge, e.g. `..=usize::MAX`).
    InclusiveOverflow,
}

impl SubsliceRangeFailure {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::StartBound => "start bound",
            Self::EndBound => "end bound",
            Self::Ordering => "ordering",
            Self::Bounds => "bounds",
            Self::InclusiveOverflow => "inclusive end overflow",
        }
    }
}

pub(super) fn known_length_range_value_failure(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> SubsliceRangeFailure {
    if range.start.is_valid() && expression_integer_value(program, facts, range.start).is_none() {
        return SubsliceRangeFailure::StartBound;
    }
    if range.end.is_valid() {
        match expression_integer_value(program, facts, range.end) {
            None => return SubsliceRangeFailure::EndBound,
            // The end value is known, so the only way `provable_range_bounds`
            // can fail on the end is the inclusive `b + 1` overflow edge.
            Some(end) if normalize_exclusive_end(end, range.end_inclusive).is_none() => {
                return SubsliceRangeFailure::InclusiveOverflow;
            }
            Some(_) => {}
        }
    }
    SubsliceRangeFailure::Bounds
}

pub(super) fn known_length_range_bound_failure(
    start: i64,
    end: Option<i64>,
    length: usize,
) -> Option<SubsliceRangeFailure> {
    if start < 0 || usize::try_from(start).map_or(true, |start| start > length) {
        return Some(SubsliceRangeFailure::StartBound);
    }

    let end = end?;
    if end < 0 || usize::try_from(end).map_or(true, |end| end > length) {
        return Some(SubsliceRangeFailure::EndBound);
    }
    if start > end {
        return Some(SubsliceRangeFailure::Ordering);
    }
    None
}

pub(super) fn unknown_length_range_failure(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
) -> SubsliceRangeFailure {
    let collection_label = program.expression_table.display_name(collection);
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => {
            if !range_bound_is_proven(program, facts, &collection_label, range.start) {
                SubsliceRangeFailure::StartBound
            } else {
                SubsliceRangeFailure::Bounds
            }
        }
        (false, true) => {
            if !range_end_is_proven(
                program,
                facts,
                &collection_label,
                range.end,
                range.end_inclusive,
            ) {
                SubsliceRangeFailure::EndBound
            } else {
                SubsliceRangeFailure::Bounds
            }
        }
        (true, true) => {
            if !range_end_is_proven(
                program,
                facts,
                &collection_label,
                range.end,
                range.end_inclusive,
            ) {
                return SubsliceRangeFailure::EndBound;
            }

            let start_label = program.expression_table.display_name(range.start);
            let end_label = program.expression_table.display_name(range.end);
            if expression_integer_value(program, facts, range.start) != Some(0)
                && !facts.at_most_is_proven(&start_label, &end_label)
            {
                SubsliceRangeFailure::Ordering
            } else {
                SubsliceRangeFailure::Bounds
            }
        }
        (false, false) => SubsliceRangeFailure::Bounds,
    }
}

fn range_bound_is_proven(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    bound: ExpressionHandle,
) -> bool {
    let bound_label = program.expression_table.display_name(bound);
    facts.range_bound_is_proven(collection_label, &bound_label)
        || expression_integer_value(program, facts, bound)
            .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
}

/// Mirrors `proofs::range_end_within_unknown_length_is_proven` for diagnostic
/// classification: exclusive ends use range-bound vocabulary (`b <= len`),
/// inclusive ends use the strict index vocabulary (`b < len`).
fn range_end_is_proven(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    end: ExpressionHandle,
    end_inclusive: bool,
) -> bool {
    if end_inclusive {
        let end_label = program.expression_table.display_name(end);
        facts.index_is_proven(collection_label, &end_label)
            || expression_integer_value(program, facts, end)
                .is_some_and(|end| facts.index_value_is_proven(collection_label, end))
    } else {
        range_bound_is_proven(program, facts, collection_label, end)
    }
}
