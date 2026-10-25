//! Stable numeric tags and spellings for structural arithmetic.
use crate::encoding::encode::values::crashes::Encoder;
use crate::record::PackageReviewArithmeticDomain;
use crate::record::PackageReviewIntegerBinaryKind;
use crate::record::PackageReviewIntegerComparisonKind;
use crate::record::PackageReviewPrimitiveType;

/// One row per primitive type: the persisted tag and the authored spelling,
/// so the tag byte and the canonical text read the same roster.
pub(super) const fn primitive_type_row(
    primitive_type: PackageReviewPrimitiveType,
) -> (u8, &'static str) {
    match primitive_type {
        PackageReviewPrimitiveType::Bool => (0, "bool"),
        PackageReviewPrimitiveType::F32 => (1, "f32"),
        PackageReviewPrimitiveType::F64 => (2, "f64"),
        PackageReviewPrimitiveType::I8 => (3, "i8"),
        PackageReviewPrimitiveType::I16 => (4, "i16"),
        PackageReviewPrimitiveType::I32 => (5, "i32"),
        PackageReviewPrimitiveType::I64 => (6, "i64"),
        PackageReviewPrimitiveType::U8 => (7, "u8"),
        PackageReviewPrimitiveType::U16 => (8, "u16"),
        PackageReviewPrimitiveType::U32 => (9, "u32"),
        PackageReviewPrimitiveType::U64 => (10, "u64"),
        PackageReviewPrimitiveType::Addr => (11, "addr"),
        PackageReviewPrimitiveType::Selector => (12, "selector"),
    }
}

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: PackageReviewPrimitiveType,
) {
    let (tag, name) = primitive_type_row(primitive_type);
    encoder.tag(name, tag);
}

/// One row per structural integer comparison kind: the persisted tag and the
/// authored spelling, so both directions read the same roster.
pub(crate) const fn integer_comparison_row(
    kind: PackageReviewIntegerComparisonKind,
) -> (u8, &'static str) {
    match kind {
        PackageReviewIntegerComparisonKind::Equal => (0, "equal"),
        PackageReviewIntegerComparisonKind::LessThan => (1, "less_than"),
        PackageReviewIntegerComparisonKind::LessOrEqual => (2, "less_or_equal"),
    }
}

/// One row per structural integer binary kind: the persisted tag and the
/// authored spelling. Recovery reads its inverse by tag from the same roster.
pub(crate) const fn integer_binary_row(kind: PackageReviewIntegerBinaryKind) -> (u8, &'static str) {
    match kind {
        PackageReviewIntegerBinaryKind::ExactAdd => (0, "exact_add"),
        PackageReviewIntegerBinaryKind::ExactSubtract => (1, "exact_subtract"),
        PackageReviewIntegerBinaryKind::ExactMultiply => (2, "exact_multiply"),
        PackageReviewIntegerBinaryKind::ExactDivide => (3, "exact_divide"),
        PackageReviewIntegerBinaryKind::ExactRemainder => (4, "exact_remainder"),
        PackageReviewIntegerBinaryKind::WrappingDivide => (5, "wrapping_divide"),
        PackageReviewIntegerBinaryKind::WrappingRemainder => (6, "wrapping_remainder"),
        PackageReviewIntegerBinaryKind::SaturatingDivide => (7, "saturating_divide"),
        PackageReviewIntegerBinaryKind::SaturatingRemainder => (8, "saturating_remainder"),
        PackageReviewIntegerBinaryKind::WrappingAdd => (9, "wrapping_add"),
        PackageReviewIntegerBinaryKind::SaturatingAdd => (10, "saturating_add"),
        PackageReviewIntegerBinaryKind::WrappingSubtract => (11, "wrapping_subtract"),
        PackageReviewIntegerBinaryKind::SaturatingSubtract => (12, "saturating_subtract"),
        PackageReviewIntegerBinaryKind::WrappingMultiply => (13, "wrapping_multiply"),
        PackageReviewIntegerBinaryKind::SaturatingMultiply => (14, "saturating_multiply"),
        PackageReviewIntegerBinaryKind::BitwiseAnd => (15, "bitwise_and"),
        PackageReviewIntegerBinaryKind::BitwiseOr => (16, "bitwise_or"),
        PackageReviewIntegerBinaryKind::BitwiseXor => (17, "bitwise_xor"),
        PackageReviewIntegerBinaryKind::WrappingShiftLeft => (18, "wrapping_shift_left"),
        PackageReviewIntegerBinaryKind::WrappingShiftRight => (19, "wrapping_shift_right"),
        PackageReviewIntegerBinaryKind::ExactShiftLeft => (20, "exact_shift_left"),
        PackageReviewIntegerBinaryKind::ExactShiftRight => (21, "exact_shift_right"),
        PackageReviewIntegerBinaryKind::TrappingShiftLeft => (22, "trapping_shift_left"),
        PackageReviewIntegerBinaryKind::TrappingShiftRight => (23, "trapping_shift_right"),
        PackageReviewIntegerBinaryKind::TrappingAdd => (24, "trapping_add"),
        PackageReviewIntegerBinaryKind::TrappingSubtract => (25, "trapping_subtract"),
        PackageReviewIntegerBinaryKind::TrappingMultiply => (26, "trapping_multiply"),
        PackageReviewIntegerBinaryKind::TrappingDivide => (27, "trapping_divide"),
        PackageReviewIntegerBinaryKind::TrappingRemainder => (28, "trapping_remainder"),
        PackageReviewIntegerBinaryKind::SaturatingShiftLeft => (29, "saturating_shift_left"),
        PackageReviewIntegerBinaryKind::SaturatingShiftRight => (30, "saturating_shift_right"),
    }
}

pub(super) const fn primitive_type_name(
    primitive_type: PackageReviewPrimitiveType,
) -> &'static str {
    primitive_type_row(primitive_type).1
}

pub(super) const fn arithmetic_domain_name(domain: PackageReviewArithmeticDomain) -> &'static str {
    match domain {
        PackageReviewArithmeticDomain::Exact => "Exact",
        PackageReviewArithmeticDomain::Wrapping => "Wrapping",
        PackageReviewArithmeticDomain::Saturating => "Saturating",
        PackageReviewArithmeticDomain::Trapping => "Trapping",
    }
}
