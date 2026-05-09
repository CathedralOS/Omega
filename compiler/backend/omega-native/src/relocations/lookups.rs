use crate::plan::NativePlan;
use omega_calling_conventions::HostBinding;
use omega_core::diagnostics::Diagnostic;
use omega_target_program::FunctionInstructionPlan;

pub(super) fn selected_instruction_text_offset(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
) -> Result<usize, Diagnostic> {
    let Some(machine_function) = native_plan
        .machine_code
        .functions
        .iter()
        .find(|(_, machine_function)| machine_function.symbol == function.symbol)
        .map(|(_, machine_function)| machine_function)
    else {
        return Err(Diagnostic::error(format!(
            "cannot plan relocations for `{}`: missing machine-code function",
            function.symbol
        )));
    };

    let Some(machine_instructions) = native_plan
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
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBinding> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| binding)
}
