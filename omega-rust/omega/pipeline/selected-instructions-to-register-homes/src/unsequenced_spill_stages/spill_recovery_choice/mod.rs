//! Optimizer module role: executable entrance. Epoch-one recovery-victim choice.
//!
//! This boundary consumes validated worklist custody and names one original
//! resident whose removal recovers a reload candidate. It does not authorize
//! that removal, insert a spill, assign the reload, or grant memory, frame,
//! trap, unwind, encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::spill_recovery_choice_identity;
pub use model::*;
pub use validate::validate_spill_recovery_choices;

use crate::{
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSpillRecoveryWorklist,
};

#[allow(clippy::too_many_arguments)]
pub fn choose_spill_recovery_victims(
    worklist: &ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
    reservations: &register_model::ValidatedRegisterReservationProfile,
    selected_keys: &register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: SpillRecoveryChoicePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillRecoveryChoices, SpillRecoveryChoiceError> {
    let plan = compute::compute(
        worklist,
        insertion,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_spill_recovery_choices(
        worklist,
        insertion,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
