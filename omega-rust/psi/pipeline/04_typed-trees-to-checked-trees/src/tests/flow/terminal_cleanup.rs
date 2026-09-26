//! Fixtures shared by the terminal cleanup tests: checked programs, scalar
//! discard positions and the machine entry state.

mod attached_returns_and_call_results;
mod machine_edges_and_projections;
mod structural_unit_control;

fn machine_and_entry_state(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(machine_name))
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    let state = checked
        .machine_states(machine)
        .first()
        .unwrap_or_else(|| panic!("machine `{machine_name}` has no entry state"));
    (machine.symbol, state.symbol)
}
