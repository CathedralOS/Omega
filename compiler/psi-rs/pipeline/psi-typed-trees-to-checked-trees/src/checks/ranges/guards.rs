mod bounds;

use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::facts::RangeFacts;
use bounds::{
    seed_at_most_fact, seed_at_most_len_range_bound_fact, seed_index_at_most_integer_fact,
    seed_index_less_than_integer_fact, seed_length_at_least_fact, seed_length_equality_fact,
    seed_length_greater_than_fact, seed_length_not_zero_fact, seed_less_than_len_fact,
    seed_non_negative_fact, seed_successor_at_most_len_fact,
};

/// R1 value-vs-value endpoint mints: a guard comparing two PLACES
/// transfers the RHS's ENFORCED declared range endpoint onto the LHS
/// (`i < k` with `k: u32 [0..=8]` proves `i < 8`; `<=` shifts by one;
/// `>`/`>=` mirror). Store-enforced Exact ranges only (the resolver's
/// gate); literal RHS comparisons stay with the literal seeders. Needs
/// machine/state context, so it runs beside `seed_guard_facts` at the
/// sites that have it (the co-located transition arm).
pub(super) fn seed_value_vs_value_endpoints(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
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
        BinaryOperator::And => {
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.right);
            return;
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
            return;
        }
        _ => {}
    }
    let (bounded, bound_source, inclusive_shift) = match binary.operator {
        BinaryOperator::Less => (binary.left, binary.right, 0),
        BinaryOperator::LessOrEqual => (binary.left, binary.right, 1),
        BinaryOperator::Greater => (binary.right, binary.left, 0),
        BinaryOperator::GreaterOrEqual => (binary.right, binary.left, 1),
        _ => return,
    };
    // Literal bounds are the literal seeders' job.
    if crate::checks::ranges::expressions::expression_integer_value(program, facts, bound_source)
        .is_some()
    {
        return;
    }
    let Some((_, high)) = crate::checks::ranges::types::expression_enforced_declared_range(
        program,
        machine,
        state,
        bound_source,
    ) else {
        return;
    };
    let Some(exclusive) = high.checked_add(inclusive_shift) else {
        return;
    };
    facts.prove_index_upper_bound(program.expression_table.display_name(bounded), exclusive);
}

pub(super) fn seed_guard_facts(
    program: &psi_typed_trees::TypedTrees,
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
            // `K < right` (left a constant) floors `right` at `K + 1`.
            seed_non_negative_fact(program, facts, binary.right, binary.left, false);
        }
        BinaryOperator::Greater => {
            seed_length_greater_than_fact(program, facts, binary.left, binary.right);
            // `left > K` (right a constant) floors `left` at `K + 1`.
            seed_non_negative_fact(program, facts, binary.left, binary.right, false);
        }
        BinaryOperator::LessOrEqual => {
            seed_length_at_least_fact(program, facts, binary.right, binary.left);
            seed_successor_at_most_len_fact(program, facts, binary.left, binary.right);
            seed_at_most_len_range_bound_fact(program, facts, binary.left, binary.right);
            seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
            seed_at_most_fact(program, facts, binary.left, binary.right);
            // `K <= right` (left a constant) floors `right` at `K`.
            seed_non_negative_fact(program, facts, binary.right, binary.left, true);
        }
        BinaryOperator::GreaterOrEqual => {
            seed_length_at_least_fact(program, facts, binary.left, binary.right);
            seed_successor_at_most_len_fact(program, facts, binary.right, binary.left);
            seed_at_most_len_range_bound_fact(program, facts, binary.right, binary.left);
            // `left >= K` (right a constant) floors `left` at `K`.
            seed_non_negative_fact(program, facts, binary.left, binary.right, true);
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
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
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
    program: &psi_typed_trees::TypedTrees,
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
    program: &psi_typed_trees::TypedTrees,
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
