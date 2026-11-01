//! Scalar types and integer values for primitives.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType};

pub(crate) fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    if primitive == PrimitiveType::Addr {
        return Ok(ScalarType::Integer(
            IntegerType::supported_address_representation(),
        ));
    }
    let (sign, bits) = match primitive {
        // A selector is one opaque word naming an implementation table; no
        // Terminal operation other than a dynamic call inspects it.
        PrimitiveType::Selector => (IntegerSign::Unsigned, 64),
        primitive => match primitive.integer_sign_bits() {
            Some(pair) => pair,
            None => return unsupported("only primitive integers are supported"),
        },
    };
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| LoweringError::InvalidPsiIntegerType)
}

/// The Terminal carrier of a `dyn Trait<Args>` selector.
pub(crate) fn selector_scalar_type() -> ScalarType {
    ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type"),
    )
}

pub(crate) fn terminal_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ScalarType::Boolean),
        PrimitiveType::F32 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        PrimitiveType::F64 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        primitive => integer_scalar_type(primitive),
    }
}

pub(crate) fn integer_landing_scalar_type(
    literal: &numerics::literals::IntegerLiteral,
) -> Result<ScalarType, LoweringError> {
    let primitive = PrimitiveType::from_landed_integer(
        literal
            .landing()
            .ok_or(LoweringError::UnlandedIntegerLiteral)?
            .landed_type,
    );
    integer_scalar_type(primitive)
}

pub(crate) fn integer_value(
    literal: &numerics::literals::IntegerLiteral,
    scalar_type: ScalarType,
) -> Result<IntegerValue, LoweringError> {
    let Some(integer_type) = scalar_type.as_integer() else {
        return Err(LoweringError::InvalidPsiIntegerType);
    };
    let landing = literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?;
    if landing.landed_type.bit_width() != u32::from(integer_type.bits())
        || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
    {
        return Err(LoweringError::IntegerLandingMismatch);
    }
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(
            literal
                .value_i64()
                .map(i128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            literal
                .value_u64()
                .map(u128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
    };
    if !integer_type.admits(value) {
        return Err(LoweringError::IntegerLiteralOutsidePsiType);
    }
    Ok(value)
}
