use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(super) fn lower_binary_operator(
    operator: resolved::expression::BinaryOperator,
) -> typed::expression::BinaryOperator {
    match operator {
        resolved::expression::BinaryOperator::Add => typed::expression::BinaryOperator::Add,
        resolved::expression::BinaryOperator::And => typed::expression::BinaryOperator::And,
        resolved::expression::BinaryOperator::BitwiseAnd => {
            typed::expression::BinaryOperator::BitwiseAnd
        }
        resolved::expression::BinaryOperator::BitwiseOr => {
            typed::expression::BinaryOperator::BitwiseOr
        }
        resolved::expression::BinaryOperator::BitwiseXor => {
            typed::expression::BinaryOperator::BitwiseXor
        }
        resolved::expression::BinaryOperator::Divide => typed::expression::BinaryOperator::Divide,
        resolved::expression::BinaryOperator::Equal => typed::expression::BinaryOperator::Equal,
        resolved::expression::BinaryOperator::Greater => typed::expression::BinaryOperator::Greater,
        resolved::expression::BinaryOperator::GreaterOrEqual => {
            typed::expression::BinaryOperator::GreaterOrEqual
        }
        resolved::expression::BinaryOperator::Less => typed::expression::BinaryOperator::Less,
        resolved::expression::BinaryOperator::LessOrEqual => {
            typed::expression::BinaryOperator::LessOrEqual
        }
        resolved::expression::BinaryOperator::Modulo => typed::expression::BinaryOperator::Modulo,
        resolved::expression::BinaryOperator::Multiply => {
            typed::expression::BinaryOperator::Multiply
        }
        resolved::expression::BinaryOperator::NotEqual => {
            typed::expression::BinaryOperator::NotEqual
        }
        resolved::expression::BinaryOperator::Or => typed::expression::BinaryOperator::Or,
        resolved::expression::BinaryOperator::ShiftLeft => {
            typed::expression::BinaryOperator::ShiftLeft
        }
        resolved::expression::BinaryOperator::ShiftRight => {
            typed::expression::BinaryOperator::ShiftRight
        }
        resolved::expression::BinaryOperator::Subtract => {
            typed::expression::BinaryOperator::Subtract
        }
    }
}
