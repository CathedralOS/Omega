use crate::layout::{DataShape, LayoutPlan};
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
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .and_then(|(_, data_layout)| match &data_layout.shape {
            DataShape::Enum { variants } => variants
                .iter()
                .position(|candidate| candidate == variant_name)
                .and_then(|index| i64::try_from(index).ok()),
            DataShape::Record { .. } => None,
        })
}
