//! Exhaustive selection of integer scalar-operation semantics.
use super::*;
pub(super) fn try_lower_integer_operation(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
) -> Result<bool, LoweringError> {
    let (psi_operation, result, scalar_type, value) = match operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
            };
            if !integer_type.admits(*value) {
                return Err(LoweringError::IntegerConstantOutsideType(*result));
            }
            (
                *psi_operation,
                *result,
                *integer_type,
                KnownInteger::Immediate(*value),
            )
        }
        AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingAdd,
                *psi_operation,
            )?,
        ),
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactAdd(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingAdd,
                *psi_operation,
            )?,
        ),
        AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingSubtract,
                *psi_operation,
            )?,
        ),
        AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactSubtract(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingSubtract,
                *psi_operation,
            )?,
        ),
        AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingMultiply,
                *psi_operation,
            )?,
        ),
        AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactMultiply(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingMultiply,
                *psi_operation,
            )?,
        ),
        AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactDivide(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactRemainder(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingDivide(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingRemainder(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingDivide(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_conditional_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingRemainder(*obligation),
                *psi_operation,
            )?,
        ),
        AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *scalar_type => value,
                Some(_) => {
                    return Err(LoweringError::IntegerBitwiseOperandTypeMismatch(*result));
                }
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    scalar_type
                        .bitwise_not(value)
                        .ok_or(LoweringError::IntegerBitwiseOperandTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TargetIntegerExpression::BitwiseNot {
                        psi_operation: *psi_operation,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *scalar_type, value)
        }
        AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *source_type && source_type.can_widen_to(*target_type) => {
                    value
                }
                Some(_) => return Err(LoweringError::IntegerWidenTypeMismatch(*result)),
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    source_type
                        .widen_value_to(*target_type, value)
                        .ok_or(LoweringError::IntegerWidenTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TargetIntegerExpression::IntegerWiden {
                        psi_operation: *psi_operation,
                        source_type: *source_type,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *target_type, value)
        }
        AbstractOperation::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer {
                    scalar_type: operand_type,
                    value,
                }) if operand_type == *source_type
                    && source_type.can_exact_cast_to(*target_type) =>
                {
                    value
                }
                Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            let value = KnownInteger::Runtime(TargetIntegerExpression::IntegerExactCast {
                psi_operation: *psi_operation,
                obligation: *obligation,
                source_type: *source_type,
                operand: Box::new(operand_value.into_expression(*operand)),
            });
            (*psi_operation, *result, *target_type, value)
        }
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let kind = match operation {
                AbstractOperation::IntegerBitwiseAnd { .. } => IntegerBinaryKind::BitwiseAnd,
                AbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                AbstractOperation::IntegerBitwiseXor { .. } => IntegerBinaryKind::BitwiseXor,
                _ => unreachable!(),
            };
            (
                *psi_operation,
                *result,
                *scalar_type,
                lower_conditional_integer_binary(
                    values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    kind,
                    *psi_operation,
                )?,
            )
        }
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let kind = if matches!(
                operation,
                AbstractOperation::WrappingIntegerShiftLeft { .. }
            ) {
                WrappingShiftKind::Left
            } else {
                WrappingShiftKind::Right
            };
            (
                *psi_operation,
                *result,
                *value_type,
                lower_wrapping_shift(
                    values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    kind,
                    *psi_operation,
                )?,
            )
        }
        AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_right(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?,
        ),
        AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_left(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?,
        ),
        _ => return Ok(false),
    };
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.push(psi_operation);
    Ok(true)
}
