use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::TargetOperationPlan;

use crate::code::build_assigned_target_operation_code;

pub(crate) fn build_assigned_target_operations(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationPlan {
    AssignedTargetOperationPlan {
        target: target_operations.target,
        code: build_assigned_target_operation_code(target_operations),
        semantics: target_operations.semantics.clone(),
    }
}
