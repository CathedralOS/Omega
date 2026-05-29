use super::*;

mod calls;
mod drops;
mod events;
mod moves;
mod type_references;

pub(super) use calls::append_call_ownership_events;
pub(super) use drops::append_state_exit_drop_events;
use events::{append_drop_event_for_place, append_move_event_for_place};
use moves::append_move_events_for_expression;
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
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    FlowOwnershipEventSource::Statement { statement_index },
                );
            }
        }
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {}
    }
}
