//! Boolean and integer scalar-operation tags.

use super::scalar_shapes::*;
use super::*;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            bytes.u8(9);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            encode_integer_value(bytes, *value);
        }
        O::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(42);
            bytes.id(*psi_operation);
            bytes.id(*result);
            match value {
                psi_core::IeeeFloatValue::Binary32(bits) => {
                    bytes.u8(0);
                    bytes.u32(*bits);
                }
                psi_core::IeeeFloatValue::Binary64(bits) => {
                    bytes.u8(1);
                    bytes.u64(*bits);
                }
            }
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            left,
            right,
            addend,
        } => {
            bytes.u8(43);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.u8(match format {
                psi_core::IeeeFloatFormat::Binary32 => 0,
                psi_core::IeeeFloatFormat::Binary64 => 1,
            });
            bytes.id(*left);
            bytes.id(*right);
            bytes.id(*addend);
        }
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(10);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.boolean(*value);
        }
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            bytes.u8(11);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.id(*source);
            bytes.id(*field);
        }
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => encode_untyped_unary(bytes, 12, *psi_operation, *result, *operand),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 13, *psi_operation, *result, *left, *right),
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 14, *psi_operation, *result, *left, *right),
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 15, *psi_operation, *result, *left, *right),
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 16, *psi_operation, *result, *left, *right),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => encode_typed_unary(bytes, 17, *psi_operation, *result, *scalar_type, *operand),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            18,
            *psi_operation,
            None,
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            19,
            *psi_operation,
            Some(*obligation),
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            20,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            21,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            22,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            23,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            24,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            25,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            26,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            27,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            28,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            29,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            30,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            31,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            32,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            33,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            34,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            35,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            36,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            37,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            38,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            39,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            40,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            41,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => unreachable!("operation family routing admitted a non-scalar operation"),
    }
}
