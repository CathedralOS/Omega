use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackendEntryPoint {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
}

pub(super) fn resolve_backend_entry_point(
    program: &CheckedTrees,
    entry_machine_name: Option<&str>,
) -> Result<BackendEntryPoint, Diagnostic> {
    if let Some(machine_name) = entry_machine_name {
        return find_declared_entry_point(program, machine_name).ok_or_else(|| {
            Diagnostic::error(format!(
                "build root slot names unknown entry machine `{machine_name}`"
            ))
        });
    }

    Err(Diagnostic::error(
        "no runtime entry point was selected; bind the target-owned `ProgramEntry` root in build.omg",
    ))
}

fn find_declared_entry_point(
    program: &CheckedTrees,
    machine_name: &str,
) -> Option<BackendEntryPoint> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)?;
    let entry_state = program.machine_states(machine).first()?;
    Some(BackendEntryPoint {
        machine_symbol: machine.symbol,
        state_symbol: entry_state.symbol,
    })
}
