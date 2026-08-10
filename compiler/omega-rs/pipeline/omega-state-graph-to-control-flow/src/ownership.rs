use omega_control_flow::{StateOwnershipSummary, StatePermissionEvent};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;
use crate::handles::remap_permission_event_span;

pub(crate) fn remap_permission_event_owned(
    event: omega_state_graph::StatePermissionEvent,
) -> StatePermissionEvent {
    StatePermissionEvent {
        source: event.source,
        kind: event.kind,
        multiplicity: event.multiplicity,
        access: event.access,
        claim_identity: event.claim_identity,
        provenance: event.provenance,
        root: event.root,
        segments: event.segments,
        obligation_live: event.obligation_live,
    }
}

pub(crate) fn remap_permission_events(state_graph: &StateGraph) -> Arena<StatePermissionEvent> {
    remap_arena(
        &state_graph.semantics.ownership.permissions,
        remap_permission_event_owned,
    )
}

pub(crate) fn remap_ownership_summary(
    summary: &omega_state_graph::StateOwnershipSummary,
) -> StateOwnershipSummary {
    StateOwnershipSummary {
        permissions: remap_permission_event_span(summary.permissions),
    }
}

#[cfg(test)]
mod tests;
