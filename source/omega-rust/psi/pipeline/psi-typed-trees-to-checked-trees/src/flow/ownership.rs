use super::*;

mod calls;
mod events;
mod moves;
mod place_types;
mod type_references;

use calls::append_call_ownership_events;
pub(crate) use calls::{owned_call_operand_places, owned_method_receiver_place};
use events::{DirectMoveEventSink, append_move_event_for_place};
pub(crate) use events::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, normalized_event_place_root,
};
use moves::{
    append_move_events_for_expression, append_move_events_for_operator_statement_call,
    initializer_produces_owned_value,
};
pub(crate) use place_types::{
    canonical_place_type_reference, expression_type_reference_in_state,
    project_type_reference_from_segments,
};
use type_references::type_requires_ownership;

pub(super) fn append_statement_ownership_events(
    program: &psi_typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                assignment.value,
                source,
            );
            // The assignment twin of the `let`-init owned-production seam
            // below: an operator result that is freshly owned storage
            // transfers ownership *into* the assignment target, and the
            // source-side recursion records nothing for a non-place
            // initializer. Record the production as a move into the target
            // place.
            if initializer_produces_owned_value(program, assignment.value) {
                if let Some(place) = canonical_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.target,
                ) {
                    append_move_event_for_place(program, sink, place, source);
                }
            }
        }
        StatementNode::LocalData(local_data) => {
            if type_requires_ownership(program, local_data.type_reference) {
                let source = FlowOwnershipEventSource::Statement { statement_index };
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
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
                if initializer_produces_owned_value(program, local_data.initial_value) {
                    if let Some(place) = canonical_place_from_symbol(local_data.symbol) {
                        append_move_event_for_place(program, sink, place, source);
                    }
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
            for handle in [transition.target, transition.continuation] {
                if !handle.is_valid() {
                    continue;
                }
                if let psi_typed_trees::statement::TransitionTargetNode::Value(value) =
                    program.statement_table.transition_target(handle)
                {
                    append_move_events_for_expression(
                        program,
                        sink,
                        state_symbol,
                        statement_index,
                        *value,
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
            FlowOwnershipEventSource::Statement { statement_index },
        ),
    }
}

/// Run the ownership discovery rules directly into the semantic permission
/// producer. The discovered vocabulary is private to checked lowering; only
/// normalized permission events are published.
pub(crate) fn discover_state_move_events(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    segments: &mut psi_arena::Arena<psi_facts::PlaceSegment>,
) -> Vec<DiscoveredMoveEvent> {
    let mut sink = DirectMoveEventSink::new(segments);
    let borrow_calls = borrow_state_fact(borrow, machine.symbol, state.symbol)
        .map(|(_, state)| borrow.calls.span_or_empty(state.calls))
        .unwrap_or_default();
    let mut call_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
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
            append_call_ownership_events(program, &mut sink, machine, state, call);
        }
    }

    sink.finish()
}
