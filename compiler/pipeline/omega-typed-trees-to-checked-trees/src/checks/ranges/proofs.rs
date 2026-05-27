use omega_typed_trees::expression::{ExpressionHandle, TableRangeExpression};

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;

pub(super) fn unknown_length_index_is_proven(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
) -> bool {
    facts.index_is_proven(
        &program.expression_table.display_name(collection),
        &program.expression_table.display_name(index),
    )
}

pub(super) fn unknown_length_range_is_proven(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    range: &TableRangeExpression,
) -> bool {
    let collection_label = program.expression_table.display_name(collection);
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, false) => facts.index_is_proven(
            &collection_label,
            &program.expression_table.display_name(range.start),
        ),
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
    let end_label = program.expression_table.display_name(end);
    facts.index_is_proven(collection_label, &end_label)
        || facts.is_length_of(&end_label, collection_label)
}
