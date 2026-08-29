//! Selected-CFG liveness staging.
//!
//! This entrance owns the analysis-to-independent-replay join. No liveness
//! result receives stage custody before replay reconstructs its exact receipt.

mod compute;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_liveness_custody;

use crate::StagedOptimizedSelectedInstructions;

pub fn stage_optimized_liveness(
    selected: StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
    let liveness = compute::compute_liveness(&selected)?;
    let custody = validate_optimized_liveness_custody(&selected, &liveness)?;
    Ok(StagedOptimizedLiveness {
        selected,
        liveness,
        custody,
    })
}
