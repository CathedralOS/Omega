use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::storage_places::resolve_runtime_storage_place;
use omega_control_flow::StateKey;
use omega_target_operations::{SelectedInstruction, SelectedInstructionKind};
use omega_target_operations::{TargetDataObject, TargetDataObjectHandle};
use omega_typed_trees::expression::Expression;

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
    if target_place.byte_count != input.target.pointer_size * 2 {
        return;
    }
    let Some((data, _data_object)) =
        string_literal_data_object(input, literal_source_key, statement_index, value)
    else {
        return;
    };

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

fn string_literal_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<(TargetDataObjectHandle, &'plan TargetDataObject)> {
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
    exact.or_else(|| {
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
}
