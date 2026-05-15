use omega_layout::{DataShape, LayoutPlan};
use omega_checked_trees::expression::Expression;

pub(in crate::selection) fn enum_variant_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [_, _] = path.members() else {
        return None;
    };
    let type_symbol = path.head_symbol();
    let variant_symbol = path.symbol();
    if !type_symbol.is_valid() || !variant_symbol.is_valid() {
        return None;
    }
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    variants
        .iter()
        .position(|variant| variant.symbol == variant_symbol)
        .and_then(|index| i64::try_from(index).ok())
}

pub(in crate::selection) fn static_integer_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => enum_variant_value(layouts, expression),
    }
}
