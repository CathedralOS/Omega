use omega_typed_trees::expression::{ExpressionHandle, TableRangeExpression};

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;

#[derive(Clone, Copy)]
pub(super) enum SubsliceRangeFailure {
    StartBound,
    EndBound,
    Ordering,
    Bounds,
}

impl SubsliceRangeFailure {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::StartBound => "start bound",
            Self::EndBound => "end bound",
            Self::Ordering => "ordering",
            Self::Bounds => "bounds",
        }
    }
}

pub(super) fn known_length_range_value_failure(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> SubsliceRangeFailure {
    if range.start.is_valid() && expression_integer_value(program, facts, range.start).is_none() {
        return SubsliceRangeFailure::StartBound;
    }
    if range.end.is_valid() && expression_integer_value(program, facts, range.end).is_none() {
        return SubsliceRangeFailure::EndBound;
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

    let Some(end) = end else {
        return None;
    };
    if end < 0 || usize::try_from(end).map_or(true, |end| end > length) {
        return Some(SubsliceRangeFailure::EndBound);
    }
    if start > end {
        return Some(SubsliceRangeFailure::Ordering);
    }
    None
}

pub(super) fn unknown_length_range_failure(
    program: &omega_typed_trees::TypedTrees,
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
            if !range_bound_is_proven(program, facts, &collection_label, range.end) {
                SubsliceRangeFailure::EndBound
            } else {
                SubsliceRangeFailure::Bounds
            }
        }
        (true, true) => {
            if !range_bound_is_proven(program, facts, &collection_label, range.end) {
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
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    bound: ExpressionHandle,
) -> bool {
    let bound_label = program.expression_table.display_name(bound);
    facts.range_bound_is_proven(collection_label, &bound_label)
        || expression_integer_value(program, facts, bound)
            .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
}
