//! Declared range bounds share exact anonymous arithmetic with static inference.
//!
//! Keep the authored expression and its operator selections intact. Range
//! validation, proof construction and representation readers must agree on the
//! completed integer value; the syntax-only i64 folder truncates fractional
//! intermediates and cannot supply that semantic fact. Consumers with bounded
//! storage convert only the final value, without changing its denotation.

use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Read a closed integer range endpoint without interpreting typed
/// computations as anonymous arithmetic. Unknown, fractional and invalid
/// expressions supply no bound, including when an authored operator is selected.
pub fn closed_integer_range_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let ExpressionNode::Integer(value) = program.expression_table.expression(expression) {
        return value.value_bignum();
    }
    if let Some(value) = crate::evaluate_anonymous_numeric_expression_with_selected_match_arms(
        program,
        expression,
        &[],
        |expression| crate::has_anonymous_operator_meaning(program, expression),
    ) {
        return value.to_integer_exact();
    }
    crate::arithmetic_domains::closed_integer_expression_value(program, expression)
        .map(BigInt::from_i64)
}
