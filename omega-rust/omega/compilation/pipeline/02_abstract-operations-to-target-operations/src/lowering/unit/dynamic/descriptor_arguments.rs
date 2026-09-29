//! Direct calls whose ABI arguments are dynamic descriptor pairs.

use super::{LoweredDynamicCall, dynamic_result_home};
use crate::LoweringError;
use crate::lowering::structural_layout::resolve_structural_field_path;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractDynamicCallResult, AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    TargetDynamicDescriptorArgument, TargetDynamicDescriptorInstanceArgument,
    TargetDynamicDescriptorInstanceSource, TargetDynamicDescriptorParameterAbi,
    TargetDynamicDispatch, TargetStructuralParameter,
};
use terminal_psi::StructuralPathSegment;

/// Lower one direct Unit- or scalar-result call that forwards descriptors.
/// Both result roles share the callee join and the descriptor-pair plan; the
/// role selects only the plan's result placement and whether a scalar home is
/// defined.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    caller_dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<LoweredDynamicCall, LoweringError> {
    let AbstractOperation::CallStructural {
        psi_operation,
        result,
        callee,
        arguments,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        unreachable!("descriptor-argument lowering receives only direct calls")
    };
    let result = match result {
        AbstractStructuralCallResult::Unit => AbstractDynamicCallResult::Unit,
        AbstractStructuralCallResult::Scalar(result) => AbstractDynamicCallResult::Scalar(*result),
        AbstractStructuralCallResult::Structural { .. } => {
            unreachable!("a structural-result call never lowers as a dynamic call")
        }
    };
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    // A non-entry helper may forward its own borrowed descriptor parameters
    // without an attachment; projected or rebound sources still require the
    // attached structural-parameter lane they read from. A call forwarding
    // descriptors carries no scalar arguments.
    if (function.attachment.is_none()
        && dynamic_arguments.iter().any(|argument| {
            !matches!(
                argument.source,
                AbstractDynamicDescriptorSource::Parameter(_)
            )
        }))
        || !arguments.is_empty()
        || !structural_arguments.is_empty()
        || dynamic_arguments.is_empty()
    {
        return Err(invalid());
    }
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let callee_dynamic_parameters = callee_function
        .operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        .filter_map(|operation| match operation {
            AbstractOperation::DynamicDescriptorParameter { parameter } => Some(parameter),
            _ => None,
        })
        .collect::<Vec<_>>();
    let result_matches = match (&callee_function.result, result) {
        (AbstractFunctionResult::Unit, AbstractDynamicCallResult::Unit) => true,
        (AbstractFunctionResult::Scalar(declared), AbstractDynamicCallResult::Scalar(result)) => {
            declared.scalar_type == result.scalar_type
        }
        _ => false,
    };
    if !result_matches
        || !callee_function.parameters.is_empty()
        || !callee_function.structural_parameters.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || callee_dynamic_parameters.len() != dynamic_arguments.len()
        || dynamic_arguments
            .iter()
            .enumerate()
            .any(|(ordinal, argument)| {
                argument.target != *callee_dynamic_parameters[ordinal]
                    || argument.target.ordinal != u32::try_from(ordinal).unwrap_or(u32::MAX)
                    || argument.target.source_position != u32::try_from(ordinal).unwrap_or(u32::MAX)
                    || !argument.has_complete_custody(function.machine, *psi_operation, *callee)
            })
    {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(u32::from(pointer_size), pointer_alignment);
    let result_home = dynamic_result_home(*psi_operation, result)?;
    let result_shape = result_home.map(|home| home.shape);
    let descriptor_parameter_count = dynamic_arguments.len().checked_mul(2).ok_or_else(invalid)?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape; descriptor_parameter_count],
            result: result_shape,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != result_shape
        || call_plan.parameters.len() != descriptor_parameter_count
    {
        return Err(invalid());
    }
    let arguments = prepare_dynamic_arguments(
        function.machine,
        *psi_operation,
        dynamic_arguments,
        &call_plan,
        parameters_by_place,
        caller_dynamic_parameters,
        structural_types,
        shape_cache,
        active,
    )?;
    Ok(LoweredDynamicCall {
        dispatch: TargetDynamicDispatch::DescriptorArguments {
            callee: *callee,
            arguments,
            claim_transfers: claim_transfers.clone(),
        },
        call_plan,
        result_home,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_dynamic_arguments(
    machine: MachineId,
    psi_operation: OperationId,
    dynamic_arguments: &[AbstractDynamicDescriptorArgument],
    call_plan: &CallPlan,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    caller_dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    structural_types: &StructuralTypeLookup<'_>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Vec<TargetDynamicDescriptorArgument>, LoweringError> {
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine,
        operation: psi_operation,
    };
    dynamic_arguments
        .iter()
        .enumerate()
        .map(|(ordinal, custody)| {
            let instance_index = ordinal.checked_mul(2).ok_or_else(invalid)?;
            let (Some(instance_destination), Some(table_destination)) = (
                call_plan.parameters.get(instance_index),
                call_plan.parameters.get(instance_index + 1),
            ) else {
                return Err(invalid());
            };
            // A caller's own borrowed descriptor parameter passes both words
            // through unchanged: its signature-bound placements are the
            // argument sources, and the callee plan supplies the destinations.
            if let AbstractDynamicDescriptorSource::Parameter(parameter) = &custody.source {
                let parameter = caller_dynamic_parameters
                    .iter()
                    .find(|abi| abi.parameter == *parameter)
                    .ok_or_else(invalid)?;
                return Ok(TargetDynamicDescriptorArgument {
                    custody: custody.clone(),
                    instance: TargetDynamicDescriptorInstanceSource::Parameter {
                        parameter: parameter.clone(),
                        destination: instance_destination.clone(),
                    },
                    table_destination: table_destination.clone(),
                });
            }
            let selection = match &custody.source {
                AbstractDynamicDescriptorSource::Selection { selection, .. } => selection,
                AbstractDynamicDescriptorSource::Rebound { rebound, .. } => rebound,
                AbstractDynamicDescriptorSource::Parameter(_) => {
                    unreachable!("parameter sources return before projection")
                }
            };
            let root = parameters_by_place
                .get(&selection.source.place)
                .copied()
                .ok_or_else(invalid)?;
            if custody.target.access != selection.source.access
                || selection.source.path.is_empty()
                || selection
                    .source
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(invalid());
            }
            let (projected_type, projected_shape, source_byte_offset) =
                resolve_structural_field_path(
                    root.structural_type,
                    &selection.source.path,
                    structural_types,
                    shape_cache,
                    active,
                )
                .map_err(|_| invalid())?;
            if source_byte_offset
                .checked_add(projected_shape.byte_size)
                .is_none_or(|end| end > root.shape.byte_size)
            {
                return Err(invalid());
            }
            Ok(TargetDynamicDescriptorArgument {
                custody: custody.clone(),
                instance: TargetDynamicDescriptorInstanceSource::Projection(
                    TargetDynamicDescriptorInstanceArgument {
                        place: selection.source.place,
                        access: selection.source.access,
                        path: selection.source.path.clone(),
                        root_structural_type: root.structural_type,
                        structural_type: projected_type,
                        shape: projected_shape,
                        source_byte_offset,
                        source: root.placement.clone(),
                        destination: instance_destination.clone(),
                    },
                ),
                table_destination: table_destination.clone(),
            })
        })
        .collect()
}
