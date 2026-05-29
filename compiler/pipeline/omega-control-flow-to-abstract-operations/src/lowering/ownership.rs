use omega_abstract_operations::{
    AbstractDropEvent, AbstractMoveEvent, AbstractOwnershipEventSource, AbstractOwnershipSummary,
};
use omega_control_flow::{ControlFlowPlan, StateOwnershipEventSource};

pub(super) fn build_abstract_ownership_summary(
    control_flow: &ControlFlowPlan,
) -> AbstractOwnershipSummary {
    let mut summary = AbstractOwnershipSummary::with_capacity(
        control_flow.semantics.ownership_segments.len(),
        control_flow.semantics.move_events.len(),
        control_flow.semantics.drop_events.len(),
    );

    for (_, state) in control_flow.states.iter() {
        for event in control_flow
            .semantics
            .move_events
            .span_or_empty(state.ownership.moves)
        {
            summary.moves.insert(AbstractMoveEvent {
                source_key: state.key,
                source: remap_ownership_event_source(event.source),
                root: event.root,
                segments: summary.segments.insert_many(
                    control_flow
                        .semantics
                        .ownership_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            });
        }

        for event in control_flow
            .semantics
            .drop_events
            .span_or_empty(state.ownership.drops)
        {
            summary.drops.insert(AbstractDropEvent {
                source_key: state.key,
                source: remap_ownership_event_source(event.source),
                root: event.root,
                segments: summary.segments.insert_many(
                    control_flow
                        .semantics
                        .ownership_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            });
        }
    }

    summary
}

fn remap_ownership_event_source(source: StateOwnershipEventSource) -> AbstractOwnershipEventSource {
    match source {
        StateOwnershipEventSource::Statement { statement_index } => {
            AbstractOwnershipEventSource::Statement { statement_index }
        }
        StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => AbstractOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        StateOwnershipEventSource::StateExit => AbstractOwnershipEventSource::StateExit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_control_flow::{
        ControlFlowPlan, StateDropEvent, StateFlow, StateKey, StateMoveEvent,
        StateOwnershipEventSource,
    };
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn copies_control_flow_ownership_events() {
        let mut control_flow = ControlFlowPlan::default();
        let segment = omega_facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(7),
        };
        let segments = control_flow
            .semantics
            .ownership_segments
            .insert_many([segment]);
        let mut state = StateFlow {
            key: StateKey {
                machine: SymbolHandle::from_arena_index(1),
                state: SymbolHandle::from_arena_index(2),
                segment_index: 0,
            },
            ..StateFlow::default()
        };
        control_flow.semantics.move_events.append_to_span(
            &mut state.ownership.moves,
            StateMoveEvent {
                source: StateOwnershipEventSource::Statement { statement_index: 3 },
                root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(9)),
                segments,
            },
        );
        control_flow.semantics.drop_events.append_to_span(
            &mut state.ownership.drops,
            StateDropEvent {
                source: StateOwnershipEventSource::StateExit,
                root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(10)),
                segments: HandleSpan::empty(),
            },
        );
        control_flow.states.insert(state);

        let summary = build_abstract_ownership_summary(&control_flow);

        assert_eq!(summary.moves.len(), 1);
        assert_eq!(summary.drops.len(), 1);
        let move_event = summary.moves.iter().next().map(|(_, event)| event).unwrap();
        assert_eq!(
            move_event.source,
            AbstractOwnershipEventSource::Statement { statement_index: 3 }
        );
        assert_eq!(
            summary.segments.span_or_empty(move_event.segments),
            &[segment]
        );
        let drop_event = summary.drops.iter().next().map(|(_, event)| event).unwrap();
        assert_eq!(drop_event.source, AbstractOwnershipEventSource::StateExit);
        assert!(
            summary
                .segments
                .span_or_empty(drop_event.segments)
                .is_empty()
        );
    }
}
