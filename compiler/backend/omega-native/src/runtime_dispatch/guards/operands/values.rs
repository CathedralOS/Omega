use omega_layout::{DataShape, LayoutPlan};
use omega_typed_program::expression::Expression;

pub(super) fn resolved_guard_operand_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    match expression {
        Expression::Boolean(value) => return Some(i64::from(*value)),
        Expression::Integer(value) => return Some(*value),
        _ => {}
    }

    let Expression::Name(path) = expression else {
        return None;
    };
    let [_, _] = path.as_slice() else {
        return None;
    };
    let type_symbol = path.head_symbol();
    let variant_symbol = path.symbol();
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
