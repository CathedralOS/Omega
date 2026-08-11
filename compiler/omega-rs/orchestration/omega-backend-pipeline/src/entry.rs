use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};

/// Transitional entry name used only while the corpus moves to explicit
/// target-owned `ProgramEntry` bindings.
const LEGACY_MAIN_MACHINE_NAME: &str = "Main::main";
const LEGACY_MAIN_STATE_NAME: &str = "main";

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

    if let Some(entry_point) =
        find_entry_point(program, LEGACY_MAIN_MACHINE_NAME, LEGACY_MAIN_STATE_NAME)
    {
        return Ok(entry_point);
    }

    Err(Diagnostic::error(
        "unknown runtime entry point `Main::main`",
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

fn find_entry_point(
    program: &CheckedTrees,
    machine_name: &str,
    state_name: &str,
) -> Option<BackendEntryPoint> {
    let machine_symbol =
        find_root_child_by_name_and_kind(program, machine_name, SymbolKind::Machine)?;
    let state_symbol =
        find_child_by_name_and_kind(program, machine_symbol, state_name, SymbolKind::State)?;

    Some(BackendEntryPoint {
        machine_symbol,
        state_symbol,
    })
}

fn find_root_child_by_name_and_kind(
    program: &CheckedTrees,
    name: &str,
    kind: SymbolKind,
) -> Option<SymbolHandle> {
    find_child_by_name_and_kind(program, program.symbols.root(), name, kind)
}

fn find_child_by_name_and_kind(
    program: &CheckedTrees,
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
