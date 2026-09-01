//! Exact projected integer store and structural-scalar call lowering for an
//! attached Unit body.

use super::super::scalar::scalar_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    direct_integer_field_offset, resolve_structural_field_path, structural_shape,
};
use super::scalar_call::KnownUnitInteger;

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
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("structural-scalar call lowering receives only structural scalar calls")
    };
    if function.attachment.is_none() || structural_arguments.is_empty() {
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
    if !callee_function.parameters.is_empty()
        || callee_result.scalar_type != result.scalar_type
        || !callee_function.published_service_ceiling.is_empty()
        || structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
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
            parameters: callee_shapes.clone(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape) {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes)
        .zip(&call_plan.parameters)
        .map(|(((argument, callee_parameter), shape), destination)| {
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            if argument.path.is_empty()
                || argument
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee: *callee,
                    place: argument.place,
                });
            }
            let (projected_type, projected_shape, source_byte_offset) =
                resolve_structural_field_path(
                    source.structural_type,
                    &argument.path,
                    structural_types,
                    shape_cache,
                    active,
                )
                .map_err(|_| {
                    LoweringError::StructuralCallArgumentTypeMismatch {
                        callee: *callee,
                        place: argument.place,
                    }
                })?;
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
        arguments,
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
