use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::patterns;

pub(super) fn argument_is_parameter_tail_slice(
    program: &psi_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &psi_typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
        return false;
    };
    if !patterns::expression_is_parameter(program, indexed.collection, parameter) {
        return false;
    }
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if !range.start.is_valid() {
        return false;
    }

    matches!(
        program.expression_table.expression(range.start),
        ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
    ) && !range.end.is_valid()
}
