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
/// boundary-trait receiver -- the owning signature's parameters.
pub(crate) fn call_target_parameters<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<&'program [omega_typed_trees::signature::StateParameter]> {
    if let Some(state) = find_state(program, target_state_symbol) {
        return Some(program.state_parameters(state));
    }

    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(program.state_signature_parameters(signature));
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
        .map(|signature| program.state_signature_parameters(signature))
}

/// The generic declaration context that owns a call target's formal
/// parameters. Carry and other structural properties must interpret `T`
/// against these bounds, not against an unrelated caller's same-spelled type
/// parameter and not as an invented concrete type.
pub(crate) fn call_target_type_parameters(
    program: &omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> &[omega_typed_trees::data::TypeParameter] {
    if let Some(machine) = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state_symbol)
    }) {
        return program.machine_type_parameters(machine);
    }

    if let Some((machine, _)) = program.machine_parameter_signature(target_state_symbol) {
        return program.machine_type_parameters(machine);
    }

    if let Some(trait_definition) = program.traits().iter().find(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.symbol == target_state_symbol)
    }) {
        return program.trait_type_parameters(trait_definition);
    }

    &[]
}
