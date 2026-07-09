use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind, StateGuardOperator,
};
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_control_flow::StateKey;

use super::super::super::storage_places::{
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
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != 1 {
        return false;
    }

    let ExpressionNode::Binary(binary) = expressions.expression(value) else {
        return false;
    };
    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return false,
    };

    let left_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        binary.left,
    );
    let right_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        binary.right,
    );
    let string_descriptor_size = input.runtime_abi.string_descriptor_size();

    let literal_buffer_and_place = if let Some(literal) =
        expressions.string_literal_value(binary.left)
    {
        let source_place = right_place.filter(|place| place.byte_count == string_descriptor_size);
        source_place.map(|source_place| {
            (
                string_literal_data_handle(input, value_source_key, statement_index, &literal),
                source_place,
            )
        })
    } else if let Some(literal) = expressions.string_literal_value(binary.right) {
        let source_place = left_place.filter(|place| place.byte_count == string_descriptor_size);
        source_place.map(|source_place| {
            (
                string_literal_data_handle(input, value_source_key, statement_index, &literal),
                source_place,
            )
        })
    } else {
        None
    };

    let Some((buffer, source_place)) = literal_buffer_and_place else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset,
            byte_size: slot.byte_size,
            value: 1,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset,
            byte_size: slot.byte_size,
            value: 0,
        },
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
            return Some(SelectedInstructionKind::WriteRuntimePointeeBoundedBuffer {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                literal: std::sync::Arc::from(value.to_string()),
            });
        }
        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(SelectedInstructionKind::WriteRuntimePointeeBoundedBuffer {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                literal: std::sync::Arc::from(value.to_string()),
            });
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
        return Some(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
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
        return Some(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
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
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
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
        return Some(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: indexed_target.descriptor_offset,
            field_byte_offset,
            data,
            byte_length: value.len(),
        });
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
        RuntimeStorageRegion::RuntimeFrame => Some(SelectedInstructionKind::WriteRuntimeFrameString {
            byte_offset: target_place.byte_offset,
            data,
            byte_length: value.len(),
        }),
        RuntimeStorageRegion::Machine => Some(SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset: target_place.byte_offset,
            data,
            byte_length: value.len(),
        }),
    }
}
