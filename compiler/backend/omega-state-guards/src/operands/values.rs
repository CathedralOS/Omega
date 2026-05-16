use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_layout::{DataShape, LayoutPlan};

pub(super) fn resolved_guard_operand_value(
    layouts: &LayoutPlan,
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    match table.expression(expression) {
        ExpressionNode::Boolean(value) => return Some(i64::from(*value)),
        ExpressionNode::Integer(value) => return Some(*value),
        _ => {}
    }

    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };
    let [_, _] = table.name_path_members(path.members) else {
        return None;
    };
    let type_symbol = path.head_symbol;
    let variant_symbol = path.symbol;
    if !type_symbol.is_valid() || !variant_symbol.is_valid() {
        return None;
    }

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .and_then(|(_, data_layout)| match &data_layout.shape {
            DataShape::Enum { variants } => variants
                .iter()
                .position(|candidate| candidate.symbol == variant_symbol)
                .and_then(|index| i64::try_from(index).ok()),
            DataShape::Record { .. } => None,
        })
}
