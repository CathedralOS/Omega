//! `value as T in Wrapping` compositions: the modular image `value mod 2^B`
//! read on T's carrier, where B is T's width in bits.
//!
//! Every pair spells that image from operations Terminal Psi already admits,
//! and every exact cast's operand is bound by the mask or remainder that
//! produced it, so the composition carries each cast's range evidence in its
//! own shape rather than asking the bound machinery for an interval through a
//! fold it does not model.
//!
//! - Identical carriers return the operand, and a widening carrier already
//!   holds every source value, so both compose without a modular operator.
//! - An unsigned destination takes the modular image directly: `operand mod
//!   2^B` as an exact remainder for an unsigned source, or `operand & (2^B -
//!   1)` inside a signed source's carrier when B fits it. A signed source
//!   reaching an equal or wider unsigned destination borrows the smallest
//!   signed carrier wider than B for the mask; B = 64 has none, so the
//!   residue's low 63 bits and its sign bit cross separately and reassemble
//!   on `u64`.
//! - A signed destination receives the image assembled from two halves,
//!   `low | (bit <<% (B - 1))`: the residue's low `B - 1` bits and its
//!   sign-position bit are masked separately, landed on `i_B` by exact casts
//!   whose operands each carry a `[0, 2^(B-1) - 1]` or `[0, 1]` bound from
//!   the mask that made them, then reassembled. This is the fold a signed
//!   carrier needs — `(masked ^ 2^(B-1)) - 2^(B-1)` — spelled so each piece's
//!   range is the bound its cast requires: `low | (bit <<% (B-1))` is the
//!   same value, and an `i_C -> i_B` cast of the folded whole has no native
//!   carrier, while each half's `u_C -> i_B` cast does.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

/// Lower `operand as target in Wrapping` for `operand : source_type`.
pub(crate) fn wrapping_cast(
    operand: LoweredDirectExpression,
    source_type: IntegerType,
    target: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let ScalarType::Integer(target_type) = target else {
        return unsupported("wrapping conversion requires fixed integer carriers");
    };
    if source_type == target_type {
        return Ok(operand);
    }
    // A widening carrier already contains every source value, so the modular
    // image is the widened operand itself; this holds for signed sources and
    // signed targets without a signed modular operator.
    if source_type.can_widen_to(target_type) {
        return Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: target,
            operand: Box::new(operand),
        });
    }
    match (source_type.sign(), target_type.sign()) {
        (IntegerSign::Unsigned, IntegerSign::Unsigned) => {
            unsigned_narrowing(operand, source_type, target, target_type)
        }
        (IntegerSign::Signed, IntegerSign::Unsigned) => {
            signed_source_unsigned_target(operand, source_type, target, target_type)
        }
        (IntegerSign::Unsigned, IntegerSign::Signed) => {
            // The half masks only read bits below B, so the halves extract
            // straight from the source carrier even when it is wider than B.
            assemble_halves(operand, source_type, target, target_type)
        }
        (IntegerSign::Signed, IntegerSign::Signed) => {
            // Land the residue on the unsigned `u_B` carrier first — the same
            // mask the unsigned destination uses — because an `i_C -> i_B`
            // exact cast has no native carrier, and `u_B -> i_B` casts are
            // what each half's bound discharges through.
            let parts_carrier = unsigned_carrier(target_type.bits())?;
            let residue =
                unsigned_residue(operand, source_type, target_type.bits(), parts_carrier)?;
            assemble_halves(residue, parts_carrier, target, target_type)
        }
    }
}

/// `operand mod 2^B` for unsigned carriers: an exact remainder supplies the
/// destination's bound. Narrowing is guaranteed — same-width and widening
/// pairs return before this point.
fn unsigned_narrowing(
    operand: LoweredDirectExpression,
    source_type: IntegerType,
    target: ScalarType,
    target_type: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let modulus =
        1_u128
            .checked_shl(u32::from(target_type.bits()))
            .ok_or(LoweringError::Unsupported(
                "wrapping conversion modulus exceeds u128",
            ))?;
    Ok(exact_cast(
        binary(
            LoweredIntegerBinaryKind::ExactRemainder,
            source_type,
            operand,
            integer_literal(source_type, modulus)?,
        ),
        target,
    ))
}

/// `operand mod 2^B` read on an unsigned destination for a signed source.
fn signed_source_unsigned_target(
    operand: LoweredDirectExpression,
    source_type: IntegerType,
    target: ScalarType,
    target_type: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    if target_type.bits() < source_type.bits() {
        // The mask `2^B - 1` fits inside the source carrier, and `operand &
        // mask` is the modular image itself, already inside the destination.
        // This is the case truncation toward zero gets wrong — it would
        // carry a negative operand's sign instead of its image.
        return unsigned_residue(operand, source_type, target_type.bits(), target_type);
    }
    if target_type.bits() < 64 {
        // An equal or wider destination has no mask inside the source
        // carrier, so the residue is computed in the smallest signed carrier
        // strictly wider than B — the widened operand's low B bits are the
        // modular image.
        let carrier = signed_carrier_above(target_type.bits())?;
        let widened = widen(operand, carrier);
        return unsigned_residue(widened, carrier, target_type.bits(), target_type);
    }
    // `u64` is the one unsigned destination no wider signed carrier can mask
    // for: the residue's low 63 bits and its sign bit cross separately and
    // reassemble on the destination.
    let carrier = signed_carrier(64)?;
    let residue = if source_type.bits() < 64 {
        widen(operand, carrier)
    } else {
        operand
    };
    assemble_halves(residue, carrier, target, target_type)
}

