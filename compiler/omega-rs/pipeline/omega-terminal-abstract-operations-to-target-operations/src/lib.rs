#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::BTreeMap;

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
    TerminalPsiProvenance, TerminalScalarParameterLocation,
    TerminalTargetConditionalIntegerExpression, TerminalTargetFunction,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{IntegerType, IntegerValue, MachineId, ScalarType, ValueId};

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
            ScalarType::Boolean => KnownScalar::BooleanParameter {
                parameter_index,
                location,
            },
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
        return lower_integer_conditional(function, &values);
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
                    KnownScalar::BooleanParameter {
                        parameter_index,
                        location,
                    } => TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
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
    let [entry, _, _] = function.block_entries.as_slice() else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    if entry.block != function.entry || entry.operation_offset != 0 {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    }
    let TerminalAbstractOperation::Conditional {
        condition,
        when_true,
        when_false,
    } = function.operations.first().ok_or(
        LoweringError::ConditionalControlFlowRequiresBlockLowering(function.machine),
    )?
    else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };
    let KnownScalar::BooleanParameter {
        parameter_index: condition_parameter_index,
        location: condition_location,
    } = values
        .get(condition)
        .ok_or(LoweringError::UnknownValue(*condition))?
    else {
        return Err(LoweringError::ConditionalConditionMustBeRuntimeParameter(
            *condition,
        ));
    };
    let ScalarType::Integer(result_type) = function.result.scalar_type else {
        return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        ));
    };

    let lower_arm = |successor: &omega_terminal_abstract_operations::TerminalAbstractSuccessor| -> Result<
        (
            TerminalTargetConditionalIntegerExpression,
            Vec<psi_core::OperationId>,
        ),
        LoweringError,
    > {
        let Some((entry_index, block_entry)) = function
            .block_entries
            .iter()
            .enumerate()
            .find(|(_, block_entry)| block_entry.block == successor.target)
        else {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        };
        let block_end = function
            .block_entries
            .get(entry_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        let operations = &function.operations[block_entry.operation_offset..block_end];
        let Some((
            TerminalAbstractOperation::Return {
                psi_edge: psi_return_edge,
                result,
                value,
                scalar_type,
            },
            body,
        )) = operations.split_last()
        else {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        };
        let [binding] = successor.bindings.as_slice() else {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        };
        if *result != function.result.value
            || *scalar_type != function.result.scalar_type
            || binding.scalar_type != function.result.scalar_type
        {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
        let KnownScalar::Integer {
            scalar_type: argument_type,
            value:
                KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                    parameter_index,
                    location,
                    ..
                }),
        } = values
            .get(&binding.argument)
            .ok_or(LoweringError::UnknownValue(binding.argument))?
        else {
            return Err(LoweringError::ConditionalArmMustBindRuntimeParameter(
                successor.psi_edge,
            ));
        };
        if *argument_type != result_type {
            return Err(LoweringError::ValueTypeMismatch(binding.argument));
        }
        let mut arm_values = BTreeMap::new();
        insert_value(
            &mut arm_values,
            binding.parameter,
            KnownScalar::Integer {
                scalar_type: result_type,
                value: KnownInteger::Runtime(TerminalTargetIntegerExpression::Parameter {
                    source_value: binding.parameter,
                    parameter_index: *parameter_index,
                    location: *location,
                }),
            },
        )?;
        let mut operations_provenance = Vec::new();
        for operation in body {
            if !lower_conditional_integer_operation(
                operation,
                &mut arm_values,
                &mut operations_provenance,
            )? {
                return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                    function.machine,
                ));
            }
        }
        let KnownScalar::Integer {
            scalar_type: returned_type,
            value: returned,
        } = arm_values
            .get(value)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*value))?
        else {
            return Err(LoweringError::ValueTypeMismatch(*value));
        };
        if returned_type != result_type {
            return Err(LoweringError::ValueTypeMismatch(*value));
        }
        Ok((
            TerminalTargetConditionalIntegerExpression {
                psi_edge: successor.psi_edge,
                psi_return_edge: *psi_return_edge,
                source_value: *value,
                expression: returned.into_expression(*value),
            },
            operations_provenance,
        ))
    };
    let (when_true, mut true_operations) = lower_arm(when_true)?;
    let (when_false, false_operations) = lower_arm(when_false)?;
    true_operations.extend(false_operations);
    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance: TerminalPsiProvenance {
            operations: true_operations,
            edges: vec![
                when_true.psi_edge,
                when_false.psi_edge,
                when_true.psi_return_edge,
                when_false.psi_return_edge,
            ],
        },
        operation: TerminalTargetOperation::ReturnIntegerConditionalExpressions {
            condition_source: *condition,
            condition_parameter_index: *condition_parameter_index,
            condition_location: *condition_location,
            scalar_type: result_type,
            when_true,
            when_false,
        },
    })
}

fn lower_conditional_integer_operation(
    operation: &TerminalAbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
) -> Result<bool, LoweringError> {
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
        _ => return Ok(false),
    };
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.push(psi_operation);
    Ok(true)
}

#[derive(Clone, Copy)]
enum IntegerBinaryKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
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
    Integer {
        scalar_type: IntegerType,
        value: KnownInteger,
    },
    BooleanParameter {
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
}

impl KnownScalar {
    const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
            Self::BooleanParameter { .. } => ScalarType::Boolean,
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EntryFunctionMissing(MachineId),
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    FunctionResultMismatch(MachineId),
    ConditionalControlFlowRequiresBlockLowering(MachineId),
    ConditionalConditionMustBeRuntimeParameter(ValueId),
    ConditionalArmMustBindRuntimeParameter(psi_core::EdgeId),
    DuplicateValue(ValueId),
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
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
        TerminalAbstractParameter, TerminalAbstractResult,
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
