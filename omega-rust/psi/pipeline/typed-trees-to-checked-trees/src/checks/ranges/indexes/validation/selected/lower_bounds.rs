//! An upper collection-relative fact is not evidence of non-negativity.

use super::*;
use crate::checks::ranges::expressions::expression_integer_value;
use crate::checks::ranges::types::{
    expression_enforced_declared_range, expression_is_unsigned_integer,
};
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
    operands
        .into_iter()
        .enumerate()
        .all(|(ordinal, expression)| {
            if positions & (4 << ordinal) == 0 {
                return true;
            }
            if !expression.is_valid() {
                // Omitted range endpoints mean zero and the collection length.
                // Neither default can introduce a negative operand.
                return spelling == OperatorSpelling::Range;
            }
            if let Some(value) = expression_integer_value(program, facts, expression) {
                return value >= 0;
            }
            let label = program.expression_table.display_name(expression);
            expression_is_unsigned_integer(program, machine, state, expression)
                || expression_enforced_declared_range(program, machine, state, expression)
                    .is_some_and(|(minimum, _)| minimum >= 0)
                || facts.non_negative_is_proven(&label)
                || facts.non_negative_is_proven_via_ordering(&label)
        })
}
