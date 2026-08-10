use omega_state_graph::{StateGraph, StateKey, StateOwnershipSummary, StatePermissionEvent};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;

pub(crate) fn state_ownership_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateOwnershipSummary {
    let mut permissions = HandleSpan::empty();
    for event in program
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == key.machine && event.state_symbol == key.state)
                .then_some(event)
        })
    {
        state_graph.semantics.ownership.permissions.append_to_span(
            &mut permissions,
            StatePermissionEvent {
                source: event.source,
                kind: event.kind,
                multiplicity: event.multiplicity,
                access: event.access,
                claim_identity: event.claim_identity,
                provenance: event.provenance,
                root: event.root,
                segments: state_graph.semantics.ownership.segments.insert_many(
                    program
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
                obligation_live: event.obligation_live,
            },
        );
    }

    StateOwnershipSummary { permissions }
}
