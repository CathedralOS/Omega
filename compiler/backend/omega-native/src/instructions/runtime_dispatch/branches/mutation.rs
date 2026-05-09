use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;

use super::super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::super::storage_places::{resolve_machine_owned_place, static_integer_value};
use super::super::writes::runtime_storage_copy;

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_resolved_mutation_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation_key: StateKey,
    _source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_key.machine,
        operation_key.machine,
        resolved_target,
    ) && let Some(value) = static_integer_value(&native_plan.layouts, resolved_value)
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
        native_plan,
        dispatch_index,
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
