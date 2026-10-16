use validation::CallSite;
use validation::CanonicalPlace;
use validation::call_site_argument_expressions;
use validation::call_target_parameters;
use validation::find_call_site;

use crate::flow::FlowOwnershipEventSource;
use crate::flow::ownership::DirectMoveEventSink;
use crate::flow::ownership::append_move_event_for_place;
use crate::flow::ownership::moves;
use crate::flow::ownership::type_requires_ownership;

use checked_trees::BorrowCallFact;
use checked_trees::expression::ExpressionHandle;

pub(in crate::flow) fn append_call_ownership_events(
    program: &dyn ::validation::CheckingInput,
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
                parameter.type_reference,
            );
            continue;
        }

        // A claim-bearing construction is an anonymous owner: its operands
        // move into it when it completes at this statement, before the call
        // transfers the completed owner whole. Permission production
        // establishes that owner and records its transfer.
        let operand_source = if crate::checks::owned_temporary_type(
            program,
            state.symbol,
            borrow_call.statement_index,
            parameter,
            *argument,
        )
        .is_some()
        {
            FlowOwnershipEventSource::Statement {
                statement_index: borrow_call.statement_index,
            }
        } else {
            source
        };
        moves::append_move_events_for_expression(
            program,
            sink,
            state.symbol,
            borrow_call.statement_index,
            *argument,
            parameter.type_reference,
            operand_source,
        );
    }
}

fn affine_call_result_type(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let call = program.expression_table.call_node(expression)?;
    let (receiver_symbol, receiver_path) = validation::call_receiver_parts(program, call.receiver);
    let target = validation::resolve_state_call_target(
        program,
        machine,
        state,
        receiver_symbol,
        call.target_symbol,
        receiver_path.as_deref(),
        &call.target,
    );
    let result = validation::call_target_return_type(program, target)?;
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
/// Canonical caller storage moved by this call's retained operand occurrences.
/// Discovery ran once before claim expansion. Call-local loan checks consume
/// that roster rather than reclassifying the source or rebuilding move events.
pub(crate) fn owned_call_operand_places(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &checked_trees::FlowStateFact,
    borrow_call: &BorrowCallFact,
) -> Vec<CanonicalPlace> {
    facts
        .flow
        .ownership
        .storage_transfers
        .span_or_empty(state.storage_transfers)
        .iter()
        .filter(|event| {
            event.source
                == FlowOwnershipEventSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                }
        })
        .map(|event| CanonicalPlace {
            root: event.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .to_vec(),
        })
        .filter(|place| !super::result_storage::is_private_result_place(program, place))
        .collect()
}
