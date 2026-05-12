use crate::StateGuardOperandKind;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(super) fn classify_guard_operand(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    has_resolved_value: bool,
) -> StateGuardOperandKind {
    match table.expression(expression) {
        ExpressionNode::Name(_) if has_resolved_value => StateGuardOperandKind::StaticSymbol,
        ExpressionNode::Name(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Mutable(_) => StateGuardOperandKind::Place,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => StateGuardOperandKind::Literal,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::StructLiteral(_) => StateGuardOperandKind::OtherExpression,
    }
}
