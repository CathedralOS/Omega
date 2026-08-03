use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::super::expressions::expression_integer_value;
use super::super::super::facts::RangeFacts;

pub(in crate::checks::ranges::guards) fn seed_length_greater_than_fact(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_lower_bound: ExpressionHandle,
) {
    let Some(lower_bound) = expression_integer_value(program, facts, possible_lower_bound) else {
        return;
    };
    seed_minimum_length_fact(program, facts, possible_length, lower_bound + 1);
}

pub(in crate::checks::ranges::guards) fn seed_length_at_least_fact(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_lower_bound: ExpressionHandle,
) {
    let Some(lower_bound) = expression_integer_value(program, facts, possible_lower_bound) else {
        return;
    };
    seed_minimum_length_fact(program, facts, possible_length, lower_bound);
}

pub(in crate::checks::ranges::guards) fn seed_length_not_zero_fact(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_zero: ExpressionHandle,
) {
    if expression_integer_value(program, facts, possible_zero) != Some(0) {
        return;
    }
    seed_minimum_length_fact(program, facts, possible_length, 1);
}

pub(in crate::checks::ranges::guards) fn seed_length_equality_fact(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) {
    if seed_length_equality_side(program, facts, left, right) {
        return;
    }
    seed_length_equality_side(program, facts, right, left);
}

fn seed_minimum_length_fact(
    program: &psi_typed_trees::TypedTrees,
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

fn seed_length_equality_side(
    program: &psi_typed_trees::TypedTrees,
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
