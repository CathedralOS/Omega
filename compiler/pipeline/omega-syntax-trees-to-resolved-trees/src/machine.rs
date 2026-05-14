use crate::program::Lowerer;
use crate::state::lower_state;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::machine::Machine;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_machine_into(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    machine: &syntax::item::Machine,
) -> Result<(), Diagnostic> {
    let states = machine
        .states
        .iter()
        .map(|state| lower_state(lowerer, syntax_trees, state))
        .collect::<Result<Vec<_>, _>>()?;
    let machine_name = crate::name::lower_name(&machine.name);

    if let Some(existing_machine) = lowerer
        .program
        .machines
        .iter_mut()
        .find(|existing_machine| existing_machine.name == machine_name)
    {
        existing_machine.states.extend(states);
        return Ok(());
    }

    lowerer.program.machines.push(Machine {
        symbol: SymbolHandle::invalid(),
        name: machine_name,
        contains: Vec::new(),
        owned_data: Vec::new(),
        states,
    });
    Ok(())
}
