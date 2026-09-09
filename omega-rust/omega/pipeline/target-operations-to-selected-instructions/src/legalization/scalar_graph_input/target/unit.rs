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
            TargetUnitOperation::EstablishScalarCase {
                psi_operation,
                result_home,
                result_case,
                fields,
            },
            AbstractOperation::EstablishScalarCase {
                psi_operation: expected_operation,
                result,
                result_case: expected_case,
                fields: expected_fields,
            },
        ) => {
            if psi_operation != expected_operation
                || result_case != expected_case
                || fields != expected_fields
                || *result_home
                    != super::super::scalar_sums::result_home(optimized, result.place, plan)?
            {
                return Err(invalid);
            }
            let declaration = plan
                .structural_types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
                .ok_or(invalid.clone())?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Err(invalid);
            };
            let case = cases
                .iter()
                .find(|case| case.id == *result_case)
                .ok_or(invalid.clone())?;
            if fields.len() != case.fields.len() {
                return Err(invalid);
            }
            for (field, declared) in fields.iter().zip(&case.fields) {
                if field.field != declared.id || !sources.iter().any(|(value, source)| *value == field.value && declared.field_type.scalar_type() == Some(source.scalar_type()))
                    || matches!(declared.field_type, terminal_psi::StructuralFieldType::BoundedInteger(_)) != field.range_obligation.is_some()
                    || field.range_obligation.is_some_and(|obligation| !unit.accepted_obligation_facts.iter().any(|fact|
                        fact.machine == optimized.machine && fact.operation == *psi_operation && fact.obligation == obligation)
                        || !optimized.facts.iter().any(|fact| matches!(fact,
                            optimization_unit::OptimizationFact::OperationObligationReference { obligation: retained, support }
                            if *retained == obligation && support == psi_operation)))
                { return Err(invalid); }
            }
        }
        (
            TargetUnitOperation::StructuralResultCall {
                psi_operation,
                result,
                callee,
                callee_result,
                result_home,
                call_plan,
                scalar_arguments,
                arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            AbstractOperation::CallStructural {
                psi_operation: expected_operation,
                result: expected_result,
                callee: expected_callee,
                arguments: values,
                structural_arguments,
                claim_transfers: expected_claims,
                returned_claim_transfers: expected_returns,
                requirement_obligations: expected_requirements,
                crash_continuations: expected_crashes,
                selected_evidence,
            },
        ) => {
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(invalid.clone())?;
            let expected_plan = super::super::callee_plan(*callee, native, plan, unit)?;
            if psi_operation != expected_operation
                || result != expected_result
                || callee != expected_callee
                || called.result.structural() != Some(callee_result)
                || result.structural_type != callee_result.structural_type
                || result.multiplicity != callee_result.multiplicity
                || result_home.as_ref()
                    != Some(&super::super::scalar_sums::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?)
                || *call_plan != expected_plan
                || !claim_transfers.is_empty()
                || !expected_claims.is_empty()
                || !returned_claim_transfers.is_empty()
                || !expected_returns.is_empty()
                || !requirement_obligations.is_empty()
                || !expected_requirements.is_empty()
                || !crash_continuations.is_empty()
                || !expected_crashes.is_empty()
                || !selected_evidence.is_empty()
                || values.len() != called.parameters.len()
                || scalar_arguments.len() != values.len()
                || structural_arguments.len() != called.structural_parameters.len()
                || arguments.len() != structural_arguments.len()
            {
                return Err(invalid);
            }
            for (position, ((value, parameter), argument)) in values
                .iter()
                .zip(&called.parameters)
                .zip(scalar_arguments)
                .enumerate()
            {
                if argument.parameter_index as usize != position
                    || argument.placement != expected_plan.parameters[position]
                    || argument.source.scalar_type() != parameter.scalar_type
                    || !sources
                        .iter()
                        .any(|(identity, source)| identity == value && *source == argument.source)
                {
                    return Err(invalid);
                }
            }
            for (position, (semantic, retained)) in
                structural_arguments.iter().zip(arguments).enumerate()
            {
                if *retained
                    != super::super::scalar_sums::call_argument(
                        semantic,
                        position,
                        optimized,
                        called,
                        &expected_plan,
                        native,
                        plan,
                    )?
                {
                    return Err(invalid);
                }
            }
        }
        (
            TargetUnitOperation::ByteSequenceWrite {
                psi_operation,
                destination,
                view,
                index,
                value,
                length,
                obligation,
            },
            AbstractOperation::ByteSequenceWrite {
                psi_operation: expected_operation,
                destination: expected_destination,
                index: expected_index,
                value: expected_value,
                length: expected_length,
                obligation: expected_obligation,
            },
        ) if psi_operation == expected_operation
            && destination.place == *expected_destination
            && length == expected_length
            && obligation == expected_obligation
            && optimized
                .structural_parameters
                .iter()
                .chain(
                    optimized
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .any(|parameter| parameter == destination)
            && checker.mutable_byte_view(view, *expected_destination)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_index && source == index)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_value && source == value) => {}
        (
            TargetUnitOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
                shape,
            },
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation: expected_operation,
                result: expected,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && result == expected
            && value == expected_value
            && super::super::scalar_shape(value.scalar_type) == Some(*shape)
            && sources.iter().any(|(identity, source)| {
                *identity == value.value && source.scalar_type() == value.scalar_type
            }) => {}
        (
            TargetUnitOperation::PrimitiveLocalStore {
                psi_operation,
                destination,
                value,
            },
            AbstractOperation::PrimitiveLocalStore {
                psi_operation: expected_operation,
                destination: expected,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && destination == expected
            && value == expected_value
            && sources.iter().any(|(identity, source)| {
                *identity == value.value && source.scalar_type() == value.scalar_type
            }) => {}
        (
            TargetUnitOperation::PrimitiveScalarRead {
                psi_operation,
                result,
                source,
            },
            AbstractOperation::PrimitiveScalarRead {
                psi_operation: expected_operation,
                result: expected,
                source: expected_source,
            },
        ) if psi_operation == expected_operation
            && result == expected
            && source == expected_source =>
        {
            sources.push((
                result.value,
                Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape: super::super::scalar_shape(result.scalar_type).ok_or(invalid.clone())?,
                }),
            ));
        }
        (
            TargetUnitOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes,
            },
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: expected_operation,
                place: expected_place,
                structural_type: expected_type,
                bytes: expected_bytes,
            },
        ) if psi_operation == expected_operation
            && place == expected_place
            && structural_type == expected_type
            && bytes == expected_bytes => {}
        (
            TargetUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            },
            AbstractOperation::IeeeFloatConstant {
                psi_operation: expected_operation,
                result: expected_result,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && result == expected_result
            && value == expected_value =>
        {
            sources.push((
                *result,
                Source::IeeeFloatImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::WriteOnlyPrimitiveStore { .. },
            AbstractOperation::WriteOnlyPrimitiveStore { .. },
        ) => {
            primitive_store::validate(target, abstracted, parameters, sources, optimized, unit)?;
        }
        (
            TargetUnitOperation::ScalarDefinition { result_home, .. },
            AbstractOperation::ByteSequenceLength { .. }
            | AbstractOperation::ByteSequenceRead { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::ExactIntegerSubtract { .. },
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
            AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(_),
                ..
            },
        ) => {
            super::super::read_byte::validate(target, abstracted, native.target, plan, unit)?;
        }
        (
            TargetUnitOperation::BoundarySettlement { .. },
            AbstractOperation::BoundaryCall { .. },
        ) => {
            super::hosted_scalar::validate(target, abstracted, native.target, plan, unit, sources)?;
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
            if let TargetUnitOperation::StructuralScalarCall {
                psi_operation,
                result,
                ..
            } = target
            {
                sources.push((
                    result.value,
                    Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                        defining_operation: *psi_operation,
                        source_value: result.value,
                        scalar_type: result.scalar_type,
                        shape: super::super::scalar_shape(result.scalar_type)
                            .ok_or(invalid.clone())?,
                    }),
                ));
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
        ) if psi_edge == edge
            && cleanup_actions == cleanup
            && (cleanup.is_empty() || super::super::read_byte::cleanup(optimized, cleanup)) => {}
        _ => return Err(invalid),
    }
    Ok(())
}
