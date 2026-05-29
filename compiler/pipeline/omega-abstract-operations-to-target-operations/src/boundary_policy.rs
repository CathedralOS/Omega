use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractBoundarySummary,
};
use omega_calling_conventions::{HostAbiPlan, HostBinding, HostOperationKey};
use omega_core::arena::Handle;

pub(crate) fn validate_boundary_policies(
    host_abi: &HostAbiPlan,
    boundaries: &mut AbstractBoundarySummary,
) {
    boundaries.policy_checks.clear();

    let linked_edges: Vec<_> = boundaries
        .links
        .iter()
        .map(|(_, link)| {
            let lowered_edge = boundaries.edges.get(link.lowered_edge);
            (
                link.source_edge,
                link.lowered_edge,
                lowered_edge.operation_key,
            )
        })
        .collect();

    for (source_edge, lowered_edge, operation_key) in linked_edges {
        append_policy_check(
            host_abi,
            boundaries,
            source_edge,
            lowered_edge,
            operation_key,
        );
    }

    let unlinked_edges: Vec<_> = boundaries
        .edges
        .iter()
        .filter_map(|(lowered_edge, edge)| {
            let has_source_link = boundaries
                .links
                .iter()
                .any(|(_, link)| link.lowered_edge == lowered_edge);
            (!has_source_link).then_some((lowered_edge, edge.operation_key))
        })
        .collect();

    for (lowered_edge, operation_key) in unlinked_edges {
        let binding = host_binding(host_abi, operation_key);
        boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                source_edge: Handle::invalid(),
                lowered_edge,
                operation_key,
                boundary_policy: binding
                    .map(|binding| binding.boundary_policy.clone())
                    .unwrap_or_default(),
                verdict: if binding.is_some() {
                    AbstractBoundaryPolicyVerdict::MissingSourceBoundary
                } else {
                    AbstractBoundaryPolicyVerdict::MissingHostBinding
                },
            });
    }
}

fn append_policy_check(
    host_abi: &HostAbiPlan,
    boundaries: &mut AbstractBoundarySummary,
    source_edge: Handle<omega_abstract_operations::AbstractSourceBoundaryEdge>,
    lowered_edge: Handle<omega_abstract_operations::AbstractBoundaryEdge>,
    operation_key: HostOperationKey,
) {
    let Some(binding) = host_binding(host_abi, operation_key) else {
        boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                source_edge,
                lowered_edge,
                operation_key,
                boundary_policy: Default::default(),
                verdict: AbstractBoundaryPolicyVerdict::MissingHostBinding,
            });
        return;
    };

    boundaries
        .policy_checks
        .insert(AbstractBoundaryPolicyCheck {
            source_edge,
            lowered_edge,
            operation_key,
            boundary_policy: binding.boundary_policy.clone(),
            verdict: AbstractBoundaryPolicyVerdict::Accepted,
        });
}

fn host_binding(host_abi: &HostAbiPlan, operation_key: HostOperationKey) -> Option<&HostBinding> {
    host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding))
}
