mod context;
mod host_operation;
mod runtime_storage;
mod runtime_storage_addresses;
mod runtime_storage_compares;
mod runtime_storage_copies;
mod runtime_storage_strings;
mod runtime_text;
mod runtime_text_read;
mod runtime_values;

use super::data_addresses::collect_data_address_relocations;
use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_object_file::{ObjectSymbolHandle, RelocationPlan};
use omega_target_operations::{SelectedInstruction, SelectedInstructionKind};

pub(super) fn collect_instruction_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) {
    let mut context = InstructionRelocationContext {
        input,
        function_symbol_handle,
        selected_instruction_index,
        selected_text_offset,
        relocation_plan,
    };

    match &instruction.kind {
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } => {
            collect_data_address_relocations(
                input,
                function_symbol_handle,
                selected_instruction_index,
                Some(*operation_key),
                *operands,
                selected_text_offset,
                context.relocation_plan,
            );
            host_operation::collect_host_operation_relocation(&mut context, &instruction.kind);
        }
        _ if runtime_storage::collect_runtime_storage_relocations(
            &mut context,
            &instruction.kind,
        ) => {}
        _ => runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind),
    }
}
