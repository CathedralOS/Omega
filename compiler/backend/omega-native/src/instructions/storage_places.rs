mod expressions;
mod machine_owned;
mod model;
mod nested_fields;
mod static_values;

pub(super) use expressions::indexed_expression_path;
pub(super) use machine_owned::resolve_machine_owned_place;
pub(super) use model::RuntimeStoragePlace;
pub(super) use static_values::{enum_variant_value, static_integer_value};

use crate::control_flow::StateKey;
use crate::object::{machine_storage_symbol_name, runtime_frame_storage_symbol_name};
use crate::plan::NativePlan;
use expressions::normalized_storage_expression;
use nested_fields::resolve_nested_field_layout;
use omega_layout::{FieldLayout, TypeLayout};
use omega_typed_program::expression::Expression;

pub(super) fn resolve_runtime_storage_place(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    _source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    if let Some((byte_offset, byte_count)) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_machine_name(),
        source_machine,
        expression,
    ) {
        return Some(RuntimeStoragePlace {
            symbol: machine_storage_symbol_name(native_plan.entry_machine_name()),
            byte_offset,
            byte_count,
        });
    }

    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let slot = native_plan
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot.name == *root_name
        })
        .or_else(|| {
            native_plan
                .runtime_storage
                .frame_slots
                .iter()
                .find(|(_, slot)| slot.dispatch_index == dispatch_index && slot.name == *root_name)
        })
        .map(|(_, slot)| slot)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout(&native_plan.layouts, &root_field, suffix)?;

    Some(RuntimeStoragePlace {
        symbol: runtime_frame_storage_symbol_name(),
        byte_offset,
        byte_count: layout.size,
    })
}
