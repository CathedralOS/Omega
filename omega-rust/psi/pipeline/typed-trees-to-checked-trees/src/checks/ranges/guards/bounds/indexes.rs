use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::super::expressions::expression_integer_value;
use super::super::super::facts::RangeFacts;
use super::super::super::types::expression_is_unsigned_integer;

pub(in crate::checks::ranges::guards) fn seed_less_than_len_fact(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let ExpressionNode::Member(member) = program.expression_table.expression(upper_bound) else {
        return;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return;
    }

    let collection_label = program.expression_table.display_name(member.receiver);
    facts.prove_index(
        collection_label.clone(),
        program.expression_table.display_name(index),
    );
    facts.prove_range_bound(
        collection_label.clone(),
        program.expression_table.display_name(index),
    );
    seed_index_length_floor(program, machine, state, facts, index, &collection_label);
}

/// A proven `index < len` floors the collection at the index's own lower
/// bound plus one: a literal index pins it exactly (`3 < len` ⇒ `len >= 4`),
/// an unsigned or proven-non-negative index gives `len >= 1`. The floor is
/// what a later literal-`0` element read (`self.slice[0]` as a comparison
/// side) consults through `index_value_is_proven`.
fn seed_index_length_floor(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    collection_label: &str,
) {
    let lower = expression_integer_value(program, facts, index)
        .filter(|value| *value >= 0)
        .or_else(|| {
            (expression_is_unsigned_integer(program, machine, state, index)
                || facts.non_negative_is_proven(&program.expression_table.display_name(index)))
            .then_some(0)
        });
    if let Some(lower) = lower {
        facts.prove_minimum_length(collection_label.to_owned(), lower + 1);
    }
}

/// Seeds the end-window fact for `bound <= coll.len`: an exclusive subslice
/// end is valid exactly when it is at most the collection length, so the
/// at-most-len comparison IS the `[..bound]` range-bound obligation. This is
/// the `<=` counterpart of the strict `bound < coll.len` seeding in
/// `seed_less_than_len_fact` (which additionally proves `bound` as an index).
pub(in crate::checks::ranges::guards) fn seed_at_most_len_range_bound_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    bound: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let ExpressionNode::Member(member) = program.expression_table.expression(upper_bound) else {
        return;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return;
    }

    facts.prove_range_bound(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(bound),
    );
}

pub(in crate::checks::ranges::guards) fn seed_successor_at_most_len_fact(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    possible_successor: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let Some(index) = positive_offset_base(program, facts, possible_successor) else {
        return;
    };

    let ExpressionNode::Member(member) = program.expression_table.expression(upper_bound) else {
        return;
    };
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
    {
        return;
    }

    let collection_label = program.expression_table.display_name(member.receiver);
    facts.prove_index(
        collection_label.clone(),
        program.expression_table.display_name(index),
    );
    facts.prove_range_bound(
        collection_label.clone(),
        program.expression_table.display_name(index),
    );
    seed_index_length_floor(program, machine, state, facts, index, &collection_label);
}

pub(in crate::checks::ranges::guards) fn seed_index_less_than_integer_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let Some(upper_bound) = expression_integer_value(program, facts, upper_bound) else {
        return;
    };
    facts.prove_index_upper_bound(program.expression_table.display_name(index), upper_bound);
}

pub(in crate::checks::ranges::guards) fn seed_index_at_most_integer_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let Some(upper_bound) = expression_integer_value(program, facts, upper_bound) else {
        return;
    };
    let Some(exclusive_upper_bound) = upper_bound.checked_add(1) else {
        return;
    };
    facts.prove_index_upper_bound(
        program.expression_table.display_name(index),
        exclusive_upper_bound,
    );
}

fn positive_offset_base(
    program: &typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Add {
        return None;
    }

    if expression_integer_value(program, facts, binary.right).is_some_and(|offset| offset > 0) {
        return Some(binary.left);
    }
    if expression_integer_value(program, facts, binary.left).is_some_and(|offset| offset > 0) {
        return Some(binary.right);
    }
    None
}
