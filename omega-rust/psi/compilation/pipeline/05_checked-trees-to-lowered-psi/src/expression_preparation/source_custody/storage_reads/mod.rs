//! Rejoin scalar reads with authored bindings, storage, and operand positions.

use super::{CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, unsupported};
pub(super) mod case_membership;

/// Guard lowering erases the builtin `subject == true` wrapper, not its subject.
pub(super) fn guard_subject(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return expression;
    };
    if binary.operator != checked_trees::expression::BinaryOperator::Equal
        || checked
            .facts
            .operators
            .expression_use(expression)
            .is_some_and(|operator| {
                operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    {
        return expression;
    }
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (ExpressionNode::Boolean(true), _) => binary.right,
        (_, ExpressionNode::Boolean(true)) => binary.left,
        _ => expression,
    }
}
