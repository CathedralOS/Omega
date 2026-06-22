mod bounds;

use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::facts::RangeFacts;
use bounds::{
    seed_at_most_fact, seed_at_most_len_range_bound_fact, seed_index_at_most_integer_fact,
    seed_index_less_than_integer_fact, seed_length_at_least_fact, seed_length_equality_fact,
    seed_length_greater_than_fact, seed_length_not_zero_fact, seed_less_than_len_fact,
    seed_successor_at_most_len_fact,
};

pub(super) fn seed_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    if !guard.is_valid() {
        return;
    }

    if let ExpressionNode::Name(path) = program.expression_table.expression(guard) {
        let name = program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str());
        if let Some(alias_guard) = facts.boolean_guard_local(path.symbol, name)
            && alias_guard != guard
        {
            seed_guard_facts(program, facts, alias_guard);
        }
        return;
    }

    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };

    match binary.operator {
        BinaryOperator::Less => {
            seed_length_greater_than_fact(program, facts, binary.right, binary.left);
            seed_less_than_len_fact(program, facts, binary.left, binary.right);
            seed_index_less_than_integer_fact(program, facts, binary.left, binary.right);
            seed_at_most_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::Greater => {
            seed_length_greater_than_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::LessOrEqual => {
            seed_length_at_least_fact(program, facts, binary.right, binary.left);
            seed_successor_at_most_len_fact(program, facts, binary.left, binary.right);
            seed_at_most_len_range_bound_fact(program, facts, binary.left, binary.right);
            seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
            seed_at_most_fact(program, facts, binary.left, binary.right);
        }
        BinaryOperator::GreaterOrEqual => {
            seed_length_at_least_fact(program, facts, binary.left, binary.right);
            seed_successor_at_most_len_fact(program, facts, binary.right, binary.left);
            seed_at_most_len_range_bound_fact(program, facts, binary.right, binary.left);
        }
        BinaryOperator::And => {
            seed_guard_facts(program, facts, binary.left);
            seed_guard_facts(program, facts, binary.right);
        }
        BinaryOperator::Equal => {
            seed_length_equality_fact(program, facts, binary.left, binary.right);
            seed_boolean_equality_guard_facts(program, facts, binary.left, binary.right);
            seed_boolean_equality_guard_facts(program, facts, binary.right, binary.left);
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

fn seed_boolean_equality_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    possible_boolean: ExpressionHandle,
    guard: ExpressionHandle,
) {
    match program.expression_table.expression(possible_boolean) {
        ExpressionNode::Boolean(true) => seed_guard_facts(program, facts, guard),
        ExpressionNode::Boolean(false) => seed_negated_guard_facts(program, facts, guard),
        _ => {}
    }
}

pub(super) fn seed_negated_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };

    if binary.operator == BinaryOperator::Equal {
        seed_length_not_zero_fact(program, facts, binary.left, binary.right);
        seed_length_not_zero_fact(program, facts, binary.right, binary.left);
    }
}
