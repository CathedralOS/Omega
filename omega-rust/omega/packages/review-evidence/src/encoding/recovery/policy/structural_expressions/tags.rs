// Inverses of the shared structural-expression closed vocabulary for primitive types and integer binary operators.

use crate::encoding::recovery::policy::structural_expressions::Reader;
use crate::record::PackageReviewIntegerBinaryKind;
use crate::record::PackageReviewPrimitiveType;

use super::Error;
use numerics::integer_carrier::FixedInteger;
pub(in crate::encoding::recovery) fn primitive_type(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPrimitiveType, Error> {
    use PackageReviewPrimitiveType as Primitive;
    Ok(super::super::tag!(reader => {
        0 => Primitive::Bool,
        1 => Primitive::F32,
        2 => Primitive::F64,
        3 => Primitive::Integer(FixedInteger::signed(8)),
        4 => Primitive::Integer(FixedInteger::signed(16)),
        5 => Primitive::Integer(FixedInteger::signed(32)),
        6 => Primitive::Integer(FixedInteger::signed(64)),
        7 => Primitive::Integer(FixedInteger::unsigned(8)),
        8 => Primitive::Integer(FixedInteger::unsigned(16)),
        9 => Primitive::Integer(FixedInteger::unsigned(32)),
        10 => Primitive::Integer(FixedInteger::unsigned(64)),
        11 => Primitive::Addr,
    }))
}

pub(in crate::encoding::recovery) fn primitive_type_name(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPrimitiveType, Error> {
    use PackageReviewPrimitiveType as Primitive;
    Ok(match reader.string()?.as_str() {
        "bool" => Primitive::Bool,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        "addr" => Primitive::Addr,
        /*
            The published spellings are the eight carriers the encoder's
            `primitive_type_row` writes; any other width is not a record this
            format ever held.
        */
        name => FixedInteger::from_decimal_spelling(name)
            .filter(|carrier| matches!(carrier.width(), 8 | 16 | 32 | 64))
            .map(Primitive::Integer)
            .ok_or(Error::InvalidTag)?,
    })
}

pub(in crate::encoding::recovery) fn integer_binary(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewIntegerBinaryKind, Error> {
    use PackageReviewIntegerBinaryKind as Kind;
    Ok(super::super::tag!(reader => {
        0 => Kind::ExactAdd,
        1 => Kind::ExactSubtract,
        2 => Kind::ExactMultiply,
        3 => Kind::ExactDivide,
        4 => Kind::ExactRemainder,
        5 => Kind::WrappingDivide,
        6 => Kind::WrappingRemainder,
        7 => Kind::SaturatingDivide,
        8 => Kind::SaturatingRemainder,
        9 => Kind::WrappingAdd,
        10 => Kind::SaturatingAdd,
        11 => Kind::WrappingSubtract,
        12 => Kind::SaturatingSubtract,
        13 => Kind::WrappingMultiply,
        14 => Kind::SaturatingMultiply,
        15 => Kind::BitwiseAnd,
        16 => Kind::BitwiseOr,
        17 => Kind::BitwiseXor,
        18 => Kind::WrappingShiftLeft,
        19 => Kind::WrappingShiftRight,
        20 => Kind::ExactShiftLeft,
        21 => Kind::ExactShiftRight,
        22 => Kind::TrappingShiftLeft,
        23 => Kind::TrappingShiftRight,
        24 => Kind::TrappingAdd,
        25 => Kind::TrappingSubtract,
        26 => Kind::TrappingMultiply,
        27 => Kind::TrappingDivide,
        28 => Kind::TrappingRemainder,
        29 => Kind::SaturatingShiftLeft,
        30 => Kind::SaturatingShiftRight,
    }))
}
