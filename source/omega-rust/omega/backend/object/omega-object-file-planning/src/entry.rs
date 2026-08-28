use crate::input::ObjectPlanningInput;
use omega_layout::MachineLayout;
use omega_machine_bytes::EncodedMachineFunction;
use omega_object_file::entry_symbol_name;
use psi_diagnostics::Diagnostic;

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
    let mut matches = input
        .encoded_machine
        .code
        .functions
        .iter()
        .filter(|(_, function)| function.symbol.as_ref() == entry_symbol_name(input.target));
    let Some((_, entry)) = matches.next() else {
        return Err(Diagnostic::error(format!(
            "missing encoded entry function `{}` for identity {:?}",
            entry_symbol_name(input.target),
            input.entry_function_identity,
        )));
    };
    if matches.next().is_some() {
        Err(Diagnostic::error(format!(
            "encoded entry function `{}` is ambiguous for identity {:?}",
            entry_symbol_name(input.target),
            input.entry_function_identity,
        )))
    } else {
        if entry.identity != input.entry_function_identity {
            return Err(Diagnostic::error(format!(
                "encoded entry function `{}` has identity {:?}, not selected identity {:?}",
                entry_symbol_name(input.target),
                entry.identity,
                input.entry_function_identity,
            )));
        }
        if !entry.identity.is_valid() {
            return Err(Diagnostic::error(format!(
                "encoded entry function `{}` has invalid identity {:?}",
                entry_symbol_name(input.target),
                entry.identity,
            )));
        }
        Ok(entry)
    }
}
