//! Optimizer module role: executable entrance. Optimized instruction-selection stage.
//!
//! This entrance owns the exact target-register environment, construction,
//! and the custody receipt over what construction produced. Constraint
//! projection and lowering mechanics descend into named leaves.

mod constraints;
mod construction;
mod custody;
mod input;
mod model;

pub use input::TargetSelectionInput;
pub use model::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt,
};

use register_environment::ValidatedTargetRegisterEnvironment;

pub fn stage_optimized_instruction_selection(
    input: TargetSelectionInput<'_>,
    register_environment: ValidatedTargetRegisterEnvironment,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let (legalized, selected) =
        construction::construct_optimized_instruction_selection(&input, &register_environment)?;
    let custody =
        custody::selection_custody_receipt(&input, &register_environment, &legalized, &selected);
    Ok(StagedOptimizedSelectedInstructions {
        register_environment,
        selections: input.selections,
        budget_per_pass: input.budget_per_pass,
        selected,
        custody,
    })
}
