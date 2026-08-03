use omega_abstract_operations::{
    AbstractBoundaryEdge, AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict,
    AbstractBoundarySummary, AbstractSourceBoundaryEdge,
};
use omega_calling_conventions::{HostAbiPlan, HostBinding, HostOperationKey};
use psi_arena::Handle;

pub(crate) fn validate_boundary_policies(
    host_abi: &HostAbiPlan,
    boundaries: &mut AbstractBoundarySummary,
) {
    boundaries.policy_checks.clear();

    for linked_edge in linked_boundary_edges(boundaries) {
        boundaries
            .policy_checks
            .insert(policy_check_for_linked_edge(host_abi, linked_edge));
    }

    for unlinked_edge in unlinked_boundary_edges(boundaries) {
        boundaries
            .policy_checks
            .insert(policy_check_for_unlinked_edge(host_abi, unlinked_edge));
    }
}

#[derive(Debug, Clone, Copy)]
struct LinkedBoundaryEdge {
    source_edge: Handle<AbstractSourceBoundaryEdge>,
    lowered_edge: Handle<AbstractBoundaryEdge>,
    operation_key: HostOperationKey,
}

#[derive(Debug, Clone, Copy)]
struct UnlinkedBoundaryEdge {
    lowered_edge: Handle<AbstractBoundaryEdge>,
    operation_key: HostOperationKey,
}

fn linked_boundary_edges(boundaries: &AbstractBoundarySummary) -> Vec<LinkedBoundaryEdge> {
    boundaries
        .links
        .iter()
        .map(|(_, link)| {
            let lowered_edge = boundaries.edges.get(link.lowered_edge);
            LinkedBoundaryEdge {
                source_edge: link.source_edge,
                lowered_edge: link.lowered_edge,
                operation_key: lowered_edge.operation_key,
            }
        })
        .collect()
}

fn unlinked_boundary_edges(boundaries: &AbstractBoundarySummary) -> Vec<UnlinkedBoundaryEdge> {
    boundaries
        .edges
        .iter()
        .filter_map(|(lowered_edge, edge)| {
            (!has_source_link(boundaries, lowered_edge)).then_some(UnlinkedBoundaryEdge {
                lowered_edge,
                operation_key: edge.operation_key,
            })
        })
        .collect()
}

fn has_source_link(
    boundaries: &AbstractBoundarySummary,
    lowered_edge: Handle<AbstractBoundaryEdge>,
) -> bool {
    boundaries
        .links
        .iter()
        .any(|(_, link)| link.lowered_edge == lowered_edge)
}

fn policy_check_for_linked_edge(
    host_abi: &HostAbiPlan,
    edge: LinkedBoundaryEdge,
) -> AbstractBoundaryPolicyCheck {
    let Some(binding) = host_binding(host_abi, edge.operation_key) else {
        return AbstractBoundaryPolicyCheck {
            source_edge: edge.source_edge,
            lowered_edge: edge.lowered_edge,
            operation_key: edge.operation_key,
            boundary_policy: Default::default(),
            verdict: AbstractBoundaryPolicyVerdict::MissingHostBinding,
        };
    };

    AbstractBoundaryPolicyCheck {
        source_edge: edge.source_edge,
        lowered_edge: edge.lowered_edge,
        operation_key: edge.operation_key,
        boundary_policy: binding.boundary_policy.clone(),
        verdict: if host_abi.allows_boundary_policy(binding.boundary_policy.as_ref()) {
            AbstractBoundaryPolicyVerdict::Accepted
        } else {
            AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy
        },
    }
}

fn policy_check_for_unlinked_edge(
    host_abi: &HostAbiPlan,
    edge: UnlinkedBoundaryEdge,
) -> AbstractBoundaryPolicyCheck {
    let binding = host_binding(host_abi, edge.operation_key);

    AbstractBoundaryPolicyCheck {
        source_edge: Handle::invalid(),
        lowered_edge: edge.lowered_edge,
        operation_key: edge.operation_key,
        boundary_policy: binding
            .map(|binding| binding.boundary_policy.clone())
            .unwrap_or_default(),
        verdict: if binding.is_some() {
            AbstractBoundaryPolicyVerdict::MissingSourceBoundary
        } else {
            AbstractBoundaryPolicyVerdict::MissingHostBinding
        },
    }
}

fn host_binding(host_abi: &HostAbiPlan, operation_key: HostOperationKey) -> Option<&HostBinding> {
    host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding))
}
