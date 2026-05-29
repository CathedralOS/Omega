use crate::InstructionSelectionInput;
use crate::selection::runtime_dispatch::writes::mutation::operators::supports_scalar_integer_write;
use crate::selection::storage_places::{
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::SelectedInstructionKind;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value_in_table,
    set_runtime_static_value_in_table,
};

pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_static_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    _statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
) -> Option<SelectedInstructionKind> {
    let value =
        resolve_runtime_static_integer_value_in_table(input, expressions, value, static_values)?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            value,
        });
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
                value,
            },
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset: indexed_target.base_byte_offset,
            index_region: indexed_target.index_region,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            value,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(pointer_target.pointee_byte_size)
    {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            value,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            value,
        });
    }

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(target_place.byte_count) {
        return None;
    }

    set_runtime_static_value_in_table(static_values, expressions, target, value);
    Some(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: target_place.region,
        byte_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        value,
    })
}
