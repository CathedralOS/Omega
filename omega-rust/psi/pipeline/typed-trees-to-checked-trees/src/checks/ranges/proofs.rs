use typed_trees::expression::{ExpressionHandle, TableRangeExpression};

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;

/// Proves that a range's end bound is within an unknown slice length.
///
/// Exclusive ends use the range-bound vocabulary (`b <= len`); inclusive ends
/// use the strict index vocabulary (`b < len`). This is what connects range
/// validity to index validity: an inclusive subslice `a..=b` is valid exactly
/// when `b` is a valid index, so it reuses the same index proofs rather than
/// duplicating bound logic.
fn range_end_within_unknown_length_is_proven(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    end: ExpressionHandle,
    end_inclusive: bool,
) -> bool {
    if end_inclusive {
        index_is_within_unknown_length_proven(program, facts, collection_label, end)
    } else {
        range_bound_is_proven(program, facts, collection_label, end)
    }
}

/// Proves an index expression is `< len` for an unknown slice length using the
/// index vocabulary (`prove_index` / `index_value_is_proven`).
fn index_is_within_unknown_length_proven(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    index: ExpressionHandle,
) -> bool {
    let index_label = program.expression_table.display_name(index);
    facts.index_is_proven(collection_label, &index_label)
        || expression_integer_value(program, facts, index)
            .is_some_and(|index| facts.index_value_is_proven(collection_label, index))
}

pub(super) fn unknown_length_index_is_proven(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    let index_label = program.expression_table.display_name(index);
    facts.index_is_proven(&collection_label, &index_label)
        || expression_integer_value(program, facts, index)
            .is_some_and(|index| facts.index_value_is_proven(&collection_label, index))
}

pub(super) fn unknown_length_range_is_proven(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => range_bound_is_proven(program, facts, &collection_label, range.start),
        (false, true) => range_end_within_unknown_length_is_proven(
            program,
            facts,
            &collection_label,
            range.end,
            range.end_inclusive,
        ),
        (true, true) => {
            let start_label = program.expression_table.display_name(range.start);
            let end_label = program.expression_table.display_name(range.end);
            // The ordering obligation `start <= end` holds when the start is
            // the trivial zero, when both bounds fold to constants that
            // compare, or when the ordering was established as a carried fact.
            let folded_start = expression_integer_value(program, facts, range.start);
            let folded_end = expression_integer_value(program, facts, range.end);
            let start_is_at_most_end = folded_start.is_some_and(|start| start == 0)
                || folded_start
                    .zip(folded_end)
                    .is_some_and(|(start, end)| start <= end)
                || facts.at_most_is_proven(&start_label, &end_label);

            start_is_at_most_end
                && range_end_within_unknown_length_is_proven(
                    program,
                    facts,
                    &collection_label,
                    range.end,
                    range.end_inclusive,
                )
        }
        (false, false) => true,
    }
}

fn range_bound_is_proven(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    bound: ExpressionHandle,
) -> bool {
    let bound_label = program.expression_table.display_name(bound);
    facts.range_bound_is_proven(collection_label, &bound_label)
        || expression_integer_value(program, facts, bound)
            .is_some_and(|bound| facts.range_bound_value_is_proven(collection_label, bound))
}
