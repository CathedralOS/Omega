//! `value as T in Saturating` compositions: `clamp(value, low, high)` spelled
//! on the SOURCE carrier with total operations, then the wrapping conversion
//! carries the clamped value across — an operand already inside the
//! destination range is its own modular image, so every exact cast in the
//! crossing keeps the bound evidence of the mask that produced it.
//!
//! The clamps are branch-free on both signs. An unsigned carrier floors
//! `value -% (value sat_sub high)` at zero, which is `min(value, high)`. A
//! signed carrier reads `max(0, x)` off the arithmetic sign mask spelled
//! `x & !(x >>% (C - 1))`: `min(value, high)` is `value -% max(0, value
//! sat_sub high)` — a signed saturating subtraction floors at the carrier
//! minimum, not at zero, so the nonnegative part is taken off the distance
//! rather than assumed of the difference — and `max(value, low)` is
//! `value +% max(0, low -% value)`, where `low -% value` is exact on the
//! post-clamp domain `value <= high` because its endpoints `low - high`
//! and `low - i_C.min` both stay inside `i_C` when `i_B` narrows it.
//!
//! - An unsigned destination from a signed source always binds at zero; the
//!   ceiling `2^B - 1` only binds when `u_B` is narrower than the source's
//!   positive half (`B + 1 < C`).
//! - A signed destination binds on both sides; the clamped value then
//!   crosses through `u_B` and the halves, since an `i_C -> i_B` exact cast
//!   has no native carrier — the same route the wrapping composition takes.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::expression_preparation::wrapping_cast::{self, binary, mask_value, shift_right, widen};
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

/// Lower `operand as target in Saturating` for `operand : source_type`.
pub(crate) fn saturating_cast(
    operand: LoweredDirectExpression,
    source_type: IntegerType,
    target: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let ScalarType::Integer(target_type) = target else {
        return unsupported("saturating conversion requires fixed integer carriers");
    };
    if source_type == target_type {
        return Ok(operand);
    }
    if source_type.is_address() || target_type.is_address() {
        return unsupported("saturating conversion requires fixed integer carriers");
    }
    // A widening carrier already contains every source value, so the
    // widened operand is the saturating image.
    if source_type.can_widen_to(target_type) {
        return Ok(widen(operand, target_type));
    }
    let clamped = match (source_type.sign(), target_type.sign()) {
        (IntegerSign::Unsigned, _) => {
            // Only narrowing and same-width sign changes reach here, so the
            // destination maximum fits the unsigned source carrier.
            let high = target_maximum(target_type)?;
            unsigned_min(operand, source_type, high)?
        }
        (IntegerSign::Signed, IntegerSign::Unsigned) => {
            // The floor at zero always binds; the ceiling `2^B - 1` only
            // when `u_B` is narrower than the source's positive half — that
            // is `2^B - 1 < i_C.max`, equivalently `B + 1 < C`.
            let clamped = if target_type.bits() + 1 < source_type.bits() {
                signed_min(operand, source_type, mask_value(target_type.bits())?)?
            } else {
                operand
            };
            signed_floor_at_zero(clamped, source_type)?
        }
        (IntegerSign::Signed, IntegerSign::Signed) => {
            // Narrowing is guaranteed — a wider pair widens and an identical
            // pair returned above — so `i_B`'s bounds fit inside `i_C`.
            let high = mask_value(target_type.bits() - 1)?;
            let low = -(1_i128 << (target_type.bits() - 1));
            let below_ceiling = signed_min(operand, source_type, high)?;
            signed_raised_floor(below_ceiling, source_type, low)?
        }
    };
    // The clamped value lies inside the destination range, so its residue is
    // the value itself; the wrapping composition's mask and half crossings
    // carry each exact cast's bound unchanged.
    wrapping_cast::wrapping_cast(clamped, source_type, target)
}

