use super::*;

mod calls;
mod drops;
mod events;
mod moves;
mod place_types;
mod type_references;

pub(super) use calls::append_call_ownership_events;
pub(super) use drops::append_state_exit_drop_events;
use events::{append_drop_event_for_place, append_move_event_for_place};
use moves::{append_move_events_for_expression, initializer_produces_owned_value};
use type_references::type_requires_ownership;

pub(super) fn append_statement_ownership_events(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::Assignment(assignment) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            assignment.value,
            FlowOwnershipEventSource::Statement { statement_index },
        ),
        StatementNode::LocalData(local_data) => {
            if type_requires_ownership(program, local_data.type_reference) {
                let source = FlowOwnershipEventSource::Statement { statement_index };
                append_move_events_for_expression(
                    program,
                    ctx,
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
                        append_move_event_for_place(ctx, place, source);
                    }
                }
            }
        }
        // Transition target/continuation arguments transfer ownership through
        // their borrow-call facts (resolved as `CallSite::TransitionNamed`), so
        // their type-aware move events are emitted by
        // `append_call_ownership_events` during call-flow construction. Emitting
        // them here as well would double-count the transfer.
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {}
    }
}
