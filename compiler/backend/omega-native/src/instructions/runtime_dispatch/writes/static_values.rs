use crate::plan::NativePlan;
use omega_control_flow::StateKey;
use omega_typed_program::expression::Expression;

use super::super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use super::super::super::storage_places::enum_variant_value;

pub(super) type RuntimeStaticValues = Vec<(Expression, i64)>;

pub(super) fn resolve_runtime_static_integer_value(
    native_plan: &NativePlan,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &[(Expression, i64)],
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&native_plan.layouts, expression).or_else(|| {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression)
                .map(|(_, value)| *value)
        }),
        Expression::Indexed(_) | Expression::Mutable(_) => {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression)
                .map(|(_, value)| *value)
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

pub(super) fn set_runtime_static_value(
    static_values: &mut RuntimeStaticValues,
    target: Expression,
    value: i64,
) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        static_values.push((target, value));
    }
}
