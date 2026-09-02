//! Exact projected integer store and structural-scalar call lowering for an
//! attached Unit body.

use super::super::scalar::scalar_shape;
use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    direct_integer_field_offset, resolve_structural_field_path, structural_shape,
};
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
use omega_abstract_operations::AbstractDynamicDescriptorArgument;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_field_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        unreachable!("projected field-store lowering receives only field stores")
    };
    if !function
        .structural_parameters
        .iter()
        .any(|parameter| parameter == destination)
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || path.is_empty()
        || path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let target_parameter = parameters_by_place
        .get(&destination.place)
        .copied()
        .filter(|parameter| {
            parameter.structural_type == destination.structural_type
                && parameter.multiplicity == destination.multiplicity
                && parameter.access == destination.access
                && parameter.projected_qualifications == destination.projected_qualifications
        })
        .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ))?;
    let ScalarType::Integer(integer_type) = value.scalar_type else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    };
    let known_value = scalar_values
        .get(&value.value)
        .copied()
        .ok_or(LoweringError::UnknownValue(value.value))?;
    if known_value.scalar_type() != integer_type
        || !matches!(known_value, KnownUnitInteger::Immediate { .. })
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let (carrier_type, _, carrier_byte_offset) = resolve_structural_field_path(
        destination.structural_type,
        path,
        structural_types,
        shape_cache,
        active,
    )?;
    let scalar_byte_offset =
        direct_integer_field_offset(carrier_type, *field, integer_type, structural_types)?;
    let field_byte_offset = carrier_byte_offset
        .checked_add(scalar_byte_offset)
        .filter(|offset| {
            offset
                .checked_add(u32::from(integer_type.bits().div_ceil(8)))
                .is_some_and(|end| end <= u32::from(target_parameter.shape.byte_size))
        })
        .ok_or(LoweringError::StructuralTypeTooLarge(
            destination.structural_type,
        ))?;
    operations.push(TargetUnitOperation::StructuralScalarFieldStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_placement: target_parameter.placement.clone(),
        field_byte_offset,
        source: known_value.into_target_source(value.value),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_structural_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        arguments: scalar_argument_values,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("structural-scalar call lowering receives only structural scalar calls")
    };
    if structural_arguments.is_empty() {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let Some(callee_result) = callee_function.result.scalar() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    };
    if callee_result.scalar_type != result.scalar_type
        || !callee_function.published_service_ceiling.is_empty()
        || scalar_argument_values.len() != callee_function.parameters.len()
        || structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    let free_whole_affine = function.attachment.is_none()
        && function.structural_parameters.len() == structural_arguments.len()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.structural_parameters.iter().all(|parameter| {
            parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.access == StructuralAccess::Owned
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && callee_function
            .structural_parameters
            .iter()
            .all(|parameter| {
                parameter.multiplicity == StructuralMultiplicity::Affine
                    && parameter.access == StructuralAccess::Owned
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
        && structural_arguments.iter().all(|argument| {
            argument.path.is_empty()
                && argument.access == StructuralAccess::Owned
                && function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == argument.place)
        })
        && structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<BTreeSet<_>>()
            .len()
            == structural_arguments.len();
    let attached_projection = function.attachment.is_some();
    if !free_whole_affine && !attached_projection {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter.scalar_type else {
                return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
            };
            fixed_native_integer_shape(integer_type)
                .ok_or(LoweringError::UnitCallTargetKindMismatch(*callee))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_shapes = callee_function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                shape_cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(callee_shapes.iter().copied())
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape) {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    let scalar_arguments = scalar_argument_values
        .iter()
        .zip(&callee_function.parameters)
        .zip(&scalar_shapes)
        .zip(&call_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let known = scalar_values
                    .get(source_value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*source_value))?;
                let ScalarType::Integer(parameter_type) = parameter.scalar_type else {
                    return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                };
                if known.scalar_type() != parameter_type || placement.shape != *expected_shape {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::UnitCallTargetKindMismatch(*callee))?,
                    source: known.into_target_source(*source_value),
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes)
        .zip(call_plan.parameters.iter().skip(scalar_arguments.len()))
        .map(|(((argument, callee_parameter), shape), destination)| {
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            let (projected_type, projected_shape, source_byte_offset) =
                match argument.path.as_slice() {
                    [] if free_whole_affine => (source.structural_type, source.shape, 0),
                    path @ [StructuralPathSegment::Field(_), ..]
                        if attached_projection
                            && path.iter().all(|segment| {
                                matches!(segment, StructuralPathSegment::Field(_))
                            }) =>
                    {
                        resolve_structural_field_path(
                            source.structural_type,
                            path,
                            structural_types,
                            shape_cache,
                            active,
                        )
                        .map_err(|_| {
                            LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            }
                        })?
                    }
                    _ => {
                        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                            callee: *callee,
                            place: argument.place,
                        });
                    }
                };
            if projected_type != callee_parameter.structural_type
                || projected_shape != shape
                || u32::from(shape.byte_size)
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > u32::from(source.shape.byte_size))
            {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee: *callee,
                    place: argument.place,
                });
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: source.structural_type,
                structural_type: projected_type,
                shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                source: source.placement.clone(),
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    operations.push(TargetUnitOperation::StructuralScalarCall {
        psi_operation: *psi_operation,
        result: *result,
        callee: *callee,
        call_plan,
        scalar_arguments,
        arguments,
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_dynamic_argument_scalar_call(
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
pub(super) fn lower_dynamic_argument_unit_call(
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
