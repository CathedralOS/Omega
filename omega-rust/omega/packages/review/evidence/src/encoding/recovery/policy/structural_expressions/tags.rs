//! Inverses of the shared structural-expression closed vocabulary.

use super::*;

pub(super) fn primitive_type(reader: &mut Reader<'_>) -> Result<PackageReviewPrimitiveType, Error> {
    use PackageReviewPrimitiveType as Primitive;
    Ok(match reader.byte()? {
        0 => Primitive::Bool,
        1 => Primitive::F32,
        2 => Primitive::F64,
        3 => Primitive::I8,
        4 => Primitive::I16,
        5 => Primitive::I32,
        6 => Primitive::I64,
        7 => Primitive::U8,
        8 => Primitive::U16,
        9 => Primitive::U32,
        10 => Primitive::U64,
        11 => Primitive::Addr,
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn primitive_type_name(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPrimitiveType, Error> {
    use PackageReviewPrimitiveType as Primitive;
    Ok(match reader.string()?.as_str() {
        "bool" => Primitive::Bool,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        "i8" => Primitive::I8,
        "i16" => Primitive::I16,
        "i32" => Primitive::I32,
        "i64" => Primitive::I64,
        "u8" => Primitive::U8,
        "u16" => Primitive::U16,
        "u32" => Primitive::U32,
        "u64" => Primitive::U64,
        "addr" => Primitive::Addr,
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn integer_binary(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewIntegerBinaryKind, Error> {
    use PackageReviewIntegerBinaryKind as Kind;
    Ok(match reader.byte()? {
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
        _ => return Err(Error::InvalidTag),
    })
}
