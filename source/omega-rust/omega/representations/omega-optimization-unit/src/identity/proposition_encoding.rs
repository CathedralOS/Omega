//! Canonical proof proposition, content term, and scalar term encoding.

use super::*;

pub(super) fn encode_proposition(bytes: &mut CanonicalBytes, proposition: &Proposition) {
    match proposition {
        Proposition::Truth => bytes.u8(1),
        Proposition::Falsehood => bytes.u8(2),
        Proposition::Atom(id) => {
            bytes.u8(3);
            bytes.id(*id);
        }
        Proposition::Equal(left, right) => {
            bytes.u8(4);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::LessThan(left, right) => {
            bytes.u8(5);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::LessOrEqual(left, right) => {
            bytes.u8(6);
            encode_scalar_term(bytes, left);
            encode_scalar_term(bytes, right);
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            bytes.u8(match proposition {
                Proposition::IntegerMathEqual(_, _) => 14,
                Proposition::IntegerMathLessThan(_, _) => 15,
                Proposition::IntegerMathLessOrEqual(_, _) => 16,
                _ => unreachable!(),
            });
            encode_integer_math_term(bytes, left);
            encode_integer_math_term(bytes, right);
        }
        Proposition::Conjunction(values) => {
            bytes.u8(7);
            bytes.slice(values, encode_proposition);
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            bytes.u8(8);
            encode_proposition(bytes, premise);
            encode_proposition(bytes, conclusion);
        }
        Proposition::ContentConservation(value) => {
            bytes.u8(9);
            encode_content_algebra(bytes, value.algebra());
            encode_content_term(bytes, value.left());
            encode_content_term(bytes, value.right());
        }
        Proposition::Disjunction(values) => {
            bytes.u8(10);
            bytes.slice(values, encode_proposition);
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            bytes.u8(11);
            encode_float_comparison(bytes, *kind);
            encode_float_format(bytes, *format);
            encode_float_field(bytes, left);
            encode_float_field(bytes, right);
        }
        Proposition::ByteSequenceEqual { left, right } => {
            bytes.u8(12);
            encode_byte_field(bytes, left);
            encode_byte_field(bytes, right);
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            bytes.u8(13);
            encode_case_subject(bytes, subject);
            bytes.id(*case);
        }
    }
}

fn encode_integer_math_term(bytes: &mut CanonicalBytes, term: &IntegerMathTerm) {
    match term {
        IntegerMathTerm::IntegerLiteral(literal) => {
            bytes.u8(1);
            bytes.boolean(literal.negative());
            bytes.u128(literal.magnitude());
        }
        IntegerMathTerm::MathValue { source_type, value } => {
            bytes.u8(2);
            encode_integer_type(bytes, *source_type);
            bytes.id(*value);
        }
        IntegerMathTerm::Add(left, right)
        | IntegerMathTerm::Subtract(left, right)
        | IntegerMathTerm::Multiply(left, right) => {
            bytes.u8(match term {
                IntegerMathTerm::Add(_, _) => 3,
                IntegerMathTerm::Subtract(_, _) => 4,
                IntegerMathTerm::Multiply(_, _) => 5,
                _ => unreachable!(),
            });
            encode_integer_math_term(bytes, left);
            encode_integer_math_term(bytes, right);
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            bytes.u8(6);
            encode_integer_math_term(bytes, value);
            encode_integer_math_term(bytes, count);
        }
    }
}

pub(super) fn encode_content_term(bytes: &mut CanonicalBytes, term: &ContentTerm) {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            bytes.u8(1);
            bytes.id(projection.domain);
            bytes.u64(projection.projection_report_fingerprint);
            encode_content_place(bytes, subject);
        }
        ContentTerm::Separate(terms) => {
            bytes.u8(2);
            bytes.slice(terms, encode_content_term);
        }
    }
}

