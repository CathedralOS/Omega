//! Optimizer module role: executable entrance. Deterministic bounded pressure-victim selection entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

pub use identity::spill_choice_identity;
pub use model::*;
pub use validate::validate_spill_choices;

/// Select the deterministic recovery victim at each first supported local
/// pressure point without materializing spill or recovery instructions.
pub fn choose_spill_victims(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: SpillChoicePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    let plan = compute::compute_terminal_spill_choices(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_spill_choices(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
