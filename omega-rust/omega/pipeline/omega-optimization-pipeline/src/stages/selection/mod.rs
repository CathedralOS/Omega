//! Optimizer module role: stage group. Target-operation lowering, assignment, and selected-instruction custody.

#[cfg(test)]
pub(crate) mod assignment;
#[cfg(test)]
pub use assignment::*;
pub use omega_abstract_operations_to_target_operations::{
    ValidatedOptimizedTargetOperations, lower_optimized_to_target_operations,
    lower_optimized_to_target_operations_with_ieee_float_fma_settlements,
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};
pub use omega_target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment, validate_optimized_selection_custody,
    validate_target_register_environment, validate_target_register_environment_with_reservations,
};

/// Transitional convenience entrance for callers that have not yet split
/// target-register construction from instruction selection. Production
/// coordination should sequence those stages explicitly.
pub fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedSelectionPipelineError::RegisterEnvironment)?;
    omega_target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        optimized_target,
        environment,
    )
}
