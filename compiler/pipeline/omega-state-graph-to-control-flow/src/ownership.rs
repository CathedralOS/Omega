use omega_control_flow::{
    StateDropEvent, StateMoveEvent, StateOwnershipEventSource, StateOwnershipSummary,
};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

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
    let mut moves = Arena::with_capacity(state_graph.semantics.move_events.len());

    for (_, event) in state_graph.semantics.move_events.iter() {
        moves.append(remap_move_event_owned(event.clone()));
    }

    moves
}

pub(crate) fn remap_drop_events(state_graph: &StateGraph) -> Arena<StateDropEvent> {
    let mut drops = Arena::with_capacity(state_graph.semantics.drop_events.len());

    for (_, event) in state_graph.semantics.drop_events.iter() {
        drops.append(remap_drop_event_owned(event.clone()));
    }

    drops
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
mod tests {
    use super::*;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn remap_ownership_summary_preserves_move_and_drop_event_handles() {
        let target_symbol = SymbolHandle::from_arena_index(1);

        let move_event = omega_state_graph::StateMoveEvent {
            source: omega_state_graph::StateOwnershipEventSource::Call {
                statement_index: 2,
                call_ordinal: 3,
                target_symbol,
            },
            root: Default::default(),
            segments: Default::default(),
        };
        let drop_event = omega_state_graph::StateDropEvent {
            source: omega_state_graph::StateOwnershipEventSource::StateExit,
            root: Default::default(),
            segments: Default::default(),
        };
        let mut moves = Arena::new();
        let mut drops = Arena::new();
        let mut move_span = omega_core::arena::HandleSpan::empty();
        let mut drop_span = omega_core::arena::HandleSpan::empty();
        moves.append_to_span(&mut move_span, move_event);
        drops.append_to_span(&mut drop_span, drop_event);

        let summary = remap_ownership_summary(&omega_state_graph::StateOwnershipSummary {
            moves: move_span,
            drops: drop_span,
        });

        assert_eq!(summary.moves.count(), 1);
        assert_eq!(summary.drops.count(), 1);
        assert_eq!(
            summary.moves.start().arena_index(),
            move_span.start().arena_index()
        );
        assert_eq!(
            summary.drops.start().arena_index(),
            drop_span.start().arena_index()
        );
    }

    #[test]
    fn remap_owned_move_event_preserves_call_source_and_place() {
        let target_symbol = SymbolHandle::from_arena_index(1);
        let event = omega_state_graph::StateMoveEvent {
            source: omega_state_graph::StateOwnershipEventSource::Call {
                statement_index: 2,
                call_ordinal: 3,
                target_symbol,
            },
            root: Default::default(),
            segments: Default::default(),
        };

        let remapped = remap_move_event_owned(event);

        assert_eq!(remapped.root, Default::default());
        assert_eq!(
            remapped.source,
            StateOwnershipEventSource::Call {
                statement_index: 2,
                call_ordinal: 3,
                target_symbol,
            }
        );
    }
}
