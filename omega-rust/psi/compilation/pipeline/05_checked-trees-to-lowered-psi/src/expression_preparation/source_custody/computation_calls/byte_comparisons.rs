//! Exact source custody for a byte-content query over a structural operand.
//! Preparation never changes which operand the authored comparison observes.
use super::{CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, unsupported};

pub(crate) fn operands(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<(ExpressionHandle, &[u8], bool), LoweringError> {
    use checked_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return unsupported("byte comparison lost its authored operation");
    };
    let negated = binary.operator == BinaryOperator::NotEqual;
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (_, ExpressionNode::String(bytes)) => Ok((binary.left, bytes, negated)),
        (ExpressionNode::String(bytes), _) => Ok((binary.right, bytes, negated)),
        _ => unsupported("byte comparison lost its finite literal"),
    }
}
