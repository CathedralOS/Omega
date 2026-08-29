use omega_assigned_target_operations::AssignedOperationPlan;
use omega_target_operations_to_assigned_target_operations::assign_registers;

use crate::{
    ValidatedOptimizedTargetOperations, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment,
};

use super::OptimizedAssignmentPipelineError;

pub(super) fn construct_optimized_assignment(
    optimized_target: &ValidatedOptimizedTargetOperations,
) -> Result<
    (ValidatedTargetRegisterEnvironment, AssignedOperationPlan),
    OptimizedAssignmentPipelineError,
> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedAssignmentPipelineError::RegisterEnvironment)?;
    let assigned = assign_registers(optimized_target.target_operations())
        .map_err(OptimizedAssignmentPipelineError::Assignment)?;
    Ok((register_environment, assigned))
}
