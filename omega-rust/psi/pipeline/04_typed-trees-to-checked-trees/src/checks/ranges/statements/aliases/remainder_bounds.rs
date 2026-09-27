//! A bound established by the remainder value captured at this statement.

use checked_trees::CheckedOperatorResolutionStatus;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

use super::RangeFacts;

#[cfg(test)]
mod tests;

pub(super) fn seed(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
    target: &str,
) {
    let Some(upper) = unsigned_remainder_upper_bound(program, machine, state, facts, expression)
    else {
        return;
    };
    // This names the captured result, not a future read of its operands.
    // Both statement walkers reach this after evaluation and target invalidation;
    // ordinary reassignment/call-write invalidation retires the target's facts.
    facts.prove_index_upper_bound(target.to_owned(), upper);
    facts.prove_non_negative(target.to_owned());
}

fn unsigned_remainder_upper_bound(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<i64> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Modulo
        || !validation::has_builtin_binary_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        )
        || facts.checked_operators.is_some_and(|operators| {
            operators.uses.iter().any(|(_, operator)| {
                operator.expression == expression
                    && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
            })
        })
    {
        return None;
    }
    let result_type =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    let primitive = program.primitive_type_reference(result_type)?;
    if !matches!(
        primitive,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    ) {
        // Signed remainder follows the dividend's sign, not Euclidean modulo.
        return None;
    }
    let divisor = program.closed_integer_value_in(binary.right, machine.symbol)?;
    typed_trees::closed_numeric::land_integer(&divisor.value, primitive)?;
    let upper = divisor.value.to_i64()?;
    // No bound on the dividend's magnitude is needed, even for a full u64 or
    // a Saturating expression. A positive closed divisor needs no mutable
    // snapshot premise; arithmetic formation still checks the original term.
    (upper > 0).then_some(upper)
}
