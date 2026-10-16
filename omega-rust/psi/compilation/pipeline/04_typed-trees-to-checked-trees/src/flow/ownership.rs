//! Ownership move events and borrowed operand events. `discover_state_move_events` walks one
//! state's statements and borrow calls and returns the moves they make, for the
//! multiplicity checks and terminal cleanup. A place's declared type names
//! storage; the selected operand type names the value transferred from it.
//! In particular, forwarding an exclusive reference moves its descriptor,
//! while a consumption of its exact referent opens restoration debt. Carry
//! that distinction from local, store, call and result contexts through the
//! same expression walk; a later checker must not guess it from an empty path.
//! Exact place types stay with validation's value-custody projection owner.
use validation::canonical_place_from_expression_in_state;

use crate::flow::borrow_state_fact;

use crate::flow::canonical_place_from_symbol;
use checked_trees::BorrowFacts;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

pub(crate) mod calls;
mod events;
mod moves;
mod result_storage;
mod type_references;

use calls::append_call_ownership_events;
pub(crate) use calls::owned_call_operand_places;
use events::{DirectMoveEventSink, append_move_event_for_place};
pub(crate) use events::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, call_permission_evaluation,
};
use moves::{
    append_move_events_for_expression, append_move_events_for_operator_statement_call,
    initializer_produces_owned_value,
};
use type_references::type_requires_ownership;

pub(super) fn append_statement_ownership_events(
    program: &dyn ::validation::CheckingInput,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            moves::observations::append(
                program,
                sink,
                state_symbol,
                statement_index,
                binding.receiver,
                FlowOwnershipEventSource::Statement { statement_index },
            );
            if binding.implementation_operand.is_valid() {
                moves::observations::append(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    binding.implementation_operand,
                    FlowOwnershipEventSource::Statement { statement_index },
                );
            }
        }
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            // The store's prepared receiving type identifies the transferred
            // value. Reference forwarding must not turn into a pointee move
            // merely because the destination is a local rather than a field.
            let stored_type = validation::find_state(program, state_symbol)
                .and_then(|(machine, state)| {
                    validation::assignment_stored_value_type(program, machine, state, assignment)
                })
                .unwrap_or_default();
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                assignment.value,
                stored_type,
                source,
            );
            // The assignment twin of the `let`-init owned-production seam
            // below: an operator result that is freshly owned storage
            // transfers ownership *into* the assignment target, and the
            // source-side recursion records nothing for a non-place
            // initializer. Record the production as a move into the target
            // place.
            if initializer_produces_owned_value(program, assignment.value)
                && let Some(place) = canonical_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.target,
                )
            {
                append_move_event_for_place(
                    sink,
                    place,
                    source,
                    Default::default(),
                    Default::default(),
                );
            }
        }
        StatementNode::LocalData(local_data) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            if !type_requires_ownership(program, local_data.type_reference) {
                moves::observations::append(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    source,
                );
            } else {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    local_data.type_reference,
                    source,
                );
                // An owned binding whose initializer is a value-producing
                // slice/string/collection operator (a conversion that yields
                // freshly owned storage) transfers ownership *into* the bound
                // local. `append_move_events_for_expression` only records the
                // source-side move for a place-like initializer, so the produced
                // owned value would otherwise leave no ownership event. Record the
                // production as a move into the bound local place; this is the
                // slice/string operator-result extension of ownership events.
                if initializer_produces_owned_value(program, local_data.initial_value)
                    && let Some(place) = canonical_place_from_symbol(local_data.symbol)
                {
                    append_move_event_for_place(
                        sink,
                        place,
                        source,
                        Default::default(),
                        local_data.type_reference,
                    );
                }
            }
        }
        // A transition `Named` target's continuation arguments transfer
        // ownership through their borrow-call facts (resolved as
        // `CallSite::TransitionNamed`), so their type-aware move events are
        // emitted by `append_call_ownership_events` during call-flow
        // construction. A transition `Value` target, however, is a plain value
        // expression (`_ -> <expr>`, the state's result) for which call-flow
        // discovery records no ownership-bearing borrow call, so an owned value
        // moved out through it (a place read, an aggregate, or an operator-call
        // result) would otherwise leave no ownership event. Record the move for
        // both the primary target and the continuation `Value` targets via the
        // type-aware recursion, which itself skips any nested state borrow calls
        // to avoid double-counting. `Named`/`Terminal` targets are handled by the
        // call-flow pass / produce no transfer.
        StatementNode::Transition(transition) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                sink.evaluation = checked_trees::FlowPermissionEvaluation::TransitionGuard;
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    guard,
                    Default::default(),
                    source,
                );
            }
            for handle in transition.targets() {
                sink.evaluation = checked_trees::FlowPermissionEvaluation::TransitionTarget(handle);
                if let typed_trees::statement::TransitionTargetNode::Value(value) =
                    program.statement_table.transition_target(handle)
                {
                    append_move_events_for_expression(
                        program,
                        sink,
                        state_symbol,
                        statement_index,
                        *value,
                        sink.state.return_type,
                        source,
                    );
                }
            }
        }
        // A statement-level call that dispatches to a machine state is owned by
        // the call-flow pass (its `BorrowCallFact` drives
        // `append_call_ownership_events`). A statement-level call that resolves
        // to an operator/boundary definition gets no borrow-call fact, so its
        // owned by-value argument transfers are recorded here instead.
        StatementNode::Call(call) => append_move_events_for_operator_statement_call(
            program,
            sink,
            state_symbol,
            statement_index,
            call,
            FlowOwnershipEventSource::Statement { statement_index },
        ),
        // A bare/terminal value expression (the parser's brace-terminated state
        // result or an `expr;` statement) moves owned values it reads out of
        // their places: the type-aware recursion records those source-side
        // moves. Nested state borrow calls are skipped inside the recursion, so
        // the call-flow pass keeps sole ownership of their argument events.
        StatementNode::Expression(expression) => append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            *expression,
            if statement_index + 1
                == program
                    .statement_table
                    .statements(sink.state.statement_nodes)
                    .len()
            {
                sink.state.return_type
            } else {
                Default::default()
            },
            FlowOwnershipEventSource::Statement { statement_index },
        ),
    }
}

/// Discover storage-transfer candidates once in admitted execution order.
/// Permission production retains this roster before expanding claims, so loan
/// checks and cleanup share exact source occurrences even for claim-free values.
pub(crate) fn discover_state_move_events(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    segments: &mut arena::Arena<facts::PlaceSegment>,
) -> Vec<DiscoveredMoveEvent> {
    let mut sink = DirectMoveEventSink::new(segments, operators, machine, state);
    let borrow_calls = borrow_state_fact(borrow, machine.symbol, state.symbol)
        .map(|(_, state)| borrow.calls.span_or_empty(state.calls))
        .unwrap_or_default();
    let mut call_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        sink.evaluation = checked_trees::FlowPermissionEvaluation::Statement;
        append_statement_ownership_events(
            program,
            &mut sink,
            state.symbol,
            statement_index,
            statement,
        );
        while let Some(call) = borrow_calls.get(call_index) {
            if call.statement_index != statement_index {
                break;
            }
            call_index += 1;
            sink.evaluation = call_permission_evaluation(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            );
            append_call_ownership_events(program, &mut sink, machine, state, call);
        }
    }

    sink.finish()
}
