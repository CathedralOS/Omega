use super::aliases::{PlaceKey, canonical_place_key, shallow_canonical_place_key};
use super::evaluation::resolve_static_value;
use crate::StateScheduleContext;
use omega_control_flow::{OperationKind, StateFlow};
use omega_typed_program::expression::Expression;

pub(crate) fn apply_static_operations(
    context: &StateScheduleContext,
    state: &StateFlow,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &mut Vec<(PlaceKey, String)>,
) {
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
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

pub(crate) fn set_static_value(
    values: &mut Vec<(PlaceKey, String)>,
    target: PlaceKey,
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
    aliases: &[(PlaceKey, PlaceKey)],
    values: &mut Vec<(PlaceKey, String)>,
) {
    let Some(target_key) = shallow_canonical_place_key(target, aliases) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target = target_key.append_member(field.name.clone());
            if let Some(source_key) = canonical_place_key(&field.value, aliases) {
                copy_static_prefix(values, &source_key, &field_target);
            }
            if let Some(field_value) = resolve_static_value(&field.value, aliases, values) {
                set_static_value(values, field_target, field_value);
            }
        }
        return;
    }

    if let Some(source_key) = canonical_place_key(value, aliases) {
        copy_static_prefix(values, &source_key, &target_key);
    }

    let Some(value) = resolve_static_value(value, aliases, values) else {
        return;
    };

    set_static_value(values, target_key, value);
}

fn copy_static_prefix(
    values: &mut Vec<(PlaceKey, String)>,
    source_key: &PlaceKey,
    target_key: &PlaceKey,
) {
    let copied_values = values
        .iter()
        .filter_map(|(existing_key, value)| {
            if !existing_key.starts_with(source_key) {
                return None;
            }

            Some((
                existing_key.replace_prefix(source_key, target_key),
                value.clone(),
            ))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(values, copied_name, copied_value);
    }
}
