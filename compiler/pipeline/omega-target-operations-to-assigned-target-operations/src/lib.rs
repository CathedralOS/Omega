use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::InstructionPlan;

pub fn build_assigned_target_operations(
    target_operations: &InstructionPlan,
) -> AssignedTargetOperationPlan {
    target_operations.clone().into()
}
