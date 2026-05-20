use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_layout::{DataShape, LayoutPlan};

pub(in crate::selection) fn enum_variant_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    let (type_symbol, variant_symbol, type_name, variant_name) = match expression {
        Expression::Name(path) => {
            let [type_name, variant_name] = path.members() else {
                return None;
            };
            (
                path.head_symbol(),
                path.symbol(),
                type_name.as_str(),
                variant_name.as_str(),
            )
        }
        Expression::Member(member) => {
            let Expression::Name(path) = &member.receiver else {
                return None;
            };
            let type_name = path.last()?;
            (
                path.symbol(),
                member.member_symbol,
                type_name.as_str(),
                member.member.as_str(),
            )
        }
        _ => return None,
    };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| {
            (type_symbol.is_valid() && data_layout.symbol == type_symbol)
                || data_layout.name.as_str() == type_name
        })
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    layouts
        .variants
        .span_or_empty(*variants)
        .iter()
        .position(|variant| {
            (variant_symbol.is_valid() && variant.symbol == variant_symbol)
                || variant.name.as_str() == variant_name
        })
        .and_then(|index| i64::try_from(index).ok())
}

pub(in crate::selection) fn static_integer_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Boolean(value) => Some(i64::from(*value)),
        _ => enum_variant_value(layouts, expression),
    }
}

pub(in crate::selection) fn static_integer_value_in_table(
    layouts: &LayoutPlan,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    match expressions.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Boolean(value) => Some(i64::from(*value)),
        _ => enum_variant_value_in_table(layouts, expressions, expression),
    }
}

pub(in crate::selection) fn enum_variant_value_in_table(
    layouts: &LayoutPlan,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    let (type_symbol, variant_symbol, type_name, variant_name) =
        match expressions.expression(expression) {
            ExpressionNode::Name(path) => {
                let [type_name, variant_name] = expressions.name_path_members(path.members) else {
                    return None;
                };
                (
                    path.head_symbol,
                    path.symbol,
                    type_name.as_str(),
                    variant_name.as_str(),
                )
            }
            ExpressionNode::Member(member) => {
                let ExpressionNode::Name(path) = expressions.expression(member.receiver) else {
                    return None;
                };
                let type_name = expressions.name_path_members(path.members).last()?;
                (
                    path.symbol,
                    member.member_symbol,
                    type_name.as_str(),
                    member.member.as_str(),
                )
            }
            _ => return None,
        };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| {
            (type_symbol.is_valid() && data_layout.symbol == type_symbol)
                || data_layout.name.as_str() == type_name
        })
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    layouts
        .variants
        .span_or_empty(*variants)
        .iter()
        .position(|variant| {
            (variant_symbol.is_valid() && variant.symbol == variant_symbol)
                || variant.name.as_str() == variant_name
        })
        .and_then(|index| i64::try_from(index).ok())
}
