// Stable numeric tags and spellings for structural arithmetic, primitive types, and integer comparisons.
// Each row helper pairs the persisted wire tag with the authored canonical spelling so binary encoding, canonical text, and recovery share one roster.

use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::values::crashes::Encoder;
use crate::record::PackageReviewArithmeticDomain;
use crate::record::PackageReviewIntegerBinaryKind;
use crate::record::PackageReviewIntegerComparisonKind;
use crate::record::PackageReviewPrimitiveType;

/*
    This published form names an integer by one tag per carrier and has rows
    for the eight conventional carriers only. An integer of any other width
    has no row, so the review record that mentions it is refused rather than
    written under another carrier's tag.
*/
pub(super) const fn primitive_type_row(
    primitive_type: PackageReviewPrimitiveType,
) -> Option<(u8, &'static str)> {
    Some(match primitive_type {
        PackageReviewPrimitiveType::Bool => (0, "bool"),
        PackageReviewPrimitiveType::F32 => (1, "f32"),
        PackageReviewPrimitiveType::F64 => (2, "f64"),
        PackageReviewPrimitiveType::Integer(carrier) => {
            match (carrier.is_signed(), carrier.width()) {
                (true, 8) => (3, "i8"),
                (true, 16) => (4, "i16"),
                (true, 32) => (5, "i32"),
                (true, 64) => (6, "i64"),
                (false, 8) => (7, "u8"),
                (false, 16) => (8, "u16"),
                (false, 32) => (9, "u32"),
                (false, 64) => (10, "u64"),
                _ => return None,
            }
        }
        PackageReviewPrimitiveType::Addr => (11, "addr"),
        PackageReviewPrimitiveType::Selector => (12, "selector"),
    })
}

const UNPUBLISHED_INTEGER_WIDTH: &str =
    "package review has no published form for an integer of this width";

pub(crate) fn encode_primitive_type(
    encoder: &mut Encoder,
    primitive_type: PackageReviewPrimitiveType,
) -> Result<(), PackageReviewEncodingError> {
    let (tag, name) = primitive_type_row(primitive_type)
        .ok_or_else(|| PackageReviewEncodingError::new(UNPUBLISHED_INTEGER_WIDTH))?;
    encoder.tag(name, tag);
    Ok(())
}

pub(crate) const fn integer_comparison_row(
    kind: PackageReviewIntegerComparisonKind,
) -> (u8, &'static str) {
    match kind {
        PackageReviewIntegerComparisonKind::Equal => (0, "equal"),
        PackageReviewIntegerComparisonKind::LessThan => (1, "less_than"),
        PackageReviewIntegerComparisonKind::LessOrEqual => (2, "less_or_equal"),
    }
}

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

pub(super) fn primitive_type_name(
    primitive_type: PackageReviewPrimitiveType,
) -> Result<&'static str, PackageReviewEncodingError> {
    primitive_type_row(primitive_type)
        .map(|(_, name)| name)
        .ok_or_else(|| PackageReviewEncodingError::new(UNPUBLISHED_INTEGER_WIDTH))
}

pub(super) const fn arithmetic_domain_name(domain: PackageReviewArithmeticDomain) -> &'static str {
    match domain {
        PackageReviewArithmeticDomain::Exact => "Exact",
        PackageReviewArithmeticDomain::Wrapping => "Wrapping",
        PackageReviewArithmeticDomain::Saturating => "Saturating",
        PackageReviewArithmeticDomain::Trapping => "Trapping",
    }
}

#[cfg(test)]
mod tests {
    use super::{primitive_type_name, primitive_type_row};
    use crate::record::PackageReviewPrimitiveType as Primitive;
    use numerics::integer_carrier::FixedInteger;

    /*
        The published rows keep their tags and spellings for the eight
        conventional carriers, and a carrier with no row is refused instead
        of borrowing a neighbour's tag.
    */
    #[test]
    fn integer_rows_are_keyed_by_carrier_and_other_widths_are_refused() {
        let rows = [
            (FixedInteger::signed(8), 3, "i8"),
            (FixedInteger::signed(16), 4, "i16"),
            (FixedInteger::signed(32), 5, "i32"),
            (FixedInteger::signed(64), 6, "i64"),
            (FixedInteger::unsigned(8), 7, "u8"),
            (FixedInteger::unsigned(16), 8, "u16"),
            (FixedInteger::unsigned(32), 9, "u32"),
            (FixedInteger::unsigned(64), 10, "u64"),
        ];
        for (carrier, tag, name) in rows {
            assert_eq!(
                primitive_type_row(Primitive::Integer(carrier)),
                Some((tag, name))
            );
            assert_eq!(carrier.to_string(), name);
        }
        assert_eq!(primitive_type_row(Primitive::Bool), Some((0, "bool")));
        assert_eq!(primitive_type_row(Primitive::Addr), Some((11, "addr")));
        for carrier in [
            FixedInteger::unsigned(13),
            FixedInteger::signed(1),
            FixedInteger::unsigned(128),
        ] {
            assert_eq!(primitive_type_row(Primitive::Integer(carrier)), None);
            assert!(primitive_type_name(Primitive::Integer(carrier)).is_err());
        }
    }
}
