use crate::InstructionSelectionInput;
use crate::selection::storage_places::resolve_machine_owned_place;
use omega_control_flow::StateKey;
use omega_target_program::{NativeDataObject, NativeDataObjectHandle};
use omega_target_program::{SelectedInstruction, SelectedInstructionKind};
use omega_typed_program::expression::Expression;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_string_descriptor_write(
    native_plan: &InstructionSelectionInput<'_>,
    literal_source_key: StateKey,
    target_source_key: StateKey,
    _source_machine: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_key.machine,
        target_source_key.machine,
        resolved_target,
    ) else {
        return;
    };
    if byte_size != native_plan.target.pointer_size * 2 {
        return;
    }
    let Some((data, _data_object)) =
        string_literal_data_object(native_plan, literal_source_key, statement_index, value)
    else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data,
            byte_length: value.len(),
        },
        source_key: literal_source_key,
        source_statement: statement_index,
    });
}

fn string_literal_data_object<'plan>(
    native_plan: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<(NativeDataObjectHandle, &'plan NativeDataObject)> {
    native_plan.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == source_key
            && data_object.source_statement == statement_index
            && native_plan
                .data
                .bytes
                .span(data_object.bytes)
                .is_some_and(|bytes| {
                    bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                })
    })
}
