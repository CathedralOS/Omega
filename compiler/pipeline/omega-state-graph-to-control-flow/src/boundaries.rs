use omega_control_flow::{StateBoundaryEdge, StateBoundarySummary};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

use crate::handles::remap_boundary_edge_span;

pub(crate) fn remap_boundary_edges(state_graph: &StateGraph) -> Arena<StateBoundaryEdge> {
    let mut boundary_edges = Arena::with_capacity(state_graph.semantics.boundaries.edges.len());
    for (_, edge) in state_graph.semantics.boundaries.edges.iter() {
        boundary_edges.append(remap_boundary_edge(edge));
    }
    boundary_edges
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

fn remap_boundary_edge(edge: &omega_state_graph::StateBoundaryEdge) -> StateBoundaryEdge {
    StateBoundaryEdge {
        statement_index: edge.statement_index,
        call_ordinal: edge.call_ordinal,
        receiver_symbol: edge.receiver_symbol,
        target_symbol: edge.target_symbol,
        boundary_trait_symbol: edge.boundary_trait_symbol,
        boundary_signature_symbol: edge.boundary_signature_symbol,
    }
}

#[cfg(test)]
mod tests;
