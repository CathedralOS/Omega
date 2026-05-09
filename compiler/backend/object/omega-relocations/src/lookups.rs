use crate::RelocationPlanningInput;
use omega_calling_conventions::HostBinding;
use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;
use omega_target_program::FunctionInstructionPlan;

pub(super) fn selected_instruction_text_offset(
    input: RelocationPlanningInput<'_>,
    function_handle: Handle<FunctionInstructionPlan>,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
) -> Result<usize, Diagnostic> {
    let Some(machine_function) = input
        .machine_code
        .functions
        .iter()
        .find(|(_, machine_function)| machine_function.source_function == function_handle)
        .map(|(_, machine_function)| machine_function)
    else {
        return Err(Diagnostic::error(format!(
            "cannot plan relocations for `{}`: missing machine-code function",
            function.symbol
        )));
    };

    let Some(machine_instructions) = input
        .machine_code
        .instructions
        .span(machine_function.instructions)
    else {
        return Err(Diagnostic::error(format!(
            "cannot plan relocations for `{}`: invalid machine instruction span",
            function.symbol
        )));
    };

    machine_instructions
        .iter()
        .find(|instruction| instruction.selected_instruction_index == selected_instruction_index)
        .map(|instruction| instruction.offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "cannot plan relocation for `{}` selected instruction #{}: missing machine-code instruction",
                function.symbol, selected_instruction_index
            ))
        })
}

pub(super) fn find_host_binding<'plan>(
    input: RelocationPlanningInput<'plan>,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBinding> {
    let operation_key =
        omega_calling_conventions::HostOperationKey::from_names(capability, operation);

    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}
