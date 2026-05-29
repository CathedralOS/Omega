use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::expressions::expression_integer_value;
use super::super::facts::RangeFacts;

pub(super) fn seed_length_greater_than_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_lower_bound: ExpressionHandle,
) {
    let Some(lower_bound) = expression_integer_value(program, facts, possible_lower_bound) else {
        return;
    };
    seed_minimum_length_fact(program, facts, possible_length, lower_bound + 1);
}

pub(super) fn seed_length_at_least_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_lower_bound: ExpressionHandle,
) {
    let Some(lower_bound) = expression_integer_value(program, facts, possible_lower_bound) else {
        return;
    };
    seed_minimum_length_fact(program, facts, possible_length, lower_bound);
}

pub(super) fn seed_length_not_zero_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_zero: ExpressionHandle,
) {
    if expression_integer_value(program, facts, possible_zero) != Some(0) {
        return;
    }
    seed_minimum_length_fact(program, facts, possible_length, 1);
}

pub(super) fn seed_less_than_len_fact(
    program: &omega_typed_trees::TypedTrees,
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

pub(super) fn seed_successor_at_most_len_fact(
    program: &omega_typed_trees::TypedTrees,
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

pub(super) fn seed_length_equality_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) {
    if seed_length_equality_side(program, facts, left, right) {
        return;
    }
    seed_length_equality_side(program, facts, right, left);
}

pub(super) fn seed_at_most_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    lower: ExpressionHandle,
    upper: ExpressionHandle,
) {
    facts.prove_at_most(
        program.expression_table.display_name(lower),
        program.expression_table.display_name(upper),
    );
}

pub(super) fn seed_index_less_than_integer_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    index: ExpressionHandle,
    upper_bound: ExpressionHandle,
) {
    let Some(upper_bound) = expression_integer_value(program, facts, upper_bound) else {
        return;
    };
    facts.prove_index_upper_bound(program.expression_table.display_name(index), upper_bound);
}

pub(super) fn seed_index_at_most_integer_fact(
    program: &omega_typed_trees::TypedTrees,
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

fn seed_minimum_length_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    minimum_length: i64,
) {
    let ExpressionNode::Member(member) = program.expression_table.expression(possible_length)
    else {
        return;
    };
    if member.member.as_str() != "len" {
        return;
    }

    let collection = program.expression_table.display_name(member.receiver);
    facts.prove_minimum_length(collection, minimum_length);
}

fn positive_offset_base(
    program: &omega_typed_trees::TypedTrees,
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

fn seed_length_equality_side(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    possible_length: ExpressionHandle,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(possible_length)
    else {
        return false;
    };
    if member.member.as_str() != "len" {
        return false;
    }

    facts.prove_range_bound(
        program.expression_table.display_name(member.receiver),
        program.expression_table.display_name(value),
    );
    true
}
