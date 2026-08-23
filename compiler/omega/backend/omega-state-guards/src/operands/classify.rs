use crate::StateGuardOperandKind;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(super) fn classify_guard_operand(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    has_resolved_value: bool,
    has_storage_layout: bool,
) -> StateGuardOperandKind {
    if table.expression_is_stored_place(expression) && !has_resolved_value {
        return StateGuardOperandKind::Place;
    }

    match table.expression(expression) {
        ExpressionNode::Name(_) if has_resolved_value => StateGuardOperandKind::StaticSymbol,
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::Mutable(_) => StateGuardOperandKind::Place,
        ExpressionNode::Call(_) if has_storage_layout => StateGuardOperandKind::Place,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => StateGuardOperandKind::Literal,
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => StateGuardOperandKind::OtherExpression,
    }
}
