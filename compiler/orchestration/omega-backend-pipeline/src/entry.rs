use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::Program;

pub(super) const ENTRY_MACHINE_NAME: &str = "main";
pub(super) const ENTRY_STATE_NAME: &str = "entry";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackendEntryPoint {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
}

pub(super) fn resolve_backend_entry_point(
    program: &Program,
) -> Result<BackendEntryPoint, Diagnostic> {
    let machine_symbol = program
        .symbols
        .find_child_by_name(program.symbols.root(), ENTRY_MACHINE_NAME)
        .ok_or_else(|| Diagnostic::error("unknown runtime machine `main`"))?;
    let state_symbol = program
        .symbols
        .find_child_by_name(machine_symbol, ENTRY_STATE_NAME)
        .ok_or_else(|| Diagnostic::error("unknown runtime state `main.entry`"))?;

    Ok(BackendEntryPoint {
        machine_symbol,
        state_symbol,
    })
}
