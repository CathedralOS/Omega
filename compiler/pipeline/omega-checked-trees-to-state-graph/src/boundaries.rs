use omega_checked_trees::{CheckedTrees, FlowBoundaryEdgeFact};
use omega_core::arena::HandleSpan;
use omega_state_graph::{StateBoundaryEdge, StateBoundarySummary, StateGraph, StateKey};

pub(crate) fn state_boundary_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateBoundarySummary {
    let Some(flow_state) = program
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == key.machine && state.state_symbol == key.state)
                .then_some(state)
        })
    else {
        return StateBoundarySummary::default();
    };

    StateBoundarySummary {
        edges: append_boundary_edges(
            state_graph,
            &program.facts.flow.boundaries.edges,
            flow_state.boundary_edges,
        ),
    }
}

fn append_boundary_edges(
    state_graph: &mut StateGraph,
    source_edges: &omega_core::arena::Arena<FlowBoundaryEdgeFact>,
    edges: HandleSpan<FlowBoundaryEdgeFact>,
) -> HandleSpan<StateBoundaryEdge> {
    state_graph.semantics.boundaries.edges.insert_many(
        source_edges
            .span_or_empty(edges)
            .iter()
            .map(|edge| StateBoundaryEdge {
                statement_index: edge.statement_index,
                call_ordinal: edge.call_ordinal,
                receiver_symbol: edge.receiver_symbol,
                target_symbol: edge.target_symbol,
                boundary_trait_symbol: edge.boundary_trait_symbol,
                boundary_signature_symbol: edge.boundary_signature_symbol,
            }),
    )
}

pub(crate) fn remap_state_boundary_summary(
    target: &mut StateGraph,
    source_edges: &omega_core::arena::Arena<StateBoundaryEdge>,
    summary: &StateBoundarySummary,
) -> StateBoundarySummary {
    StateBoundarySummary {
        edges: target
            .semantics
            .boundaries
            .edges
            .insert_many(source_edges.span_or_empty(summary.edges).iter().cloned()),
    }
}

#[cfg(test)]
mod tests;
