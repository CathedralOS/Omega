//! Exact and wrapping integer-shift semantics.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::lowering::scalar) enum WrappingShiftKind {
    Left,
    Right,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::scalar) fn lower_wrapping_shift(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    kind: WrappingShiftKind,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::WrappingShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(match (value, count) {
        (KnownInteger::Immediate(value), KnownInteger::Immediate(count)) => {
            let shifted = match kind {
                WrappingShiftKind::Left => value_type.wrapping_shift_left(value, count_type, count),
                WrappingShiftKind::Right => {
                    value_type.wrapping_shift_right(value, count_type, count)
                }
            }
            .ok_or(LoweringError::WrappingShiftOperandTypeMismatch(result))?;
            KnownInteger::Immediate(shifted)
        }
        (value, count) => {
            let value = Box::new(value.into_expression(value_id));
            let count = Box::new(count.into_expression(count_id));
            KnownInteger::Runtime(match kind {
                WrappingShiftKind::Left => TargetIntegerExpression::WrappingShiftLeft {
                    psi_operation,
                    count_type,
                    value,
                    count,
                },
                WrappingShiftKind::Right => TargetIntegerExpression::WrappingShiftRight {
                    psi_operation,
                    count_type,
                    value,
                    count,
                },
            })
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::scalar) fn lower_exact_shift_right(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    psi_operation: psi_core::OperationId,
    obligation: psi_core::ObligationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::ExactShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(KnownInteger::Runtime(
        TargetIntegerExpression::ExactShiftRight {
            psi_operation,
            obligation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        },
    ))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::scalar) fn lower_exact_shift_left(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    psi_operation: psi_core::OperationId,
    obligation: psi_core::ObligationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(LoweringError::ExactShiftOperandTypeMismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    Ok(KnownInteger::Runtime(
        TargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            obligation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        },
    ))
}
