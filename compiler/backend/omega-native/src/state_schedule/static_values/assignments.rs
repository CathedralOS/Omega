use super::aliases::{canonical_place_name, shallow_canonical_place_name};
use super::evaluation::resolve_static_value;
use crate::control_flow::{OperationKind, StateFlow};
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;

pub(in crate::state_schedule) fn apply_static_operations(
    native_plan: &NativePlan,
    state: &StateFlow,
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return;
    };

    for operation in operations {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                apply_static_assignment(target, value, aliases, values);
            }
            _ => {}
        };
    }
}

pub(in crate::state_schedule) fn set_static_value(
    values: &mut Vec<(String, String)>,
    target: String,
    value: String,
) {
    if let Some((_, existing_value)) = values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        values.push((target, value));
    }
}

fn apply_static_assignment(
    target: &Expression,
    value: &Expression,
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(target_name) = shallow_canonical_place_name(target, aliases) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target = format!("{target_name}::{}", field.name);
            if let Some(source_name) = canonical_place_name(&field.value, aliases) {
                copy_static_prefix(values, &source_name, &field_target);
            }
            if let Some(field_value) = resolve_static_value(&field.value, aliases, values) {
                set_static_value(values, field_target, field_value);
            }
        }
        return;
    }

    if let Some(source_name) = canonical_place_name(value, aliases) {
        copy_static_prefix(values, &source_name, &target_name);
    }

    let Some(value) = resolve_static_value(value, aliases, values) else {
        return;
    };

    set_static_value(values, target_name, value);
}

fn copy_static_prefix(values: &mut Vec<(String, String)>, source_name: &str, target_name: &str) {
    let source_prefix = format!("{source_name}::");
    let copied_values = values
        .iter()
        .filter_map(|(existing_name, value)| {
            existing_name
                .strip_prefix(&source_prefix)
                .map(|suffix| (format!("{target_name}::{suffix}"), value.clone()))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(values, copied_name, copied_value);
    }
}
