use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::super::expressions::expression_integer_value;
use super::super::super::facts::RangeFacts;

pub(in crate::checks::ranges::guards) fn seed_less_than_len_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let ExpressionNode::Member(member) = program.expression_table.expression(upper_bound) else {
        return;
    };
    if member.member.as_str() != "len" {
        return;
    }

    facts.prove_index(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(index),
    );
    facts.prove_range_bound(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(index),
    );
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
    if member.member.as_str() != "len" {
        return;
    }

    facts.prove_range_bound(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(bound),
    );
}

pub(in crate::checks::ranges::guards) fn seed_successor_at_most_len_fact(
    program: &typed_trees::TypedTrees,
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
    if member.member.as_str() != "len" {
        return;
    }

    facts.prove_index(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(index),
    );
    facts.prove_range_bound(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(index),
    );
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
