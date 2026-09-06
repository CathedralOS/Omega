//! The existing declaration/custody owner, not token spelling, admits arithmetic.

use super::*;
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::UnaryOperator;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn builtin(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Option<TypeReferenceHandle>> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => Some(None),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            (!parameter.is_self && !parameter.is_mutable && !parameter.is_const)
                .then_some(Some(parameter.type_reference))
        }
        ExpressionNode::Atomic(atomic) => builtin(program, machine, state, atomic.value, depth + 1),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            builtin(program, machine, state, unary.operand, depth + 1)?;
            Some(None)
        }
        ExpressionNode::Binary(binary) => {
            let left = builtin(program, machine, state, binary.left, depth + 1)?;
            let right = builtin(program, machine, state, binary.right, depth + 1)?;
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And | BinaryOperator::Or => return Some(None),
                _ => return None,
            };
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &[left, right],
            ) {
                return None;
            }
            // Only already-admitted builtin arithmetic inherits its operand
            // carrier. Literal types remain wildcard inputs to the owner.
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Modulo
            ) {
                if left.zip(right).is_some_and(|(left, right)| {
                    program.primitive_type_reference(left)
                        != program.primitive_type_reference(right)
                }) {
                    return None;
                }
                Some(left.or(right))
            } else {
                Some(None)
            }
        }
        _ => None,
    }
}
