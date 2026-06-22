use crate::input::ObjectPlanningInput;
use omega_core::diagnostics::Diagnostic;
use omega_layout::MachineLayout;
use omega_machine_bytes::EncodedMachineFunction;

pub(crate) fn entry_machine_layout<'plan>(
    input: &ObjectPlanningInput<'plan>,
) -> Result<&'plan MachineLayout, Diagnostic> {
    input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == input.entry_machine_symbol)
        .map(|(_, layout)| layout)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing native layout for entry machine `{}`",
                input.entry_machine_name
            ))
        })
}

pub(crate) fn entry_function<'plan>(
    input: &ObjectPlanningInput<'plan>,
) -> Result<&'plan EncodedMachineFunction, Diagnostic> {
    input
        .encoded_machine
        .code
        .functions
        .iter()
        .find(|(_, function)| function.source_key == input.entry_state_key)
        .map(|(_, function)| function)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "missing encoded entry function for state key {:?}",
                input.entry_state_key
            ))
        })
}
