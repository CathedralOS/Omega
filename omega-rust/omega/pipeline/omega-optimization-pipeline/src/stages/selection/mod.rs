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
    validate_optimized_selection_custody,
};
pub use omega_target_to_register_environment::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment, validate_target_register_environment,
    validate_target_register_environment_with_reservations,
};

/// Internal test shorthand. Production coordination must preserve the target
/// register environment as its own stage and pass it into instruction selection.
#[cfg(test)]
pub(crate) fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let environment = baseline_target_register_environment(optimized_target.target())
        .expect("the baseline test register environment must validate");
    omega_target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        optimized_target,
        environment,
    )
}
