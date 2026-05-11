use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_typed_trees::expression::Expression;

use super::super::super::storage_places::{
    resolve_runtime_frame_indexed_target, resolve_runtime_storage_place, static_integer_value,
};
use super::super::writes::runtime_storage_copy;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_target_operations::{SelectedInstruction, SelectedInstructionKind};

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_resolved_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    _source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
    ) && let Some(value) = static_integer_value(&input.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: target_place.region,
                byte_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                value,
            },
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        input,
        dispatch_index,
        operation_key,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        operation_key,
        resolved_target,
    ) {
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            operation_key,
            operation_machine,
            operation_state,
            resolved_value,
        ) && source_place.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
                source_key: operation_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(value) = static_integer_value(&input.layouts, resolved_value) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_size: indexed_target.byte_count,
                    value,
                },
                source_key: operation_key,
                source_statement: statement_index,
            });
        }
    }
}
