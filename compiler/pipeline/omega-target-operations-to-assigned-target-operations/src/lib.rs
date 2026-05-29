use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::TargetOperationPlan;

mod builder;
mod functions;
mod operations;
mod registers;
#[cfg(test)]
mod tests;
mod values;

pub fn build_assigned_target_operations(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationPlan {
    builder::build_assigned_target_operations(target_operations)
}
