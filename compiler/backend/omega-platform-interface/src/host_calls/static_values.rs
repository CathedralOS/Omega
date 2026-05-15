use crate::place_keys::PlaceKey;
use omega_checked_trees::expression::Expression;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::Call;
use omega_checked_trees::Program;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StaticValue {
    Integer(i64),
    Expression(Expression),
    Text(String),
}

pub(crate) fn initial_static_values(
    program: &Program,
    machine: &Machine,
) -> Vec<(PlaceKey, StaticValue)> {
    program
        .machine_owned_data(machine)
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

            Some((
                PlaceKey::from_symbol_name(owned_data.symbol, owned_data.name.clone()),
                value,
            ))
        })
        .collect()
}

pub(crate) fn apply_static_assignment(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    target: &Expression,
    value: &Expression,
) {
    let Some(target_key) = static_place_key(target) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            if let Some(field_value) = resolve_static_value(&field.value, static_values) {
                set_static_value(
                    static_values,
                    target_key.append_member(field.name.clone()),
                    field_value,
                );
            }
        }
        return;
    }

    if let Some(source_key) = static_place_key(value) {
        copy_static_prefix(static_values, &source_key, &target_key);
    }

    let Some(value) = resolve_static_value(value, static_values) else {
        return;
    };

    set_static_value(static_values, target_key, value);
}

pub(crate) fn apply_call_static_effects(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    program: &Program,
    call: &Call,
) {
    for argument in program.call_arguments(call) {
        let Expression::Mutable(target) = argument else {
            continue;
        };

        let Some(target_key) = static_place_key(target) else {
            continue;
        };

        invalidate_static_prefix(static_values, &target_key);
    }
}

pub(crate) fn resolve_static_value(
    expression: &Expression,
    static_values: &[(PlaceKey, StaticValue)],
) -> Option<StaticValue> {
    match expression {
        Expression::Integer(value) => Some(StaticValue::Integer(*value)),
        Expression::String(value) => Some(StaticValue::Text(value.clone())),
        Expression::Name(path) => {
            let key = static_place_key(expression)?;
            static_values
                .iter()
                .find(|(target, _)| target == &key)
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

fn static_place_key(expression: &Expression) -> Option<PlaceKey> {
    match expression {
        Expression::Name(path) if !path.is_empty() => PlaceKey::from_expression(expression),
        Expression::Indexed(_) => PlaceKey::from_expression(expression),
        _ => None,
    }
}

fn is_static_symbol_path(path: &omega_checked_trees::expression::NamePath) -> bool {
    path.symbol().is_valid()
}

fn set_static_value(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    target_key: PlaceKey,
    value: StaticValue,
) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_key, _)| existing_key == &target_key)
    {
        *existing_value = value;
    } else {
        static_values.push((target_key, value));
    }
}

fn copy_static_prefix(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    source_key: &PlaceKey,
    target_key: &PlaceKey,
) {
    let initial_value_count = static_values.len();
    for index in 0..initial_value_count {
        let (existing_key, value) = &static_values[index];
        if !existing_key.starts_with(source_key) {
            continue;
        }

        let copied_key = existing_key.replace_prefix(source_key, target_key);
        let copied_value = value.clone();
        set_static_value(static_values, copied_key, copied_value);
    }
}

fn invalidate_static_prefix(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    target_key: &PlaceKey,
) {
    static_values.retain(|(existing_key, _)| !existing_key.starts_with(target_key));
}
