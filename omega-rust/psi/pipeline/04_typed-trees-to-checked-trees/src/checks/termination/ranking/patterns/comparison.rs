use super::GuardFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

/// Preserve an exact edge's polarity while removing Boolean wrappers. Callers
/// establish selected builtin meaning before using the resulting integer order.
pub(in crate::checks::termination::ranking) fn comparison(
    program: &typed_trees::TypedTrees,
    fact: GuardFact,
) -> Option<(ExpressionHandle, BinaryOperator, ExpressionHandle)> {
    let mut expression = fact.expression;
    let mut holds = fact.holds;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                holds = !holds;
                expression = unary.operand;
            }
            ExpressionNode::Binary(binary) => {
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    let wrapped = match (
                        program.expression_table.expression(binary.left),
                        program.expression_table.expression(binary.right),
                    ) {
                        (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                        (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                        _ => None,
                    };
                    if let Some((operand, value)) = wrapped {
                        holds = holds == (value == (binary.operator == BinaryOperator::Equal));
                        expression = operand;
                        continue;
                    }
                }
                let operator = if holds {
                    binary.operator
                } else {
                    match binary.operator {
                        BinaryOperator::Equal => BinaryOperator::NotEqual,
                        BinaryOperator::NotEqual => BinaryOperator::Equal,
                        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
                        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
                        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
                        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
                        _ => return None,
                    }
                };
                return Some((binary.left, operator, binary.right));
            }
            _ => return None,
        }
    }
}
