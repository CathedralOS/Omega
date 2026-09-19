//! Optimizer module role: executable entrance. Replayed register-home assignment.
//!
//! Baseline legality and post-copy reanalysis are distinct source families.
//! This entrance grants custody only after each constructed home/manifest pair
//! independently replays through its matching source-family validator.

mod construction;
mod custody;
mod model;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

pub use model::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::{
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};

use crate::StagedOptimizedAllocationLegality;
use crate::StagedOptimizedSelectedReanalysis;

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

pub(crate) fn stage_register_homes_with_assignment(
    legality: StagedOptimizedAllocationLegality,
    homes: crate::ValidatedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let staged = construction::construct_with_assignment(legality, homes)?;
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

/// Post-copy assignment probe the recovery route runs while it still owns the
/// reanalysis, so residual `NoCompatibleHome` pressure can be answered by
/// runtime spill instead of failing the fixed-view sequence.
pub(crate) fn assign_optimized_register_homes_after_fixed_view_copies(
    reanalysis: &StagedOptimizedSelectedReanalysis,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    construction::assign_optimized_register_homes_after_fixed_view_copies(reanalysis)
}

pub(crate) fn stage_register_homes_after_fixed_view_copies_with_assignment(
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let staged =
        construction::construct_optimized_register_homes_after_fixed_view_copies_with_assignment(
            reanalysis, homes,
        )?;
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
