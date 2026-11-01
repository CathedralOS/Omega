use typed_trees::expression::ExpressionHandle;

use super::super::super::expressions::expression_integer_value;
use super::super::super::facts::RangeFacts;

pub(in crate::checks::ranges::guards) fn seed_length_greater_than_fact(
    program: &dyn ::validation::CheckingInput,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    possible_lower_bound: ExpressionHandle,
) {
    let Some(lower_bound) = expression_integer_value(program, facts, possible_lower_bound) else {
        return;
    };
    seed_minimum_length_fact(
        program,
        facts,
        possible_length,
        lower_bound.saturating_add(1),
    );
}

pub(in crate::checks::ranges::guards) fn seed_length_at_least_fact(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    facts: &mut RangeFacts<'_>,
    possible_length: ExpressionHandle,
    minimum_length: i64,
) {
    let Some(collection) = facts.length_collection(program, possible_length) else {
        return;
    };
    let collection = program.expression_table.display_name(collection);
    facts.prove_minimum_length(collection, minimum_length);
}

fn seed_length_equality_side(
    program: &dyn ::validation::CheckingInput,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    possible_length: ExpressionHandle,
) -> bool {
    let Some(collection) = facts.length_collection(program, possible_length) else {
        return false;
    };
    if let Some(length) = expression_integer_value(program, facts, value) {
        facts.prove_exact_length(program.expression_table.display_name(collection), length);
    }
    facts.prove_range_bound(
        program.expression_table.display_name(collection),
        program.expression_table.display_name(value),
    );
    true
}
