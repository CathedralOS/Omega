use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::Call;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::host_calls) enum StaticValue {
    Integer(i64),
    Expression(Expression),
    Text(String),
}

pub(in crate::host_calls) fn initial_static_values(machine: &Machine) -> Vec<(String, StaticValue)> {
    machine
        .owned_data
        .iter()
        .filter_map(|owned_data| {
            let value = match owned_data.initial_value.as_ref()? {
                Expression::Integer(value) => StaticValue::Integer(*value),
                Expression::String(value) => StaticValue::Text(value.clone()),
                Expression::Name(path) if is_static_symbol_path(path) => {
                    StaticValue::Expression(Expression::Name(path.clone()))
                }
                _ => return None,
            };

            Some((owned_data.name.to_string(), value))
        })
        .collect()
}

pub(in crate::host_calls) fn apply_static_assignment(
    static_values: &mut Vec<(String, StaticValue)>,
    target: &Expression,
    value: &Expression,
) {
    let Some(target_name) = static_place_name(target) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            if let Some(field_value) = resolve_static_value(&field.value, static_values) {
                set_static_value(
                    static_values,
                    format!("{target_name}::{}", field.name),
                    field_value,
                );
            }
        }
        return;
    }

    if let Some(source_name) = static_place_name(value) {
        copy_static_prefix(static_values, &source_name, &target_name);
    }

    let Some(value) = resolve_static_value(value, static_values) else {
        return;
    };

    set_static_value(static_values, target_name, value);
}

pub(in crate::host_calls) fn apply_call_static_effects(
    static_values: &mut Vec<(String, StaticValue)>,
    call: &Call,
) {
    for argument in &call.arguments {
        let Expression::Mutable(target) = argument else {
            continue;
        };

        let Some(target_name) = static_place_name(target) else {
            continue;
        };

        invalidate_static_prefix(static_values, &target_name);
    }
}

pub(in crate::host_calls) fn resolve_static_value(
    expression: &Expression,
    static_values: &[(String, StaticValue)],
) -> Option<StaticValue> {
    match expression {
        Expression::Integer(value) => Some(StaticValue::Integer(*value)),
        Expression::String(value) => Some(StaticValue::Text(value.clone())),
        Expression::Name(path) => {
            let name = expression.display_name();
            static_values
                .iter()
                .find(|(target, _)| target == &name)
                .map(|(_, value)| value.clone())
                .or_else(|| {
                    if is_static_symbol_path(path) {
                        Some(StaticValue::Expression(Expression::Name(path.clone())))
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

fn static_place_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Name(path) if !path.is_empty() => Some(expression.display_name()),
        Expression::Indexed(_) => Some(expression.display_name()),
        _ => None,
    }
}

fn is_static_symbol_path(path: &[ProgramName]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}

fn set_static_value(
    static_values: &mut Vec<(String, StaticValue)>,
    target_name: String,
    value: StaticValue,
) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_name, _)| existing_name == &target_name)
    {
        *existing_value = value;
    } else {
        static_values.push((target_name, value));
    }
}

fn copy_static_prefix(
    static_values: &mut Vec<(String, StaticValue)>,
    source_name: &str,
    target_name: &str,
) {
    let source_prefix = format!("{source_name}::");
    let copied_values = static_values
        .iter()
        .filter_map(|(existing_name, value)| {
            existing_name
                .strip_prefix(&source_prefix)
                .map(|suffix| (format!("{target_name}::{suffix}"), value.clone()))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(static_values, copied_name, copied_value);
    }
}

fn invalidate_static_prefix(static_values: &mut Vec<(String, StaticValue)>, target_name: &str) {
    let target_prefix = format!("{target_name}::");
    static_values.retain(|(existing_name, _)| {
        existing_name != target_name && !existing_name.starts_with(&target_prefix)
    });
}
