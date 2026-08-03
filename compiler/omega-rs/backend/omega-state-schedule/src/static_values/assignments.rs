use super::StaticValue;
use super::aliases::{PlaceKey, canonical_place_key, shallow_canonical_place_key};
use super::evaluation::resolve_static_value;
use crate::StateScheduleContext;
use omega_control_flow::{OperationExpressionRefs, StateFlow};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(crate) fn apply_static_operations(
    context: &StateScheduleContext,
    state: &StateFlow,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &mut Vec<(PlaceKey, StaticValue)>,
) {
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return;
    };

    for operation in operations {
        if let OperationExpressionRefs::Assignment { target, value } = operation.expressions {
            apply_static_assignment(
                &context.control_flow.expressions,
                target,
                value,
                aliases,
                values,
            );
        };
    }
}

pub(crate) fn set_static_value(
    values: &mut Vec<(PlaceKey, StaticValue)>,
    target: PlaceKey,
    value: StaticValue,
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
    table: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &mut Vec<(PlaceKey, StaticValue)>,
) {
    let Some(target_key) = shallow_canonical_place_key(table, target, aliases) else {
        return;
    };

    if let ExpressionNode::StructLiteral(struct_literal) = table.expression(value) {
        for field in table.struct_fields(struct_literal.fields) {
            let field_target = target_key.append_member(field.name.clone());
            if let Some(source_key) = canonical_place_key(table, field.value, aliases) {
                copy_static_prefix(values, &source_key, &field_target);
            }
            if let Some(field_value) = resolve_static_value(table, field.value, aliases, values) {
                set_static_value(values, field_target, field_value);
            }
        }
        return;
    }

    if let Some(source_key) = canonical_place_key(table, value, aliases) {
        copy_static_prefix(values, &source_key, &target_key);
    }

    let Some(value) = resolve_static_value(table, value, aliases, values) else {
        return;
    };

    set_static_value(values, target_key, value);
}

fn copy_static_prefix(
    values: &mut Vec<(PlaceKey, StaticValue)>,
    source_key: &PlaceKey,
    target_key: &PlaceKey,
) {
    let initial_value_count = values.len();
    for index in 0..initial_value_count {
        let (existing_key, value) = &values[index];
        if !existing_key.starts_with(source_key) {
            continue;
        }

        let copied_name = existing_key.replace_prefix(source_key, target_key);
        let copied_value = value.clone();
        set_static_value(values, copied_name, copied_value);
    }
}
