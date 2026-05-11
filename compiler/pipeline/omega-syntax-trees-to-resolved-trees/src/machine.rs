use crate::program::Lowerer;
use crate::state::lower_state;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees as syntax;
use omega_resolved_trees::machine::Machine;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &syntax::item::Machine,
) -> Result<Machine, Diagnostic> {
    let states = machine
        .states
        .iter()
        .map(|state| lower_state(lowerer, state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Machine {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&machine.name),
        contains: Vec::new(),
        owned_data: Vec::new(),
        states,
    })
}
