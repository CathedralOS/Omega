use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardOperator,
};
use omega_control_flow::StateKey;
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

use super::super::super::storage_places::{
    resolve_runtime_frame_base_double_indexed_source_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use super::super::text_writes::string_literal_data_handle;

pub(in crate::selection) fn emit_runtime_frame_slot_text_comparison_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != 1 {
        return false;
    }

    let ExpressionNode::Binary(binary) = expressions.expression(value) else {
        return false;
    };
    let Some(text_equals) = super::mutation::resolve_runtime_text_equals_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        binary.operator,
        binary.left,
        binary.right,
        runtime_value_operands,
    ) else {
        return false;
    };
    let zero = runtime_value_operands.insert(RuntimeValueOperand::Immediate(0));
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_binary_direct(
            RuntimeStorageRegion::RuntimeFrame,
            slot.byte_offset,
            slot.byte_size,
            text_equals,
            StateGuardOperator::Or,
            zero,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            false,
        ),
        source_key: value_source_key,
        source_statement: statement_index,
    });
    true
}

pub(super) fn select_runtime_string_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = expressions.string_literal_value(value)?;
    let data = string_literal_data_handle(input, operation_source_key, statement_index, &value);

    if let Some(indexed_target) = resolve_runtime_frame_base_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && indexed_target.is_bounded_byte_buffer
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_bounded_buffer_frame_base_double_indexed(
                indexed_target.base_byte_offset,
                indexed_target.outer_index_offset,
                indexed_target.outer_index_byte_size,
                indexed_target.outer_stride,
                indexed_target.inner_index_offset,
                indexed_target.inner_index_byte_size,
                indexed_target.inner_stride,
                indexed_target.field_byte_offset,
                value.clone(),
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
        && indexed_target.is_bounded_byte_buffer
    {
        return Some(SelectedInstructionKind::WritePlaceBoundedBuffer {
            target: crate::selection::runtime_dispatch::frame_base_indexed_place_with_index_region(
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
            ),
            literal: value.clone(),
        });
    }

    // An owned `[u8; N]` carrier must NOT be claimed as a `{ptr, len}` String
    // descriptor (its `{len, bytes}` size can even equal the descriptor size, e.g.
    // `[u8; 8]` -> 16 bytes). A carrier reached THROUGH a pointer (a slice
    // element's / pointee field, `rooms[0].label = "Gate"`) writes its `{len,
    // bytes}` inline through the pointer; a direct machine-resident carrier defers
    // to the mutation pass (which emits WriteRuntimeMachineBoundedBuffer).
    if crate::selection::storage_places::resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(crate::selection::runtime_dispatch::write_place_bounded_buffer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value.clone(),
            ));
        }
        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(crate::selection::runtime_dispatch::write_place_bounded_buffer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value.clone(),
            ));
        }
        return None;
    }

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_base_double_indexed_source_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_frame_base_double_indexed(
                indexed_target.base_byte_offset,
                indexed_target.outer_index_offset,
                indexed_target.outer_index_byte_size,
                indexed_target.outer_stride,
                indexed_target.inner_index_offset,
                indexed_target.inner_index_byte_size,
                indexed_target.inner_stride,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) =
            resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_frame_base_indexed_with_index_region(
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
        && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                indexed_target.descriptor_offset,
                field_byte_offset,
                data,
                value.len(),
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
    if target_place.byte_count != input.runtime_abi.string_descriptor_size() || !data.is_valid() {
        return None;
    }

    // Honor the resolved region: a frame-resident place (e.g. a `let` local's
    // String field) must be a frame write, not a machine-storage write. Emitting
    // WriteRuntimeMachineString unconditionally aimed the write at machine offset
    // `target_place.byte_offset`, which for a frame slot at offset 0 collided with
    // the machine-storage region base -- corrupting unrelated state.
    match target_place.region {
        region @ (RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine) => Some(
            crate::selection::runtime_dispatch::write_place_string_direct(
                region,
                target_place.byte_offset,
                data,
                value.len(),
            ),
        ),
    }
}
