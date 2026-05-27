use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;

use super::super::super::storage_places::{
    resolve_fixed_array_length, resolve_fixed_array_length_in_table,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
};
use super::fixed_array_slices::{
    literal_fixed_array_slice_source_in_table, literal_subslice_bounds,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn emit_runtime_frame_slot_slice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    if emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        slot,
        value,
        selected_instructions,
        statement_index,
    ) {
        return true;
    }

    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        let receiver = expressions.to_tree(call.receiver);
        let simplified_receiver = super::mutation::simplify_runtime_expression_with_state_locals(
            input,
            value_source_key,
            statement_index,
            &receiver,
        );
        if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) {
            let Some(length) = resolve_fixed_array_length(
                input,
                dispatch_index,
                value_source_key,
                &simplified_receiver,
            ) else {
                return false;
            };

            emit_indexed_slice_descriptor(
                input,
                value_source_key,
                statement_index,
                slot,
                indexed_target.base_byte_offset,
                indexed_target.index_offset,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                length,
                selected_instructions,
            );
            return true;
        }

        let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            value_source_key,
            "",
            "",
            &simplified_receiver,
        ) else {
            return false;
        };
        let Some(length) = resolve_fixed_array_length(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) else {
            return false;
        };

        emit_static_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            source_place.region,
            source_place.byte_offset,
            length,
            selected_instructions,
        );
        return true;
    };

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };

        emit_indexed_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            indexed_target.base_byte_offset,
            indexed_target.index_offset,
            indexed_target.element_byte_size,
            indexed_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source_place.region,
        source_place.byte_offset,
        length,
        selected_instructions,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn emit_indexed_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_static_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region,
            source_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

fn emit_slice_descriptor_length(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + input.runtime_abi.pointer_size,
            byte_size: input.runtime_abi.pointer_size,
            value: length as i64,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
    statement_index: usize,
) -> bool {
    let ExpressionNode::Indexed(indexed) = expressions.expression(value) else {
        return false;
    };
    let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
        return false;
    };
    let Some(source) = literal_fixed_array_slice_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) else {
        return false;
    };
    let Some((start, length)) = literal_subslice_bounds(expressions, range, source.length) else {
        return false;
    };
    let Some(source_offset) = start
        .checked_mul(source.element_byte_size)
        .and_then(|offset| source.place.byte_offset.checked_add(offset))
    else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source.place.region,
        source_offset,
        length,
        selected_instructions,
    );
    true
}
