use omega_checked_trees::Program;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{SymbolHandle, SymbolKind};

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
    let machine_symbol =
        find_root_child_by_name_and_kind(program, ENTRY_MACHINE_NAME, SymbolKind::Machine)
            .ok_or_else(|| Diagnostic::error("unknown runtime machine `main`"))?;
    let state_symbol =
        find_child_by_name_and_kind(program, machine_symbol, ENTRY_STATE_NAME, SymbolKind::State)
            .ok_or_else(|| Diagnostic::error("unknown runtime state `main.entry`"))?;

    Ok(BackendEntryPoint {
        machine_symbol,
        state_symbol,
    })
}

fn find_root_child_by_name_and_kind(
    program: &Program,
    name: &str,
    kind: SymbolKind,
) -> Option<SymbolHandle> {
    find_child_by_name_and_kind(program, program.symbols.root(), name, kind)
}

fn find_child_by_name_and_kind(
    program: &Program,
    parent: SymbolHandle,
    name: &str,
    kind: SymbolKind,
) -> Option<SymbolHandle> {
    let children = program.symbols.child_handles(parent)?;

    for child in children {
        let symbol = program.symbols.get(child);
        if symbol.kind == kind && program.symbols.name(child) == name {
            return Some(child);
        }
    }

    None
}
