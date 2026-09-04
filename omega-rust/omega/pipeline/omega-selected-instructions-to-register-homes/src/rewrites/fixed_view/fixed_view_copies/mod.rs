//! Optimizer module role: executable entrance. Exact fixed-view-copy recovery stage.
//!
//! This entrance validates source legality, materializes the requested exact
//! policy, independently replays the copy plan, and only then grants custody.

mod compute;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_fixed_view_copy_custody;

use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::FixedViewCopyPolicy;

use crate::StagedOptimizedFixedPrecoloredSegmentHomes;

pub fn stage_optimized_fixed_view_copies(
    source: StagedOptimizedFixedPrecoloredSegmentHomes,
    policy: FixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    validation::validate_source(&source)?;
    let copies = compute::compute_fixed_view_copies(&source, policy, budget)?;
    let custody = validate_optimized_fixed_view_copy_custody(&source, &copies)?;
    Ok(StagedOptimizedFixedViewCopies {
        source,
        copies,
        custody,
    })
}
