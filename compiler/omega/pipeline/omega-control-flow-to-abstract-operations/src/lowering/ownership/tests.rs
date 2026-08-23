use super::*;
use omega_control_flow::{ControlFlowPlan, StateFlow, StateKey, StatePermissionEvent};
use psi_symbols::SymbolHandle;

#[test]
fn lowers_only_semantic_permission_events() {
    let mut control_flow = ControlFlowPlan::default();
    let segment = psi_facts::PlaceSegment::Field {
        symbol: SymbolHandle::from_arena_index(7),
    };
    let segments = control_flow
        .semantics
        .ownership
        .segments
        .insert_many([segment]);
    let claim_identity = psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol: SymbolHandle::from_arena_index(1),
        state_symbol: SymbolHandle::from_arena_index(2),
        source: psi_language_semantics::PermissionEventSource::Statement { statement_index: 4 },
        ordinal: 7,
    };
    let mut state = StateFlow {
        key: StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        },
        ..StateFlow::default()
    };
    control_flow.semantics.ownership.permissions.append_to_span(
        &mut state.ownership.permissions,
        StatePermissionEvent {
            source: psi_language_semantics::PermissionEventSource::Statement { statement_index: 4 },
            kind: psi_language_semantics::PermissionEventKind::Consume,
            multiplicity: psi_language_semantics::Multiplicity::Linear,
            access: psi_language_semantics::PermissionAccess::Owned,
            claim_identity,
            provenance: psi_language_semantics::PermissionProvenance::Unknown,
            root: psi_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(11)),
            segments,
            obligation_live: true,
        },
    );
    control_flow.states.insert(state);

    let summary = build_abstract_ownership_summary(&control_flow);

    assert_eq!(summary.permissions.len(), 1);
    let permission = summary
        .permissions
        .iter()
        .next()
        .map(|(_, event)| event)
        .unwrap();
    assert_eq!(
        permission.kind,
        psi_language_semantics::PermissionEventKind::Consume
    );
    assert!(permission.obligation_live);
    assert_eq!(permission.claim_identity, claim_identity);
    assert_eq!(
        summary.segments.span_or_empty(permission.segments),
        &[segment]
    );
}
