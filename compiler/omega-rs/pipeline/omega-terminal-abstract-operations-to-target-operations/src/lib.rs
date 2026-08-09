#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    CallSignature, CallingPolicy, PlanDiagnostic, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
    TerminalAbstractParameter,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalScalarParameterLocation, TerminalTargetBooleanControl,
    TerminalTargetBooleanExpression, TerminalTargetConditionalBooleanArm,
    TerminalTargetConditionalIntegerArm, TerminalTargetFunction, TerminalTargetIntegerControl,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{
    BlockId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

pub fn lower_to_target_operations(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalTargetOperationPlan {
        terminal_psi: plan.terminal_psi,
        target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| lower_function(function, target))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
) -> Result<TerminalTargetFunction, LoweringError> {
    let mut values = BTreeMap::new();
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    let signature = CallSignature {
        parameters: function
            .parameters
            .iter()
            .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
            .collect::<Result<Vec<_>, _>>()?,
        result: Some(scalar_shape(
            function.result.value,
            function.result.scalar_type,
            false,
        )?),
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != function.parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    for (parameter_index, (parameter, placement)) in function
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .enumerate()
    {
        let location = scalar_parameter_location(parameter, placement)?;
        let value = match parameter.scalar_type {
            ScalarType::Boolean => {
                KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                })
            }
            ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                scalar_type,
                value: KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location,
                }),
            },
        };
        insert_value(&mut values, parameter.value, value)?;
    }

    if function
        .operations
        .iter()
        .any(|operation| matches!(operation, TerminalAbstractOperation::Conditional { .. }))
    {
        return match function.result.scalar_type {
            ScalarType::Integer(_) => lower_integer_conditional(function, &values),
            ScalarType::Boolean => lower_boolean_conditional(function, &values),
        };
    }

    for operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
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
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanNot {
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
            TerminalAbstractOperation::BooleanEqual {
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
            TerminalAbstractOperation::IntegerEqual {
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
            TerminalAbstractOperation::IntegerLessThan {
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
            } => {
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
                    TerminalAbstractOperation::IntegerBitwiseOr { .. } => {
                        IntegerBinaryKind::BitwiseOr
                    }
                    TerminalAbstractOperation::IntegerBitwiseXor { .. } => {
                        IntegerBinaryKind::BitwiseXor
                    }
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
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerWiden {
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
            TerminalAbstractOperation::IntegerExactCast {
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
                        && source_type.can_exact_cast_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        source_type
                            .exact_cast_value_to(*target_type, value)
                            .ok_or(LoweringError::IntegerExactCastTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
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
            TerminalAbstractOperation::ExactIntegerShiftRight {
                psi_operation,
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
            TerminalAbstractOperation::WrappingIntegerAdd {
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
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                }
                let value = match (left, right) {
                    (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                        KnownInteger::Immediate(
                            scalar_type
                                .wrapping_add(left, right)
                                .ok_or(LoweringError::WrappingAddOperandTypeMismatch(*result))?,
                        )
                    }
                    (left, right) => {
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::WrappingAdd {
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
            TerminalAbstractOperation::SaturatingIntegerAdd {
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
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingAdd {
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
            TerminalAbstractOperation::WrappingIntegerSubtract {
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
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingSubtractOperandTypeMismatch(*result));
                }
                let value =
                    match (left, right) {
                        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_sub(left, right).ok_or(
                                LoweringError::WrappingSubtractOperandTypeMismatch(*result),
                            )?)
                        }
                        (left, right) => KnownInteger::Runtime(
                            TerminalTargetIntegerExpression::WrappingSubtract {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            },
                        ),
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
            TerminalAbstractOperation::SaturatingIntegerSubtract {
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
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingSubtract {
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
            TerminalAbstractOperation::WrappingIntegerMultiply {
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
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingMultiplyOperandTypeMismatch(*result));
                }
                let value =
                    match (left, right) {
                        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                            KnownInteger::Immediate(scalar_type.wrapping_mul(left, right).ok_or(
                                LoweringError::WrappingMultiplyOperandTypeMismatch(*result),
                            )?)
                        }
                        (left, right) => KnownInteger::Runtime(
                            TerminalTargetIntegerExpression::WrappingMultiply {
                                psi_operation: *psi_operation,
                                left: Box::new(left.into_expression(left_id)),
                                right: Box::new(right.into_expression(right_id)),
                            },
                        ),
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
            TerminalAbstractOperation::SaturatingIntegerMultiply {
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
                        KnownInteger::Runtime(TerminalTargetIntegerExpression::SaturatingMultiply {
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
            TerminalAbstractOperation::Jump {
                psi_edge, bindings, ..
            } => {
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
            TerminalAbstractOperation::Conditional { .. } => {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            TerminalAbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
            } => {
                if *result != function.result.value || *scalar_type != function.result.scalar_type {
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
                returned = Some(match returned_value {
                    KnownScalar::Boolean(boolean) => {
                        TerminalTargetOperation::ReturnBooleanImmediate {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            value: boolean,
                        }
                    }
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Immediate(integer),
                    } => TerminalTargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value:
                            KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                                parameter_index,
                                location,
                                ..
                            }),
                    } => TerminalTargetOperation::ReturnIntegerParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        parameter_index,
                        location,
                    },
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(expression),
                    } => TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        expression,
                    },
                    KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                        parameter_index,
                        location,
                        ..
                    }) => TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
                    KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Not {
                        operand,
                        ..
                    }) if matches!(*operand, TerminalTargetBooleanExpression::Parameter { .. }) => {
                        let TerminalTargetBooleanExpression::Parameter {
                            parameter_index,
                            location,
                            ..
                        } = *operand
                        else {
                            unreachable!("guard requires a parameter operand")
                        };
                        TerminalTargetOperation::ReturnBooleanNotParameter {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            parameter_index,
                            location,
                        }
                    }
                    KnownScalar::BooleanRuntime(expression) => {
                        TerminalTargetOperation::ReturnBooleanExpression {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            expression,
                        }
                    }
                });
            }
        }
    }

    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}

fn lower_integer_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let ScalarType::Integer(result_type) = function.result.scalar_type else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let lowered = lower_conditional_block(
        function,
        result_type,
        values.clone(),
        function.entry,
        BTreeSet::new(),
    )?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance: conditional_provenance(function, lowered.operations, lowered.edges),
        operation: target_operation_from_integer_control(lowered.control, result_type),
    })
}

fn lower_boolean_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let lowered = lower_boolean_block(function, values.clone(), function.entry, BTreeSet::new())?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance: conditional_provenance(function, lowered.operations, lowered.edges),
        operation: target_operation_from_boolean_control(lowered.control),
    })
}

