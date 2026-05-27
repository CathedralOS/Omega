use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::expressions::expression_integer_value;
use super::facts::RangeFacts;

pub(super) fn seed_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    if !guard.is_valid() {
        return;
    }

    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };

    match binary.operator {
        BinaryOperator::Less => {
            seed_length_greater_than_fact(program, facts, binary.right, binary.left);
            seed_less_than_len_fact(program, facts, binary.left, binary.right);
            seed_at_most_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::Greater => {
            seed_length_greater_than_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::LessOrEqual => {
            seed_length_at_least_fact(program, facts, binary.right, binary.left);
            seed_at_most_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::GreaterOrEqual => {
            seed_length_at_least_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::And => {
            seed_guard_facts(program, facts, binary.left);
            seed_guard_facts(program, facts, binary.right);
        }
        BinaryOperator::Equal => {
            seed_length_equality_fact(program, facts, binary.left, binary.right);
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                seed_guard_facts(program, facts, binary.left);
            }
        }
        BinaryOperator::NotEqual => {
            seed_length_not_zero_fact(program, facts, binary.left, binary.right);
            seed_length_not_zero_fact(program, facts, binary.right, binary.left);
        }
        BinaryOperator::Add
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }
}

fn seed_length_greater_than_fact(
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

fn seed_length_at_least_fact(
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

fn seed_length_not_zero_fact(
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

fn seed_less_than_len_fact(
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

fn seed_length_equality_fact(
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

fn seed_at_most_fact(
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
