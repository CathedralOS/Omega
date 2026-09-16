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
/// encoded fault surface and drops its zeroed auxiliary `Use` operands —
/// or the literal `1` at the divisor operand of a wrapping remainder into a
/// `MaterializeI64` of the constant zero — a remainder by one is always
/// zero — discharging the consumer's encoded fault surface and dropping its
/// dividend `Use` and dead scratch `Def` operands — or the literal `0` at
/// either operand of a bitwise-and into a `MaterializeI64` of the constant
/// zero — `x & 0` is `0` for every `x` — dropping the other `Use` and dead
/// scratch `Def` operands — or the literal `0` at either operand of a
/// bitwise-xor into a `CopyI64` of the surviving operand — `x ^ 0` and
/// `0 ^ x` are both `x` — or the literal `0` at either operand of a
/// wrapping add into a `CopyI64` of the surviving operand — `x + 0` and
/// `0 + x` are both `x` modulo 2^64 — or the all-ones literal at either
/// operand of a bitwise-and into a `CopyI64` of the surviving operand —
/// `x & MAX` and `MAX & x` are both `x`, disjoint on the literal's value
/// from the bitwise-and annihilator fold — or the literal `0` at the
/// dividend operand of a wrapping remainder into a `MaterializeI64` of
/// the constant zero — `0 % x` is `0` for every `x` — discharging the
/// consumer's encoded fault surface under the nonzero-divisor obligation
/// the remainder kind already carries, and dropping its divisor `Use`
/// and dead scratch `Def` operands; disjoint on the folded literal's
/// operand position from the divisor-one fold.
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