struct LoweredBooleanArm {
    arm: TerminalTargetConditionalBooleanArm,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_boolean_arm(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    successor: &omega_terminal_abstract_operations::TerminalAbstractSuccessor,
    visited: &BTreeSet<BlockId>,
) -> Result<LoweredBooleanArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_boolean_block(function, values, successor.target, visited.clone())?;
    lowered.edges.insert(0, successor.psi_edge);
    Ok(LoweredBooleanArm {
        arm: TerminalTargetConditionalBooleanArm {
            psi_edge: successor.psi_edge,
            control: Box::new(lowered.control),
        },
        operations: lowered.operations,
        edges: lowered.edges,
    })
}

struct LoweredBooleanControl {
    control: TerminalTargetBooleanControl,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_boolean_block(
    function: &TerminalAbstractFunction,
    mut values: BTreeMap<ValueId, KnownScalar>,
    block: BlockId,
    mut visited: BTreeSet<BlockId>,
) -> Result<LoweredBooleanControl, LoweringError> {
    if !visited.insert(block) {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    }
    let Some((block_index, block_entry)) = function
        .block_entries
        .iter()
        .enumerate()
        .find(|(_, block_entry)| block_entry.block == block)
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let block_end = function
        .block_entries
        .get(block_index + 1)
        .map_or(function.operations.len(), |next| next.operation_offset);
    let Some((terminator, body)) = function
        .operations
        .get(block_entry.operation_offset..block_end)
        .and_then(|operations| operations.split_last())
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let mut operations = Vec::new();
    for operation in body {
        if !lower_conditional_scalar_operation(operation, &mut values, &mut operations)? {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
    }
    match terminator {
        TerminalAbstractOperation::Jump {
            psi_edge,
            target,
            bindings,
        } => {
            bind_conditional_values(&mut values, bindings, *psi_edge)?;
            let mut lowered = lower_boolean_block(function, values, *target, visited)?;
            operations.append(&mut lowered.operations);
            lowered.operations = operations;
            lowered.edges.insert(0, *psi_edge);
            Ok(lowered)
        }
        TerminalAbstractOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => match values
            .get(condition)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*condition))?
        {
            KnownScalar::Boolean(selected_true_arm) => {
                let selected = if selected_true_arm {
                    when_true
                } else {
                    when_false
                };
                let mut lowered = lower_boolean_arm(function, &values, selected, &visited)?;
                operations.append(&mut lowered.operations);
                Ok(LoweredBooleanControl {
                    control: *lowered.arm.control,
                    operations,
                    edges: lowered.edges,
                })
            }
            KnownScalar::BooleanRuntime(expression) => {
                let direct = direct_boolean_condition(expression.clone(), *condition);
                let invert = matches!(direct, Ok((_, _, true)));
                let (selected_true, selected_false) = if invert {
                    (when_false, when_true)
                } else {
                    (when_true, when_false)
                };
                let lowered_true = lower_boolean_arm(function, &values, selected_true, &visited)?;
                let lowered_false = lower_boolean_arm(function, &values, selected_false, &visited)?;
                operations.extend(lowered_true.operations);
                operations.extend(lowered_false.operations);
                let mut edges = lowered_true.edges;
                edges.extend(lowered_false.edges);
                let control = match direct {
                    Ok((parameter_index, location, _)) => {
                        TerminalTargetBooleanControl::Conditional {
                            condition_source: *condition,
                            condition_parameter_index: parameter_index,
                            condition_location: location,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                        TerminalTargetBooleanControl::ConditionalExpression {
                            condition_source: *condition,
                            condition: expression,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(error) => return Err(error),
                };
                Ok(LoweredBooleanControl {
                    control,
                    operations,
                    edges,
                })
            }
            KnownScalar::Integer { .. } => {
                Err(LoweringError::ConditionalConditionMustBeBoolean(*condition))
            }
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
        } => {
            if *result != function.result.value || *scalar_type != ScalarType::Boolean {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            let returned = values
                .get(value)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*value))?;
            let control = match returned {
                KnownScalar::Boolean(returned_value) => {
                    TerminalTargetBooleanControl::ReturnImmediate {
                        psi_return_edge: *psi_edge,
                        source_value: *value,
                        value: returned_value,
                    }
                }
                KnownScalar::BooleanRuntime(expression) => {
                    match direct_boolean_condition(expression.clone(), *value) {
                        Ok((parameter_index, location, invert)) if invert => {
                            TerminalTargetBooleanControl::ReturnNotParameter {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                parameter_index,
                                location,
                            }
                        }
                        Ok((parameter_index, location, _)) => {
                            TerminalTargetBooleanControl::ReturnParameter {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                parameter_index,
                                location,
                            }
                        }
                        Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                            TerminalTargetBooleanControl::ReturnExpression {
                                psi_return_edge: *psi_edge,
                                source_value: *value,
                                expression,
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
                KnownScalar::Integer { .. } => {
                    return Err(LoweringError::ValueTypeMismatch(*value));
                }
            };
            Ok(LoweredBooleanControl {
                control,
                operations,
                edges: vec![*psi_edge],
            })
        }
        _ => Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        )),
    }
}

fn target_operation_from_boolean_control(
    control: TerminalTargetBooleanControl,
) -> TerminalTargetOperation {
    match control {
        TerminalTargetBooleanControl::ReturnImmediate {
            psi_return_edge,
            source_value,
            value,
        } => TerminalTargetOperation::ReturnBooleanImmediate {
            psi_edge: psi_return_edge,
            source_value,
            value,
        },
        TerminalTargetBooleanControl::ReturnParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge: psi_return_edge,
            source_value,
            parameter_index,
            location,
        },
        TerminalTargetBooleanControl::ReturnNotParameter {
            psi_return_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalTargetOperation::ReturnBooleanNotParameter {
            psi_edge: psi_return_edge,
            source_value,
            parameter_index,
            location,
        },
        TerminalTargetBooleanControl::ReturnExpression {
            psi_return_edge,
            source_value,
            expression,
        } => TerminalTargetOperation::ReturnBooleanExpression {
            psi_edge: psi_return_edge,
            source_value,
            expression,
        },
        TerminalTargetBooleanControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnBooleanConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        },
        TerminalTargetBooleanControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnBooleanExpressionConditionalControl {
            condition_source,
            condition,
            when_true,
            when_false,
        },
    }
}

struct LoweredConditionalArm {
    arm: TerminalTargetConditionalIntegerArm,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_conditional_arm(
    function: &TerminalAbstractFunction,
    result_type: IntegerType,
    values: &BTreeMap<ValueId, KnownScalar>,
    successor: &omega_terminal_abstract_operations::TerminalAbstractSuccessor,
    visited: &BTreeSet<BlockId>,
) -> Result<LoweredConditionalArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_conditional_block(
        function,
        result_type,
        values,
        successor.target,
        visited.clone(),
    )?;
    lowered.edges.insert(0, successor.psi_edge);
    Ok(LoweredConditionalArm {
        arm: TerminalTargetConditionalIntegerArm {
            psi_edge: successor.psi_edge,
            control: Box::new(lowered.control),
        },
        operations: lowered.operations,
        edges: lowered.edges,
    })
}

struct LoweredIntegerControl {
    control: TerminalTargetIntegerControl,
    operations: Vec<OperationId>,
    edges: Vec<EdgeId>,
}

fn lower_conditional_block(
    function: &TerminalAbstractFunction,
    result_type: IntegerType,
    mut values: BTreeMap<ValueId, KnownScalar>,
    block: BlockId,
    mut visited: BTreeSet<BlockId>,
) -> Result<LoweredIntegerControl, LoweringError> {
    if !visited.insert(block) {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    }
    let Some((block_index, block_entry)) = function
        .block_entries
        .iter()
        .enumerate()
        .find(|(_, block_entry)| block_entry.block == block)
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let block_end = function
        .block_entries
        .get(block_index + 1)
        .map_or(function.operations.len(), |next| next.operation_offset);
    let Some((terminator, body)) = function
        .operations
        .get(block_entry.operation_offset..block_end)
        .and_then(|operations| operations.split_last())
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let mut operations = Vec::new();
    for operation in body {
        if !lower_conditional_scalar_operation(operation, &mut values, &mut operations)? {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
    }
    match terminator {
        TerminalAbstractOperation::Jump {
            psi_edge,
            target,
            bindings,
        } => {
            bind_conditional_values(&mut values, bindings, *psi_edge)?;
            let mut lowered =
                lower_conditional_block(function, result_type, values, *target, visited)?;
            operations.append(&mut lowered.operations);
            lowered.operations = operations;
            lowered.edges.insert(0, *psi_edge);
            Ok(lowered)
        }
        TerminalAbstractOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => match values
            .get(condition)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*condition))?
        {
            KnownScalar::Boolean(selected_true_arm) => {
                let selected = if selected_true_arm {
                    when_true
                } else {
                    when_false
                };
                let mut lowered =
                    lower_conditional_arm(function, result_type, &values, selected, &visited)?;
                operations.append(&mut lowered.operations);
                Ok(LoweredIntegerControl {
                    control: *lowered.arm.control,
                    operations,
                    edges: lowered.edges,
                })
            }
            KnownScalar::BooleanRuntime(expression) => {
                let direct = direct_boolean_condition(expression.clone(), *condition);
                let invert = matches!(direct, Ok((_, _, true)));
                let (selected_true, selected_false) = if invert {
                    (when_false, when_true)
                } else {
                    (when_true, when_false)
                };
                let lowered_true =
                    lower_conditional_arm(function, result_type, &values, selected_true, &visited)?;
                let lowered_false = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected_false,
                    &visited,
                )?;
                operations.extend(lowered_true.operations);
                operations.extend(lowered_false.operations);
                let mut edges = lowered_true.edges;
                edges.extend(lowered_false.edges);
                let control = match direct {
                    Ok((parameter_index, location, _)) => {
                        TerminalTargetIntegerControl::Conditional {
                            condition_source: *condition,
                            condition_parameter_index: parameter_index,
                            condition_location: location,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(LoweringError::UnsupportedRuntimeBooleanCondition(_)) => {
                        TerminalTargetIntegerControl::ConditionalExpression {
                            condition_source: *condition,
                            condition: expression,
                            when_true: lowered_true.arm,
                            when_false: lowered_false.arm,
                        }
                    }
                    Err(error) => return Err(error),
                };
                Ok(LoweredIntegerControl {
                    control,
                    operations,
                    edges,
                })
            }
            KnownScalar::Integer { .. } => {
                Err(LoweringError::ConditionalConditionMustBeBoolean(*condition))
            }
        },
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
        } => {
            if *result != function.result.value || *scalar_type != function.result.scalar_type {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
            let KnownScalar::Integer {
                scalar_type: returned_type,
                value: returned,
            } = values
                .get(value)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*value))?
            else {
                return Err(LoweringError::ValueTypeMismatch(*value));
            };
            if returned_type != result_type {
                return Err(LoweringError::ValueTypeMismatch(*value));
            }
            Ok(LoweredIntegerControl {
                control: TerminalTargetIntegerControl::Return {
                    psi_return_edge: *psi_edge,
                    source_value: *value,
                    expression: returned.into_expression(*value),
                },
                operations,
                edges: vec![*psi_edge],
            })
        }
        _ => Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        )),
    }
}

