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
pub fn choose_generalized_spill_recovery_victims(
    worklist: &crate::ValidatedGeneralizedSpillRecoveryWorklist,
    homes: &crate::ValidatedGeneralizedReloadValueHomes,
    legality: &crate::ValidatedAllocationLegality,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    constraints: &omega_register_model::ValidatedRegisterConstraintCatalog,
    reservations: &omega_register_model::ValidatedRegisterReservationProfile,
    selected_keys: omega_register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedSpillRecoveryChoicePolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillRecoveryChoices, GeneralizedSpillRecoveryChoiceError> {
    let plan = compute::compute(
        worklist,
        homes,
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
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
