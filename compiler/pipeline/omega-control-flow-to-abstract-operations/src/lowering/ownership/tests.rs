use super::*;
use omega_control_flow::{
    ControlFlowPlan, StateDropEvent, StateFlow, StateKey, StateMoveEvent, StateOwnershipEventSource,
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
        .ownership
        .segments
        .insert_many([segment]);
    let mut state = StateFlow {
        key: StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        },
        ..StateFlow::default()
    };
    control_flow.semantics.ownership.moves.append_to_span(
        &mut state.ownership.moves,
        StateMoveEvent {
            source: StateOwnershipEventSource::Statement { statement_index: 3 },
            root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(9)),
            segments,
        },
    );
    control_flow.semantics.ownership.drops.append_to_span(
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
