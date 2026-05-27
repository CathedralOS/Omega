use omega_typed_trees::expression::{ExpressionHandle, TableRangeExpression};

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;

pub(super) fn unknown_length_index_is_proven(
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => range_bound_is_proven(program, facts, &collection_label, range.start),
        (false, true) => range_end_is_proven(program, facts, &collection_label, range.end),
        (true, true) => {
            let start_label = program.expression_table.display_name(range.start);
            let end_label = program.expression_table.display_name(range.end);
            let start_is_at_most_end = expression_integer_value(program, facts, range.start)
                .is_some_and(|start| start == 0)
                || facts.at_most_is_proven(&start_label, &end_label);

            start_is_at_most_end
                && range_end_is_proven(program, facts, &collection_label, range.end)
        }
        (false, false) => true,
    }
}

fn range_end_is_proven(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection_label: &str,
    end: ExpressionHandle,
) -> bool {
    range_bound_is_proven(program, facts, collection_label, end)
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
