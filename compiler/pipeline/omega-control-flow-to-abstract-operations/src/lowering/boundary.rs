use omega_abstract_operations::{AbstractBoundaryEdge, AbstractBoundarySummary};
use omega_platform_interface::HostCallPlan;

pub(super) fn build_abstract_boundary_summary(
    host_calls: &HostCallPlan,
) -> AbstractBoundarySummary {
    let mut summary = AbstractBoundarySummary::with_capacity(host_calls.operations.len());

    for (_, call) in host_calls.calls.iter() {
        for operation in host_calls.operations.span_or_empty(call.operations) {
            summary.edges.insert(AbstractBoundaryEdge {
                source_key: call.source_key,
                statement_index: call.statement_index,
                operation_key: operation.operation_key,
            });
        }
    }

    summary
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
        call.arguments = HandleSpan::empty();
        host_calls.calls.insert(call);

        let summary = build_abstract_boundary_summary(&host_calls);

        let edge = summary.edges.iter().next().map(|(_, edge)| edge).unwrap();
        assert_eq!(summary.edges.len(), 1);
        assert_eq!(edge.statement_index, 5);
        assert_eq!(edge.operation_key, operation_key);
    }
}
