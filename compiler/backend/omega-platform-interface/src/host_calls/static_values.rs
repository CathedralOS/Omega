use crate::place_keys::PlaceKey;
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionNode};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::{TableAssignment, TableCall};
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
    program: &Program,
    assignment: TableAssignment,
) {
    let Some(target_key) = static_place_key_handle(program, assignment.target) else {
        return;
    };

    if let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(assignment.value)
    {
        for field in program.expression_table.struct_fields(struct_literal.fields) {
            if let Some(field_value) =
                resolve_static_value_handle(program, field.value, static_values)
            {
                set_static_value(
                    static_values,
                    target_key.append_member(field.name.clone()),
                    field_value,
                );
            }
        }
        return;
    }

    if let Some(source_key) = static_place_key_handle(program, assignment.value) {
        copy_static_prefix(static_values, &source_key, &target_key);
    }

    let Some(value) = resolve_static_value_handle(program, assignment.value, static_values) else {
        return;
    };

    set_static_value(static_values, target_key, value);
}

pub(crate) fn apply_call_static_effects(
    static_values: &mut Vec<(PlaceKey, StaticValue)>,
    program: &Program,
    call: &TableCall,
) {
    for argument in program.statement_table.expression_handles(call.arguments) {
        let ExpressionNode::Mutable(target) = program.expression_table.expression(*argument) else {
            continue;
        };

        let Some(target_key) = static_place_key_handle(program, *target) else {
            continue;
        };

        invalidate_static_prefix(static_values, &target_key);
    }
}

pub(crate) fn resolve_static_value_handle(
    program: &Program,
    expression: ExpressionHandle,
    static_values: &[(PlaceKey, StaticValue)],
) -> Option<StaticValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(StaticValue::Integer(*value)),
        ExpressionNode::String(value) => Some(StaticValue::Text(value.clone())),
        ExpressionNode::Name(path) => {
            let key = static_place_key_handle(program, expression)?;
            static_values
                .iter()
                .find(|(target, _)| target == &key)
                .map(|(_, value)| value.clone())
                .or_else(|| {
                    if path.symbol.is_valid() {
                        Some(StaticValue::Expression(
                            program.expression_table.to_tree(expression),
                        ))
                    } else {
                        None
                    }
                })
        }
        _ => None,
    }
}

fn static_place_key_handle(program: &Program, expression: ExpressionHandle) -> Option<PlaceKey> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if !program.expression_table.name_path_members(path.members).is_empty() => {
            PlaceKey::from_expression_handle(&program.expression_table, expression)
        }
        ExpressionNode::Indexed(_) => {
            PlaceKey::from_expression_handle(&program.expression_table, expression)
        }
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
