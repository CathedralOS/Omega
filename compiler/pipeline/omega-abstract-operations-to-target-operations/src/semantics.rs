use omega_abstract_operations::{AbstractOperationPlan, AbstractSemanticSummary};
use omega_calling_conventions::HostAbiPlan;

use crate::boundary_policy::validate_boundary_policies;

pub(crate) fn build_target_semantic_summary(
    host_abi: &HostAbiPlan,
    abstract_operations: &AbstractOperationPlan,
) -> AbstractSemanticSummary {
    let mut semantics = abstract_operations.semantics.clone();
    validate_boundary_policies(host_abi, &mut semantics.boundaries);
    semantics
}
