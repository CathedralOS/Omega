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
mod tests {
    use super::*;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn remap_boundary_summary_preserves_edge_handles() {
        let edge = omega_state_graph::StateBoundaryEdge {
            statement_index: 1,
            call_ordinal: 2,
            receiver_symbol: SymbolHandle::from_arena_index(3),
            target_symbol: SymbolHandle::from_arena_index(4),
            boundary_trait_symbol: SymbolHandle::from_arena_index(5),
            boundary_signature_symbol: SymbolHandle::from_arena_index(6),
        };
        let mut edges = Arena::new();
        let mut span = omega_core::arena::HandleSpan::empty();
        edges.append_to_span(&mut span, edge);

        let summary =
            remap_boundary_summary(&omega_state_graph::StateBoundarySummary { edges: span });

        assert_eq!(summary.edges.count(), 1);
        assert_eq!(
            summary.edges.start().arena_index(),
            span.start().arena_index()
        );
    }
}
