use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::TargetOperationPlan;

use crate::code::build_assigned_target_operation_code;
use crate::semantics::build_assigned_semantic_summary;

pub(crate) fn build_assigned_target_operations(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationPlan {
    AssignedTargetOperationPlan::with_roots(
        target_operations.target,
        build_assigned_target_operation_code(target_operations),
        build_assigned_semantic_summary(target_operations),
    )
}
