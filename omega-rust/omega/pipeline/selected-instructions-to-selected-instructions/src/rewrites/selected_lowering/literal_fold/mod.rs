//! Optimizer module role: executable entrance. Exact incoming-literal fold and independent replay entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod pair_rule;
pub(crate) mod validate;

#[cfg(test)]
mod tests;

pub use identity::literal_fold_identity;
pub use model::*;
pub use pair_rule::*;
pub use validate::validate_literal_fold;

/// Fold one classified incoming literal into its immediately following enabled
/// consumer: an unsigned-12-bit literal into an exact-add, exact-subtract, or
/// compare immediate form, or an unsigned 64-bit literal through a unary
/// extension into a direct `MaterializeI64`.
pub fn fold_selected_incoming_literal<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    let plan = compute::compute_terminal_literal_fold(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_literal_fold(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
