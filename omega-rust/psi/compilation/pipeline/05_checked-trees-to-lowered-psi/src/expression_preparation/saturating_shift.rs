//! Saturating left shift composes from positive, representable scale factors.
//!
//! Checking must prove `0 <= count < width`; saturation never licenses an
//! invalid count. For a valid count, split it into an even part and its low
//! bit. Both `2^even` and `2^low` fit even a signed carrier: the largest even
//! count is `width - 2`. Multiplication by positive powers of two preserves
//! sign, so clamping after each product equals clamping the full product.
//! This avoids forming the unrepresentable signed factor `2^(width - 1)`.
//! The generated shifts are total wrapping operations over those bounded
//! counts, not a weakening of the authored count obligation. Emission binds
//! both operands once before expanding this recipe and retains an exact
//! right shift as the independently checked count-range obligation.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::expression_preparation::wrapping_cast::binary;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};

pub(super) fn left(
    scalar_type: ScalarType,
    operand: LoweredDirectExpression,
    count: LoweredDirectExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    let ScalarType::Integer(carrier) = scalar_type else {
        return unsupported("saturating shift requires a fixed integer carrier");
    };
    if carrier.is_address() || !matches!(carrier.bits(), 8 | 16 | 32 | 64) {
        return unsupported("saturating shift requires a supported fixed integer width");
    }
    let ScalarType::Integer(count_carrier) = count.scalar_type() else {
        return unsupported("saturating shift requires an integer count");
    };
    if count_carrier.is_address() || count_carrier.bits() < 8 {
        return unsupported("saturating shift requires a fixed integer count carrier");
    }
    Ok(LoweredDirectExpression::SaturatingShiftLeft {
        scalar_type,
        left: Box::new(operand),
        right: Box::new(count),
    })
}

/// The operands here are references to evaluated values, or mathematical
/// terms. Runtime callers must bind them before expanding the repeated count.
pub(crate) fn expansion(
    scalar_type: ScalarType,
    operand: LoweredDirectExpression,
    count: LoweredDirectExpression,
) -> LoweredDirectExpression {
    let ScalarType::Integer(carrier) = scalar_type else {
        unreachable!("prepared saturating shift has an integer carrier");
    };
    let ScalarType::Integer(count_carrier) = count.scalar_type() else {
        unreachable!("prepared saturating shift has an integer count");
    };
    let even = binary(
        LoweredIntegerBinaryKind::BitwiseAnd,
        count_carrier,
        count.clone(),
        literal(count_carrier, carrier.bits() - 2),
    );
    let low = binary(
        LoweredIntegerBinaryKind::BitwiseAnd,
        count_carrier,
        count,
        literal(count_carrier, 1),
    );
    let mut product = operand;
    for exponent in [even, low] {
        let factor = binary(
            LoweredIntegerBinaryKind::WrappingShiftLeft,
            carrier,
            literal(carrier, 1),
            exponent,
        );
        product = binary(
            LoweredIntegerBinaryKind::SaturatingMultiply,
            carrier,
            product,
            factor,
        );
    }
    product
}

fn literal(carrier: IntegerType, value: u16) -> LoweredDirectExpression {
    LoweredDirectExpression::IntegerLiteral {
        value: match carrier.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(value)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value)),
        },
        scalar_type: ScalarType::Integer(carrier),
    }
}
