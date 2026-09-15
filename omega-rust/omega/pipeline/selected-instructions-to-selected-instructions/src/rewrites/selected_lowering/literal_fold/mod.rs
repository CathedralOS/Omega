//! Optimizer module role: executable entrance. Exact incoming-literal fold and independent replay entrance.
use crate::ValidatedAllocationLegality;
use crate::ValidatedAllocatorAvailability;
use crate::ValidatedLiveRanges;
use crate::ValidatedRecoveryClassifications;
use crate::ValidatedSpillChoices;
use crate::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, ValidatedSelectedAnalysis,
};

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
/// consumer: an unsigned-12-bit literal at either operand of the commutative
/// exact-add, at the right operand of exact-subtract/compare, at the index
/// operand of the indexed byte load, or at the offset operand of the
/// byte-view address projection into the matching immediate or
/// constant-offset form, an unsigned 64-bit literal through a unary
/// extension or copy into a direct `MaterializeI64`, or the literal `1` at
/// the divisor operand of an unsigned exact divide into a `CopyI64` of the
/// dividend — the divide-by-one identity that discharges the consumer's
/// encoded fault surface and drops its zeroed auxiliary `Use` operands.
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
    effect_catalog: &selected_instructions::ValidatedMachineEffectCatalog,
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
        effect_catalog,
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
        effect_catalog,
        plan,
    )
}