fn bind_conditional_values(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    bindings: &[omega_terminal_abstract_operations::TerminalValueBinding],
    edge: EdgeId,
) -> Result<(), LoweringError> {
    let pending = bindings
        .iter()
        .map(|binding| {
            let value = values
                .get(&binding.argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(binding.argument))?;
            if binding.scalar_type != value.scalar_type() {
                return Err(LoweringError::ConditionalArmBindingTypeMismatch(edge));
            }
            Ok((
                binding.parameter,
                value.rebind_direct_parameter(binding.parameter),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (parameter, value) in pending {
        insert_value(values, parameter, value)?;
    }
    Ok(())
}

fn target_operation_from_integer_control(
    control: TerminalTargetIntegerControl,
    scalar_type: IntegerType,
) -> TerminalTargetOperation {
    match control {
        TerminalTargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        } => match expression {
            TerminalTargetIntegerExpression::Immediate { value, .. } => {
                TerminalTargetOperation::ReturnIntegerImmediate {
                    psi_edge: psi_return_edge,
                    source_value,
                    scalar_type,
                    value,
                }
            }
            TerminalTargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            } => TerminalTargetOperation::ReturnIntegerParameter {
                psi_edge: psi_return_edge,
                source_value,
                scalar_type,
                parameter_index,
                location,
            },
            expression => TerminalTargetOperation::ReturnIntegerExpression {
                psi_edge: psi_return_edge,
                source_value,
                scalar_type,
                expression,
            },
        },
        TerminalTargetIntegerControl::Conditional {
            condition_source,
            condition_parameter_index,
            condition_location,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnIntegerConditionalControl {
            condition_source,
            condition_parameter_index,
            condition_location,
            scalar_type,
            when_true,
            when_false,
        },
        TerminalTargetIntegerControl::ConditionalExpression {
            condition_source,
            condition,
            when_true,
            when_false,
        } => TerminalTargetOperation::ReturnIntegerExpressionConditionalControl {
            condition_source,
            condition,
            scalar_type,
            when_true,
            when_false,
        },
    }
}

fn lower_conditional_scalar_operation(
    operation: &TerminalAbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
) -> Result<bool, LoweringError> {
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
            let value = match operand_value {
                KnownInteger::Immediate(value) => KnownInteger::Immediate(
                    source_type
                        .exact_cast_value_to(*target_type, value)
                        .ok_or(LoweringError::IntegerExactCastTypeMismatch(*result))?,
                ),
                KnownInteger::Runtime(expression) => {
                    KnownInteger::Runtime(TerminalTargetIntegerExpression::IntegerExactCast {
                        psi_operation: *psi_operation,
                        source_type: *source_type,
                        operand: Box::new(expression),
                    })
                }
            };
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
            )?,
        ),
        _ => return Ok(false),
    };
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.push(psi_operation);
    Ok(true)
}

