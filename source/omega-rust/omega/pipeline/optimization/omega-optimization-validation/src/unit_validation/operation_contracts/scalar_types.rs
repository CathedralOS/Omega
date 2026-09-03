use super::*;

pub(crate) fn operation_scalar_types_match(
    function: &PsiOptimizationFunction,
    operation: &O,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> bool {
    let scalar = |value: ValueId| definitions.get(&value).map(|row| row.scalar_type);
    let integer = |value: ValueId, expected: IntegerType| {
        scalar(value) == Some(ScalarType::Integer(expected))
    };
    let ieee_float =
        |value: ValueId, expected| scalar(value) == Some(ScalarType::IeeeFloat(expected));
    let fixed = |integer: IntegerType| integer.carrier() == IntegerCarrier::Fixed;
    let binary = |left: ValueId, right: ValueId, expected: IntegerType| {
        integer(left, expected) && integer(right, expected)
    };
    match operation {
        O::DynamicDescriptorParameter { parameter } => {
            parameter.owner == function.machine
                && !parameter.trait_identity.is_empty()
                && !parameter.requirements.is_empty()
                && parameter
                    .requirements
                    .iter()
                    .enumerate()
                    .all(|(slot, requirement)| {
                        u32::try_from(slot) == Ok(requirement.slot)
                            && !requirement.declaring_trait_identity.is_empty()
                            && !requirement.public_requirement_identity.is_empty()
                    })
        }
        O::StoreDynamicDescriptor {
            psi_operation,
            stored,
        } => stored.has_complete_custody(function.machine, *psi_operation),
        O::WriteOnlyPrimitiveStore { value, .. } | O::StructuralScalarFieldStore { value, .. } => {
            scalar(value.value) == Some(value.scalar_type)
        }
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. }
        | O::PortWrite { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => true,
        O::IntegerStructuralField { result, .. } => {
            matches!(result.scalar_type, ScalarType::Integer(_))
        }
        O::IntegerConstant {
            scalar_type, value, ..
        } => match scalar_type {
            ScalarType::Integer(integer) => integer.admits(*value),
            ScalarType::Boolean => false,
            ScalarType::IeeeFloat(_) => false,
        },
        O::IeeeFloatConstant { .. } => true,
        O::NearestIeeeFloatFusedMultiplyAdd {
            format,
            left,
            right,
            addend,
            ..
        } => {
            ieee_float(*left, *format)
                && ieee_float(*right, *format)
                && ieee_float(*addend, *format)
        }
        O::BooleanConstant { .. } => true,
        O::BooleanNot { operand, .. } => scalar(*operand) == Some(ScalarType::Boolean),
        O::BooleanEqual { left, right, .. } => {
            scalar(*left) == Some(ScalarType::Boolean)
                && scalar(*right) == Some(ScalarType::Boolean)
        }
        O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. } => {
            matches!(scalar(*left), Some(ScalarType::Integer(_))) && scalar(*left) == scalar(*right)
        }
        O::IntegerBitwiseNot {
            scalar_type,
            operand,
            ..
        } => integer(*operand, *scalar_type),
        O::IntegerWiden {
            source_type,
            target_type,
            operand,
            ..
        } => integer(*operand, *source_type) && source_type.can_widen_to(*target_type),
        O::IntegerExactCast {
            source_type,
            target_type,
            operand,
            ..
        } => {
            integer(*operand, *source_type)
                && source_type.can_exact_cast_to(*target_type)
                && !source_type.can_widen_to(*target_type)
                && source_type != target_type
        }
        O::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
            ..
        }
        | O::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        } => binary(*left, *right, *scalar_type),
        O::ExactIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
            ..
        } => fixed(*scalar_type) && binary(*left, *right, *scalar_type),
        O::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => integer(*value, *value_type) && integer(*count, *count_type),
        O::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | O::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
            ..
        } => {
            fixed(*value_type)
                && fixed(*count_type)
                && integer(*value, *value_type)
                && integer(*count, *count_type)
        }
        O::Jump { .. } => true,
        O::Conditional { condition, .. } => scalar(*condition) == Some(ScalarType::Boolean),
        O::Return {
            result,
            value,
            scalar_type,
            ..
        } => {
            scalar(*value) == Some(*scalar_type)
                && matches!(
                    function.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.value == *result && signature.scalar_type == *scalar_type
                )
        }
        O::Call {
            result: _,
            scalar_type,
            callee,
            arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.structural_parameters.is_empty()
                && callee.declared_places.is_empty()
                && callee.entry_claim_declarations.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == *scalar_type
                )
                && arguments.len() == callee.parameters.len()
                && arguments
                    .iter()
                    .zip(&callee.parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(parameter.scalar_type))
        }),
        O::CallUnit { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Unit
                )
        }),
        O::CallUnitWithDynamicArguments {
            psi_operation,
            callee,
            dynamic_arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Unit
                )
                && dynamic_arguments_match(function, *psi_operation, callee, dynamic_arguments)
        }),
        O::CallStructuralScalar { result, callee, .. } => {
            functions.get(callee).is_some_and(|callee| {
                callee.parameters.is_empty()
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            })
        }
        O::CallStructuralScalarWithDynamicArguments {
            psi_operation,
            result,
            callee,
            dynamic_arguments,
            ..
        } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                callee.result,
                omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                        if signature.scalar_type == result.scalar_type
                )
                && dynamic_arguments_match(function, *psi_operation, callee, dynamic_arguments)
        }),
        O::CallDynamicScalar {
            psi_operation,
            result,
            dynamic_dispatch,
            ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                dynamic_dispatch.has_complete_application_custody(function.machine, *psi_operation)
                    && scalar_result_class(result.scalar_type)
                        .is_some_and(|result| rebound_result_matches(dynamic_dispatch, result))
                    && callee.parameters.is_empty()
                    && callee.structural_parameters.len() == 1
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            }),
        O::CallStoredDynamicScalar {
            psi_operation,
            result,
            dynamic_dispatch,
            ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                dynamic_dispatch.has_complete_custody(function.machine, *psi_operation)
                    && scalar_result_class(result.scalar_type)
                        .is_some_and(|result| stored_result_matches(dynamic_dispatch, result))
                    && callee.parameters.is_empty()
                    && callee.structural_parameters.len() == 1
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Scalar(signature)
                            if signature.scalar_type == result.scalar_type
                    )
            }),
        O::CallDynamicParameterScalar {
            psi_operation,
            result,
            dynamic_dispatch,
            ..
        } => {
            let parameter = &dynamic_dispatch.parameter;
            let dispatch = &dynamic_dispatch.dispatch;
            let requirement = parameter
                .requirements
                .iter()
                .filter(|requirement| requirement.slot == dispatch.requirement_slot)
                .collect::<Vec<_>>();
            matches!(requirement.as_slice(), [requirement]
            if parameter.owner == function.machine
                && dispatch.owner == function.machine
                && dispatch.operation == *psi_operation
                && dispatch.parameter_ordinal == parameter.ordinal
                && match requirement.result {
                    psi_terminal::ClosedConformanceCallableResult::Unit => false,
                    psi_terminal::ClosedConformanceCallableResult::I32 => {
                        result.scalar_type == ScalarType::Integer(
                            IntegerType::new(psi_core::IntegerSign::Signed, 32)
                                .expect("i32 is valid"),
                        )
                    }
                    psi_terminal::ClosedConformanceCallableResult::Bool => {
                        result.scalar_type == ScalarType::Boolean
                    }
                })
        }
        O::CallDynamicUnit {
            psi_operation,
            dynamic_dispatch,
            ..
        } => functions
            .get(&dynamic_dispatch.dispatch.realization)
            .is_some_and(|callee| {
                dynamic_dispatch.has_complete_application_custody(function.machine, *psi_operation)
                    && rebound_result_matches(
                        dynamic_dispatch,
                        psi_terminal::ClosedConformanceCallableResult::Unit,
                    )
                    && callee.parameters.is_empty()
                    && callee.structural_parameters.len() == 1
                    && matches!(
                        callee.result,
                        omega_abstract_operations::AbstractFunctionResult::Unit
                    )
            }),
        O::CallDynamicParameterUnit {
            psi_operation,
            dynamic_dispatch,
            ..
        } => {
            let parameter = &dynamic_dispatch.parameter;
            let dispatch = &dynamic_dispatch.dispatch;
            let requirements = parameter
                .requirements
                .iter()
                .filter(|requirement| requirement.slot == dispatch.requirement_slot)
                .collect::<Vec<_>>();
            matches!(requirements.as_slice(), [requirement]
                if parameter.owner == function.machine
                    && dispatch.owner == function.machine
                    && dispatch.operation == *psi_operation
                    && dispatch.parameter_ordinal == parameter.ordinal
                    && requirement.result
                        == psi_terminal::ClosedConformanceCallableResult::Unit)
        }
        O::CallStructural { callee, .. } => functions.get(callee).is_some_and(|callee| {
            callee.parameters.is_empty()
                && matches!(
                    callee.result,
                    omega_abstract_operations::AbstractFunctionResult::Structural(_)
                )
        }),
        O::BoundaryCall {
            result,
            boundary,
            arguments,
            ..
        } => boundary_machines.get(boundary).is_some_and(|boundary| {
            result.as_ref().map(|result| result.scalar_type) == boundary.result
                && arguments.len() == boundary.scalar_parameters.len()
                && arguments
                    .iter()
                    .zip(&boundary.scalar_parameters)
                    .all(|(argument, parameter)| scalar(*argument) == Some(*parameter))
        }),
    }
}

