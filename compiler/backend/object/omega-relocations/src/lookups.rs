use crate::RelocationPlanningInput;
use omega_calling_conventions::{HostBinding, HostOperationKey};
use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;
use omega_target_operations::FunctionInstructionPlan;

pub(super) fn selected_instruction_text_offset(
    input: RelocationPlanningInput<'_>,
    _function_handle: Handle<FunctionInstructionPlan>,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
) -> Result<usize, Diagnostic> {
    let mut offset = 0usize;

    for (_, instruction) in input.encoded_machine.code.instructions.iter() {
        let byte_len = input
            .encoded_machine
            .code
            .bytes
            .span(instruction.bytes)
            .map(|bytes| bytes.len())
            .unwrap_or(0);

        if instruction.selected_instruction_index == selected_instruction_index {
            return Ok(offset);
        }

        offset += byte_len;
    }

    Err(Diagnostic::error(format!(
        "cannot plan relocation for `{}` selected instruction #{}: missing encoded instruction bytes",
        function.symbol, selected_instruction_index
    )))
}

pub(super) fn find_host_binding<'plan>(
    input: RelocationPlanningInput<'plan>,
    operation_key: HostOperationKey,
) -> Option<&'plan HostBinding> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}
