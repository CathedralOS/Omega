use super::*;

/// Resolve the owned `self` place bound through a method-form call receiver.
///
/// Static spelling includes `self` in the positional argument span, while
/// method spelling binds it through the receiver and exposes only the
/// non-self arguments. Ownership discovery and permission-event
/// classification must use the same distinction so the latter does not
/// mistake a terminal method consume for an ordinary transfer.
pub(crate) fn owned_method_receiver_place(
    program: &psi_typed_trees::TypedTrees,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &CallSite<'_>,
    target_state: &psi_typed_trees::state::State,
    fallback_receiver_symbol: SymbolHandle,
) -> Option<CanonicalPlace> {
    let arguments = call_site_argument_expressions(program, call_site);
    let declared_parameters = program.state_parameters(target_state);
    let positional_parameter_count = declared_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    if arguments.len() != positional_parameter_count
        || !declared_parameters.iter().any(|parameter| {
            parameter.is_self && type_requires_ownership(program, parameter.type_reference)
        })
    {
        return None;
    }

    match call_site {
        CallSite::Expression { call, .. } => canonical_place_from_expression_in_state(
            program,
            caller_state_symbol,
            statement_index,
            call.receiver,
        ),
        CallSite::Statement(call) => canonical_place_from_symbol(call.receiver_symbol),
        CallSite::TransitionNamed(_) => None,
    }
    .or_else(|| canonical_place_from_symbol(fallback_receiver_symbol))
}

pub(in crate::flow) fn append_call_ownership_events(
    program: &psi_typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
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
    let includes_explicit_self = declared_parameters
        .iter()
        .any(|parameter| parameter.is_self)
        && arguments.len() == declared_parameters.len();
    let source = FlowOwnershipEventSource::Call {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        target_symbol: borrow_call.target_symbol,
    };

    // A method-form call binds declared `self` through the receiver rather
    // than through the positional argument span. Borrowed `&self`/`&mut self`
    // carries no ownership, but a by-value `self` is the terminal transfer of
    // that receiver and must emit the same move event as the explicit static
    // spelling (`Type::consume(value)`). Without this edge, `value.finish()`
    // left the original linear obligation live at scope exit.
    if !includes_explicit_self && borrow_call.has_receiver {
        if let Some(receiver) = owned_method_receiver_place(
            program,
            state.symbol,
            borrow_call.statement_index,
            &call_site,
            target_state,
            borrow_call.receiver_symbol,
        ) {
            append_move_event_for_place(program, sink, receiver, source);
        }
    }

    let parameters = declared_parameters
        .iter()
        .filter(|parameter| includes_explicit_self || !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments.iter()) {
        if !type_requires_ownership(program, parameter.type_reference) {
            continue;
        }

        moves::append_move_events_for_expression(
            program,
            sink,
            state.symbol,
            borrow_call.statement_index,
            *argument,
            source,
        );
    }
}
