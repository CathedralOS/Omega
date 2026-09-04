//! Optimizer module role: executable entrance. Register-home staging after selected-lowering transformations.
//!
//! One-step literal-fold chains and complete selected-lowering runs retain
//! distinct source custody. This entrance grants either result custody only
//! after independent home and manifest replay.

mod construction;
mod custody;
mod model;
mod projection;
mod validation;

pub use model::*;
pub use validation::{
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};

use crate::{StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationRun};

pub fn stage_optimized_register_homes_after_literal_folds(
    folds: StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedRegisterHomesAfterLiteralFolds, OptimizedPostLiteralFoldHomeCustodyError>
{
    let staged = construction::construct_register_homes_after_literal_folds(folds)?;
    validate_optimized_register_home_after_literal_fold_custody(&staged)?;
    Ok(staged)
}

pub fn stage_optimized_register_homes_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let staged = construction::construct_register_homes_after_selected_lowering(run)?;
    validate_optimized_register_home_after_selected_lowering_custody(&staged)?;
    Ok(staged)
}
