//! An upper collection-relative fact is not evidence of non-negativity.
use super::{ExpressionHandle, Machine, OperatorSpelling, State, TableIndexedExpression};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::expressions::{ensured_call_result_bounds, expression_integer_value};
use crate::checks::ranges::types::{
    expression_enforced_declared_range, expression_is_unsigned_integer,
};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(super) fn prove(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    indexed: &TableIndexedExpression,
    spelling: OperatorSpelling,
    positions: u8,
) -> bool {
    let operands = match (spelling, program.expression_table.expression(indexed.index)) {
        (OperatorSpelling::Range, ExpressionNode::Range(range)) => [range.start, range.end],
        (OperatorSpelling::Index, _) => [indexed.index, ExpressionHandle::invalid()],
        _ => return false,
    };
    let non_negative = operands.map(|expression| {
        if !expression.is_valid() {
            // Omitted range endpoints mean zero and the collection length.
            // Neither default can introduce a negative operand.
            return spelling == OperatorSpelling::Range;
        }
        if let Some(value) = expression_integer_value(program, facts, expression) {
            return value >= 0;
        }
        let label = program.expression_table.display_name(expression);
        validation::collection_length_receiver(program, machine, Some(state), expression).is_some()
            || crate::checks::ranges::proofs::length_difference_is_within_collection(
                program,
                machine,
                state,
                facts,
                indexed.collection,
                expression,
            )
            || expression_is_unsigned_integer(program, machine, state, expression)
            || expression_enforced_declared_range(program, machine, state, expression)
                .is_some_and(|(minimum, _)| minimum >= 0)
            // A call result's ensured `result >= K` conjunct is discharged at
            // every callee exit, so a non-negative `K` supplies the lower half
            // a signed result still owes — the same contract the known-length
            // route reads for its call indexes.
            || ensured_call_result_bounds(program, expression)
                .and_then(|(low, _)| low)
                .is_some_and(|low| low >= 0)
            || facts.non_negative_is_proven(&label)
            || facts.non_negative_is_proven_via_ordering(&label)
    });
    operands
        .into_iter()
        .enumerate()
        .all(|(ordinal, expression)| {
            if positions & (4 << ordinal) == 0 || non_negative[ordinal] {
                return true;
            }
            // A declared or unsigned start can supply the lower half for a signed
            // end, but only through an independently retained, live ordering.
            // Keep this inference local: do not mutate the caller's proof facts or
            // override a negative constant with contradictory contextual evidence.
            ordinal == 1
                && operands[0].is_valid()
                && expression.is_valid()
                && non_negative[0]
                && expression_integer_value(program, facts, expression).is_none()
                && facts.at_most_is_proven(
                    &program.expression_table.display_name(operands[0]),
                    &program.expression_table.display_name(expression),
                )
        })
}
