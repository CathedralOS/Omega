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
            // Lending the completed argument does not lend the operands used
            // to construct it. Observe existing places, but retain moves into
            // fresh caller-side values at their actual evaluation occurrence.
            // The borrowed formal receives no ownership or claim transfer.
            if !parameter.is_const && !parameter.relevance.is_erased() && !proof_call {
                moves::observations::append(
                    program,
                    sink,
                    state.symbol,
                    borrow_call.statement_index,
                    *argument,
                    FlowOwnershipEventSource::Statement {
                        statement_index: borrow_call.statement_index,
                    },
                );
            }
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
    transfers_for_source(
        facts,
        state,
        FlowOwnershipEventSource::Call {
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
            target_symbol: borrow_call.target_symbol,
        },
    )
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

/// The evaluated storage coordinate of one consuming operand. Mutable-reference
/// extraction can move a referent while the expression still names its descriptor
/// slot; reconstructing that slot from source would select a different owner.
/// This association identifies storage only. Live claim authority still comes
/// from the exact committed permission occurrence, not from a move candidate.
pub(crate) fn owned_operand_transfer_place(
    facts: &checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    source: language_semantics::PermissionEventSource,
) -> Result<Option<CanonicalPlace>, &'static str> {
    use language_semantics::PermissionEventSource;
    if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
        return Err("owned operand has no permission evaluation coordinate");
    }
    let source = match source {
        PermissionEventSource::Statement { statement_index } => {
            FlowOwnershipEventSource::Statement { statement_index }
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        _ => return Ok(None),
    };
    let state = facts
        .flow
        .control
        .state(machine, state)
        .ok_or("owned operand has no state transfer roster")?;
    let mut selected = None;
    for event in transfers_for_source(facts, state, source)
        .filter(|event| event.expression == expression && event.evaluation == evaluation)
    {
        if !event.root.is_valid() {
            return Err("owned operand transfer has no evaluated storage root");
        }
        let place = CanonicalPlace {
            root: event.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span(event.segments)
                .ok_or("owned operand transfer has stale storage selectors")?
                .to_vec(),
        };
        if selected.as_ref().is_some_and(|previous| *previous != place) {
            return Err("owned operand has ambiguous evaluated storage coordinates");
        }
        selected = Some(place);
    }
    Ok(selected)
}

fn transfers_for_source<'facts>(
    facts: &'facts checked_trees::CheckFacts,
    state: &checked_trees::FlowStateFact,
    source: FlowOwnershipEventSource,
) -> impl Iterator<Item = &'facts checked_trees::FlowStorageTransferFact> {
    facts
        .flow
        .ownership
        .storage_transfers
        .span_or_empty(state.storage_transfers)
        .iter()
        .filter(move |event| event.source == source)
}
