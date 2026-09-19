//! Optimizer module role: executable entrance. Active-resident pressure-rematerialization stage.
//!
//! The producer rebuilds all allocation facts from the transformed selected
//! CFG. This entrance grants stage custody only after independent replay
//! validation reconstructs that complete chain.

mod compute;
mod custody;
mod model;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

pub use model::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::{
    validate_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization_pressure,
};

use crate::{PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy};
use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedAllocationLegality;

/// Stage the proven rematerialization sweep without attempting terminal
/// homes assignment: the result is custody for the route's own decision —
/// complete the sweep when assignment succeeds, or hand the same proven
/// prefix to runtime-spill recovery when residual `NoCompatibleHome`
/// pressure remains.
#[allow(clippy::too_many_arguments)]
pub fn stage_optimized_active_resident_rematerialization_pressure(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerializationPressure,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::compute_active_resident_rematerialization_pressure(
        source,
        choice_policy,
        classification_policy,
        rematerialization_policy,
        budget,
    )?;
    validate_optimized_active_resident_rematerialization_pressure(&staged)?;
    Ok(staged)
}

/// Terminal completion of a proven sweep: the caller supplies the homes a
/// successful assignment over the rebuilt facts produced. Custody is granted
/// only after independent replay reconstructs the complete chain, exactly as
/// the one-shot entrance requires.
pub fn complete_optimized_active_resident_rematerialization(
    pressure: StagedOptimizedActiveResidentRematerializationPressure,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    let staged = compute::complete_active_resident_rematerialization(pressure, homes)?;
    validate_optimized_active_resident_rematerialization(&staged)?;
    Ok(staged)
}

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
