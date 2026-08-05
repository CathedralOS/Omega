use crate::InstructionSelectionInput;
use crate::selection::runtime_dispatch::writes::mutation::operators::supports_scalar_integer_write;
use crate::selection::storage_places::{
    resolve_runtime_bit_field_place_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{RuntimeBitFieldFragment, SelectedInstructionKind};
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

use super::super::static_values::{
    RuntimeStaticInteger, RuntimeStaticValues, resolve_runtime_static_float_value_in_table,
    resolve_runtime_static_integer_in_table, set_runtime_static_value_in_table,
};

/// A compile-time-constant scalar destined for a runtime storage slot. Integers
/// store directly; floats are narrowed to the destination width and stored as
/// their IEEE-754 bit pattern (an 8-byte slot holds the `f64` bits, a 4-byte
/// slot the `f32` bits), so a constant float write is just a scalar integer
/// write of those bits — no SSE register traffic required.
enum StaticWriteValue {
    Integer(RuntimeStaticInteger),
    Float(f64),
}

impl StaticWriteValue {
    fn stored_integer(&self, byte_size: usize) -> RuntimeStaticInteger {
        match self {
            Self::Integer(value) => *value,
            Self::Float(value) if byte_size <= 4 => {
                RuntimeStaticInteger::anonymous(i64::from((*value as f32).to_bits()))
            }
            Self::Float(value) => RuntimeStaticInteger::anonymous(value.to_bits() as i64),
        }
    }
}

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
        match resolve_runtime_static_integer_in_table(input, expressions, value, static_values) {
            Some(integer) => StaticWriteValue::Integer(integer),
            None => StaticWriteValue::Float(resolve_runtime_static_float_value_in_table(
                expressions,
                value,
            )?),
        };

    if let Some(bit_target) = resolve_runtime_bit_field_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let value = value.stored_integer(bit_target.value_byte_count);
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteStorageBitField {
            region: bit_target.region,
            base_byte_offset: bit_target.base_byte_offset,
            fragments: bit_target
                .fragments
                .into_iter()
                .map(|fragment| RuntimeBitFieldFragment {
                    container_byte_offset: fragment.container_byte_offset,
                    container_width_bits: fragment.container_width_bits,
                    destination_lsb: fragment.destination_lsb,
                    source_lsb: fragment.source_lsb,
                    width: fragment.width,
                })
                .collect(),
            value: value.bits(),
        });
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                value.stored_integer(indexed_target.byte_count).bits(),
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_base_indexed(
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                value.stored_integer(indexed_target.byte_count).bits(),
                indexed_target.byte_count,
            ),
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
        let value = value.stored_integer(indexed_target.byte_count);
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_base_indexed(
                omega_target_operations::RuntimeStorageRegion::Machine,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                value.bits(),
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
        && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
    {
        let value = value.stored_integer(indexed_target.byte_count);
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_pointee(
                indexed_target.descriptor_offset,
                field_byte_offset,
                value.bits(),
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(pointer_target.pointee_byte_size)
    {
        let value = value.stored_integer(pointer_target.pointee_byte_size);
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value.bits(),
                pointer_target.pointee_byte_size,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let value = value.stored_integer(pointer_target.pointee_byte_size);
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value.bits(),
                pointer_target.pointee_byte_size,
            ),
        );
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

    let value = value.stored_integer(target_place.byte_count);
    set_runtime_static_value_in_table(static_values, expressions, target, value);
    Some(
        crate::selection::runtime_dispatch::write_place_integer_direct(
            target_place.region,
            target_place.byte_offset,
            value.bits(),
            target_place.byte_count,
        ),
    )
}
