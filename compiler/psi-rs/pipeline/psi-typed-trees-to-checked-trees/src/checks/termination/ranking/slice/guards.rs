use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::patterns;

pub(super) fn guard_is_non_empty_slice(
    program: &psi_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &psi_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_matches_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(0)
        )
}
