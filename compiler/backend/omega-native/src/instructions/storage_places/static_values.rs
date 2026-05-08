use omega_layout::{DataShape, LayoutPlan};
use omega_typed_program::expression::Expression;

pub(in crate::instructions) fn enum_variant_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    variants
        .iter()
        .position(|variant| variant == variant_name)
        .and_then(|index| i64::try_from(index).ok())
}

pub(in crate::instructions) fn static_integer_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => enum_variant_value(layouts, expression),
    }
}
