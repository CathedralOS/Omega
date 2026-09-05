//! Optimizer module role: executable entrance. Active-resident pressure-rematerialization stage.
//!
//! The producer rebuilds all allocation facts from the transformed selected
//! CFG. This entrance grants stage custody only after independent replay
//! validation reconstructs that complete chain.

mod compute;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_active_resident_rematerialization;

use crate::{PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy};
use omega_optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedAllocationLegality;

#[allow(clippy::too_many_arguments)]
pub fn stage_optimized_active_resident_rematerialization(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::compute_active_resident_rematerialization(
        source,
        choice_policy,
        classification_policy,
        rematerialization_policy,
        budget,
    )?;
    validate_optimized_active_resident_rematerialization(&staged)?;
    Ok(staged)
}
