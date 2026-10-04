//! Optimizer module role: executable entrance. Optimized instruction-selection stage.
//!
//! This entrance owns the exact target-register environment, construction,
//! and the custody receipt over what construction produced. Constraint
//! projection and lowering mechanics descend into named leaves.

mod constraints;
mod construction;
mod custody;
mod model;

pub use constraints::selection_constraints;
pub use model::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt,
};

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
use register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (legalized, selected) = construction::construct_optimized_instruction_selection(
        &optimized_target,
        &register_environment,
    )?;
    let custody = custody::selection_custody_receipt(
        &optimized_target,
        &register_environment,
        &legalized,
        &selected,
    );
    // Admit the governing selections and budget as stage-owned data so later
    // stages read them without reaching into the optimized-target product.
    let selections = optimized_target.optimized().selections().clone();
    let budget_per_pass = optimized_target.optimized().budget_per_pass();
    Ok(StagedOptimizedSelectedInstructions {
        optimized_target: std::sync::Arc::new(optimized_target),
        register_environment,
        selections,
        budget_per_pass,
        selected,
        custody,
    })
}
