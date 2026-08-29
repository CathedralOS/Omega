//! Add, subtract, and multiply lowering across exact, wrapping, and saturating semantics.

use super::*;

pub(super) fn lower_integer_arithmetic(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let exact_obligation = match operation {
                AbstractOperation::ExactIntegerAdd { obligation, .. } => Some(*obligation),
                _ => None,
            };
            let left_id = *left;
            let right_id = *right;
            let left = values
                .get(left)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*left))?;
            let right = values
                .get(right)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*right))?;
            let (
                KnownScalar::Integer {
                    scalar_type: left_type,
                    value: left,
                },
                KnownScalar::Integer {
                    scalar_type: right_type,
                    value: right,
                },
            ) = (left, right)
            else {
                return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
            };
            if left_type != *scalar_type || right_type != *scalar_type {
                return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
            }
            let value = match (exact_obligation, left, right) {
                (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(
                        scalar_type
                            .wrapping_add(left, right)
                            .ok_or(LoweringError::WrappingAddOperandTypeMismatch(*result))?,
                    )
                }
                (Some(obligation), left, right) => {
                    KnownInteger::Runtime(TargetIntegerExpression::ExactAdd {
                        psi_operation: *psi_operation,
                        obligation,
                        left: Box::new(left.into_expression(left_id)),
                        right: Box::new(right.into_expression(right_id)),
                    })
                }
                (None, left, right) => {
                    KnownInteger::Runtime(TargetIntegerExpression::WrappingAdd {
                        psi_operation: *psi_operation,
                        left: Box::new(left.into_expression(left_id)),
                        right: Box::new(right.into_expression(right_id)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let left_id = *left;
            let right_id = *right;
            let (left, right) = typed_operands(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                LoweringError::SaturatingAddOperandTypeMismatch,
            )?;
            let value = match (left, right) {
                (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(
                        scalar_type
                            .saturating_add(left, right)
                            .ok_or(LoweringError::SaturatingAddOperandTypeMismatch(*result))?,
                    )
                }
                (left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::SaturatingAdd {
                        psi_operation: *psi_operation,
                        left: Box::new(left_value.into_expression(left_id)),
                        right: Box::new(right_value.into_expression(right_id)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let exact_obligation = match operation {
                AbstractOperation::ExactIntegerSubtract { obligation, .. } => Some(*obligation),
                _ => None,
            };
            let (left_value, right_value) = typed_operands(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                LoweringError::WrappingSubtractOperandTypeMismatch,
            )?;
            let value = match (exact_obligation, left_value, right_value) {
                (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(
                        scalar_type
                            .wrapping_sub(left, right)
                            .ok_or(LoweringError::WrappingSubtractOperandTypeMismatch(*result))?,
                    )
                }
                (Some(obligation), left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::ExactSubtract {
                        psi_operation: *psi_operation,
                        obligation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
                (None, left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::WrappingSubtract {
                        psi_operation: *psi_operation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left_value, right_value) = typed_operands(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                LoweringError::SaturatingSubtractOperandTypeMismatch,
            )?;
            let value = match (left_value, right_value) {
                (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(scalar_type.saturating_sub(left, right).ok_or(
                        LoweringError::SaturatingSubtractOperandTypeMismatch(*result),
                    )?)
                }
                (left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::SaturatingSubtract {
                        psi_operation: *psi_operation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            let exact_obligation = match operation {
                AbstractOperation::ExactIntegerMultiply { obligation, .. } => Some(*obligation),
                _ => None,
            };
            let (left_value, right_value) = typed_operands(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                LoweringError::WrappingMultiplyOperandTypeMismatch,
            )?;
            let value = match (exact_obligation, left_value, right_value) {
                (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(
                        scalar_type
                            .wrapping_mul(left, right)
                            .ok_or(LoweringError::WrappingMultiplyOperandTypeMismatch(*result))?,
                    )
                }
                (Some(obligation), left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::ExactMultiply {
                        psi_operation: *psi_operation,
                        obligation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
                (None, left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::WrappingMultiply {
                        psi_operation: *psi_operation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left_value, right_value) = typed_operands(
                values,
                *result,
                *scalar_type,
                *left,
                *right,
                LoweringError::SaturatingMultiplyOperandTypeMismatch,
            )?;
            let value = match (left_value, right_value) {
                (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                    KnownInteger::Immediate(scalar_type.saturating_mul(left, right).ok_or(
                        LoweringError::SaturatingMultiplyOperandTypeMismatch(*result),
                    )?)
                }
                (left_value, right_value) => {
                    KnownInteger::Runtime(TargetIntegerExpression::SaturatingMultiply {
                        psi_operation: *psi_operation,
                        left: Box::new(left_value.into_expression(*left)),
                        right: Box::new(right_value.into_expression(*right)),
                    })
                }
            };
            insert_integer(values, *result, *scalar_type, value)?;
            provenance.operations.push(*psi_operation);
        }
        _ => unreachable!("integer-arithmetic routing admits only its declared operations"),
    }
    Ok(())
}

fn typed_operands(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    mismatch: fn(ValueId) -> LoweringError,
) -> Result<(KnownInteger, KnownInteger), LoweringError> {
    let left_value = values
        .get(&left)
        .cloned()
        .ok_or(LoweringError::UnknownValue(left))?;
    let right_value = values
        .get(&right)
        .cloned()
        .ok_or(LoweringError::UnknownValue(right))?;
    let (
        KnownScalar::Integer {
            scalar_type: left_type,
            value: left_value,
        },
        KnownScalar::Integer {
            scalar_type: right_type,
            value: right_value,
        },
    ) = (left_value, right_value)
    else {
        return Err(mismatch(result));
    };
    if left_type != scalar_type || right_type != scalar_type {
        return Err(mismatch(result));
    }
    Ok((left_value, right_value))
}

fn insert_integer(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    scalar_type: IntegerType,
    value: KnownInteger,
) -> Result<(), LoweringError> {
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })
}
