//! Structural calls whose ABI arguments are dynamic descriptor pairs.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractDynamicDescriptorArgument;

use super::{
    AbstractDynamicDescriptorSource, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    CallPlan, CallSignature, CallingPolicy, KnownUnitInteger, LoweringError, MachineId,
    NativeTarget, OperationId, PlaceId, ScalarType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, TargetDynamicDescriptorArgument,
    TargetDynamicDescriptorInstanceArgument, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarHomeRequirement, TerminalPsiProvenance, ValueId, ValueShape,
    evaluate_call_plan, insert_known_unit_integer, resolve_structural_field_path, scalar_shape,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::unit) fn lower_dynamic_argument_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    let AbstractOperation::CallStructuralScalarWithDynamicArguments {
        psi_operation,
        result,
        callee,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("dynamic-argument scalar lowering receives only its exact role")
    };
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    if function.attachment.is_none()
        || !structural_arguments.is_empty()
        || dynamic_arguments.is_empty()
    {
        return Err(invalid());
    }
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let callee_result = callee_function.result.scalar().ok_or_else(invalid)?;
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
    if !callee_function.parameters.is_empty()
        || !callee_function.structural_parameters.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
        || callee_result.scalar_type != result.scalar_type
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
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    if !matches!(
        result.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            result.value,
        ));
    }
    let descriptor_parameter_count = dynamic_arguments.len().checked_mul(2).ok_or_else(invalid)?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape; descriptor_parameter_count],
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape)
        || call_plan.parameters.len() != descriptor_parameter_count
    {
        return Err(invalid());
    }
    let target_dynamic_arguments = prepare_dynamic_arguments(
        function.machine,
        *psi_operation,
        dynamic_arguments,
        &call_plan,
        parameters_by_place,
        structural_types,
        shape_cache,
        active,
    )?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: result_shape,
    };
    if matches!(result.scalar_type, ScalarType::Integer(_)) {
        insert_known_unit_integer(
            scalar_values,
            result.value,
            KnownUnitInteger::Home(result_home),
        )?;
    }
    operations.push(
        TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
            psi_operation: *psi_operation,
            result: *result,
            callee: *callee,
            call_plan,
            result_home,
            structural_arguments: Vec::new(),
            dynamic_arguments: target_dynamic_arguments,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    );
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::unit) fn lower_dynamic_argument_unit_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallUnitWithDynamicArguments {
        psi_operation,
        callee,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("dynamic-argument Unit lowering receives only its exact role")
    };
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    if function.attachment.is_none()
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
    if callee_function.result != AbstractFunctionResult::Unit
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
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let descriptor_parameter_count = dynamic_arguments.len().checked_mul(2).ok_or_else(invalid)?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape; descriptor_parameter_count],
            result: None,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.is_some() || call_plan.parameters.len() != descriptor_parameter_count {
        return Err(invalid());
    }
    let target_dynamic_arguments = prepare_dynamic_arguments(
        function.machine,
        *psi_operation,
        dynamic_arguments,
        &call_plan,
        parameters_by_place,
        structural_types,
        shape_cache,
        active,
    )?;
    operations.push(
        TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
            psi_operation: *psi_operation,
            callee: *callee,
            call_plan,
            structural_arguments: Vec::new(),
            dynamic_arguments: target_dynamic_arguments,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    );
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_dynamic_arguments(
    machine: MachineId,
    psi_operation: OperationId,
    dynamic_arguments: &[AbstractDynamicDescriptorArgument],
    call_plan: &CallPlan,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
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
            let selection = match &custody.source {
                AbstractDynamicDescriptorSource::Selection { selection, .. } => selection,
                AbstractDynamicDescriptorSource::Rebound { rebound, .. } => rebound,
                AbstractDynamicDescriptorSource::Parameter(_) => return Err(invalid()),
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
                .checked_add(u32::from(projected_shape.byte_size))
                .is_none_or(|end| end > u32::from(root.shape.byte_size))
            {
                return Err(invalid());
            }
            let instance_index = ordinal.checked_mul(2).ok_or_else(invalid)?;
            Ok(TargetDynamicDescriptorArgument {
                custody: custody.clone(),
                instance: TargetDynamicDescriptorInstanceArgument {
                    place: selection.source.place,
                    access: selection.source.access,
                    path: selection.source.path.clone(),
                    root_structural_type: root.structural_type,
                    structural_type: projected_type,
                    shape: projected_shape,
                    source_byte_offset,
                    source: root.placement.clone(),
                    destination: call_plan.parameters[instance_index].clone(),
                },
                table_destination: call_plan.parameters[instance_index + 1].clone(),
            })
        })
        .collect()
}