/// `operand mod 2^B` held on the unsigned `u_B` carrier: mask inside the
/// source carrier, then an exact cast whose operand's `[0, 2^B - 1]` bound
/// is the mask's image.
fn unsigned_residue(
    operand: LoweredDirectExpression,
    source_type: IntegerType,
    target_bits: u16,
    target_type: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let mask = mask_value(target_bits)?;
    Ok(exact_cast(
        and_mask(operand, source_type, mask)?,
        ScalarType::Integer(target_type),
    ))
}

/// `residue`'s low `B - 1` bits OR'd with its sign-position bit shifted into
/// place, each half exact-cast onto `target_type` first. The halves' mask
/// bounds — `[0, 2^(B-1) - 1]` and `[0, 1]` — are exactly what the casts
/// need, so every cast's operand carries its own range evidence.
fn assemble_halves(
    residue: LoweredDirectExpression,
    carrier: IntegerType,
    target: ScalarType,
    target_type: IntegerType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let sign_position = u32::from(target_type.bits()) - 1;
    let low = and_mask(residue.clone(), carrier, (1_u128 << sign_position) - 1)?;
    let bit = and_mask(shift_right(residue, carrier, sign_position)?, carrier, 1)?;
    let low = exact_cast(low, target);
    let bit = exact_cast(bit, target);
    let shifted = shift_left(bit, target_type, sign_position)?;
    Ok(binary(
        LoweredIntegerBinaryKind::BitwiseOr,
        target_type,
        low,
        shifted,
    ))
}

fn unsigned_carrier(bits: u16) -> Result<IntegerType, LoweringError> {
    IntegerType::new(IntegerSign::Unsigned, bits)
        .map_err(|_| LoweringError::Unsupported("wrapping conversion requires a fixed carrier"))
}

fn signed_carrier(bits: u16) -> Result<IntegerType, LoweringError> {
    IntegerType::new(IntegerSign::Signed, bits)
        .map_err(|_| LoweringError::Unsupported("wrapping conversion requires a fixed carrier"))
}

/// The smallest signed carrier strictly wider than `bits`, borrowed for the
/// residue mask when a signed source reaches an equal or wider unsigned
/// destination below 64 bits.
fn signed_carrier_above(bits: u16) -> Result<IntegerType, LoweringError> {
    let carrier_bits = match bits {
        0..=8 => 16,
        9..=16 => 32,
        17..=32 => 64,
        _ => {
            return unsupported("wrapping conversion has no wider signed carrier");
        }
    };
    signed_carrier(carrier_bits)
}

/// `2^bits - 1` — the residue mask — or `unsupported` if the width exceeds
/// what a literal can hold.
fn mask_value(bits: u16) -> Result<u128, LoweringError> {
    1_u128
        .checked_shl(u32::from(bits))
        .and_then(|modulus| modulus.checked_sub(1))
        .ok_or(LoweringError::Unsupported(
            "wrapping conversion mask exceeds u128",
        ))
}

fn and_mask(
    operand: LoweredDirectExpression,
    carrier: IntegerType,
    mask: u128,
) -> Result<LoweredDirectExpression, LoweringError> {
    Ok(binary(
        LoweredIntegerBinaryKind::BitwiseAnd,
        carrier,
        operand,
        integer_literal(carrier, mask)?,
    ))
}

fn shift_right(
    operand: LoweredDirectExpression,
    carrier: IntegerType,
    count: u32,
) -> Result<LoweredDirectExpression, LoweringError> {
    Ok(binary(
        LoweredIntegerBinaryKind::WrappingShiftRight,
        carrier,
        operand,
        integer_literal(carrier, u128::from(count))?,
    ))
}

fn shift_left(
    operand: LoweredDirectExpression,
    carrier: IntegerType,
    count: u32,
) -> Result<LoweredDirectExpression, LoweringError> {
    Ok(binary(
        LoweredIntegerBinaryKind::WrappingShiftLeft,
        carrier,
        operand,
        integer_literal(carrier, u128::from(count))?,
    ))
}

fn binary(
    kind: LoweredIntegerBinaryKind,
    carrier: IntegerType,
    left: LoweredDirectExpression,
    right: LoweredDirectExpression,
) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerBinary {
        kind,
        scalar_type: ScalarType::Integer(carrier),
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn exact_cast(operand: LoweredDirectExpression, target: ScalarType) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerExactCast {
        scalar_type: target,
        operand: Box::new(operand),
    }
}

fn widen(operand: LoweredDirectExpression, carrier: IntegerType) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerWiden {
        scalar_type: ScalarType::Integer(carrier),
        operand: Box::new(operand),
    }
}

/// A literal admitted by `carrier`; the masks and counts this module emits
/// always fit by construction, and `admits` is the check that keeps that
/// invariant explicit.
fn integer_literal(
    carrier: IntegerType,
    value: u128,
) -> Result<LoweredDirectExpression, LoweringError> {
    let value = match carrier.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::try_from(value).map_err(|_| {
            LoweringError::Unsupported("wrapping conversion literal exceeds the carrier")
        })?),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    };
    if !carrier.admits(value) {
        return unsupported("wrapping conversion literal exceeds the carrier");
    }
    Ok(LoweredDirectExpression::IntegerLiteral {
        value,
        scalar_type: ScalarType::Integer(carrier),
    })
}
