use omega_control_flow::{
    StateDropEvent, StateMoveEvent, StateOwnershipEventSource, StateOwnershipSummary,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

use crate::arena_remap::remap_arena;
use crate::handles::{remap_drop_event_span, remap_move_event_span};

pub(crate) fn remap_move_event_owned(event: omega_state_graph::StateMoveEvent) -> StateMoveEvent {
    StateMoveEvent {
        source: remap_ownership_event_source(event.source),
        root: event.root,
        segments: event.segments,
    }
}

pub(crate) fn remap_drop_event_owned(event: omega_state_graph::StateDropEvent) -> StateDropEvent {
    StateDropEvent {
        source: remap_ownership_event_source(event.source),
        root: event.root,
        segments: event.segments,
    }
}

pub(crate) fn remap_move_events(state_graph: &StateGraph) -> Arena<StateMoveEvent> {
    remap_arena(
        &state_graph.semantics.ownership.moves,
        remap_move_event_owned,
    )
}

pub(crate) fn remap_drop_events(state_graph: &StateGraph) -> Arena<StateDropEvent> {
    remap_arena(
        &state_graph.semantics.ownership.drops,
        remap_drop_event_owned,
    )
}

pub(crate) fn remap_ownership_summary(
    summary: &omega_state_graph::StateOwnershipSummary,
) -> StateOwnershipSummary {
    StateOwnershipSummary {
        moves: remap_move_event_span(summary.moves),
        drops: remap_drop_event_span(summary.drops),
    }
}

fn remap_ownership_event_source(
    source: omega_state_graph::StateOwnershipEventSource,
) -> StateOwnershipEventSource {
    match source {
        omega_state_graph::StateOwnershipEventSource::Statement { statement_index } => {
            StateOwnershipEventSource::Statement { statement_index }
        }
        omega_state_graph::StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        omega_state_graph::StateOwnershipEventSource::StateExit => {
            StateOwnershipEventSource::StateExit
        }
    }
}

#[cfg(test)]
mod tests;