pub(super) fn encode_scalar_term(bytes: &mut CanonicalBytes, term: &ScalarTerm) {
    use ScalarTerm as S;
    match term {
        S::Value { id, scalar_type } => {
            bytes.u8(1);
            bytes.id(*id);
            encode_scalar_type(bytes, *scalar_type);
        }
        S::Boolean(value) => {
            bytes.u8(2);
            bytes.boolean(*value);
        }
        S::Integer { scalar_type, value } => {
            bytes.u8(3);
            encode_integer_type(bytes, *scalar_type);
            encode_integer_value(bytes, *value);
        }
        S::BooleanField { root, path } => {
            bytes.u8(4);
            bytes.id(*root);
            encode_canonical_path(bytes, path);
        }
        S::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            bytes.u8(5);
            bytes.id(*root);
            encode_canonical_path(bytes, path);
            encode_integer_type(bytes, *scalar_type);
        }
        S::BooleanNot { operand } => encode_scalar_unary(bytes, 6, None, operand),
        S::BooleanEqual { left, right } => encode_scalar_binary(bytes, 7, None, left, right),
        S::IntegerEqual {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 8, Some(*scalar_type), left, right),
        S::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 9, Some(*scalar_type), left, right),
        S::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 10, Some(*scalar_type), left, right),
        S::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => encode_scalar_unary(bytes, 11, Some(*scalar_type), operand),
        S::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => encode_scalar_cast(bytes, 12, *source_type, *target_type, operand),
        S::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => encode_scalar_cast(bytes, 13, *source_type, *target_type, operand),
        S::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 14, Some(*scalar_type), left, right),
        S::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 15, Some(*scalar_type), left, right),
        S::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 16, Some(*scalar_type), left, right),
        S::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 17, *value_type, *count_type, value, count),
        S::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 18, *value_type, *count_type, value, count),
        S::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 19, *value_type, *count_type, value, count),
        S::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => encode_scalar_shift(bytes, 20, *value_type, *count_type, value, count),
        S::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 21, Some(*scalar_type), left, right),
        S::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 22, Some(*scalar_type), left, right),
        S::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 23, Some(*scalar_type), left, right),
        S::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 24, Some(*scalar_type), left, right),
        S::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 25, Some(*scalar_type), left, right),
        S::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 26, Some(*scalar_type), left, right),
        S::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 27, Some(*scalar_type), left, right),
        S::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 28, Some(*scalar_type), left, right),
        S::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 29, Some(*scalar_type), left, right),
        S::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 30, Some(*scalar_type), left, right),
        S::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 31, Some(*scalar_type), left, right),
        S::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 32, Some(*scalar_type), left, right),
        S::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 33, Some(*scalar_type), left, right),
        S::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 34, Some(*scalar_type), left, right),
        S::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => encode_scalar_binary(bytes, 35, Some(*scalar_type), left, right),
    }
}

pub(super) fn encode_scalar_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    scalar_type: Option<IntegerType>,
    operand: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
        encode_integer_type(bytes, *value)
    });
    encode_scalar_term(bytes, operand);
}
pub(super) fn encode_scalar_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    scalar_type: Option<IntegerType>,
    left: &ScalarTerm,
    right: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_optional(bytes, scalar_type.as_ref(), |bytes, value| {
        encode_integer_type(bytes, *value)
    });
    encode_scalar_term(bytes, left);
    encode_scalar_term(bytes, right);
}
pub(super) fn encode_scalar_cast(
    bytes: &mut CanonicalBytes,
    tag: u8,
    source: IntegerType,
    target: IntegerType,
    operand: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_integer_type(bytes, source);
    encode_integer_type(bytes, target);
    encode_scalar_term(bytes, operand);
}
pub(super) fn encode_scalar_shift(
    bytes: &mut CanonicalBytes,
    tag: u8,
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) {
    bytes.u8(tag);
    encode_integer_type(bytes, value_type);
    encode_integer_type(bytes, count_type);
    encode_scalar_term(bytes, value);
    encode_scalar_term(bytes, count);
}

pub(super) fn encode_canonical_path(
    bytes: &mut CanonicalBytes,
    path: &[CanonicalStructuralPathSegment],
) {
    bytes.len(path.len());
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Field(value) => {
                bytes.u8(1);
                bytes.id(*value);
            }
            CanonicalStructuralPathSegment::FixedIndex(value) => {
                bytes.u8(2);
                bytes.u64(*value);
            }
            CanonicalStructuralPathSegment::Case(value) => {
                bytes.u8(3);
                bytes.id(*value);
            }
        }
    }
}
pub(super) fn encode_float_format(bytes: &mut CanonicalBytes, value: IeeeFloatFormat) {
    bytes.u8(match value {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}
pub(super) fn encode_float_comparison(bytes: &mut CanonicalBytes, value: IeeeFloatComparisonKind) {
    bytes.u8(match value {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
    });
}
pub(super) fn encode_float_field(bytes: &mut CanonicalBytes, value: &IeeeFloatStructuralField) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
pub(super) fn encode_byte_field(bytes: &mut CanonicalBytes, value: &ByteSequenceStructuralField) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
pub(super) fn encode_case_subject(bytes: &mut CanonicalBytes, value: &StructuralCaseSubject) {
    bytes.id(value.root());
    encode_canonical_path(bytes, value.path());
}
