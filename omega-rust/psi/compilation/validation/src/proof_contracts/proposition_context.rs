//! Logical connective children retain proposition context. Calls and ordinary
//! value expressions keep their declared argument roles; comparison spelling
//! supplies no separate identity interpretation.

use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::PrimitiveType;

/// Logical composition supplies proposition context to its children. Ordinary
/// expression-call arguments are value terms; nominal proposition applications
/// have their own declared proof telescope, outside this expression grammar.
pub fn logical_proposition_children(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> [Option<ExpressionHandle>; 2] {
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            [Some(unary.operand), None]
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                || (matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && [binary.left, binary.right].into_iter().all(|operand| {
                    super::mathematical_applications::operand_type_reference(program, operand)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        == Some(PrimitiveType::Bool)
                })) =>
        {
            [Some(binary.left), Some(binary.right)]
        }
        _ => [None, None],
    }
}
