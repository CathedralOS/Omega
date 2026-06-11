use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_layout::{DataShape, LayoutPlan};

pub(in crate::selection) fn enum_variant_value(
    layouts: &LayoutPlan,
    expression: &Expression,
) -> Option<i64> {
    let (type_symbol, variant_symbol, type_name, variant_name) = match expression {
        Expression::Mutable(inner) => return enum_variant_value(layouts, inner),
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
        // A `mut` wrapper around a value operand is transparent: an inlined
        // branching argument binds as `mut <expr>` even for a by-value parameter
        // (e.g. `mut 2`), so the constant must still fold through it.
        Expression::Mutable(inner) => static_integer_value(layouts, inner),
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
        ExpressionNode::Mutable(inner) => static_integer_value_in_table(layouts, expressions, *inner),
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
            ExpressionNode::Mutable(inner) => {
                return enum_variant_value_in_table(layouts, expressions, *inner);
            }
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

/// TAG-ONLY case comparison in a runtime value tree: when one side of an
/// equality names a CASE of an enum-shaped data (the lowered form of `in`,
/// or a payload-less case `==`), the other side must read only the tag
/// prefix, never the full tag-plus-payload value -- a payload-carrying enum
/// is wider than its tag, and stale payload bytes must not affect the
/// compare. Mirrors the guard-operand clamp in omega-state-guards.
pub(in crate::selection) fn clamp_runtime_case_comparison_operands_in_table(
    layouts: &LayoutPlan,
    expressions: &ExpressionTable,
    operator: omega_checked_trees::expression::BinaryOperator,
    left_expression: ExpressionHandle,
    right_expression: ExpressionHandle,
    left: omega_abstract_operations::RuntimeValueOperandHandle,
    right: omega_abstract_operations::RuntimeValueOperandHandle,
    runtime_value_operands: &mut omega_core::arena::Arena<
        omega_abstract_operations::RuntimeValueOperand,
    >,
) {
    if !matches!(
        operator,
        omega_checked_trees::expression::BinaryOperator::Equal
            | omega_checked_trees::expression::BinaryOperator::NotEqual
    ) {
        return;
    }
    if enum_variant_value_in_table(layouts, expressions, left_expression).is_some() {
        clamp_runtime_case_comparison_operand(runtime_value_operands, right);
    }
    if enum_variant_value_in_table(layouts, expressions, right_expression).is_some() {
        clamp_runtime_case_comparison_operand(runtime_value_operands, left);
    }
}

/// Non-table twin of `clamp_runtime_case_comparison_operands_in_table` for
/// resolvers that walk owned `Expression` trees.
pub(in crate::selection) fn clamp_runtime_case_comparison_operands(
    layouts: &LayoutPlan,
    operator: omega_checked_trees::expression::BinaryOperator,
    left_expression: &Expression,
    right_expression: &Expression,
    left: omega_abstract_operations::RuntimeValueOperandHandle,
    right: omega_abstract_operations::RuntimeValueOperandHandle,
    runtime_value_operands: &mut omega_core::arena::Arena<
        omega_abstract_operations::RuntimeValueOperand,
    >,
) {
    if !matches!(
        operator,
        omega_checked_trees::expression::BinaryOperator::Equal
            | omega_checked_trees::expression::BinaryOperator::NotEqual
    ) {
        return;
    }
    if enum_variant_value(layouts, left_expression).is_some() {
        clamp_runtime_case_comparison_operand(runtime_value_operands, right);
    }
    if enum_variant_value(layouts, right_expression).is_some() {
        clamp_runtime_case_comparison_operand(runtime_value_operands, left);
    }
}

fn clamp_runtime_case_comparison_operand(
    runtime_value_operands: &mut omega_core::arena::Arena<
        omega_abstract_operations::RuntimeValueOperand,
    >,
    operand: omega_abstract_operations::RuntimeValueOperandHandle,
) {
    use omega_abstract_operations::RuntimeValueOperand;

    let byte_size = match runtime_value_operands.get_mut(operand) {
        RuntimeValueOperand::Storage { byte_size, .. }
        | RuntimeValueOperand::Pointee { byte_size, .. }
        | RuntimeValueOperand::FrameIndexed { byte_size, .. }
        | RuntimeValueOperand::FrameBaseIndexed { byte_size, .. }
        | RuntimeValueOperand::FrameFixedIndexed { byte_size, .. } => byte_size,
        _ => return,
    };
    if *byte_size > omega_layout::ENUM_TAG_BYTES {
        *byte_size = omega_layout::ENUM_TAG_BYTES;
    }
}
