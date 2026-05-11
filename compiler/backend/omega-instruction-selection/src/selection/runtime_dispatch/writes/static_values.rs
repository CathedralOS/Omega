use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_typed_trees::expression::{Expression, ExpressionTable};

use super::super::super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, strip_mutable_expression,
};
use super::super::super::storage_places::enum_variant_value;

pub(super) type RuntimeStaticValues = Vec<(Expression, i64)>;

pub(super) fn resolve_runtime_static_integer_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &[(Expression, i64)],
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&input.layouts, expression).or_else(|| {
            resolve_runtime_resolved_static_integer_value(
                input,
                resolve_runtime_alias_expression(expression, source_key, aliases, alias_expressions),
                static_values,
            )
        }),
        Expression::Indexed(_) | Expression::Member(_) | Expression::Mutable(_) => {
            resolve_runtime_resolved_static_integer_value(
                input,
                resolve_runtime_alias_expression(expression, source_key, aliases, alias_expressions),
                static_values,
            )
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

fn resolve_runtime_resolved_static_integer_value(
    input: &InstructionSelectionInput<'_>,
    expression: Expression,
    static_values: &[(Expression, i64)],
) -> Option<i64> {
    let expression = strip_mutable_expression(expression);
    match expression {
        Expression::Integer(value) => Some(value),
        Expression::Boolean(value) => Some(i64::from(value)),
        Expression::Name(_)
        | Expression::Indexed(_)
        | Expression::Member(_) => enum_variant_value(&input.layouts, &expression).or_else(|| {
            static_values
                .iter()
                .find(|(target, _)| target == &expression)
                .map(|(_, value)| *value)
        }),
        Expression::Mutable(_)
        | Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Call(_)
        | Expression::Cast(_)
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
