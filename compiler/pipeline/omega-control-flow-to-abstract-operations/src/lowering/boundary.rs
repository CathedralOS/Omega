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
        control_flow.semantics.boundary_edges.len(),
        host_calls.operations.len(),
    );

    for (_, state) in control_flow.states.iter() {
        for edge in control_flow
            .semantics
            .boundary_edges
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
    lowered_edge: omega_core::arena::Handle<AbstractBoundaryEdge>,
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
mod tests {
    use super::*;
    use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
    use omega_control_flow::StateKey;
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use omega_platform_interface::{HostCall, LoweredHostOperation};

    #[test]
    fn copies_host_operations_as_boundary_edges() {
        let mut host_calls = HostCallPlan::default();
        let mut call = HostCall {
            source_key: StateKey {
                machine: SymbolHandle::from_arena_index(1),
                state: SymbolHandle::from_arena_index(2),
                segment_index: 0,
            },
            statement_index: 5,
            call_ordinal: 2,
            ..HostCall::default()
        };
        let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
        host_calls.operations.append_to_span(
            &mut call.operations,
            LoweredHostOperation {
                operation_key,
                fixed_leading_immediate: None,
            },
        );
        host_calls.operations.append_to_span(
            &mut call.operations,
            LoweredHostOperation {
                operation_key: HostOperationKey::new(
                    HostCapability::Stdout,
                    HostOperation::WriteFile,
                ),
                fixed_leading_immediate: None,
            },
        );
        call.arguments = HandleSpan::empty();
        host_calls.calls.insert(call);

        let summary = build_abstract_boundary_summary(&ControlFlowPlan::default(), &host_calls);

        let edges: Vec<_> = summary.edges.iter().map(|(_, edge)| edge).collect();
        assert_eq!(summary.edges.len(), 2);
        assert_eq!(edges[0].statement_index, 5);
        assert_eq!(edges[0].call_ordinal, 2);
        assert_eq!(edges[0].operation_ordinal, 0);
        assert_eq!(edges[0].operation_key, operation_key);
        assert_eq!(edges[1].operation_ordinal, 1);
    }

    #[test]
    fn copies_control_flow_boundary_edges_as_source_boundary_edges() {
        let mut control_flow = ControlFlowPlan::default();
        let state_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let mut edge_span = HandleSpan::empty();
        control_flow.semantics.boundary_edges.append_to_span(
            &mut edge_span,
            omega_control_flow::StateBoundaryEdge {
                statement_index: 8,
                call_ordinal: 1,
                receiver_symbol: SymbolHandle::from_arena_index(3),
                target_symbol: SymbolHandle::from_arena_index(4),
                boundary_trait_symbol: SymbolHandle::from_arena_index(5),
                boundary_signature_symbol: SymbolHandle::from_arena_index(6),
            },
        );
        control_flow.states.insert(omega_control_flow::StateFlow {
            key: state_key,
            boundaries: omega_control_flow::StateBoundarySummary { edges: edge_span },
            ..Default::default()
        });

        let summary = build_abstract_boundary_summary(&control_flow, &HostCallPlan::default());

        let edge = summary
            .source_edges
            .iter()
            .next()
            .map(|(_, edge)| edge)
            .unwrap();
        assert_eq!(summary.source_edges.len(), 1);
        assert_eq!(edge.source_key, state_key);
        assert_eq!(edge.statement_index, 8);
        assert_eq!(edge.call_ordinal, 1);
        assert_eq!(
            edge.boundary_trait_symbol,
            SymbolHandle::from_arena_index(5)
        );
        assert_eq!(
            edge.boundary_signature_symbol,
            SymbolHandle::from_arena_index(6)
        );
    }

    #[test]
    fn links_source_boundary_edges_to_lowered_host_operations() {
        let mut control_flow = ControlFlowPlan::default();
        let state_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let mut edge_span = HandleSpan::empty();
        control_flow.semantics.boundary_edges.append_to_span(
            &mut edge_span,
            omega_control_flow::StateBoundaryEdge {
                statement_index: 8,
                call_ordinal: 1,
                receiver_symbol: SymbolHandle::from_arena_index(3),
                target_symbol: SymbolHandle::from_arena_index(4),
                boundary_trait_symbol: SymbolHandle::from_arena_index(5),
                boundary_signature_symbol: SymbolHandle::from_arena_index(6),
            },
        );
        control_flow.states.insert(omega_control_flow::StateFlow {
            key: state_key,
            boundaries: omega_control_flow::StateBoundarySummary { edges: edge_span },
            ..Default::default()
        });

        let mut host_calls = HostCallPlan::default();
        let mut call = HostCall {
            source_key: state_key,
            statement_index: 8,
            call_ordinal: 1,
            ..HostCall::default()
        };
        let operation_key = HostOperationKey::new(HostCapability::Stdout, HostOperation::Write);
        host_calls.operations.append_to_span(
            &mut call.operations,
            LoweredHostOperation {
                operation_key,
                fixed_leading_immediate: None,
            },
        );
        host_calls.calls.insert(call);

        let summary = build_abstract_boundary_summary(&control_flow, &host_calls);

        assert_eq!(summary.source_edges.len(), 1);
        assert_eq!(summary.edges.len(), 1);
        let link = summary.links.iter().next().map(|(_, link)| link).unwrap();
        assert_eq!(summary.links.len(), 1);
        assert_eq!(
            summary
                .source_edges
                .get(link.source_edge)
                .boundary_signature_symbol,
            SymbolHandle::from_arena_index(6)
        );
        assert_eq!(
            summary.edges.get(link.lowered_edge).operation_key,
            operation_key
        );
    }

    #[test]
    fn does_not_link_distinct_call_ordinals_on_same_statement() {
        let mut control_flow = ControlFlowPlan::default();
        let state_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let mut edge_span = HandleSpan::empty();
        control_flow.semantics.boundary_edges.append_to_span(
            &mut edge_span,
            omega_control_flow::StateBoundaryEdge {
                statement_index: 8,
                call_ordinal: 1,
                receiver_symbol: SymbolHandle::from_arena_index(3),
                target_symbol: SymbolHandle::from_arena_index(4),
                boundary_trait_symbol: SymbolHandle::from_arena_index(5),
                boundary_signature_symbol: SymbolHandle::from_arena_index(6),
            },
        );
        control_flow.states.insert(omega_control_flow::StateFlow {
            key: state_key,
            boundaries: omega_control_flow::StateBoundarySummary { edges: edge_span },
            ..Default::default()
        });

        let mut host_calls = HostCallPlan::default();
        let mut call = HostCall {
            source_key: state_key,
            statement_index: 8,
            call_ordinal: 2,
            ..HostCall::default()
        };
        host_calls.operations.append_to_span(
            &mut call.operations,
            LoweredHostOperation {
                operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
                fixed_leading_immediate: None,
            },
        );
        host_calls.calls.insert(call);

        let summary = build_abstract_boundary_summary(&control_flow, &host_calls);

        assert_eq!(summary.source_edges.len(), 1);
        assert_eq!(summary.edges.len(), 1);
        assert_eq!(summary.links.len(), 0);
    }
}