fn dynamic_arguments_match(
    function: &PsiOptimizationFunction,
    psi_operation: psi_core::OperationId,
    callee: &PsiOptimizationFunction,
    dynamic_arguments: &[omega_abstract_operations::AbstractDynamicDescriptorArgument],
) -> bool {
    !dynamic_arguments.is_empty()
        && dynamic_arguments.iter().all(|argument| {
            argument.has_complete_custody(function.machine, psi_operation, callee.machine)
                && callee
                    .blocks
                    .iter()
                    .find(|block| block.id == callee.entry)
                    .is_some_and(|entry| {
                        entry.nodes.iter().any(|node| {
                            matches!(
                                &node.operation,
                                O::DynamicDescriptorParameter { parameter }
                                    if parameter == &argument.target
                            )
                        })
                    })
        })
        && dynamic_arguments
            .windows(2)
            .all(|pair| pair[0].target.ordinal < pair[1].target.ordinal)
}

fn rebound_result_matches(
    dynamic_dispatch: &omega_abstract_operations::AbstractReboundDynamicDispatch,
    expected: psi_terminal::ClosedConformanceCallableResult,
) -> bool {
    dynamic_dispatch
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.source_callable_identity
                == dynamic_dispatch.dispatch.realization_callable_identity
                && callable.machine == dynamic_dispatch.dispatch.realization
        })
        .is_some_and(|callable| callable.result == expected)
}

fn stored_result_matches(
    dynamic_dispatch: &omega_abstract_operations::AbstractStoredDynamicDispatch,
    expected: psi_terminal::ClosedConformanceCallableResult,
) -> bool {
    dynamic_dispatch
        .stored
        .application
        .realization_callables
        .iter()
        .find(|callable| {
            callable.source_callable_identity
                == dynamic_dispatch.dispatch.realization_callable_identity
                && callable.machine == dynamic_dispatch.dispatch.realization
        })
        .is_some_and(|callable| callable.result == expected)
}

fn scalar_result_class(
    scalar_type: ScalarType,
) -> Option<psi_terminal::ClosedConformanceCallableResult> {
    if scalar_type == ScalarType::Boolean {
        Some(psi_terminal::ClosedConformanceCallableResult::Bool)
    } else if scalar_type
        == ScalarType::Integer(
            IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32 is valid"),
        )
    {
        Some(psi_terminal::ClosedConformanceCallableResult::I32)
    } else {
        None
    }
}
