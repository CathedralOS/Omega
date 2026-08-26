use super::*;

pub(super) fn lower_integer_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let function_result = scalar_function_result(function)?;
    let ScalarType::Integer(result_type) = function_result.scalar_type else {
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
        target,
        functions,
    )?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: conditional_provenance(function, lowered.operations, lowered.edges),
        operation: target_operation_from_integer_control(lowered.control, result_type),
    })
}

pub(super) fn lower_boolean_conditional(
    function: &TerminalAbstractFunction,
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<TerminalTargetFunction, LoweringError> {
    let lowered = lower_boolean_block(
        function,
        values.clone(),
        function.entry,
        BTreeSet::new(),
        target,
        functions,
        &[],
        &BTreeMap::new(),
    )?;
    Ok(TerminalTargetFunction {
        machine: function.machine,
        attachment: function.attachment,
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
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<LoweredBooleanArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_boolean_block(
        function,
        values,
        successor.target,
        visited.clone(),
        target,
        functions,
        structural_parameters,
        structural_types,
    )?;
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

pub(super) struct LoweredBooleanControl {
    pub(super) control: TerminalTargetBooleanControl,
    pub(super) operations: Vec<OperationId>,
    pub(super) edges: Vec<EdgeId>,
}

pub(super) fn lower_boolean_block(
    function: &TerminalAbstractFunction,
    mut values: BTreeMap<ValueId, KnownScalar>,
    block: BlockId,
    mut visited: BTreeSet<BlockId>,
    native_target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
    structural_parameters: &[TerminalTargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
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
        if !lower_conditional_scalar_operation(
            operation,
            function.machine,
            &mut values,
            &mut operations,
            native_target,
            functions,
            structural_parameters,
            structural_types,
        )? {
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
            let mut lowered = lower_boolean_block(
                function,
                values,
                *target,
                visited,
                native_target,
                functions,
                structural_parameters,
                structural_types,
            )?;
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
                let mut lowered = lower_boolean_arm(
                    function,
                    &values,
                    selected,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
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
                let lowered_true = lower_boolean_arm(
                    function,
                    &values,
                    selected_true,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
                let lowered_false = lower_boolean_arm(
                    function,
                    &values,
                    selected_false,
                    &visited,
                    native_target,
                    functions,
                    structural_parameters,
                    structural_types,
                )?;
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
            ..
        } => {
            if *result != scalar_function_result(function)?.value
                || *scalar_type != ScalarType::Boolean
            {
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
        TerminalAbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Ok(LoweredBooleanControl {
            control: TerminalTargetBooleanControl::Crash {
                psi_crash_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            },
            operations,
            edges: vec![*psi_edge],
        }),
        _ => Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
            function.machine,
        )),
    }
}

fn target_operation_from_boolean_control(
    control: TerminalTargetBooleanControl,
) -> TerminalTargetOperation {
    match control {
        TerminalTargetBooleanControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalTargetOperation::Crash {
            psi_edge: psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        },
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
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
) -> Result<LoweredConditionalArm, LoweringError> {
    let mut values = values.clone();
    bind_conditional_values(&mut values, &successor.bindings, successor.psi_edge)?;
    let mut lowered = lower_conditional_block(
        function,
        result_type,
        values,
        successor.target,
        visited.clone(),
        target,
        functions,
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
    native_target: NativeTarget,
    functions: &BTreeMap<MachineId, &TerminalAbstractFunction>,
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
        if !lower_conditional_scalar_operation(
            operation,
            function.machine,
            &mut values,
            &mut operations,
            native_target,
            functions,
            &[],
            &BTreeMap::new(),
        )? {
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
            let mut lowered = lower_conditional_block(
                function,
                result_type,
                values,
                *target,
                visited,
                native_target,
                functions,
            )?;
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
                let mut lowered = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected,
                    &visited,
                    native_target,
                    functions,
                )?;
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
                let lowered_true = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected_true,
                    &visited,
                    native_target,
                    functions,
                )?;
                let lowered_false = lower_conditional_arm(
                    function,
                    result_type,
                    &values,
                    selected_false,
                    &visited,
                    native_target,
                    functions,
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
            ..
        } => {
            let function_result = scalar_function_result(function)?;
            if *result != function_result.value || *scalar_type != function_result.scalar_type {
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
        TerminalAbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Ok(LoweredIntegerControl {
            control: TerminalTargetIntegerControl::Crash {
                psi_crash_edge: *psi_edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            },
            operations,
            edges: vec![*psi_edge],
        }),
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
        TerminalTargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => TerminalTargetOperation::Crash {
            psi_edge: psi_crash_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        },
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
