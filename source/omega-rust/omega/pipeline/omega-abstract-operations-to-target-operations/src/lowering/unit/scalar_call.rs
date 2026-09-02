use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KnownUnitInteger {
    Parameter {
        parameter_index: u32,
        scalar_type: IntegerType,
    },
    Immediate {
        defining_operation: OperationId,
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Home(TargetUnitScalarHomeRequirement),
}

impl KnownUnitInteger {
    pub(super) const fn scalar_type(self) -> IntegerType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::Immediate { scalar_type, .. } => scalar_type,
            Self::Home(home) => home.scalar_type,
        }
    }

    pub(super) const fn into_target_source(
        self,
        source_value: ValueId,
    ) -> TargetUnitScalarArgumentSource {
        match self {
            Self::Parameter {
                parameter_index,
                scalar_type,
            } => TargetUnitScalarArgumentSource::Parameter {
                parameter_index,
                source_value,
                scalar_type,
            },
            Self::Immediate {
                defining_operation,
                scalar_type,
                value,
            } => TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            },
            Self::Home(home) => TargetUnitScalarArgumentSource::Home(home),
        }
    }
}

pub(super) fn insert_known_unit_integer(
    values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    value: ValueId,
    known: KnownUnitInteger,
) -> Result<(), LoweringError> {
    if values.insert(value, known).is_some() {
        return Err(LoweringError::DuplicateValue(value));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("attached-Unit scalar-call lowering receives only scalar calls")
    };

    if function.attachment.is_none() {
        return Err(LoweringError::UnitScalarCallRequiresAttachedMachine {
            machine: function.machine,
            operation: *psi_operation,
        });
    }

    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    if !callee_function.published_service_ceiling.is_empty() {
        return Err(LoweringError::UnitScalarCallTargetPublishesServices(
            *callee,
        ));
    }
    if !callee_function.structural_parameters.is_empty() || !callee_function.entry_claims.is_empty()
    {
        return Err(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee));
    }
    let Some(callee_result) = callee_function.result.scalar() else {
        return Err(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee));
    };
    let ScalarType::Integer(result_type) = scalar_type else {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(*result));
    };
    let ScalarType::Integer(callee_result_type) = callee_result.scalar_type else {
        return Err(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee));
    };
    let result_shape = fixed_native_integer_shape(*result_type)
        .ok_or(LoweringError::UnitScalarCallIntegerTypeUnsupported(*result))?;
    if *result_type != callee_result_type {
        return Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee: *callee,
            result: *result,
        });
    }

    let parameter_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter.scalar_type else {
                return Err(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee));
            };
            fixed_native_integer_shape(integer_type)
                .ok_or(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes.clone(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if arguments.len() != callee_function.parameters.len()
        || arguments.len() != call_plan.parameters.len()
    {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee: *callee,
            expected: callee_function.parameters.len(),
            actual: arguments.len(),
        });
    }
    let result_placement = call_plan.result.clone().ok_or(
        LoweringError::UnitScalarCallResultPlacementUnsupported {
            callee: *callee,
            result: *result,
        },
    )?;
    if !matches!(
        result_placement.locations.as_slice(),
        [ValueLocation::Register {
            value_byte_offset: 0,
            byte_size,
            ..
        }] if *byte_size == result_shape.byte_size
    ) {
        return Err(LoweringError::UnitScalarCallResultPlacementUnsupported {
            callee: *callee,
            result: *result,
        });
    }
    let expected_target_abi = FixedIntegerScalarFunctionAbi {
        call_plan: call_plan.clone(),
        parameters: callee_function
            .parameters
            .iter()
            .zip(&call_plan.parameters)
            .map(|(parameter, placement)| {
                let ScalarType::Integer(scalar_type) = parameter.scalar_type else {
                    unreachable!("fixed scalar call parameters were checked above")
                };
                FixedIntegerScalarAbiValue {
                    value: parameter.value,
                    scalar_type,
                    placement: placement.clone(),
                }
            })
            .collect(),
        result: FixedIntegerScalarAbiValue {
            value: callee_result.value,
            scalar_type: callee_result_type,
            placement: result_placement,
        },
    };
    require_exact_target_abi(
        *callee,
        fixed_integer_scalar_abis.get(callee),
        &expected_target_abi,
    )?;

    let target_arguments = arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&parameter_shapes)
        .zip(&call_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let known = values
                    .get(source_value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*source_value))?;
                let ScalarType::Integer(parameter_type) = parameter.scalar_type else {
                    return Err(LoweringError::UnitScalarCallTargetShapeUnsupported(*callee));
                };
                if known.scalar_type() != parameter_type || placement.shape != *expected_shape {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::UnitScalarCallTargetShapeUnsupported(*callee)
                    })?,
                    source: known.into_target_source(*source_value),
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: *result,
        scalar_type: *result_type,
        shape: result_shape,
    };
    insert_known_unit_integer(values, *result, KnownUnitInteger::Home(result_home))?;
    operations.push(TargetUnitOperation::ScalarCall {
        psi_operation: *psi_operation,
        callee: *callee,
        call_plan,
        result_home,
        arguments: target_arguments,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

fn require_exact_target_abi(
    callee: MachineId,
    actual: Option<&FixedIntegerScalarFunctionAbi>,
    expected: &FixedIntegerScalarFunctionAbi,
) -> Result<(), LoweringError> {
    if actual != Some(expected) {
        return Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abi() -> FixedIntegerScalarFunctionAbi {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let shape = ValueShape::integer(4, 4);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(NativeTarget::linux_x64()),
            &CallSignature {
                parameters: vec![shape],
                result: Some(shape),
            },
        )
        .unwrap();
        FixedIntegerScalarFunctionAbi {
            parameters: vec![FixedIntegerScalarAbiValue {
                value: ValueId::new(1).unwrap(),
                scalar_type,
                placement: call_plan.parameters[0].clone(),
            }],
            result: FixedIntegerScalarAbiValue {
                value: ValueId::new(2).unwrap(),
                scalar_type,
                placement: call_plan.result.clone().unwrap(),
            },
            call_plan,
        }
    }

    #[test]
    fn exact_target_abi_gate_rejects_absence_and_semantic_or_plan_drift() {
        let callee = MachineId::new(1).unwrap();
        let expected = abi();
        assert_eq!(
            require_exact_target_abi(callee, Some(&expected), &expected),
            Ok(())
        );
        assert_eq!(
            require_exact_target_abi(callee, None, &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );

        let mut changed_value = expected.clone();
        changed_value.parameters[0].value = ValueId::new(3).unwrap();
        assert_eq!(
            require_exact_target_abi(callee, Some(&changed_value), &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );

        let mut changed_plan = expected.clone();
        changed_plan.call_plan.stack_alignment = 32;
        assert_eq!(
            require_exact_target_abi(callee, Some(&changed_plan), &expected),
            Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee))
        );
    }
}
