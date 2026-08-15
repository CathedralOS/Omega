//! Omega-owned boundary-provider admission after Psi semantic checking.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn check_boundary_provider_approval(
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    let registry = omega_effects::build_boundary_provider_approval_registry(program);
    let flow = &checked.facts.flow.control;
    let calls = flow.states.iter().flat_map(|(_, state)| {
        flow.calls
            .span_or_empty(state.calls)
            .iter()
            .map(move |call| omega_effects::BoundaryCallCoordinate {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                target_state_symbol: call.target_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            })
    });
    let unapproved = omega_effects::audit_boundary_provider_calls(program, calls, &registry);

    if unapproved.is_empty() {
        return Ok(());
    }

    Err(unapproved
        .into_iter()
        .map(|call| {
            Diagnostic::error(format!(
                "unapproved boundary call: {} in {} exercises a boundary capability with no approved provider for that exact capability",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
            ))
        })
        .collect())
}

fn symbol_name(program: &psi_typed_trees::TypedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "unknown".to_owned();
    }

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str().to_owned();
    }

    if let Some(state) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
    {
        return state.name.as_str().to_owned();
    }

    program.symbols.name(symbol).to_owned()
}
