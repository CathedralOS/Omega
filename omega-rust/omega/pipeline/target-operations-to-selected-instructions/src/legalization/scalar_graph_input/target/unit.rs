use super::*;
use target_operations::{
    ScalarAbiValue, TargetStructuralParameter, TargetUnitBody,
    TargetUnitScalarArgumentSource as Source,
};
mod primitive_store;
pub(super) fn validate(
    function: &TargetFunction,
    body: &TargetUnitBody,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if body.structural_types != plan.structural_types
        || body.structural_types != unit.structural_types
        || body.operations.len() != abstracted.operations.len()
    {
        return Err(invalid);
    }
    let mut sources = optimized
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.value,
                Source::Parameter {
                    parameter_index: index as u32,
                    source_value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            )
        })
        .collect::<Vec<_>>();
    for (target, abstracted) in body.operations.iter().zip(&abstracted.operations) {
        validate_operation(
            function,
            target,
            abstracted,
            &body.scalar_parameters,
            &body.parameters,
            &mut sources,
            optimized,
            native,
            plan,
            unit,
        )?;
    }
    Ok(())
}

/// Replay one ordered Unit operation with only the SSA sources available here.
pub(super) fn validate_operation(
    function: &TargetFunction,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    scalar_parameters: &[ScalarAbiValue],
    parameters: &[TargetStructuralParameter],
    sources: &mut Vec<(ValueId, Source)>,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let checker = Checker {
        function,
        available: Some(sources),
        optimized,
        native,
        plan,
        unit,
    };
    match (target, abstracted) {
        (
            TargetUnitOperation::WriteOnlyPrimitiveStore { .. },
            AbstractOperation::WriteOnlyPrimitiveStore { .. },
        ) => {
            primitive_store::validate(target, abstracted, parameters, sources, unit)?;
        }
        (
            TargetUnitOperation::ScalarDefinition { result_home, .. },
            AbstractOperation::ByteSequenceLength { .. }
            | AbstractOperation::ByteSequenceRead { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. },
        ) => {
            super::scalar_definitions::observation(target, abstracted, &checker)?;
            sources.push((result_home.source_value, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::ByteSequenceSubslice { result, view },
            AbstractOperation::ByteSequenceSubslice {
                result: expected,
                psi_operation,
                ..
            },
        ) => {
            if result != expected
                || !matches!(view, target_operations::TargetByteView::Subslice { psi_operation: operation, .. } if operation == psi_operation)
                || !checker.byte_view(view, result.place, &[])
            {
                return Err(invalid);
            }
        }
        (
            TargetUnitOperation::ScalarDefinition { result_home, .. },
            AbstractOperation::IntegerWiden { .. },
        ) => {
            super::scalar_definitions::validate(target, abstracted, scalar_parameters, sources)?;
            sources.push((result_home.source_value, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::BoundarySettlement { .. },
            AbstractOperation::BoundaryCall { .. },
        ) => {
            super::byte_output::validate(target, abstracted, native.target, plan, sources)?;
        }
        (
            TargetUnitOperation::StructuralScalarFieldStore {
                psi_operation,
                destination,
                path,
                field,
                destination_placement,
                field_byte_offset,
                source,
            },
            AbstractOperation::StructuralScalarFieldStore {
                psi_operation: expected_operation,
                destination: expected_destination,
                path: expected_path,
                field: expected_field,
                value,
            },
        ) => {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == destination.place)
                .ok_or(invalid.clone())?;
            let (offset, _) = crate::structural_reference_input::store(
                expected_destination.structural_type,
                expected_path,
                *expected_field,
                value.scalar_type,
                &unit.structural_types,
            )
            .ok_or(invalid.clone())?;
            if psi_operation != expected_operation
                || destination != expected_destination
                || path != expected_path
                || field != expected_field
                || destination_placement != &parameter.placement
                || *field_byte_offset != offset
                || !sources
                    .iter()
                    .any(|(identity, expected)| *identity == value.value && expected == source)
            {
                return Err(invalid);
            }
        }
        (
            TargetUnitOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            },
            AbstractOperation::BooleanConstant {
                psi_operation: actual,
                result: defined,
                value: literal,
            },
        ) if psi_operation == actual && result == defined && value == literal => {
            sources.push((
                *result,
                Source::BooleanImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::Call {
                psi_operation,
                callee,
                call_plan,
                scalar_arguments,
                arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::CallUnit {
                psi_operation: actual,
                callee: called,
                arguments: values,
                structural_arguments,
                claim_transfers: claims,
                requirement_obligations: requirements,
                crash_continuations: crashes,
            },
        )
        | (
            TargetUnitOperation::StructuralScalarCall {
                psi_operation,
                callee,
                call_plan,
                scalar_arguments,
                arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            },
            AbstractOperation::CallStructuralScalar {
                psi_operation: actual,
                callee: called,
                arguments: values,
                structural_arguments,
                claim_transfers: claims,
                requirement_obligations: requirements,
                crash_continuations: crashes,
                ..
            },
        ) => {
            let expected = callee_plan(*callee, native, plan, unit)?;
            let result_matches = match (target, abstracted) {
                (TargetUnitOperation::Call { .. }, AbstractOperation::CallUnit { .. }) => {
                    expected.result.is_none()
                }
                (
                    TargetUnitOperation::StructuralScalarCall { result, .. },
                    AbstractOperation::CallStructuralScalar { result: actual, .. },
                ) => {
                    result == actual
                        && actual.scalar_type == ScalarType::Integer(u64_type())
                        && expected.result.is_some()
                }
                _ => false,
            };
            if arguments.len() != structural_arguments.len() || arguments.len() > 1 {
                return Err(invalid);
            }
            if !result_matches
                || psi_operation != actual
                || callee != called
                || call_plan != &expected
                || claim_transfers != claims
                || requirement_obligations != requirements
                || crash_continuations != crashes
                || scalar_arguments.len() != values.len()
                || expected.parameters.len() != values.len() + arguments.len()
                || scalar_arguments
                    .iter()
                    .zip(values)
                    .zip(&expected.parameters)
                    .enumerate()
                    .any(|(position, ((argument, value), placement))| {
                        argument.parameter_index != position as u32
                            || argument.placement != *placement
                            || !sources.iter().any(|(source, definition)| {
                                source == value && *definition == argument.source
                            })
                    })
            {
                return Err(invalid);
            }
            for (argument, semantic) in arguments.iter().zip(structural_arguments) {
                super::super::structural_call::validate_argument(
                    semantic,
                    argument,
                    *psi_operation,
                    optimized,
                    *callee,
                    native,
                    plan,
                    unit,
                )?;
            }
        }
        (
            TargetUnitOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            },
            AbstractOperation::IntegerConstant {
                psi_operation: actual,
                result: defined,
                scalar_type: integer,
                value: literal,
            },
        ) if psi_operation == actual
            && result == defined
            && *integer == ScalarType::Integer(*scalar_type)
            && value == literal =>
        {
            sources.push((
                *result,
                Source::IntegerImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::ScalarCall {
                psi_operation,
                callee,
                call_plan,
                result_home,
                arguments,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::Call {
                psi_operation: actual,
                result,
                callee: called,
                arguments: values,
                scalar_type,
                requirement_obligations: requirements,
                crash_continuations: crashes,
            },
        ) => {
            let expected = callee_plan(*callee, native, plan, unit)?;
            if psi_operation != actual
                || callee != called
                || call_plan != &expected
                || result_home.defining_operation != *actual
                || result_home.source_value != *result
                || result_home.scalar_type != *scalar_type
                || Some(result_home.shape) != expected.result.as_ref().map(|value| value.shape)
                || requirement_obligations != requirements
                || crash_continuations != crashes
                || arguments.len() != values.len()
                || arguments.len() != expected.parameters.len()
                || arguments
                    .iter()
                    .zip(values)
                    .zip(&expected.parameters)
                    .enumerate()
                    .any(|(index, ((argument, value), placement))| {
                        argument.parameter_index != index as u32
                            || argument.placement != *placement
                            || !sources.iter().any(|(source, definition)| {
                                source == value && *definition == argument.source
                            })
                    })
            {
                return Err(invalid);
            }
            sources.push((*result, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::Return {
                psi_edge,
                cleanup_actions,
            },
            AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: cleanup,
            },
        ) if psi_edge == edge && cleanup_actions == cleanup && cleanup.is_empty() => {}
        _ => return Err(invalid),
    }
    Ok(())
}
