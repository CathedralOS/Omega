//! Optimizer module role: executable entrance. Fixed-boundary source-home custody.
//!
//! This stage retains fixed intervals, source segmentation, and pre-transform
//! segment homes as prerequisites for an exact recovery rule. These homes are
//! invalid after selected instructions change and never bypass reanalysis.

mod compute;
mod custody;
mod model;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

pub use model::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::validate_optimized_fixed_precolored_segment_home_custody;

use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedAllocationLegality;

/// Borrow-only admission probe for this stage: the same source custody check
/// and fixed/precolored derivation the consuming entry runs, without taking
/// custody of the legality chain. A composing route runs it to observe a
/// capacity decline (`capacity_decline()` on the reported error: placement
/// pressure or front-end work-budget exhaustion) before the sequence commits
/// — every other reported failure is exactly the error the staged entry
/// would return, so callers lose no fidelity by probing first.
pub fn probe_optimized_fixed_precolored_segment_homes(
    source: &StagedOptimizedAllocationLegality,
    budget: OptimizationWorkBudget,
) -> Result<(), OptimizedFixedPrecoloredSegmentHomeCustodyError> {
    validation::validate_source(source)?;
    compute::derive(source, budget)?;
    Ok(())
}

pub fn stage_optimized_fixed_precolored_segment_homes(
    source: StagedOptimizedAllocationLegality,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedFixedPrecoloredSegmentHomes,
    OptimizedFixedPrecoloredSegmentHomeCustodyError,
> {
    validation::validate_source(&source)?;
    let (fixed, requirements, homes) = compute::derive(&source, budget)?;
    let custody = validate_optimized_fixed_precolored_segment_home_custody(
        &source,
        &fixed,
        &requirements,
        &homes,
    )?;
    Ok(StagedOptimizedFixedPrecoloredSegmentHomes {
        source,
        fixed,
        requirements,
        homes,
        custody,
    })
}
