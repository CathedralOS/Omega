//! Scalar numerics the evaluator's operations share: integer bounds, widths
//! and wrapping, the semantic integer format of a primitive, big-integer
//! runtime values, float bit projections and landed-float projection.

use super::{EvalResult, trap};
use numerics::arithmetic::ArithmeticDomain;
use numerics::bignum::BigInt;
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics::{
    FloatFormat as SemanticFloatFormat, FloatMeaning, FloatToIntegerError,
    IntegerFormat as SemanticIntegerFormat,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType;

pub(super) fn project_landed_float(format: SemanticFloatFormat, value: f64) -> FloatMeaning {
    if format == SemanticFloatFormat::BINARY32 {
        FloatProjectionOperation::Meaning32
            .project_f32(value as f32)
            .expect("binary32 projection row accepts f32")
    } else {
        FloatProjectionOperation::Meaning64
            .project_f64(value)
            .expect("binary64 projection row accepts f64")
    }
}

pub(super) fn integer_bounds(ty: PrimitiveType) -> Option<(i64, i64)> {
    match ty {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        _ => None,
    }
}

pub(super) fn semantic_integer_format(ty: PrimitiveType) -> Option<SemanticIntegerFormat> {
    match ty {
        PrimitiveType::I8 => Some(SemanticIntegerFormat::I8),
        PrimitiveType::I16 => Some(SemanticIntegerFormat::I16),
        PrimitiveType::I32 => Some(SemanticIntegerFormat::I32),
        PrimitiveType::I64 => Some(SemanticIntegerFormat::I64),
        PrimitiveType::U8 => Some(SemanticIntegerFormat::U8),
        PrimitiveType::U16 => Some(SemanticIntegerFormat::U16),
        PrimitiveType::U32 => Some(SemanticIntegerFormat::U32),
        PrimitiveType::U64 | PrimitiveType::Addr => Some(SemanticIntegerFormat::U64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

pub(super) fn is_unsigned_integer_primitive(ty: PrimitiveType) -> bool {
    matches!(
        ty,
        PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr
    )
}

pub(super) fn big_integer_runtime_value(value: &BigInt, ty: PrimitiveType) -> i64 {
    if is_unsigned_integer_primitive(ty) {
        value
            .to_u64()
            .expect("checked unsigned conversion fits its target") as i64
    } else {
        value
            .to_i64()
            .expect("checked signed conversion fits its target")
    }
}

pub(super) fn float_to_integer_trap_message(
    target: PrimitiveType,
    reason: FloatToIntegerError,
    trapping: bool,
) -> String {
    let operation = if trapping { "Trapping" } else { "Exact" };
    let reason = match reason {
        FloatToIntegerError::NonFinite => "the value is not finite",
        FloatToIntegerError::OutOfRange => "the truncated value is out of range",
    };
    format!("float-to-int conversion failed in {operation} domain: {reason} for {target:?}")
}

/// Preserve an f32 NaN's sign and payload while carrying interpreter floats in
/// the existing f64-backed `Value::Float`. Finite values and infinities remain
/// ordinary numeric f64 values; only NaNs use this reversible payload embedding.
pub(super) fn interpreter_f32_from_bits(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return value as f64;
    }

    let sign = ((bits as u64) >> 31) << 63;
    let payload = (u64::from(bits) & 0x007f_ffff) << 29;
    f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
}

pub(super) fn interpreter_f32_to_bits(value: f64) -> u32 {
    if !value.is_nan() {
        return (value as f32).to_bits();
    }

    let bits = value.to_bits();
    let sign = ((bits >> 63) as u32) << 31;
    let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
    payload &= 0x007f_ffff;
    if payload == 0 {
        payload = 0x0040_0000;
    }
    sign | 0x7f80_0000 | payload
}

/// Apply a write target's arithmetic domain (decision 17) to a raw i64 result,
/// mirroring the native backend so the differential oracle agrees:
/// Exact/Wrapping truncate to width; Saturating clamps to [min, max]; Trapping
/// halts (overflow trap) when the value is out of range.
pub(super) fn apply_arithmetic_domain(
    raw: i64,
    ty: PrimitiveType,
    domain: ArithmeticDomain,
) -> EvalResult<i64> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => Ok(wrap_to_width(raw, ty)),
        ArithmeticDomain::Saturating => match integer_bounds(ty) {
            Some((min, max)) => Ok(raw.clamp(min, max)),
            None => Ok(wrap_to_width(raw, ty)),
        },
        ArithmeticDomain::Trapping => match integer_bounds(ty) {
            Some((min, max)) if raw < min || raw > max => trap(format!(
                "arithmetic overflow in Trapping domain: {raw} is out of range for {ty:?}"
            )),
            _ => Ok(wrap_to_width(raw, ty)),
        },
    }
}

/// The bit width a WRAPPING shift wraps at (the modular-arithmetic modulus
/// exponent). Pointer-width types are 64-bit in both engines.
pub(super) fn primitive_bit_width(ty: PrimitiveType) -> u64 {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => 8,
        PrimitiveType::I16 | PrimitiveType::U16 => 16,
        PrimitiveType::I32 | PrimitiveType::U32 => 32,
        _ => 64,
    }
}

pub(super) fn wrap_to_width(raw: i64, ty: PrimitiveType) -> i64 {
    match ty {
        PrimitiveType::I8 => raw as i8 as i64,
        PrimitiveType::U8 => raw as u8 as i64,
        PrimitiveType::I16 => raw as i16 as i64,
        PrimitiveType::U16 => raw as u16 as i64,
        PrimitiveType::I32 => raw as i32 as i64,
        PrimitiveType::U32 => raw as u32 as i64,
        // 64-bit and pointer-width types keep the full value (unsigned reinterpretation of a
        // u64 is still represented by the same bit pattern in i64).
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => raw,
        // Non-integer primitives do not reach this path.
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => raw,
    }
}

/// Byte width of an integer primitive -- the PROMOTION rank a mixed-width
/// binary node computes in. `None` for non-integer primitives.
pub(super) fn integer_primitive_byte_width(ty: PrimitiveType) -> Option<usize> {
    match ty {
        PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
        PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Addr => Some(8),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

pub(super) fn primitive_is_unsigned64(primitive: Option<PrimitiveType>) -> bool {
    matches!(primitive, Some(PrimitiveType::U64 | PrimitiveType::Addr))
}
