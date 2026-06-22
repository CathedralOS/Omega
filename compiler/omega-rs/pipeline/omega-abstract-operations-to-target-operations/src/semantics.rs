use omega_abstract_operations::{AbstractOperationPlan, AbstractSemanticSummary};
use omega_calling_conventions::HostAbiPlan;

use crate::boundary_policy::validate_boundary_policies;

pub(crate) fn build_target_semantic_summary(
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
) -> AbstractSemanticSummary {
    AbstractSemanticSummary::with_roots(
        abstract_operations.semantics.values.clone(),
        validated_boundary_summary(host_abi, abstract_operations),
        abstract_operations.semantics.ownership.clone(),
    )
}

fn validated_boundary_summary(
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
) -> omega_abstract_operations::AbstractBoundarySummary {
    let mut boundaries = abstract_operations.semantics.boundaries.clone();
    validate_boundary_policies(host_abi, &mut boundaries);
    boundaries
}
