//! Exhaustive selection of integer scalar-operation semantics.
use super::{IntegerBinaryKind, KnownInteger, KnownScalar, lower_integer_binary, lower_shift};
use crate::LoweringError;
use crate::lowering::scalar::expressions::insert_value;
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{ScalarType, ValueId};
use std::collections::BTreeMap;
use target_operations::TargetIntegerExpression;
pub(in crate::lowering) fn try_lower_integer_operation(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<semantic_vocabulary::OperationId>,
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
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            lower_integer_binary(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::Arithmetic(*arithmetic),
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
                lower_integer_binary(
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
        AbstractOperation::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_shift(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *policy,
                *direction,
                *psi_operation,
            )?,
        ),
        _ => return Ok(false),
    };
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.push(psi_operation);
    Ok(true)
}
