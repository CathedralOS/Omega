use super::*;

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression(call) => program.expression_table.expression_handles(call.arguments),
        CallSite::TransitionNamed(arguments) => {
            program.statement_table.expression_handles(*arguments)
        }
    }
}

pub(crate) fn find_state_in_machine<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

pub(crate) fn find_state<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}

/// The parameter list of a call target: a machine state's parameters, or --
/// for a call through a trait-typed receiver (boundary trait machines) or a
/// platform-typed receiver (platform entries) -- the owning signature's
/// parameters.
pub(crate) fn call_target_parameters<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<&'program [omega_typed_trees::signature::StateParameter]> {
    if let Some(state) = find_state(program, target_state_symbol) {
        return Some(program.state_parameters(state));
    }

    program
        .traits()
        .iter()
        .find_map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .find(|signature| signature.symbol == target_state_symbol)
        })
        .or_else(|| {
            program.platforms().iter().find_map(|platform| {
                program
                    .platform_state_signatures(platform)
                    .iter()
                    .find(|signature| signature.symbol == target_state_symbol)
            })
        })
        .map(|signature| program.state_signature_parameters(signature))
}
