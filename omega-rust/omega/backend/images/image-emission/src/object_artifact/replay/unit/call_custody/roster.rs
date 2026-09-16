//! The roster of one internal Unit call checked against the callee ABI it is
//! called through: an argument-free call, a scalar parameter ABI, a mixed
//! structural/scalar ABI or a mixed structural return, each followed by the
//! exact argument bytes and order the plan requires.

use super::super::scalar_call_custody::{expected_argument_bytes, validate_source};
use crate::{ObjectError, ObjectUnitStack};
use machine_code::{MachineCodeFunction, SemanticCodeSite, StructuralReturnRecord};
use target::{Architecture, NativeTarget};
use target_operations::{CallSiteOwner, MixedStructuralScalarFunctionAbi};

use super::InternalUnitCallCustody;
use super::call_facts::{CallInputs, CallSpan, StackFacts};

/// A call with no roster only needs a valid site: no result, an owner
/// that is either an operation attributed once to exactly these bytes or a
/// cleanup action of the function's affine cleanup that invokes the target
/// nominal, and bytes that contain the relocation.
pub(super) fn validate_argument_free_call(
    call: &CallInputs<'_>,
    span: &CallSpan<'_>,
) -> Result<(), ObjectError> {
    let CallInputs {
        provenance,
        attribution,
        custody,
        affine_cleanup,
        machine,
        ..
    } = *call;
    let CallSpan {
        relocation,
        end,
        relocation_end,
        ..
    } = *span;
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    if custody.result.is_some() || custody.structural_result.is_some() {
        return Err(invalid());
    }
    let owner_valid = match custody.owner {
        CallSiteOwner::Operation(operation) => {
            provenance.operations.contains(&operation)
                && attribution
                    .iter()
                    .filter(|attribution| {
                        attribution.site == SemanticCodeSite::Operation(operation)
                            && attribution.operation_ordinal == custody.operation_ordinal
                            && attribution.code_offset == custody.code_offset
                            && attribution.byte_count == custody.byte_count
                    })
                    .count()
                    == 1
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            let Some(cleanup) = affine_cleanup else {
                return Err(invalid());
            };
            let Some(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                usize::try_from(action_ordinal)
                    .ok()
                    .and_then(|ordinal| cleanup.actions.get(ordinal))
            else {
                return Err(invalid());
            };
            let cleanup_end = cleanup
                .code_offset
                .checked_add(cleanup.byte_count)
                .ok_or_else(invalid)?;
            provenance.edges.contains(&edge)
                && cleanup.psi_edge == edge
                && nominal.cleanup_machine == custody.target
                && cleanup.code_offset <= custody.code_offset
                && end <= cleanup_end
                && attribution
                    .iter()
                    .filter(|attribution| {
                        attribution.site == SemanticCodeSite::Edge(edge)
                            && attribution.operation_ordinal == custody.operation_ordinal
                            && attribution.code_offset == cleanup.code_offset
                            && attribution.byte_count == cleanup.byte_count
                    })
                    .count()
                    == 1
        }
    };
    if custody.byte_count == 0
        || custody.code_offset > relocation.offset
        || relocation_end > end
        || !owner_valid
    {
        return Err(invalid());
    }
    Ok(())
}

/// A scalar parameter ABI: the plan matches, there is no result, each
/// scalar argument sits in its parameter's placement, and each Unit
/// argument matches its parameter's type, access, shape and placement
/// unless it is an exact borrowed projection.
pub(super) fn validate_parameter_abi(
    call: &InternalUnitCallCustody<'_>,
    abi: &machine_code::ParameterFunctionAbiRecord,
) -> Result<(), ObjectError> {
    let CallInputs {
        target,
        function,
        callee_unit_parameters,
        custody,
        machine,
        ..
    } = call.inputs;
    let CallSpan { relocation, .. } = call.span;
    let StackFacts {
        validated_function_stack,
        ..
    } = call.stacks;
    let expected_plan = &call.expected_plan;
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    if *expected_plan != abi.call_plan
        || custody.result.is_some()
        || custody.structural_result.is_some()
        || custody.scalar_arguments.len() != abi.parameters.len()
        || custody.arguments.len() != callee_unit_parameters.len()
        || custody
            .scalar_arguments
            .iter()
            .zip(&abi.parameters)
            .enumerate()
            .any(|(index, (argument, parameter))| {
                usize::try_from(argument.parameter_index) != Ok(index)
                    || argument.destination != parameter.placement
                    || argument.source.scalar_type() != parameter.scalar_type
            })
        || custody
            .arguments
            .iter()
            .zip(callee_unit_parameters)
            .zip(&abi.call_plan.parameters[abi.parameters.len()..])
            .enumerate()
            .any(|(index, ((argument, parameter), placement))| {
                !call.exact_borrowed_argument(index, argument)
                    && (argument.root_structural_type != parameter.structural_type
                        || argument.structural_type != parameter.structural_type
                        || argument.access != parameter.access
                        || argument.shape != parameter.shape
                        || argument.destination != *placement)
            })
    {
        return Err(invalid());
    }
    validate_mixed_argument_bytes_and_order(
        target,
        function,
        validated_function_stack,
        expected_plan,
        relocation,
        custody,
    )?;
    Ok(())
}

/// A mixed structural/scalar ABI: the plan and scalar result match, each
/// scalar argument sits in its parameter's placement, and each structural
/// argument is an unprojected exact match of its parameter.
pub(super) fn validate_mixed_abi(
    call: &InternalUnitCallCustody<'_>,
    abi: &MixedStructuralScalarFunctionAbi,
) -> Result<(), ObjectError> {
    let CallInputs {
        target,
        function,
        custody,
        machine,
        ..
    } = call.inputs;
    let CallSpan { relocation, .. } = call.span;
    let StackFacts {
        validated_function_stack,
        ..
    } = call.stacks;
    let expected_plan = &call.expected_plan;
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    if *expected_plan != abi.call_plan
        || custody.result != Some(abi.result.scalar_type)
        || custody.scalar_arguments.len() != abi.scalar_parameters.len()
        || custody.arguments.len() != abi.structural_parameters.len()
        || custody
            .scalar_arguments
            .iter()
            .zip(&abi.scalar_parameters)
            .enumerate()
            .any(|(index, (argument, parameter))| {
                usize::try_from(argument.parameter_index) != Ok(index)
                    || argument.destination != parameter.placement
                    || argument.source.scalar_type() != parameter.scalar_type
            })
        || custody
            .arguments
            .iter()
            .zip(&abi.structural_parameters)
            .any(|(argument, parameter)| {
                !argument.path.is_empty()
                    || argument.root_structural_type != parameter.structural_type
                    || argument.access != parameter.access
                    || argument.structural_type != parameter.structural_type
                    || argument.shape != parameter.shape
                    || argument.destination != parameter.placement
            })
    {
        return Err(invalid());
    }
    validate_mixed_argument_bytes_and_order(
        target,
        function,
        validated_function_stack,
        expected_plan,
        relocation,
        custody,
    )?;
    Ok(())
}

/// A mixed structural return: the call returns structurally, the plan's
/// parameters and result placements are the return's, each scalar argument
/// sits in its parameter's placement, and each structural argument is an
/// unprojected exact match of its parameter placement.
pub(super) fn validate_mixed_structural_return(
    call: &InternalUnitCallCustody<'_>,
    returned: &StructuralReturnRecord,
) -> Result<(), ObjectError> {
    let CallInputs {
        target,
        function,
        custody,
        machine,
        ..
    } = call.inputs;
    let CallSpan { relocation, .. } = call.span;
    let StackFacts {
        validated_function_stack,
        ..
    } = call.stacks;
    let expected_plan = &call.expected_plan;
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
    if custody.result.is_some()
        || custody.structural_result.is_none()
        || expected_plan.parameters.len()
            != returned.scalar_parameters.len() + returned.parameters.len()
        || expected_plan.parameters[..returned.scalar_parameters.len()]
            != returned
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.placement.clone())
                .collect::<Vec<_>>()
        || expected_plan.parameters[returned.scalar_parameters.len()..]
            != returned.parameter_placements
        || expected_plan.result.as_ref() != Some(&returned.result_placement)
        || custody.scalar_arguments.len() != returned.scalar_parameters.len()
        || custody.arguments.len() != returned.parameters.len()
        || custody
            .scalar_arguments
            .iter()
            .zip(&returned.scalar_parameters)
            .enumerate()
            .any(|(index, (argument, parameter))| {
                usize::try_from(argument.parameter_index) != Ok(index)
                    || argument.destination != parameter.placement
                    || argument.source.scalar_type() != parameter.scalar_type
            })
        || custody
            .arguments
            .iter()
            .zip(&returned.parameters)
            .zip(&returned.parameter_placements)
            .any(|((argument, parameter), placement)| {
                !argument.path.is_empty()
                    || argument.root_structural_type != parameter.structural_type
                    || argument.access != parameter.access
                    || argument.structural_type != parameter.structural_type
                    || argument.shape != placement.shape
                    || argument.destination != *placement
            })
    {
        return Err(invalid());
    }
    validate_mixed_argument_bytes_and_order(
        target,
        function,
        validated_function_stack,
        expected_plan,
        relocation,
        custody,
    )?;
    Ok(())
}