/// `min(value, high)` on an unsigned carrier: saturating subtraction floors
/// at zero, so `value -% (value sat_sub high)` is the clamp without a branch.
fn unsigned_min(
    value: LoweredDirectExpression,
    carrier: IntegerType,
    high: u128,
) -> Result<LoweredDirectExpression, LoweringError> {
    let distance = binary(
        LoweredIntegerBinaryKind::SaturatingSubtract,
        carrier,
        value.clone(),
        literal(carrier, u128_to_i128(high)?)?,
    );
    Ok(binary(
        LoweredIntegerBinaryKind::WrappingSubtract,
        carrier,
        value,
        distance,
    ))
}

/// `min(value, high)` on a signed carrier: `value sat_sub high` clamps at
/// `i_C.min` rather than wrapping, and its nonnegative part is exactly what
/// `value` exceeds `high` by, so `value -% max(0, value sat_sub high)` is the
/// clamp.
fn signed_min(
    value: LoweredDirectExpression,
    carrier: IntegerType,
    high: u128,
) -> Result<LoweredDirectExpression, LoweringError> {
    let distance = binary(
        LoweredIntegerBinaryKind::SaturatingSubtract,
        carrier,
        value.clone(),
        literal(carrier, u128_to_i128(high)?)?,
    );
    Ok(binary(
        LoweredIntegerBinaryKind::WrappingSubtract,
        carrier,
        value,
        signed_floor_at_zero(distance, carrier)?,
    ))
}

/// `max(value, low)` on a signed carrier, for `value <= high` already: `low
/// -% value` stays inside `i_C` on that domain — its endpoints `low - high`
/// and `low - i_C.min` are admitted whenever `i_B` narrows `i_C` — so a
/// wrapping subtract carries it and `value +% max(0, low -% value)` is the
/// floor.
fn signed_raised_floor(
    value: LoweredDirectExpression,
    carrier: IntegerType,
    low: i128,
) -> Result<LoweredDirectExpression, LoweringError> {
    let deficit = binary(
        LoweredIntegerBinaryKind::WrappingSubtract,
        carrier,
        literal(carrier, low)?,
        value.clone(),
    );
    Ok(binary(
        LoweredIntegerBinaryKind::WrappingAdd,
        carrier,
        value,
        signed_floor_at_zero(deficit, carrier)?,
    ))
}

/// `max(value, 0)` on a signed carrier: the arithmetic right shift spreads
/// the sign bit across the whole word, so `value & !(value >>% (C - 1))`
/// keeps a nonnegative operand and zeroes a negative one.
fn signed_floor_at_zero(
    value: LoweredDirectExpression,
    carrier: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let sign = shift_right(value.clone(), carrier, u32::from(carrier.bits()) - 1)?;
    let nonnegative = LoweredDirectExpression::IntegerBitwiseNot {
        scalar_type: ScalarType::Integer(carrier),
        operand: Box::new(sign),
    };
    Ok(binary(
        LoweredIntegerBinaryKind::BitwiseAnd,
        carrier,
        value,
        nonnegative,
    ))
}

/// The destination's representable maximum on an unsigned source carrier;
/// reached only on pairs where that bound fits the source.
fn target_maximum(target_type: IntegerType) -> Result<u128, LoweringError> {
    match target_type.sign() {
        IntegerSign::Unsigned => mask_value(target_type.bits()),
        IntegerSign::Signed => mask_value(target_type.bits() - 1),
    }
}

/// A literal admitted by `carrier`; the bounds this module emits always fit
/// by construction, and `admits` keeps that invariant explicit.
fn literal(carrier: IntegerType, value: i128) -> Result<LoweredDirectExpression, LoweringError> {
    let value = match carrier.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(value).map_err(|_| {
            LoweringError::Unsupported("saturating conversion literal exceeds the carrier")
        })?),
    };
    if !carrier.admits(value) {
        return unsupported("saturating conversion literal exceeds the carrier");
    }
    Ok(LoweredDirectExpression::IntegerLiteral {
        value,
        scalar_type: ScalarType::Integer(carrier),
    })
}

fn u128_to_i128(value: u128) -> Result<i128, LoweringError> {
    i128::try_from(value)
        .map_err(|_| LoweringError::Unsupported("saturating conversion bound exceeds the carrier"))
}
