use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::patterns;

pub(super) fn argument_is_parameter_minus_one(
    program: &typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
}

pub(super) fn argument_is_parameter_plus_one(
    program: &typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Add)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
}

pub(super) fn argument_rebuilds_parameter_with_member_minus_one(
    program: &typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(argument)
    else {
        return false;
    };

    program
        .expression_table
        .struct_fields(struct_literal.fields)
        .iter()
        .any(|field| {
            field.name.as_str() == member_name
                && argument_is_parameter_member_minus_one(
                    program,
                    field.value,
                    parameter,
                    member_name,
                )
        })
}

fn argument_is_parameter_member_minus_one(
    program: &typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, member_name)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
}