#[derive(Clone, Copy)]
enum IntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}

#[derive(Clone, Copy)]
enum WrappingShiftKind {
    Left,
    Right,
}

#[allow(clippy::too_many_arguments)]
fn lower_wrapping_shift(
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
fn lower_exact_shift_right(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    psi_operation: psi_core::OperationId,
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
    Ok(match (value, count) {
        (KnownInteger::Immediate(value), KnownInteger::Immediate(count)) => {
            KnownInteger::Immediate(
                value_type
                    .exact_shift_right(value, count_type, count)
                    .ok_or(LoweringError::ExactShiftOperandTypeMismatch(result))?,
            )
        }
        (value, count) => KnownInteger::Runtime(TerminalTargetIntegerExpression::ExactShiftRight {
            psi_operation,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        }),
    })
}

fn lower_conditional_integer_binary(
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
            Self::WrappingAdd => LoweringError::WrappingAddOperandTypeMismatch(result),
            Self::SaturatingAdd => LoweringError::SaturatingAddOperandTypeMismatch(result),
            Self::WrappingSubtract => LoweringError::WrappingSubtractOperandTypeMismatch(result),
            Self::SaturatingSubtract => {
                LoweringError::SaturatingSubtractOperandTypeMismatch(result)
            }
            Self::WrappingMultiply => LoweringError::WrappingMultiplyOperandTypeMismatch(result),
            Self::SaturatingMultiply => {
                LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
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
            Self::SaturatingAdd => scalar_type.saturating_add(left, right),
            Self::WrappingSubtract => scalar_type.wrapping_sub(left, right),
            Self::SaturatingSubtract => scalar_type.saturating_sub(left, right),
            Self::WrappingMultiply => scalar_type.wrapping_mul(left, right),
            Self::SaturatingMultiply => scalar_type.saturating_mul(left, right),
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
            Self::SaturatingMultiply => TerminalTargetIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
        }
    }
}