fn validate_mixed_argument_bytes_and_order(
    target: NativeTarget,
    function: &MachineCodeFunction,
    function_stack: &ObjectUnitStack,
    call_plan: &calling_conventions::CallPlan,
    relocation: &machine_code::InternalCallRelocation,
    custody: &machine_code::InternalUnitCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitCallEvidence(function.machine);
    let outbound = relocation.unit_stack.ok_or_else(invalid)?.outbound;
    let outbound_bytes = outbound.map_or(0, |area| area.byte_size);
    let mut cursor = match outbound {
        Some(area) => {
            if custody.code_offset != area.allocation_offset {
                return Err(invalid());
            }
            area.allocation_offset
                .checked_add(area.allocation_byte_count)
                .ok_or_else(invalid)?
        }
        None => custody.code_offset,
    };
    for (argument_index, argument) in custody.scalar_arguments.iter().enumerate() {
        if argument.code_offset != cursor {
            return Err(invalid());
        }
        validate_source(
            function,
            custody.operation_ordinal,
            custody.code_offset,
            argument.source,
        )
        .map_err(|_| invalid())?;
        let expected = expected_argument_bytes(
            target,
            call_plan,
            &custody.scalar_arguments,
            argument_index,
            function_stack.frame_bytes,
            outbound_bytes,
        )
        .ok_or_else(invalid)?;
        let argument_end = cursor.checked_add(expected.len()).ok_or_else(invalid)?;
        if argument.byte_count != expected.len()
            || function.bytes.get(cursor..argument_end) != Some(expected.as_slice())
        {
            return Err(invalid());
        }
        cursor = argument_end;
    }
    for argument in &custody.arguments {
        if argument.code_offset != cursor {
            return Err(invalid());
        }
        cursor = cursor
            .checked_add(argument.byte_count)
            .ok_or_else(invalid)?;
    }
    let native_call_start = match target.architecture {
        Architecture::X86_64 => relocation.offset.checked_sub(1).ok_or_else(invalid)?,
        Architecture::Aarch64 => relocation.offset,
    };
    if cursor != native_call_start {
        return Err(invalid());
    }
    cursor = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    if let Some(area) = outbound {
        if area.release_offset != cursor {
            return Err(invalid());
        }
        cursor = area
            .release_offset
            .checked_add(area.release_byte_count)
            .ok_or_else(invalid)?;
    }
    if let Some(home) = custody
        .structural_result
        .as_ref()
        .and_then(|result| result.result_home.as_ref())
    {
        if home.code_offset != cursor
            || !super::result_home::exact_storage(
                target,
                custody,
                &function.internal_unit_calls,
                function_stack.frame_bytes,
                &function.unit_parameter_homes,
                &function.unit_scalar_homes,
                function.parameter_abi.as_ref(),
                function
                    .unit_stack
                    .and_then(|stack| stack.aarch64_return_link)
                    .map(|link| link.frame_byte_offset),
                !function.unit_continuations.is_empty(),
            )
        {
            return Err(invalid());
        }
        cursor = cursor.checked_add(home.byte_count).ok_or_else(invalid)?;
        if function.bytes.get(home.code_offset..cursor) != Some(home.bytes.as_slice()) {
            return Err(invalid());
        }
    }
    if custody
        .code_offset
        .checked_add(custody.byte_count)
        .is_none_or(|end| end != cursor)
    {
        return Err(invalid());
    }
    Ok(())
}
