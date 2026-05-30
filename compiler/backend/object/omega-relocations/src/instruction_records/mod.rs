mod context;
mod host_operation;
mod queries;
mod runtime_storage;
mod runtime_storage_addresses;
mod runtime_storage_compares;
mod runtime_storage_copies;
mod runtime_storage_strings;
mod runtime_storage_writes;
mod runtime_text;
mod runtime_text_append;
mod runtime_text_compare;
mod runtime_text_materialize;
mod runtime_text_read;
mod runtime_text_write;
mod runtime_values;

use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_object_file::{ObjectSymbolHandle, RelocationPlan};
use omega_target_operations::SelectedInstruction;

pub(super) fn collect_instruction_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    selected_text_width: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) {
    let mut context = InstructionRelocationContext {
        input,
        function_symbol_handle,
        selected_instruction_index,
        selected_text_offset,
        selected_text_width,
        relocation_plan,
    };

    match &instruction.kind {
        _ if host_operation::collect_host_operation_relocations(
            &mut context,
            &instruction.kind,
        ) => {}
        _ if runtime_storage::collect_runtime_storage_relocations(
            &mut context,
            &instruction.kind,
        ) => {}
        _ => runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind),
    }
}