fn scalar_shape(
    value: ValueId,
    scalar_type: ScalarType,
    require_native_parameter: bool,
) -> Result<ValueShape, LoweringError> {
    let bytes = match scalar_type {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer_type) => {
            let bits = integer_type.bits();
            if require_native_parameter && !matches!(bits, 8 | 16 | 32 | 64) {
                return Err(LoweringError::ParameterWidthNotNativelySupported { value, bits });
            }
            bits.div_ceil(8)
        }
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

fn scalar_parameter_location(
    parameter: &TerminalAbstractParameter,
    placement: &ValuePlacement,
) -> Result<TerminalScalarParameterLocation, LoweringError> {
    let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => {
            Ok(TerminalScalarParameterLocation::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(TerminalScalarParameterLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }),
        _ => Err(LoweringError::UnsupportedScalarParameterPlacement(
            parameter.value,
        )),
    }
}

fn insert_value(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    id: ValueId,
    value: KnownScalar,
) -> Result<(), LoweringError> {
    if values.insert(id, value).is_some() {
        return Err(LoweringError::DuplicateValue(id));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KnownScalar {
    Boolean(bool),
    BooleanRuntime(TerminalTargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        value: KnownInteger,
    },
}

impl KnownScalar {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::BooleanRuntime(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }

    fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Integer { scalar_type, value } => Self::Integer {
                scalar_type,
                value: value.rebind_direct_parameter(source_value),
            },
            Self::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::BooleanRuntime(TerminalTargetBooleanExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value @ (Self::Boolean(_) | Self::BooleanRuntime(_)) => value,
        }
    }
}

fn negate_boolean(
    value: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match value {
        KnownScalar::Boolean(value) => Ok(KnownScalar::Boolean(!value)),
        KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Not { operand, .. }) => {
            Ok(KnownScalar::BooleanRuntime(*operand))
        }
        KnownScalar::BooleanRuntime(expression) => Ok(KnownScalar::BooleanRuntime(
            TerminalTargetBooleanExpression::Not {
                psi_operation,
                operand: Box::new(expression),
            },
        )),
        KnownScalar::Integer { .. } => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

fn equal_boolean(
    left: KnownScalar,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match (left, right) {
        (KnownScalar::Boolean(left), KnownScalar::Boolean(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (value, KnownScalar::Boolean(true)) | (KnownScalar::Boolean(true), value) => Ok(value),
        (value, KnownScalar::Boolean(false)) | (KnownScalar::Boolean(false), value) => {
            negate_boolean(value, psi_operation, result)
        }
        (KnownScalar::BooleanRuntime(left), KnownScalar::BooleanRuntime(right)) => Ok(
            KnownScalar::BooleanRuntime(TerminalTargetBooleanExpression::Equal {
                psi_operation,
                left: Box::new(left),
                right: Box::new(right),
            }),
        ),
        _ => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

fn equal_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
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
        return Err(LoweringError::ValueTypeMismatch(result));
    };
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (left, right) => Ok(KnownScalar::BooleanRuntime(
            TerminalTargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type: left_type,
                left: Box::new(left.into_expression(left_id)),
                right: Box::new(right.into_expression(right_id)),
            },
        )),
    }
}

fn order_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
    inclusive: bool,
) -> Result<KnownScalar, LoweringError> {
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
        return Err(LoweringError::ValueTypeMismatch(result));
    };
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            let ordering = left_type
                .compare(left, right)
                .ok_or(LoweringError::ValueTypeMismatch(result))?;
            Ok(KnownScalar::Boolean(if inclusive {
                !ordering.is_gt()
            } else {
                ordering.is_lt()
            }))
        }
        (left, right) => {
            let left = Box::new(left.into_expression(left_id));
            let right = Box::new(right.into_expression(right_id));
            Ok(KnownScalar::BooleanRuntime(if inclusive {
                TerminalTargetBooleanExpression::IntegerLessOrEqual {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            } else {
                TerminalTargetBooleanExpression::IntegerLessThan {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            }))
        }
    }
}

fn direct_boolean_condition(
    expression: TerminalTargetBooleanExpression,
    value: ValueId,
) -> Result<(usize, TerminalScalarParameterLocation, bool), LoweringError> {
    match expression {
        TerminalTargetBooleanExpression::Parameter {
            parameter_index,
            location,
            ..
        } => Ok((parameter_index, location, false)),
        TerminalTargetBooleanExpression::Not { operand, .. } => match *operand {
            TerminalTargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            } => Ok((parameter_index, location, true)),
            _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
        },
        _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KnownInteger {
    Immediate(IntegerValue),
    Runtime(TerminalTargetIntegerExpression),
}

impl KnownInteger {
    fn into_expression(self, source_value: ValueId) -> TerminalTargetIntegerExpression {
        match self {
            Self::Immediate(value) => TerminalTargetIntegerExpression::Immediate {
                source_value,
                value,
            },
            Self::Runtime(expression) => expression,
        }
    }

    fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Runtime(TerminalTargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::Runtime(TerminalTargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value => value,
        }
    }
}

fn conditional_provenance(
    function: &TerminalAbstractFunction,
    operations: Vec<psi_core::OperationId>,
    edges: Vec<psi_core::EdgeId>,
) -> TerminalPsiProvenance {
    let mut operations = operations.into_iter().collect::<BTreeSet<_>>();
    let mut edges = edges.into_iter().collect::<BTreeSet<_>>();
    let mut provenance = TerminalPsiProvenance::default();
    for operation in &function.operations {
        let psi_operation = match operation {
            TerminalAbstractOperation::IntegerConstant { psi_operation, .. }
            | TerminalAbstractOperation::BooleanConstant { psi_operation, .. }
            | TerminalAbstractOperation::BooleanNot { psi_operation, .. }
            | TerminalAbstractOperation::BooleanEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerLessThan { psi_operation, .. }
            | TerminalAbstractOperation::IntegerLessOrEqual { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseNot { psi_operation, .. }
            | TerminalAbstractOperation::IntegerWiden { psi_operation, .. }
            | TerminalAbstractOperation::IntegerExactCast { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseAnd { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseOr { psi_operation, .. }
            | TerminalAbstractOperation::IntegerBitwiseXor { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerShiftLeft { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerShiftRight { psi_operation, .. }
            | TerminalAbstractOperation::ExactIntegerShiftRight { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
            | TerminalAbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
            | TerminalAbstractOperation::SaturatingIntegerMultiply { psi_operation, .. } => {
                Some(*psi_operation)
            }
            TerminalAbstractOperation::Jump { .. }
            | TerminalAbstractOperation::Conditional { .. }
            | TerminalAbstractOperation::Return { .. } => None,
        };
        if let Some(psi_operation) = psi_operation
            && operations.remove(&psi_operation)
        {
            provenance.operations.push(psi_operation);
        }
        match operation {
            TerminalAbstractOperation::Jump { psi_edge, .. }
            | TerminalAbstractOperation::Return { psi_edge, .. } => {
                if edges.remove(psi_edge) {
                    provenance.edges.push(*psi_edge);
                }
            }
            TerminalAbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for psi_edge in [when_true.psi_edge, when_false.psi_edge] {
                    if edges.remove(&psi_edge) {
                        provenance.edges.push(psi_edge);
                    }
                }
            }
            _ => {}
        }
    }
    debug_assert!(operations.is_empty());
    debug_assert!(edges.is_empty());
    provenance
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EntryFunctionMissing(MachineId),
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    FunctionResultMismatch(MachineId),
    ConditionalControlFlowRequiresBlockLowering(MachineId),
    ConditionalConditionMustBeBoolean(ValueId),
    ConditionalArmBindingTypeMismatch(psi_core::EdgeId),
    DuplicateValue(ValueId),
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    UnsupportedRuntimeBooleanCondition(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
    IntegerBitwiseOperandTypeMismatch(ValueId),
    IntegerWidenTypeMismatch(ValueId),
    IntegerExactCastTypeMismatch(ValueId),
    WrappingShiftOperandTypeMismatch(ValueId),
    ExactShiftOperandTypeMismatch(ValueId),
    WrappingAddOperandTypeMismatch(ValueId),
    SaturatingAddOperandTypeMismatch(ValueId),
    WrappingSubtractOperandTypeMismatch(ValueId),
    SaturatingSubtractOperandTypeMismatch(ValueId),
    WrappingMultiplyOperandTypeMismatch(ValueId),
    SaturatingMultiplyOperandTypeMismatch(ValueId),
    ParameterWidthNotNativelySupported { value: ValueId, bits: u16 },
    UnsupportedScalarParameterPlacement(ValueId),
    AbiPlan(PlanDiagnostic),
    AbiParameterCountMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
        TerminalAbstractParameter, TerminalAbstractResult, TerminalAbstractSuccessor,
        TerminalValueBinding,
    };
    use omega_terminal_target_operations::MachineRegister;
    use psi_core::{BlockId, EdgeId};
    use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

    #[test]
    fn refuses_a_return_whose_value_was_never_materialized() {
        let machine = MachineId::new(1).expect("machine");
        let unknown = ValueId::new(1).expect("unknown value");
        let result = ValueId::new(2).expect("result");
        let i32_type = IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(1).expect("block"),
                parameters: Vec::new(),
                result: TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(i32_type),
                },
                block_entries: Vec::new(),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    result,
                    value: unknown,
                    scalar_type: ScalarType::Integer(i32_type),
                }],
            }],
        };

        assert_eq!(
            lower_to_target_operations(&plan, NativeTarget::linux_x64()),
            Err(LoweringError::UnknownValue(unknown))
        );
    }

    #[test]
    fn selects_native_register_and_stack_locations_for_runtime_parameters() {
        let register_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ),
        ];
        for (target, expected) in register_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(1), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 0,
                    location,
                    ..
                } if location == expected
            ));
        }

        let stack_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ];
        for (target, expected) in stack_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(9), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 8,
                    location,
                    ..
                } if location == expected
            ));
        }
    }

    #[test]
    fn lowers_runtime_parameter_arithmetic_to_a_typed_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        let sum = ValueId::new(50).expect("sum");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.insert(
            0,
            TerminalAbstractOperation::WrappingIntegerAdd {
                psi_operation: psi_core::OperationId::new(50).expect("operation"),
                result: sum,
                scalar_type,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return { value, .. } = &mut function.operations[1] else {
            unreachable!("fixture ends in return")
        };
        *value = sum;

        let lowered = lower_to_target_operations(&plan, NativeTarget::host()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerExpression {
                source_value,
                scalar_type: result_type,
                expression: TerminalTargetIntegerExpression::WrappingAdd {
                    psi_operation,
                    left,
                    right,
                },
                ..
            } if *source_value == sum
                && *result_type == scalar_type
                && *psi_operation == psi_core::OperationId::new(50).expect("operation")
                && matches!(
                    left.as_ref(),
                    TerminalTargetIntegerExpression::Parameter {
                        parameter_index: 0,
                        ..
                    }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetIntegerExpression::Parameter {
                        parameter_index: 1,
                        ..
                    }
                )
        ));
    }

    #[test]
    fn folds_closed_wrapping_subtraction_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let difference = ValueId::new(52).expect("difference");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(5),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(10),
                },
                TerminalAbstractOperation::WrappingIntegerSubtract {
                    psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                    result: difference,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = difference;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(251),
                ..
            } if source_value == difference && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_saturating_subtraction_at_zero() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let difference = ValueId::new(52).expect("difference");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(5),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(10),
                },
                TerminalAbstractOperation::SaturatingIntegerSubtract {
                    psi_operation: psi_core::OperationId::new(52).expect("subtract operation"),
                    result: difference,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = difference;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(0),
                ..
            } if source_value == difference && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_wrapping_multiplication_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let product = ValueId::new(52).expect("product");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(20),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(13),
                },
                TerminalAbstractOperation::WrappingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                    result: product,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = product;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(4),
                ..
            } if source_value == product && result_type == scalar_type
        ));
    }

    #[test]
    fn folds_closed_saturating_multiplication_at_the_declared_width() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let left = ValueId::new(50).expect("left");
        let right = ValueId::new(51).expect("right");
        let product = ValueId::new(52).expect("product");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.splice(
            0..0,
            [
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(50).expect("left operation"),
                    result: left,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(20),
                },
                TerminalAbstractOperation::IntegerConstant {
                    psi_operation: psi_core::OperationId::new(51).expect("right operation"),
                    result: right,
                    scalar_type: ScalarType::Integer(scalar_type),
                    value: IntegerValue::Unsigned(13),
                },
                TerminalAbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: psi_core::OperationId::new(52).expect("multiply operation"),
                    result: product,
                    scalar_type,
                    left,
                    right,
                },
            ],
        );
        let TerminalAbstractOperation::Return { value, .. } =
            function.operations.last_mut().expect("return")
        else {
            unreachable!("fixture ends in return")
        };
        *value = product;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnIntegerImmediate {
                source_value,
                scalar_type: result_type,
                value: IntegerValue::Unsigned(255),
                ..
            } if source_value == product && result_type == scalar_type
        ));
    }

    #[test]
    fn lowers_a_boolean_runtime_parameter_with_its_selected_abi_location() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        function.parameters[0].scalar_type = ScalarType::Boolean;
        function.result.scalar_type = ScalarType::Boolean;
        let TerminalAbstractOperation::Return { scalar_type, .. } = &mut function.operations[0]
        else {
            unreachable!("fixture ends in return")
        };
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanParameter {
                parameter_index: 0,
                location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
    }

    #[test]
    fn lowers_runtime_boolean_equality_to_a_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        for parameter in &mut function.parameters {
            parameter.scalar_type = ScalarType::Boolean;
        }
        function.result.scalar_type = ScalarType::Boolean;
        let result = ValueId::new(50).expect("equality result");
        function.operations.insert(
            0,
            TerminalAbstractOperation::BooleanEqual {
                psi_operation: OperationId::new(50).expect("equality operation"),
                result,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return {
            value, scalar_type, ..
        } = &mut function.operations[1]
        else {
            unreachable!("fixture ends in return")
        };
        *value = result;
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                source_value,
                expression: TerminalTargetBooleanExpression::Equal {
                    psi_operation,
                    left,
                    right,
                },
                ..
            } if *source_value == result
                && *psi_operation == OperationId::new(50).expect("equality operation")
                && matches!(
                    left.as_ref(),
                    TerminalTargetBooleanExpression::Parameter { parameter_index: 0, .. }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetBooleanExpression::Parameter { parameter_index: 1, .. }
                )
        ));
    }

    #[test]
    fn lowers_runtime_integer_equality_to_a_typed_target_expression() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        let integer_type = match function.parameters[0].scalar_type {
            ScalarType::Integer(integer_type) => integer_type,
            ScalarType::Boolean => unreachable!("fixture has integer parameters"),
        };
        function.result.scalar_type = ScalarType::Boolean;
        let result = ValueId::new(51).expect("integer-equality result");
        function.operations.insert(
            0,
            TerminalAbstractOperation::IntegerEqual {
                psi_operation: OperationId::new(51).expect("integer-equality operation"),
                result,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return {
            value, scalar_type, ..
        } = &mut function.operations[1]
        else {
            unreachable!("fixture ends in return")
        };
        *value = result;
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            &lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanExpression {
                source_value,
                expression: TerminalTargetBooleanExpression::IntegerEqual {
                    psi_operation,
                    scalar_type,
                    left,
                    right,
                },
                ..
            } if *source_value == result
                && *psi_operation == OperationId::new(51).expect("integer-equality operation")
                && *scalar_type == integer_type
                && matches!(
                    left.as_ref(),
                    TerminalTargetIntegerExpression::Parameter { parameter_index: 0, .. }
                )
                && matches!(
                    right.as_ref(),
                    TerminalTargetIntegerExpression::Parameter { parameter_index: 1, .. }
                )
        ));
    }

    #[test]
    fn folds_a_compile_known_conditional_to_only_the_selected_arm() {
        let condition_operation = psi_core::OperationId::new(20).expect("condition operation");
        let true_operation = psi_core::OperationId::new(21).expect("true operation");
        let false_operation = psi_core::OperationId::new(22).expect("false operation");
        let true_edge = EdgeId::new(1).expect("true edge");
        let false_edge = EdgeId::new(2).expect("false edge");
        let true_return = EdgeId::new(3).expect("true return");
        let false_return = EdgeId::new(4).expect("false return");

        for (select_true, selected_operation, selected_edges) in [
            (true, true_operation, [true_edge, true_return]),
            (false, false_operation, [false_edge, false_return]),
        ] {
            let plan = constant_conditional_plan(select_true);
            let lowered =
                lower_to_target_operations(&plan, NativeTarget::linux_x64()).expect("lower");
            let function = &lowered.functions[0];
            assert_eq!(
                function.provenance.operations,
                [condition_operation, selected_operation]
            );
            assert_eq!(function.provenance.edges, selected_edges);
            assert!(
                matches!(
                    &function.operation,
                    TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge,
                        expression:
                            TerminalTargetIntegerExpression::WrappingAdd { psi_operation, .. },
                        ..
                    } if select_true && *psi_edge == true_return && *psi_operation == true_operation
                ) || matches!(
                    &function.operation,
                    TerminalTargetOperation::ReturnIntegerExpression {
                        psi_edge,
                        expression:
                            TerminalTargetIntegerExpression::SaturatingMultiply {
                                psi_operation,
                                ..
                            },
                        ..
                    } if !select_true && *psi_edge == false_return && *psi_operation == false_operation
                )
            );
        }
    }

    fn constant_conditional_plan(select_true: bool) -> TerminalAbstractOperationPlan {
        let machine = MachineId::new(20).expect("machine");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let argument = ValueId::new(1).expect("argument");
        let condition = ValueId::new(2).expect("condition");
        let true_parameter = ValueId::new(3).expect("true parameter");
        let false_parameter = ValueId::new(4).expect("false parameter");
        let true_value = ValueId::new(5).expect("true value");
        let false_value = ValueId::new(6).expect("false value");
        let result = ValueId::new(7).expect("result");
        let true_edge = EdgeId::new(1).expect("true edge");
        let false_edge = EdgeId::new(2).expect("false edge");
        let true_return = EdgeId::new(3).expect("true return");
        let false_return = EdgeId::new(4).expect("false return");
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(1).expect("entry block"),
                parameters: vec![TerminalAbstractParameter {
                    value: argument,
                    scalar_type,
                }],
                result: TerminalAbstractResult {
                    value: result,
                    scalar_type,
                },
                block_entries: vec![
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(1).expect("entry block"),
                        operation_offset: 0,
                    },
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(2).expect("true block"),
                        operation_offset: 2,
                    },
                    omega_terminal_abstract_operations::TerminalAbstractBlockEntry {
                        block: BlockId::new(3).expect("false block"),
                        operation_offset: 4,
                    },
                ],
                operations: vec![
                    TerminalAbstractOperation::BooleanConstant {
                        psi_operation: psi_core::OperationId::new(20).expect("condition operation"),
                        result: condition,
                        value: select_true,
                    },
                    TerminalAbstractOperation::Conditional {
                        condition,
                        when_true: TerminalAbstractSuccessor {
                            psi_edge: true_edge,
                            target: BlockId::new(2).expect("true block"),
                            bindings: vec![TerminalValueBinding {
                                parameter: true_parameter,
                                argument,
                                scalar_type,
                            }],
                        },
                        when_false: TerminalAbstractSuccessor {
                            psi_edge: false_edge,
                            target: BlockId::new(3).expect("false block"),
                            bindings: vec![TerminalValueBinding {
                                parameter: false_parameter,
                                argument,
                                scalar_type,
                            }],
                        },
                    },
                    TerminalAbstractOperation::WrappingIntegerAdd {
                        psi_operation: psi_core::OperationId::new(21).expect("true operation"),
                        result: true_value,
                        scalar_type: integer,
                        left: true_parameter,
                        right: true_parameter,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: true_return,
                        result,
                        value: true_value,
                        scalar_type,
                    },
                    TerminalAbstractOperation::SaturatingIntegerMultiply {
                        psi_operation: psi_core::OperationId::new(22).expect("false operation"),
                        result: false_value,
                        scalar_type: integer,
                        left: false_parameter,
                        right: false_parameter,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: false_return,
                        result,
                        value: false_value,
                        scalar_type,
                    },
                ],
            }],
        }
    }

    fn parameter_return_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
        let machine = MachineId::new(10).expect("machine");
        let result = ValueId::new(100).expect("result");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let parameters = (0..parameter_count)
            .map(|index| TerminalAbstractParameter {
                value: ValueId::new(10 + index as u64).expect("parameter"),
                scalar_type,
            })
            .collect::<Vec<_>>();
        let returned = parameters.last().expect("fixture has parameters").value;
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(10).expect("block"),
                parameters,
                result: TerminalAbstractResult {
                    value: result,
                    scalar_type,
                },
                block_entries: Vec::new(),
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(10).expect("edge"),
                    result,
                    value: returned,
                    scalar_type,
                }],
            }],
        }
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            semantic_version: SemanticVersion::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }
}
