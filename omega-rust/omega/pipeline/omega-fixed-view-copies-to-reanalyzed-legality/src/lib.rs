#![forbid(unsafe_code)]

//! Pipeline stage for complete reanalysis after selected-CFG transformation.
//!
//! No source analysis fact is reused. This entrance validates transformed
//! source custody, recomputes liveness/ranges/legality, independently replays
//! the complete chain, and only then grants reanalysis custody.

mod compute;
mod custody;
mod invariants;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_selected_reanalysis_custody;

use omega_allocation_legality_to_fixed_view_copies::StagedOptimizedFixedViewCopies;

pub fn stage_optimized_selected_reanalysis(
    transformation: StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedSelectedReanalysis, OptimizedSelectedReanalysisError> {
    validation::validate_source(&transformation)?;
    let (liveness, ranges, legality) = compute::compute_selected_reanalysis(&transformation)?;
    let custody = validate_optimized_selected_reanalysis_custody(
        &transformation,
        &liveness,
        &ranges,
        &legality,
    )?;
    Ok(StagedOptimizedSelectedReanalysis {
        transformation,
        liveness,
        ranges,
        legality,
        custody,
    })
}
