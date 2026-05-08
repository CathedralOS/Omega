use crate::control_flow::StateKey;
use crate::data::NativeDataObject;
use crate::instructions::model::{SelectedInstruction, SelectedInstructionKind};
use crate::instructions::storage_places::resolve_machine_owned_place;
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;

#[allow(clippy::too_many_arguments)]
pub(in crate::instructions) fn select_runtime_string_descriptor_write(
    native_plan: &NativePlan,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) else {
        return;
    };
    if byte_size != native_plan.target.pointer_size * 2 {
        return;
    }
    let Some(data_object) =
        string_literal_data_object(native_plan, source_key, statement_index, value)
    else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data_symbol: data_object.symbol.clone(),
            byte_length: value.len(),
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

fn string_literal_data_object<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
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
        .map(|(_, data_object)| data_object)
}
