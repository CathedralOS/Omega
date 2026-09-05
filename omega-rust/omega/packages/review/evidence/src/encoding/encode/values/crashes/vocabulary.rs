//! Stable numeric tags and spellings for structural arithmetic.

use super::*;

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: PackageReviewPrimitiveType,
) {
    encoder.tag(
        primitive_type_name(primitive_type),
        match primitive_type {
            PackageReviewPrimitiveType::Bool => 0,
            PackageReviewPrimitiveType::F32 => 1,
            PackageReviewPrimitiveType::F64 => 2,
            PackageReviewPrimitiveType::I8 => 3,
            PackageReviewPrimitiveType::I16 => 4,
            PackageReviewPrimitiveType::I32 => 5,
            PackageReviewPrimitiveType::I64 => 6,
            PackageReviewPrimitiveType::U8 => 7,
            PackageReviewPrimitiveType::U16 => 8,
            PackageReviewPrimitiveType::U32 => 9,
            PackageReviewPrimitiveType::U64 => 10,
            PackageReviewPrimitiveType::Addr => 11,
        },
    );
}

pub(crate) const fn integer_comparison_tag(kind: PackageReviewIntegerComparisonKind) -> u8 {
    match kind {
        PackageReviewIntegerComparisonKind::Equal => 0,
        PackageReviewIntegerComparisonKind::LessThan => 1,
        PackageReviewIntegerComparisonKind::LessOrEqual => 2,
    }
}

pub(crate) const fn integer_binary_tag(kind: PackageReviewIntegerBinaryKind) -> u8 {
    match kind {
        PackageReviewIntegerBinaryKind::ExactAdd => 0,
        PackageReviewIntegerBinaryKind::ExactSubtract => 1,
        PackageReviewIntegerBinaryKind::ExactMultiply => 2,
        PackageReviewIntegerBinaryKind::ExactDivide => 3,
        PackageReviewIntegerBinaryKind::ExactRemainder => 4,
        PackageReviewIntegerBinaryKind::WrappingDivide => 5,
        PackageReviewIntegerBinaryKind::WrappingRemainder => 6,
        PackageReviewIntegerBinaryKind::SaturatingDivide => 7,
        PackageReviewIntegerBinaryKind::SaturatingRemainder => 8,
        PackageReviewIntegerBinaryKind::WrappingAdd => 9,
        PackageReviewIntegerBinaryKind::SaturatingAdd => 10,
        PackageReviewIntegerBinaryKind::WrappingSubtract => 11,
        PackageReviewIntegerBinaryKind::SaturatingSubtract => 12,
        PackageReviewIntegerBinaryKind::WrappingMultiply => 13,
        PackageReviewIntegerBinaryKind::SaturatingMultiply => 14,
        PackageReviewIntegerBinaryKind::BitwiseAnd => 15,
        PackageReviewIntegerBinaryKind::BitwiseOr => 16,
        PackageReviewIntegerBinaryKind::BitwiseXor => 17,
        PackageReviewIntegerBinaryKind::WrappingShiftLeft => 18,
        PackageReviewIntegerBinaryKind::WrappingShiftRight => 19,
        PackageReviewIntegerBinaryKind::ExactShiftLeft => 20,
        PackageReviewIntegerBinaryKind::ExactShiftRight => 21,
    }
}

pub(super) const fn primitive_type_name(
    primitive_type: PackageReviewPrimitiveType,
) -> &'static str {
    match primitive_type {
        PackageReviewPrimitiveType::Bool => "bool",
        PackageReviewPrimitiveType::F32 => "f32",
        PackageReviewPrimitiveType::F64 => "f64",
        PackageReviewPrimitiveType::I8 => "i8",
        PackageReviewPrimitiveType::I16 => "i16",
        PackageReviewPrimitiveType::I32 => "i32",
        PackageReviewPrimitiveType::I64 => "i64",
        PackageReviewPrimitiveType::U8 => "u8",
        PackageReviewPrimitiveType::U16 => "u16",
        PackageReviewPrimitiveType::U32 => "u32",
        PackageReviewPrimitiveType::U64 => "u64",
        PackageReviewPrimitiveType::Addr => "addr",
    }
}

pub(super) const fn arithmetic_domain_name(domain: PackageReviewArithmeticDomain) -> &'static str {
    match domain {
        PackageReviewArithmeticDomain::Exact => "Exact",
        PackageReviewArithmeticDomain::Wrapping => "Wrapping",
        PackageReviewArithmeticDomain::Saturating => "Saturating",
        PackageReviewArithmeticDomain::Trapping => "Trapping",
    }
}
