use super::StateValueKind;
use omega_typed_program::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(super) fn value_kind(table: &ExpressionTable, expression: ExpressionHandle) -> StateValueKind {
    match table.expression(expression) {
        ExpressionNode::ArrayLiteral(_) => StateValueKind::Array,
        ExpressionNode::Binary(_) => StateValueKind::Binary,
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => StateValueKind::Literal,
        ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Member(_) => StateValueKind::Binary,
        ExpressionNode::Indexed(_) | ExpressionNode::Name(_) => StateValueKind::Place,
        ExpressionNode::Mutable(_) => StateValueKind::MutablePlace,
        ExpressionNode::StructLiteral(_) => StateValueKind::Struct,
    }
}
