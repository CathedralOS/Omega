//! Optimizer module role: executable entrance. Epoch-two recovery-victim choice.
//!
//! This boundary consumes the exact retained blocker roster and names one
//! compiler-private value whose removal recovers a candidate view. It performs
//! no eviction, spill, reload, rewrite, or physical realization.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::generalized_spill_recovery_choice_identity;
pub use model::*;
pub use validate::validate_generalized_spill_recovery_choices;

#[allow(clippy::too_many_arguments)]
pub fn choose_generalized_spill_recovery_victims<S: crate::ValidatedSelectedAnalysis>(
    worklist: &crate::ValidatedGeneralizedSpillRecoveryWorklist,
    homes: &crate::ValidatedGeneralizedReloadValueHomes,
    selected: &S,
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
    reservations: &register_model::ValidatedRegisterReservationProfile,
    selected_keys: register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedSpillRecoveryChoicePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillRecoveryChoices, GeneralizedSpillRecoveryChoiceError> {
    let plan = compute::compute(
        worklist,
        homes,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_generalized_spill_recovery_choices(
        worklist,
        homes,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
