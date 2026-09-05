//! The typed decrease rule shared by runtime and proof slice recursion.

use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::types::TypeReferenceNode;

/// A taken nonempty-slice guard makes its exact parameter's `1..` tail
/// strictly shorter. The caller owns guard dominance and binding stability;
/// this predicate does not establish either from matching expression text.
pub fn slice_tail_strictly_decreases(
    program: &TypedTrees,
    guard: ExpressionHandle,
    argument: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    if !parameter.symbol.is_valid() || !parameter_is_slice(program, parameter) {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if !names_parameter(program, indexed.collection, parameter)
        || range.end.is_valid()
        || range.end_inclusive
        || !integer_is(program, range.start, 1)
    {
        return false;
    }
    let guard = match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if binary.operator == BinaryOperator::Equal
                && matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
        {
            binary.left
        }
        _ => guard,
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    let ExpressionNode::Member(length) = program.expression_table.expression(binary.left) else {
        return false;
    };
    length.member.as_str() == "len"
        && names_parameter(program, length.receiver, parameter)
        && match binary.operator {
            BinaryOperator::Greater => integer_is(program, binary.right, 0),
            BinaryOperator::GreaterOrEqual => integer_is(program, binary.right, 1),
            _ => false,
        }
}

fn names_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if path.symbol == parameter.symbol
                && path.head_symbol == parameter.symbol
                && program.expression_table.name_path_members(path.members).len() == 1
    )
}

fn integer_is(program: &TypedTrees, expression: ExpressionHandle, value: i64) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Integer(literal) if literal.value_i64() == Some(value)
    )
}

fn parameter_is_slice(program: &TypedTrees, parameter: &StateParameter) -> bool {
    let mut reference = parameter.type_reference;
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return false,
        };
    }
    false
}
