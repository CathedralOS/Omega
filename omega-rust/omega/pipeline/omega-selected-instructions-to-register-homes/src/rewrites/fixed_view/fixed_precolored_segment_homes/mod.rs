//! Optimizer module role: executable entrance. Fixed-boundary source-home custody.
//!
//! This stage retains fixed intervals, source segmentation, and pre-transform
//! segment homes as prerequisites for an exact recovery rule. These homes are
//! invalid after selected instructions change and never bypass reanalysis.

mod compute;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_fixed_precolored_segment_home_custody;

use omega_optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedAllocationLegality;

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
