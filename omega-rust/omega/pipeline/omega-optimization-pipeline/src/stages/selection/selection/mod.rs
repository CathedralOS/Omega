//! Optimizer module role: executable entrance. Optimized instruction-selection stage.
//!
//! This entrance owns the exact target-register environment, construction,
//! independent replay, and retained-custody join. Constraint projection and
//! lowering mechanics descend into named leaves.

mod constraints;
mod construction;
mod model;
mod validation;

#[cfg(test)]
pub(crate) use constraints::selection_constraints;
pub use model::*;
pub use validation::validate_optimized_selection_custody;

use crate::ValidatedOptimizedTargetOperations;

pub fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (register_environment, legalized, selected) =
        construction::construct_optimized_instruction_selection(&optimized_target)?;
    let custody = validate_optimized_selection_custody(
        &optimized_target,
        &register_environment,
        &legalized,
        &selected,
    )
    .map_err(OptimizedSelectionPipelineError::Custody)?;
    Ok(StagedOptimizedSelectedInstructions {
        optimized_target,
        register_environment,
        legalized,
        selected,
        custody,
    })
}
