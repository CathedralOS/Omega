use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::patterns;

pub(super) fn guard_is_positive_parameter(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
}

pub(super) fn guard_is_positive_parameter_member(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, member_name)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
}

pub(super) fn guard_is_index_below_limit(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    index_parameter: &omega_typed_trees::signature::StateParameter,
    limit_parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Less)
        && patterns::expression_is_parameter(program, binary.left, index_parameter)
        && patterns::expression_matches_parameter(program, binary.right, limit_parameter)
}
