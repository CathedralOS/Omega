//! Optimizer module role: executable entrance. Optimized instruction-selection stage.
//!
//! This entrance owns the exact target-register environment, construction,
//! independent replay, and retained-custody join. Constraint projection and
//! lowering mechanics descend into named leaves.

mod constraints;
mod construction;
mod model;
mod validation;

pub use constraints::selection_constraints;
pub use model::*;
pub use validation::validate_optimized_selection_custody;

use omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (legalized, selected) = construction::construct_optimized_instruction_selection(
        &optimized_target,
        &register_environment,
    )?;
    let custody = validate_optimized_selection_custody(
        &optimized_target,
        &register_environment,
        &legalized,
        &selected,
    )
    .map_err(OptimizedSelectionPipelineError::Custody)?;
    Ok(StagedOptimizedSelectedInstructions {
        optimized_target: optimized_target.into(),
        register_environment,
        legalized,
        selected,
        custody,
    })
}
