use super::*;

pub(super) fn lower_conditional_scalar_operation(
    operation: &TerminalAbstractOperation,
    machine: MachineId,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if let TerminalAbstractOperation::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
    } = operation
    {
        let value = lower_call(
            *psi_operation,
            *result,
            *scalar_type,
            *callee,
            arguments,
            values,
            target,
            functions,
        )?;
        insert_value(values, *result, value)?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value,
    } = operation
    {
        insert_value(values, *result, KnownScalar::Boolean(*value))?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanStructuralField {
        psi_operation,
        result,
        source,
        field,
    } = operation
    {
        let parameter = structural_parameters
            .iter()
            .find(|parameter| parameter.place == *source)
            .ok_or(LoweringError::UnsupportedOperationInScalarFunction(machine))?;
        let field_byte_offset =
            direct_boolean_field_offset(parameter.structural_type, *field, structural_types)?;
        insert_value(
            values,
            *result,
            KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::StructuralField {
                psi_operation: *psi_operation,
                source_value: *result,
                source: *source,
                field: *field,
                source_placement: parameter.placement.clone(),
                field_byte_offset,
            }),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanNot {
        psi_operation,
        result,
        operand,
    } = operation
    {
        let operand = values
            .get(operand)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*operand))?;
        insert_value(
            values,
            *result,
            negate_boolean(operand, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::BooleanEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
            *result,
            equal_boolean(left_value, right_value, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::IntegerEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
            *result,
            equal_integer(
                *left,
                left_value,
                *right,
                right_value,
                *psi_operation,
                *result,
            )?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let TerminalAbstractOperation::IntegerLessThan {
        psi_operation,
        result,
        left,
        right,
    }
    | TerminalAbstractOperation::IntegerLessOrEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        let inclusive = matches!(
            operation,
            TerminalAbstractOperation::IntegerLessOrEqual { .. }
        );
        insert_value(
            values,
            *result,
            order_integer(
                *left,
                left_value,
                *right,
                right_value,
                *psi_operation,
                *result,
                inclusive,
            )?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    let (psi_operation, result, scalar_type, value) = match operation {
        TerminalAbstractOperation::IntegerConstant {
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
        TerminalAbstractOperation::WrappingIntegerAdd {
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
        TerminalAbstractOperation::ExactIntegerAdd {
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
        TerminalAbstractOperation::SaturatingIntegerAdd {
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
        TerminalAbstractOperation::WrappingIntegerSubtract {
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
        TerminalAbstractOperation::ExactIntegerSubtract {
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
        TerminalAbstractOperation::SaturatingIntegerSubtract {
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
        TerminalAbstractOperation::WrappingIntegerMultiply {
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
        TerminalAbstractOperation::ExactIntegerMultiply {
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
        TerminalAbstractOperation::SaturatingIntegerMultiply {
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
        TerminalAbstractOperation::ExactIntegerDivide {
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
        TerminalAbstractOperation::ExactIntegerRemainder {
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
        TerminalAbstractOperation::WrappingIntegerDivide {
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
        TerminalAbstractOperation::WrappingIntegerRemainder {
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
        TerminalAbstractOperation::SaturatingIntegerDivide {
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
        TerminalAbstractOperation::SaturatingIntegerRemainder {
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
        TerminalAbstractOperation::IntegerBitwiseNot {
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
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::BitwiseNot {
                        psi_operation: *psi_operation,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *scalar_type, value)
        }
        TerminalAbstractOperation::IntegerWiden {
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
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerWiden {
                        psi_operation: *psi_operation,
                        source_type: *source_type,
                        operand: Box::new(expression),
                    })
                }
            };
            (*psi_operation, *result, *target_type, value)
        }
        TerminalAbstractOperation::IntegerExactCast {
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
            let value = KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
                psi_operation: *psi_operation,
                obligation: *obligation,
                source_type: *source_type,
                operand: Box::new(operand_value.into_expression(*operand)),
            });
            (*psi_operation, *result, *target_type, value)
        }
        TerminalAbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | TerminalAbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | TerminalAbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let kind = match operation {
                TerminalAbstractOperation::IntegerBitwiseAnd { .. } => {
                    IntegerBinaryKind::BitwiseAnd
                }
                TerminalAbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                TerminalAbstractOperation::IntegerBitwiseXor { .. } => {
                    IntegerBinaryKind::BitwiseXor
                }
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
        TerminalAbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        }
        | TerminalAbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let kind = if matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
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
        TerminalAbstractOperation::ExactIntegerShiftRight {
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
        TerminalAbstractOperation::ExactIntegerShiftLeft {
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

#[derive(Clone, Copy)]
pub(super) enum IntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingAdd,
    ExactAdd(psi_core::ObligationId),
    SaturatingAdd,
    WrappingSubtract,
    ExactSubtract(psi_core::ObligationId),
    SaturatingSubtract,
    WrappingMultiply,
    ExactMultiply(psi_core::ObligationId),
    SaturatingMultiply,
    ExactDivide(psi_core::ObligationId),
    ExactRemainder(psi_core::ObligationId),
    WrappingDivide(psi_core::ObligationId),
    WrappingRemainder(psi_core::ObligationId),
    SaturatingDivide(psi_core::ObligationId),
    SaturatingRemainder(psi_core::ObligationId),
}

#[derive(Clone, Copy)]
pub(super) enum WrappingShiftKind {
    Left,
    Right,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_wrapping_shift(
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
                WrappingShiftKind::Left => TerminalTargetIntegerExpression::WrappingShiftLeft {
                    psi_operation,
                    count_type,
                    value,
                    count,
                },
                WrappingShiftKind::Right => TerminalTargetIntegerExpression::WrappingShiftRight {
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
pub(super) fn lower_exact_shift_right(
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
        TerminalTargetIntegerExpression::ExactShiftRight {
            psi_operation,
            obligation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        },
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_exact_shift_left(
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
        TerminalTargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            obligation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        },
    ))
}

pub(super) fn lower_conditional_integer_binary(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    scalar_type: IntegerType,
    left_id: ValueId,
    right_id: ValueId,
    kind: IntegerBinaryKind,
    psi_operation: psi_core::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id| match values.get(&id).cloned() {
        Some(KnownScalar::Integer {
            scalar_type: operand_type,
            value,
        }) if operand_type == scalar_type => Ok(value),
        Some(_) => Err(kind.mismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let left = operand(left_id)?;
    let right = operand(right_id)?;
    if kind.is_proof_bearing() {
        return Ok(KnownInteger::Runtime(kind.expression(
            psi_operation,
            left.into_expression(left_id),
            right.into_expression(right_id),
        )));
    }
    Ok(match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => KnownInteger::Immediate(
            kind.fold(scalar_type, left, right)
                .ok_or(kind.mismatch(result))?,
        ),
        (left, right) => KnownInteger::Runtime(kind.expression(
            psi_operation,
            left.into_expression(left_id),
            right.into_expression(right_id),
        )),
    })
}

impl IntegerBinaryKind {
    fn mismatch(self, result: ValueId) -> LoweringError {
        match self {
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => {
                LoweringError::IntegerBitwiseOperandTypeMismatch(result)
            }
            Self::WrappingAdd | Self::ExactAdd(_) => {
                LoweringError::WrappingAddOperandTypeMismatch(result)
            }
            Self::SaturatingAdd => LoweringError::SaturatingAddOperandTypeMismatch(result),
            Self::WrappingSubtract | Self::ExactSubtract(_) => {
                LoweringError::WrappingSubtractOperandTypeMismatch(result)
            }
            Self::SaturatingSubtract => {
                LoweringError::SaturatingSubtractOperandTypeMismatch(result)
            }
            Self::WrappingMultiply | Self::ExactMultiply(_) => {
                LoweringError::WrappingMultiplyOperandTypeMismatch(result)
            }
            Self::SaturatingMultiply => {
                LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
            }
            Self::ExactDivide(_) => LoweringError::ExactDivideOperandTypeMismatch(result),
            Self::ExactRemainder(_) => LoweringError::ExactRemainderOperandTypeMismatch(result),
            Self::WrappingDivide(_) => LoweringError::WrappingDivideOperandTypeMismatch(result),
            Self::WrappingRemainder(_) => {
                LoweringError::WrappingRemainderOperandTypeMismatch(result)
            }
            Self::SaturatingDivide(_) => LoweringError::SaturatingDivideOperandTypeMismatch(result),
            Self::SaturatingRemainder(_) => {
                LoweringError::SaturatingRemainderOperandTypeMismatch(result)
            }
        }
    }

    fn fold(
        self,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        match self {
            Self::BitwiseAnd => scalar_type.bitwise_and(left, right),
            Self::BitwiseOr => scalar_type.bitwise_or(left, right),
            Self::BitwiseXor => scalar_type.bitwise_xor(left, right),
            Self::WrappingAdd => scalar_type.wrapping_add(left, right),
            Self::ExactAdd(_) => scalar_type.exact_add(left, right),
            Self::SaturatingAdd => scalar_type.saturating_add(left, right),
            Self::WrappingSubtract => scalar_type.wrapping_sub(left, right),
            Self::ExactSubtract(_) => scalar_type.exact_sub(left, right),
            Self::SaturatingSubtract => scalar_type.saturating_sub(left, right),
            Self::WrappingMultiply => scalar_type.wrapping_mul(left, right),
            Self::ExactMultiply(_) => scalar_type.exact_mul(left, right),
            Self::SaturatingMultiply => scalar_type.saturating_mul(left, right),
            Self::ExactDivide(_) => scalar_type.exact_div(left, right),
            Self::ExactRemainder(_) => scalar_type.exact_rem(left, right),
            Self::WrappingDivide(_) => scalar_type.wrapping_div(left, right),
            Self::WrappingRemainder(_) => scalar_type.wrapping_rem(left, right),
            Self::SaturatingDivide(_) => scalar_type.saturating_div(left, right),
            Self::SaturatingRemainder(_) => scalar_type.saturating_rem(left, right),
        }
    }

    fn expression(
        self,
        psi_operation: psi_core::OperationId,
        left: TerminalTargetIntegerExpression,
        right: TerminalTargetIntegerExpression,
    ) -> TerminalTargetIntegerExpression {
        let left = Box::new(left);
        let right = Box::new(right);
        match self {
            Self::BitwiseAnd => TerminalTargetIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseOr => TerminalTargetIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseXor => TerminalTargetIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
            Self::WrappingAdd => TerminalTargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
            Self::ExactAdd(obligation) => TerminalTargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingAdd => TerminalTargetIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
            Self::WrappingSubtract => TerminalTargetIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::ExactSubtract(obligation) => TerminalTargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingSubtract => TerminalTargetIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::WrappingMultiply => TerminalTargetIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::ExactMultiply(obligation) => TerminalTargetIntegerExpression::ExactMultiply {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingMultiply => TerminalTargetIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::ExactDivide(obligation) => TerminalTargetIntegerExpression::ExactDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::ExactRemainder(obligation) => TerminalTargetIntegerExpression::ExactRemainder {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::WrappingDivide(obligation) => TerminalTargetIntegerExpression::WrappingDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::WrappingRemainder(obligation) => {
                TerminalTargetIntegerExpression::WrappingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                }
            }
            Self::SaturatingDivide(obligation) => {
                TerminalTargetIntegerExpression::SaturatingDivide {
                    psi_operation,
                    obligation,
                    left,
                    right,
                }
            }
            Self::SaturatingRemainder(obligation) => {
                TerminalTargetIntegerExpression::SaturatingRemainder {
                    psi_operation,
                    obligation,
                    left,
                    right,
                }
            }
        }
    }
}

impl IntegerBinaryKind {
    fn is_proof_bearing(self) -> bool {
        matches!(
            self,
            Self::ExactAdd(_)
                | Self::ExactSubtract(_)
                | Self::ExactMultiply(_)
                | Self::ExactDivide(_)
                | Self::ExactRemainder(_)
                | Self::WrappingDivide(_)
                | Self::WrappingRemainder(_)
                | Self::SaturatingDivide(_)
                | Self::SaturatingRemainder(_)
        )
    }
}
