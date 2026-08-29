//! Register-home assignment stage.
//!
//! Baseline legality and post-copy reanalysis are distinct source families.
//! This entrance grants custody only after each constructed home/manifest pair
//! independently replays through its matching source-family validator.

mod construction;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::{
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};

use crate::{StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis};

pub fn stage_optimized_register_homes(
    legality: StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let staged = construction::construct_optimized_register_homes(legality)?;
    let custody = validate_optimized_register_home_custody(
        staged.legality_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}

pub fn stage_optimized_register_homes_after_fixed_view_copies(
    reanalysis: StagedOptimizedSelectedReanalysis,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let staged =
        construction::construct_optimized_register_homes_after_fixed_view_copies(reanalysis)?;
    let custody = validate_optimized_register_home_after_fixed_view_copy_custody(
        staged.reanalysis_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedPostCopyRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}
