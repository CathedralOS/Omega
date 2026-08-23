use super::*;
use psi_symbols::SymbolHandle;

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
    let mut span = psi_arena::HandleSpan::empty();
    edges.append_to_span(&mut span, edge);

    let summary = remap_boundary_summary(&omega_state_graph::StateBoundarySummary { edges: span });

    assert_eq!(summary.edges.count(), 1);
    assert_eq!(
        summary.edges.start().arena_index(),
        span.start().arena_index()
    );
}
