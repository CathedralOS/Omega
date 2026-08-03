use omega_control_flow::{StateBoundaryEdge, StateBoundarySummary};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;
use crate::handles::remap_boundary_edge_span;

pub(crate) fn remap_boundary_edges(state_graph: &StateGraph) -> Arena<StateBoundaryEdge> {
    remap_arena(
        &state_graph.semantics.boundaries.edges,
        remap_boundary_edge_owned,
    )
}

pub(crate) fn remap_boundary_edge_owned(
    edge: omega_state_graph::StateBoundaryEdge,
) -> StateBoundaryEdge {
    StateBoundaryEdge {
        statement_index: edge.statement_index,
        call_ordinal: edge.call_ordinal,
        receiver_symbol: edge.receiver_symbol,
        target_symbol: edge.target_symbol,
        boundary_trait_symbol: edge.boundary_trait_symbol,
        boundary_signature_symbol: edge.boundary_signature_symbol,
    }
}

pub(crate) fn remap_boundary_summary(
    summary: &omega_state_graph::StateBoundarySummary,
) -> StateBoundarySummary {
    StateBoundarySummary {
        edges: remap_boundary_edge_span(summary.edges),
    }
}

#[cfg(test)]
mod tests;
