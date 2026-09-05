use super::*;

/// Resolve the owned `self` place bound through a method-form call receiver.
///
/// Static spelling includes `self` in the positional argument span, while
/// method spelling binds it through the receiver and exposes only the
/// non-self arguments. Ownership discovery and permission-event
/// classification must use the same distinction so the latter does not
/// mistake a terminal method consume for an ordinary transfer.
pub(crate) fn owned_method_receiver_place(
    program: &typed_trees::TypedTrees,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &CallSite<'_>,
    declared_parameters: &[typed_trees::signature::StateParameter],
    fallback_receiver_symbol: SymbolHandle,
) -> Option<CanonicalPlace> {
    let arguments = call_site_argument_expressions(program, call_site);
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
        CallSite::TransitionNamed { .. } => None,
    }
    .or_else(|| canonical_place_from_symbol(fallback_receiver_symbol))
}

pub(in crate::flow) fn append_call_ownership_events(
    program: &typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
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
    let declared_parameters = call_target_parameters(program, borrow_call.target_symbol);
    let Some(declared_parameters) = declared_parameters else {
        return;
    };

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
    if !includes_explicit_self
        && borrow_call.has_receiver
        && let Some(receiver) = owned_method_receiver_place(
            program,
            state.symbol,
            borrow_call.statement_index,
            &call_site,
            declared_parameters,
            borrow_call.receiver_symbol,
        )
    {
        append_move_event_for_place(program, sink, receiver, source);
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

/// Parameters of any callable target retained by typed trees. Boundary-trait
/// requirements and compile-time machine parameters have signatures but no
/// state body; their owned by-value arguments still transfer exactly as an
/// ordinary state call's arguments do.
pub(crate) fn call_target_parameters(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    if let Some(state) = find_state(program, target_symbol) {
        return Some(program.state_parameters(state));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(program.state_signature_parameters(signature));
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
            .map(|signature| program.state_signature_parameters(signature))
    })
}

/// Canonical caller places transferred into owned by-value call operands.
/// Borrow checking uses this exact parameter alignment so moving a provider
/// validity claim cannot bypass an outstanding view merely because the target
/// is bodyless.
pub(crate) fn owned_call_operand_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> Vec<CanonicalPlace> {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller_machine_symbol)
    else {
        return Vec::new();
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == caller_state_symbol)
    else {
        return Vec::new();
    };
    // Constructor values have no caller storage root. Reuse the same typed
    // ownership discovery as move checking so nested literals contribute
    // their moved operands rather than an unknown constructor place.
    let mut segments = arena::Arena::default();
    let mut sink = DirectMoveEventSink::new(&mut segments);
    append_call_ownership_events(program, &mut sink, machine, state, borrow_call);
    sink.finish()
        .into_iter()
        .map(|event| CanonicalPlace {
            root: event.root,
            segments: segments.span_or_empty(event.segments).to_vec(),
        })
        .collect()
}
