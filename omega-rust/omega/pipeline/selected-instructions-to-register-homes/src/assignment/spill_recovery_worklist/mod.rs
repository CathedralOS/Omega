//! Optimizer module role: executable entrance. Bounded recursive spill-recovery worklist seeding.
//!
//! V1 begins only from an independently reproduced logical reload-pressure
//! failure. It grants a compiler-private epoch-one work-item identity, not a
//! selected virtual register, spill decision, instruction, memory effect,
//! frame address, trap claim, encoding, emission, or publication path.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::spill_recovery_worklist_identity;
pub use model::*;
pub use validate::validate_spill_recovery_worklist;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations,
};

#[allow(clippy::too_many_arguments)]
pub fn seed_spill_recovery_worklist(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    reload_home_policy: crate::ReloadValueHomePolicy,
    reload_home_budget: optimization_core::OptimizationWorkBudget,
    policy: SpillRecoveryWorklistPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillRecoveryWorklist, SpillRecoveryWorklistError> {
    let plan = compute::compute(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        reload_home_policy,
        reload_home_budget,
        policy,
        budget,
    )?;
    validate_spill_recovery_worklist(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
