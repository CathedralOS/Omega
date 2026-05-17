use super::StateValueKind;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(super) fn value_kind(table: &ExpressionTable, expression: ExpressionHandle) -> StateValueKind {
    if table.expression_is_stored_place(expression) {
        return StateValueKind::Place;
    }

    match table.expression(expression) {
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
        ExpressionNode::Mutable(_) => StateValueKind::MutablePlace,
        ExpressionNode::StructLiteral(_) => StateValueKind::Struct,
    }
}
