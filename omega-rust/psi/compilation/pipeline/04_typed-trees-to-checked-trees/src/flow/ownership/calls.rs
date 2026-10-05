use crate::flow::CanonicalPlace;
use crate::flow::FlowOwnershipEventSource;
use crate::flow::ownership::DirectMoveEventSink;
use crate::flow::ownership::append_move_event_for_place;
use crate::flow::ownership::moves;
use crate::flow::ownership::type_requires_ownership;
use crate::semantic::calls::CallSite;
use crate::semantic::calls::call_site_argument_expressions;
use crate::semantic::calls::find_call_site;
use checked_trees::BorrowCallFact;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub(crate) use crate::semantic::calls::call_target_parameters;

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
    if let CallSite::Expression { expression, .. } = call_site
        && validation::saved_dispatch_call(program, state, borrow_call.statement_index, expression)
            .is_some_and(|(statement, canonical)| {
                statement != borrow_call.statement_index || canonical != expression
            })
    {
        return;
    }
    let arguments = call_site_argument_expressions(program, &call_site);
    let declared_parameters = call_target_parameters(program, borrow_call.target_symbol);
    let Some(declared_parameters) = declared_parameters else {
        return;
    };
    let proof_call = declared_parameters.iter().any(|parameter| {
        program.type_multiplicity(parameter.type_reference)
            == language_semantics::Multiplicity::Affine
    }) && program
        .machines()
        .iter()
        .find(|target| {
            program
                .machine_states(target)
                .iter()
                .any(|target_state| target_state.symbol == borrow_call.target_symbol)
        })
        .is_some_and(|target| sink.proof_only(program).is_proof_machine(program, target));
    let parameter_transfers = |parameter: &typed_trees::signature::StateParameter| {
        type_requires_ownership(program, parameter.type_reference)
            // Mathematical applications and citations read their affine
            // operands. Erasure does not discharge linear Type custody,
            // including conditional claims carried inside affine sums.
            && !(proof_call
                && program.type_multiplicity(parameter.type_reference)
                    == language_semantics::Multiplicity::Affine
                && !crate::checks::type_carries_linear_obligation(program, parameter.type_reference))
    };

    let source = FlowOwnershipEventSource::Call {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        target_symbol: borrow_call.target_symbol,
    };

    let parameters = declared_parameters.iter();

    for (parameter, argument) in parameters.zip(arguments.iter()) {
        if !parameter_transfers(parameter) {
            continue;
        }

        // A whole affine call result is fresh storage, not a read of its
        // arguments. Its producer's discovered call owns those argument moves;
        // this consuming call owns just the result expression's transfer.
        if affine_call_result_type(program, machine, state, *argument).is_some() {
            append_move_event_for_place(
                sink,
                CanonicalPlace {
                    root: facts::PlaceRoot::Expression(*argument),
                    segments: Vec::new(),
                },
                source,
                *argument,
            );
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

fn affine_call_result_type(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let (receiver_symbol, receiver_path) =
        crate::lookup::call_receiver_parts(program, call.receiver);
    let target = crate::lookup::resolve_state_call_target(
        program,
        machine,
        state,
        receiver_symbol,
        call.target_symbol,
        receiver_path.as_deref(),
        &call.target,
    );
    let result = super::super::calls::call_target_return_type(program, target)?;
    // Retain the owned root type before any referent/qualification stripping.
    // Linear/conditional results still need their explicit claim mapping.
    (matches!(
        program.type_reference_table.type_reference(result),
        typed_trees::types::TypeReferenceNode::Named { .. }
            | typed_trees::types::TypeReferenceNode::Generic { .. }
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
    ) && program.type_multiplicity(result) == language_semantics::Multiplicity::Affine
        && !crate::checks::type_carries_linear_obligation(program, result))
    .then_some(result)
}

/// Canonical caller places transferred into owned by-value call operands.
/// Borrow checking uses this exact parameter alignment so moving a provider
/// validity claim cannot bypass an outstanding view merely because the target
/// is bodyless.
pub(crate) fn owned_call_operand_places(
    program: &typed_trees::TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> Vec<CanonicalPlace> {
    let Some(machine) = crate::lookup::machine_by_symbol(program, caller_machine_symbol) else {
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
    let mut sink = DirectMoveEventSink::new(&mut segments, operators, machine, state);
    append_call_ownership_events(program, &mut sink, machine, state, borrow_call);
    sink.finish()
        .into_iter()
        .map(|event| CanonicalPlace {
            root: event.root,
            segments: segments.span_or_empty(event.segments).to_vec(),
        })
        // Keep the move event for permission accounting, but a projection
        // contained in a fresh owned result does not move caller storage.
        // Nested call operands have their own parameter-aligned move checks.
        .filter(|place| !super::result_storage::is_private_result_place(program, place))
        .collect()
}
