use omega_control_flow::StateBoundaryEdge;
use omega_core::arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_boundary_edge_span(
    edges: HandleSpan<omega_state_graph::StateBoundaryEdge>,
) -> HandleSpan<StateBoundaryEdge> {
    remap_span(edges)
}
