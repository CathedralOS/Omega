use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractBoundaryLink, AbstractBoundarySummary, AbstractSourceBoundaryEdge,
};
use omega_control_flow::ControlFlowPlan;
use omega_platform_interface::HostCallPlan;

pub(super) fn build_abstract_boundary_summary(
    control_flow: &ControlFlowPlan,
    host_calls: &HostCallPlan,
) -> AbstractBoundarySummary {
    let mut summary = AbstractBoundarySummary::with_source_and_host_capacity(
        control_flow.semantics.boundaries.edges.len(),
        host_calls.operations.len(),
    );

    for (_, state) in control_flow.states.iter() {
        for edge in control_flow
            .semantics
            .boundaries
            .edges
            .span_or_empty(state.boundaries.edges)
        {
            summary.source_edges.insert(AbstractSourceBoundaryEdge {
                source_key: state.key,
                statement_index: edge.statement_index,
                call_ordinal: edge.call_ordinal,
                receiver_symbol: edge.receiver_symbol,
                target_symbol: edge.target_symbol,
                boundary_trait_symbol: edge.boundary_trait_symbol,
                boundary_signature_symbol: edge.boundary_signature_symbol,
            });
        }
    }

    for (_, call) in host_calls.calls.iter() {
        for (operation_ordinal, operation) in host_calls
            .operations
            .span_or_empty(call.operations)
            .iter()
            .enumerate()
        {
            let lowered_edge = summary.edges.insert(AbstractBoundaryEdge {
                source_key: call.source_key,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                operation_ordinal,
                operation_key: operation.operation_key,
            });
            append_boundary_links(
                &mut summary,
                call.source_key,
                call.statement_index,
                call.call_ordinal,
                lowered_edge,
            );
        }
    }

    summary
}

fn append_boundary_links(
    summary: &mut AbstractBoundarySummary,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    call_ordinal: usize,
    lowered_edge: psi_arena::Handle<AbstractBoundaryEdge>,
) {
    let source_edges: Vec<_> = summary
        .source_edges
        .iter()
        .filter_map(|(source_edge, edge)| {
            (edge.source_key == source_key
                && edge.statement_index == statement_index
                && edge.call_ordinal == call_ordinal)
                .then_some(source_edge)
        })
        .collect();

    for source_edge in source_edges {
        summary.links.insert(AbstractBoundaryLink {
            source_edge,
            lowered_edge,
        });
    }
}

#[cfg(test)]
mod tests;
