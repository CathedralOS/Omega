use super::super::scalar_abi::{fixed_native_integer_shape, fixed_native_scalar_shape};
use super::super::shared::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::lowering) enum KnownUnitInteger {
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
    BlockParameter {
        block: BlockId,
        value: ValueId,
        scalar_type: IntegerType,
    },
}

impl KnownUnitInteger {
    pub(in crate::lowering) fn scalar_type(self) -> IntegerType {
        match self {
            Self::Parameter { scalar_type, .. } => scalar_type,
            Self::Immediate { scalar_type, .. } => scalar_type,
            Self::Home(home) => match home.scalar_type {
                ScalarType::Integer(integer) => integer,
                _ => unreachable!("known Unit integer home retains an integer type"),
            },
            Self::BlockParameter { scalar_type, .. } => scalar_type,
        }
    }

    pub(in crate::lowering) const fn into_target_source(
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
                scalar_type: ScalarType::Integer(scalar_type),
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
            Self::BlockParameter {
                block,
                value,
                scalar_type,
            } => TargetUnitScalarArgumentSource::BlockParameter(
                target_operations::TargetScalarBlockValue {
                    block,
                    value,
                    scalar_type: ScalarType::Integer(scalar_type),
                },
            ),
        }
    }
}

pub(in crate::lowering) fn insert_known_unit_integer(
    values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    value: ValueId,
    known: KnownUnitInteger,
) -> Result<(), LoweringError> {
    if values.insert(value, known).is_some() {
        return Err(LoweringError::DuplicateValue(value));
    }
    Ok(())
}

pub(super) fn unit_argument_source(
    value: ValueId,
    function: &AbstractFunction,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    operations: &[TargetUnitOperation],
) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
    if let Some(known) = scalar_values.get(&value) {
        return Ok(known.into_target_source(value));
    }
    if let Some((operation, literal)) = boolean_constants.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: *operation,
            source_value: value,
            value: *literal,
        });
    }
    if let Some((operation, literal)) = ieee_float_constants.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation: *operation,
            source_value: value,
            value: *literal,
        });
    }
    // The ordered row owns prior scalar call results; no parallel home index is needed.
    if let Some(home) = operations
        .iter()
        .rev()
        .find_map(|operation| match operation {
            TargetUnitOperation::ScalarCall { result_home, .. }
                if result_home.source_value == value =>
            {
                Some(*result_home)
            }
            _ => None,
        })
    {
        return Ok(TargetUnitScalarArgumentSource::Home(home));
    }
    let (position, parameter) = function
        .parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.value == value)
        .ok_or(LoweringError::UnknownValue(value))?;
    Ok(TargetUnitScalarArgumentSource::Parameter {
        parameter_index: u32::try_from(position).map_err(|_| LoweringError::UnknownValue(value))?,
        source_value: value,
        scalar_type: parameter.scalar_type,
    })
}

pub(in crate::lowering) fn lower_scalar_call(
    operation: &AbstractOperation,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    resolve_source: impl Fn(ValueId) -> Result<TargetUnitScalarArgumentSource, LoweringError>,
) -> Result<TargetUnitOperation, LoweringError> {
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
        unreachable!("ordered scalar-call lowering receives only scalar calls")
    };

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
    let result_shape = match scalar_type {
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => fixed_native_integer_shape(*integer)
            .ok_or(LoweringError::UnitScalarCallIntegerTypeUnsupported(*result))?,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    };
    if *scalar_type != callee_result.scalar_type {
        return Err(LoweringError::UnitScalarCallResultTypeMismatch {
            callee: *callee,
            result: *result,
        });
    }

    let parameter_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| {
            fixed_native_scalar_shape(parameter.scalar_type)
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
    let expected_target_abi = ScalarFunctionAbi {
        call_plan: call_plan.clone(),
        parameters: callee_function
            .parameters
            .iter()
            .zip(&call_plan.parameters)
            .map(|(parameter, placement)| ScalarAbiValue {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                placement: placement.clone(),
            })
            .collect(),
        result: ScalarAbiValue {
            value: callee_result.value,
            scalar_type: callee_result.scalar_type,
            placement: result_placement,
        },
    };
    require_exact_target_abi(*callee, scalar_abis.get(callee), &expected_target_abi)?;

    let target_arguments = arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&parameter_shapes)
        .zip(&call_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let source = resolve_source(*source_value)?;
                if source.scalar_type() != parameter.scalar_type
                    || placement.shape != *expected_shape
                {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::UnitScalarCallTargetShapeUnsupported(*callee)
                    })?,
                    source,
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: *result,
        scalar_type: *scalar_type,
        shape: result_shape,
    };
    if resolve_source(*result).is_ok() {
        return Err(LoweringError::DuplicateValue(*result));
    }
    Ok(TargetUnitOperation::ScalarCall {
        psi_operation: *psi_operation,
        callee: *callee,
        call_plan,
        result_home,
        arguments: target_arguments,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    })
}

fn require_exact_target_abi(
    callee: MachineId,
    actual: Option<&ScalarFunctionAbi>,
    expected: &ScalarFunctionAbi,
) -> Result<(), LoweringError> {
    if actual != Some(expected) {
        return Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abi() -> ScalarFunctionAbi {
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
        ScalarFunctionAbi {
            parameters: vec![ScalarAbiValue {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
                placement: call_plan.parameters[0].clone(),
            }],
            result: ScalarAbiValue {
                value: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
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
