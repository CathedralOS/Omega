use omega_assigned_target_operations::AssignedSemanticSummary;
use omega_target_operations::TargetOperationPlan;

pub(crate) fn build_assigned_semantic_summary(
    target_operations: &TargetOperationPlan,
) -> AssignedSemanticSummary {
    AssignedSemanticSummary::with_roots(
        target_operations.semantics.values.clone(),
        target_operations.semantics.boundaries.clone(),
        target_operations.semantics.ownership.clone(),
    )
}
