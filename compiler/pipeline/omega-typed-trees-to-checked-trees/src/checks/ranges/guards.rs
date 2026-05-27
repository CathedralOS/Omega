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
            seed_non_empty_length_fact(program, facts, binary.right, binary.left);
            seed_less_than_len_fact(program, facts, binary.left, binary.right);
            seed_at_most_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::Greater => {
            seed_non_empty_length_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::LessOrEqual => {
            seed_at_most_fact(program, facts, binary.left, binary.right);
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
        BinaryOperator::Add
        | BinaryOperator::Divide
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }
}

fn seed_non_empty_length_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_zero: ExpressionHandle,
) {
    if expression_integer_value(program, facts, possible_zero) != Some(0) {
        return;
    }

    let ExpressionNode::Member(member) = program.expression_table.expression(possible_length)
    else {
        return;
    };
    if member.member.as_str() != "len" {
        return;
    }

    let collection = program.expression_table.display_name(member.receiver);
    facts.prove_index(collection.clone(), "0".to_string());
    facts.prove_range_bound(collection, "1".to_string());
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
