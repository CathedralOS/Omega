use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolKind;
use omega_typed_trees::TypedTrees;

pub(crate) fn validate_entry_point(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    if has_entry_point(program, "Main::main", "main") || has_entry_point(program, "main", "entry") {
        return;
    }

    diagnostics.push(Diagnostic::error(
        "missing runtime entry point `Main::main`",
    ));
}

fn has_entry_point(program: &TypedTrees, machine_name: &str, state_name: &str) -> bool {
    let machine_symbol = program.symbols.find_child_by_name_and_kind(
        program.symbols.root(),
        machine_name,
        SymbolKind::Machine,
    );
    let Some(machine) = machine_symbol.and_then(|machine_symbol| {
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
    }) else {
        return false;
    };

    let state_symbol =
        program
            .symbols
            .find_child_by_name_and_kind(machine.symbol, state_name, SymbolKind::State);

    program
        .machine_states(machine)
        .iter()
        .any(|state| Some(state.symbol) == state_symbol)
}
