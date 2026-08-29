use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_straight_line(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    mut values: BTreeMap<ValueId, KnownScalar>,
    function_result: AbstractResult,
    call_plan: CallPlan,
    target_structural_parameters: Vec<TargetStructuralParameter>,
) -> Result<TargetFunction, LoweringError> {
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    for operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
            | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                ..
            } => {
                if result.is_some() {
                    return Err(
                        LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                            machine: function.machine,
                            operation: *psi_operation,
                            boundary: *boundary,
                        },
                    );
                }
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | AbstractOperation::CallUnit { psi_operation, .. }
            | AbstractOperation::PortWrite { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::CallStructuralScalar { .. }
            | AbstractOperation::CallStructural { .. } => {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            AbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                callee,
                arguments,
            } => {
                let value = lower_call(
                    *psi_operation,
                    *result,
                    *scalar_type,
                    *callee,
                    arguments,
                    &values,
                    target,
                    functions,
                )?;
                insert_value(&mut values, *result, value)?;
                provenance.operations.push(*psi_operation);
            }
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
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *integer_type,
                        value: KnownInteger::Immediate(*value),
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BooleanStructuralField { psi_operation, .. } => {
                return Err(LoweringError::UnitOperationInScalarFunction {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            AbstractOperation::BooleanNot {
                psi_operation,
                result,
                operand,
            } => {
                let operand = values
                    .get(operand)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*operand))?;
                insert_value(
                    &mut values,
                    *result,
                    negate_boolean(operand, *psi_operation, *result)?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::BooleanEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                insert_value(
                    &mut values,
                    *result,
                    equal_boolean(left, right, *psi_operation, *result)?,
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::IntegerEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left_value = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right_value = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                insert_value(
                    &mut values,
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
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            }
            | AbstractOperation::IntegerLessOrEqual {
                psi_operation,
                result,
                left,
                right,
            } => {
                let left_value = values
                    .get(left)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right_value = values
                    .get(right)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let inclusive = matches!(operation, AbstractOperation::IntegerLessOrEqual { .. });
                insert_value(
                    &mut values,
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
                provenance.operations.push(*psi_operation);
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
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    kind,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
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
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
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
                    }) if operand_type == *source_type
                        && source_type.can_widen_to(*target_type) =>
                    {
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
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *target_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
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
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *target_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
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
                let shifted = lower_wrapping_shift(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    kind,
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let shifted = lower_exact_shift_right(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    *psi_operation,
                    *obligation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type,
                value,
                count,
            } => {
                let shifted = lower_exact_shift_left(
                    &values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    *psi_operation,
                    *obligation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *value_type,
                        value: shifted,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
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
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
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
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(
                            scalar_type
                                .saturating_add(left, right)
                                .ok_or(LoweringError::SaturatingAddOperandTypeMismatch(*result))?,
                        )
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingAdd {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
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
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                }
                let value =
                    match (exact_obligation, left, right) {
                        (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_sub(left, right).ok_or(
                                LoweringError::WrappingSubtractOperandTypeMismatch(*result),
                            )?)
                        }
                        (Some(obligation), left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::ExactSubtract {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::WrappingSubtract {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                    };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
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
                    return Err(LoweringError::SaturatingSubtractOperandTypeMismatch(
                        *result,
                    ));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingSubtractOperandTypeMismatch(
                        *result,
                    ));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(scalar_type.saturating_sub(left, right).ok_or(
                            LoweringError::SaturatingSubtractOperandTypeMismatch(*result),
                        )?)
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingSubtract {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
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
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                }
                let value =
                    match (exact_obligation, left, right) {
                        (None, KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_mul(left, right).ok_or(
                                LoweringError::WrappingMultiplyOperandTypeMismatch(*result),
                            )?)
                        }
                        (Some(obligation), left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::ExactMultiply {
                                psi_operation: *psi_operation,
                                obligation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                        (None, left, right) => {
                            KnownInteger::Runtime(TargetIntegerExpression::WrappingMultiply {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            })
                        }
                    };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
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
                    return Err(LoweringError::SaturatingMultiplyOperandTypeMismatch(
                        *result,
                    ));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingMultiplyOperandTypeMismatch(
                        *result,
                    ));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(scalar_type.saturating_mul(left, right).ok_or(
                            LoweringError::SaturatingMultiplyOperandTypeMismatch(*result),
                        )?)
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TargetIntegerExpression::SaturatingMultiply {
                            psi_operation: *psi_operation,
                            left: Box::new(left.into_expression(left_id)),
                            right: Box::new(right.into_expression(right_id)),
                        })
                    }
                };
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::ExactDivide(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::ExactRemainder(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::WrappingDivide(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::WrappingRemainder(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::SaturatingDivide(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let value = lower_conditional_integer_binary(
                    &values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::SaturatingRemainder(*obligation),
                    *psi_operation,
                )?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            AbstractOperation::Jump {
                psi_edge,
                bindings,
                trivial_affine_discards,
                ..
            } => {
                // This ownership-only edge work is deliberately erased after
                // Terminal verification (and optimizer admission when
                // selected); it has no target instruction.
                let _ = trivial_affine_discards;
                let transferred = bindings
                    .iter()
                    .map(|binding| {
                        let value = values
                            .get(&binding.argument)
                            .cloned()
                            .ok_or(LoweringError::UnknownValue(binding.argument))?;
                        if binding.scalar_type != value.scalar_type() {
                            return Err(LoweringError::ValueTypeMismatch(binding.parameter));
                        }
                        Ok((binding.parameter, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (parameter, value) in transferred {
                    insert_value(&mut values, parameter, value)?;
                }
                provenance.edges.push(*psi_edge);
            }
            AbstractOperation::Conditional { .. } => {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            AbstractOperation::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => {
                provenance.edges.push(*psi_edge);
                returned = Some(TargetOperation::Crash {
                    psi_edge: *psi_edge,
                    cause: *cause,
                    site_guard: site_guard.clone(),
                    frontier_lower_bound: frontier_lower_bound.clone(),
                });
            }
            AbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            } => {
                if *result != function_result.value || *scalar_type != function_result.scalar_type {
                    return Err(LoweringError::FunctionResultMismatch(function.machine));
                }
                let returned_value = values
                    .get(value)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(*value))?;
                if *scalar_type != returned_value.scalar_type() {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                }
                provenance.edges.push(*psi_edge);
                let scalar = match returned_value {
                    KnownScalar::Boolean(boolean) => TargetOperation::ReturnBooleanImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        value: boolean,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Immediate(integer),
                    } => TargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value:
                            KnownInteger::Runtime(TargetIntegerExpression::Parameter {
                                parameter_index,
                                location,
                                ..
                            }),
                    } => TargetOperation::ReturnIntegerParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        parameter_index,
                        location,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(expression),
                    } => TargetOperation::ReturnIntegerExpression {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        expression,
                    },
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                        parameter_index,
                        location,
                        ..
                    }) => TargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
                    KnownScalar::BooleanRuntime(TargetBooleanExpression::Not {
                        operand, ..
                    }) if matches!(*operand, TargetBooleanExpression::Parameter { .. }) => {
                        let TargetBooleanExpression::Parameter {
                            parameter_index,
                            location,
                            ..
                        } = *operand
                        else {
                            unreachable!("guard requires a parameter operand")
                        };
                        TargetOperation::ReturnBooleanNotParameter {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            parameter_index,
                            location,
                        }
                    }
                    KnownScalar::BooleanRuntime(expression) => {
                        TargetOperation::ReturnBooleanExpression {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            expression,
                        }
                    }
                };
                if cleanup_actions.is_empty() {
                    returned = Some(scalar);
                } else {
                    validate_scalar_cleanup_frontier(
                        function.machine,
                        cleanup_actions,
                        &target_structural_parameters,
                        functions,
                        structural_types,
                    )?;
                    returned = Some(TargetOperation::ScalarReturnWithCleanup {
                        scalar: Box::new(scalar),
                        structural_types: structural_types
                            .values()
                            .map(|declaration| (*declaration).clone())
                            .collect(),
                        call_plan: call_plan.clone(),
                        structural_parameters: target_structural_parameters.clone(),
                        cleanup_actions: cleanup_actions.clone(),
                        psi_edge: *psi_edge,
                    });
                }
            }
            AbstractOperation::ReturnUnit { .. } | AbstractOperation::ReturnStructural { .. } => {
                return Err(LoweringError::FunctionResultKindMismatch(function.machine));
            }
        }
    }

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}
