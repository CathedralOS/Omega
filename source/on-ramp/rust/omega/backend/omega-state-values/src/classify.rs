use super::StateValueKind;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(super) fn value_kind(table: &ExpressionTable, expression: ExpressionHandle) -> StateValueKind {
    if table.expression_is_stored_place(expression) {
        return StateValueKind::Place;
    }

    match table.expression(expression) {
        ExpressionNode::Atomic(atomic) => value_kind(table, atomic.value),
        ExpressionNode::ArrayLiteral(_) => StateValueKind::Array,
        ExpressionNode::Binary(_) => StateValueKind::Binary,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => StateValueKind::Literal,
        ExpressionNode::Call(_) | ExpressionNode::Cast(_) => StateValueKind::Binary,
        ExpressionNode::Indexed(_) | ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
            unreachable!("stored places are classified before expression node matching")
        }
        ExpressionNode::Borrow(_) => StateValueKind::MutablePlace,
        ExpressionNode::Range(_) => StateValueKind::Binary,
        ExpressionNode::StructLiteral(_) => StateValueKind::Struct,
        ExpressionNode::Unary(_) => StateValueKind::Binary,
        ExpressionNode::ZeroValue(_) => StateValueKind::Binary,
    }
}
