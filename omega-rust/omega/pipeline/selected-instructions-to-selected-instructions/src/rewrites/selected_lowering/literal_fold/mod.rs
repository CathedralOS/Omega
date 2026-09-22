//! Optimizer module role: executable entrance. Exact incoming-literal fold and independent replay entrance.
use crate::ValidatedAllocationLegality;
use crate::ValidatedAllocatorAvailability;
use crate::ValidatedLiveRanges;
use crate::ValidatedRecoveryClassifications;
use crate::ValidatedSelectedAnalysis;
use crate::ValidatedSpillChoices;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
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
/// and dead scratch `Def` operands — or the literal `0` at the dividend
/// operand of an unsigned exact divide into a `MaterializeI64` of the
/// constant zero — `0 / x` is `0` for every `x` the consumer's proven
/// nonzero divisor admits — discharging the consumer's encoded fault
/// surface under the nonzero-divisor obligation the divide kind already
/// carries, and dropping its divisor `Use` and provably-zero auxiliary
/// `Use` operands; each zero-dividend fold stays disjoint on the folded
/// literal's operand position from its divisor-one sibling — or the
/// literal `0` at either operand of a saturating add on any carrier into
/// a `CopyI64` of the surviving operand — `x +| 0` and `0 +| x` are both
/// `x` inside the carrier's bounds — retiring the consumer's implicit
/// unit definitions under a whole-function deadness gate: the aarch64
/// rows' `nzcv` write may retire only while no instruction or terminator
/// in the function implicitly uses it, while the x86-64 rows' `rflags`
/// clobber retires unconditionally because dropping a clobber only
/// narrows destruction; every clamped carrier's row also drops the bound
/// scratch `Def` past its result under occurrence-free custody — or the
/// literal `0` at the right operand of a saturating subtract on any
/// carrier into a `CopyI64` of the left operand — `x -| 0` is `x` inside
/// the carrier's bounds — under the same retired-definition and
/// occurrence-free-scratch gates, with no left-literal *identity*
/// grammar because subtraction does not commute: `0 -| x` is not `x` —
/// or the literal `0` at the minuend operand of a saturating subtract on
/// an unsigned carrier into a `MaterializeI64` of the constant zero —
/// `0 -| x` is `0` for every `x` an unsigned carrier admits, because
/// `0 - x` underflows the carrier's lower bound and saturates to it —
/// dropping the operand-1 subtrahend `Use` the constant result never
/// reads and retiring the consumer's implicit unit definitions under
/// the same deadness gate; the zero-minuend fold stays disjoint on the
/// folded literal's operand position from its right-zero sibling and
/// admits no signed carrier, where `0 -| x` is `-x` clamped to the
/// carrier's bounds, not a constant — or the literal
/// `1` at the divisor operand of a saturating divide on any carrier into
/// a `CopyI64` of the dividend operand — `x /| 1` is `x` inside the
/// carrier's bounds — the divisor-one literal itself discharging the
/// consumer's encoded fault surface, with no left-literal grammar
/// because division does not commute: `1 /| x` is not `x`; the consumer's
/// implicit unit definitions retire under the same whole-function
/// deadness gate, and every operand past the `Def` result drops under
/// the mixed custody of provably-zero auxiliary `Use`s and
/// occurrence-free scratch `Def`s — or the literal `0` at the dividend
/// operand of a saturating divide on any carrier into a `MaterializeI64`
/// of the constant zero — `0 /| x` is `0` inside every carrier's bounds
/// — discharging the consumer's encoded fault surface under the
/// nonzero-divisor obligation the divide kind already carries rather
/// than under the folded literal, dropping the divisor `Use` and every
/// tail operand under the same mixed custody, and retiring the
/// consumer's implicit unit definitions under the same deadness gate;
/// the zero-dividend fold stays disjoint on the folded literal's operand
/// position from its divisor-one sibling — or the all-ones literal
/// `u64::MAX` — the normalized-i64 divisor `-1` — at the divisor operand
/// of a wrapping remainder into a `MaterializeI64` of the constant zero:
/// `x % -1` is `0` for every `x`, including the `i64::MIN` dividend the
/// kind's semantics defines to produce zero rather than trap, so the
/// folded divisor literal itself discharges the consumer's encoded fault
/// surface under the same `FaultDischargedByLiteral` relationship the
/// divisor-one fold declares, dropping the dividend `Use` and dead
/// scratch `Def` operands; the minus-one fold stays disjoint on the
/// folded literal's value from its divisor-one sibling at the same
/// operand position — or the carrier-maximum literal at the subtrahend
/// operand of a saturating subtract on an unsigned carrier into a
/// `MaterializeI64` of the constant zero: `x -| MAX` is `0` for every
/// `x` an unsigned carrier admits, because `x - MAX` underflows the
/// carrier's lower bound and saturates to it for every `x < MAX` and is
/// exactly zero at `x == MAX`, so the fold drops the operand-0 minuend
/// `Use` the constant result never reads and retires the consumer's
/// implicit unit definitions under the same deadness gate; the
/// upper-bound subtrahend fold stays disjoint on the folded literal's
/// value from its right-zero sibling at the same operand position and
/// position-disjoint from the zero-minuend sibling at operand 0, and
/// admits no signed carrier, where `x -| MAX` is `x - MAX` clamped to
/// the carrier's lower bound for every negative `x`, not a constant.
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
