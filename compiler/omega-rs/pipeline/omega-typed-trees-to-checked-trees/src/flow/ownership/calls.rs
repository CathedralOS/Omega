use super::*;

pub(in crate::flow) fn append_call_ownership_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) {
    let Some(call_site) = find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
    let arguments = call_site_argument_expressions(program, &call_site);
    let Some(target_state) = find_state(program, borrow_call.target_symbol) else {
        return;
    };

    let declared_parameters = program.state_parameters(target_state);
    // A static invocation of a consuming attached machine spells the by-value
    // self explicitly (`Receipt::ack(receipt)`). In that shape the argument
    // count equals the full parameter count and self participates in ordinary
    // ownership transfer. Method-form calls bind self through the receiver and
    // expose only the non-self positional arguments here.
    let includes_explicit_self = declared_parameters.iter().any(|parameter| parameter.is_self)
        && arguments.len() == declared_parameters.len();
    let parameters = declared_parameters
        .iter()
        .filter(|parameter| includes_explicit_self || !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments.iter()) {
        if !type_requires_ownership(program, parameter.type_reference) {
            continue;
        }

        moves::append_move_events_for_expression(
            program,
            ctx,
            state.symbol,
            borrow_call.statement_index,
            *argument,
            FlowOwnershipEventSource::Call {
                statement_index: borrow_call.statement_index,
                call_ordinal: borrow_call.call_ordinal,
                target_symbol: borrow_call.target_symbol,
            },
        );
    }
}
