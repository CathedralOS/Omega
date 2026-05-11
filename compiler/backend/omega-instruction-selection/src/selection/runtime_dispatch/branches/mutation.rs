use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_typed_trees::expression::Expression;

use super::super::super::storage_places::{resolve_machine_owned_place, static_integer_value};
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
    if let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &input.layouts,
        input.entry_key.machine,
        operation_key.machine,
        resolved_target,
    ) && let Some(value) = static_integer_value(&input.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
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
    }
}
