use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{SymbolHandle, SymbolKind};

/// The CANONICAL entry: `machine Main::run(&self, args: &[u8])` -- Main's
/// members are the program's statics, and `args` is the platform handoff as raw
/// bytes (cast/mint to the desired type).
///
/// MIGRATION PRECEDENCE: `Main::main` resolves FIRST while it exists -- the
/// corpus has programs whose `Main::run` is an ordinary HELPER machine beside
/// their `Main::main` entry, and canonical-first silently made the helper the
/// program entry (garbage params, wrong flow). Preferring the legacy name keeps
/// every existing program's meaning; a program with no `Main::main` gets the
/// canonical `run`. The final corpus sweep retires `main` and flips this.
pub(super) const ENTRY_MACHINE_NAME: &str = "Main::run";
pub(super) const ENTRY_STATE_NAME: &str = "run";
const LEGACY_MAIN_MACHINE_NAME: &str = "Main::main";
const LEGACY_MAIN_STATE_NAME: &str = "main";
const LEGACY_ENTRY_MACHINE_NAME: &str = "main";
const LEGACY_ENTRY_STATE_NAME: &str = "entry";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackendEntryPoint {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
}

pub(super) fn resolve_backend_entry_point(
    program: &CheckedTrees,
) -> Result<BackendEntryPoint, Diagnostic> {
    if let Some(entry_point) =
        find_entry_point(program, LEGACY_MAIN_MACHINE_NAME, LEGACY_MAIN_STATE_NAME)
    {
        return Ok(entry_point);
    }

    if let Some(entry_point) = find_entry_point(program, ENTRY_MACHINE_NAME, ENTRY_STATE_NAME) {
        return Ok(entry_point);
    }

    if let Some(entry_point) =
        find_entry_point(program, LEGACY_ENTRY_MACHINE_NAME, LEGACY_ENTRY_STATE_NAME)
    {
        return Ok(entry_point);
    }

    Err(Diagnostic::error(
        "unknown runtime entry point `Main::run`",
    ))
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
