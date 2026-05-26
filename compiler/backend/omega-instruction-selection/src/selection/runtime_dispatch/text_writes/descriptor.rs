use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::storage_places::{
    resolve_runtime_frame_indexed_target, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_storage_place,
};
use omega_abstract_operations::TargetDataObjectHandle;
use omega_abstract_operations::{SelectedInstruction, SelectedInstructionKind};
use omega_checked_trees::expression::Expression;
use omega_control_flow::StateKey;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_string_descriptor_write(
    input: &InstructionSelectionInput<'_>,
    literal_source_key: StateKey,
    target_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    dispatch_index: u32,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let data = string_literal_data_object(input, literal_source_key, statement_index, value);
    if !data.is_valid() {
        return;
    };

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeFrameIndexedString {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                data,
                byte_length: value.len(),
            },
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let mut machine_indexed_expressions =
        omega_checked_trees::expression::ExpressionTable::default();
    let target_handle = machine_indexed_expressions.insert_tree(resolved_target);
    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        &machine_indexed_expressions,
        target_handle,
    ) && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineIndexedString {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                data,
                byte_length: value.len(),
            },
            source_key: literal_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    ) else {
        return;
    };
    if target_place.byte_count != input.runtime_abi.string_descriptor_size() {
        return;
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset: target_place.byte_offset,
            data,
            byte_length: value.len(),
        },
        source_key: literal_source_key,
        source_statement: statement_index,
    });
}

pub(in crate::selection) fn string_literal_data_handle(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> TargetDataObjectHandle {
    string_literal_data_object(input, source_key, statement_index, value)
}

fn string_literal_data_object(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> TargetDataObjectHandle {
    let exact = input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == source_key
            && data_object.source_statement == statement_index
            && input
                .data
                .bytes
                .span(data_object.bytes)
                .is_some_and(|bytes| {
                    bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                })
    });
    exact
        .or_else(|| {
            input.data.objects.iter().find(|(_, data_object)| {
                input
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .is_some_and(|bytes| {
                        bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                    })
            })
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(TargetDataObjectHandle::invalid)
}
