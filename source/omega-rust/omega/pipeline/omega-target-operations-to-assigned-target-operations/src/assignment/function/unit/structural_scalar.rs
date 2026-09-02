//! Independent custody replay for the bounded attached-Unit projected store
//! and discarded structural-scalar call lane.

use crate::assignment::shared::*;
use omega_calling_conventions::{CallSignature, ValuePlacement, ValueShape};
use omega_target_operations::{
    AbstractResult, TargetStructuralArgument, TargetUnitBody, TargetUnitScalarCallArgument,
};
use psi_core::{
    IeeeFloatFormat, MachineId, OperationId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, ClaimTransfer, CrashRouteBucket, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};
use std::collections::BTreeSet;

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_field_store(
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
    body: &TargetUnitBody,
    psi_operation: OperationId,
    destination: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    destination_placement: &ValuePlacement,
    field_byte_offset: u32,
    source: TargetUnitScalarArgumentSource,
    preceding_operations: &[TargetUnitOperation],
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::StructuralScalarFieldStoreCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    if !destination.is_self
        || attachment != Some(destination.structural_type)
        || !matches!(
            destination.access,
            psi_terminal::StructuralAccess::MutableBorrow
                | psi_terminal::StructuralAccess::WriteOnlyBorrow
        )
        || parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || &parameter.placement != destination_placement
        || parameter.shape != destination_placement.shape
        || path.is_empty()
    {
        return Err(invalid());
    }
    let TargetUnitScalarArgumentSource::IntegerImmediate { scalar_type, .. } = source else {
        return Err(invalid());
    };
    let expected_shape =
        super::scalar_call::fixed_integer_shape(source.source_value(), scalar_type)
            .map_err(|_| invalid())?;
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let (carrier_type, _, carrier_offset) =
        resolve_field_path(destination.structural_type, path, &declarations).ok_or_else(invalid)?;
    let scalar_offset =
        direct_integer_field_offset(carrier_type, field, scalar_type, &declarations)
            .ok_or_else(invalid)?;
    let expected_offset = carrier_offset
        .checked_add(scalar_offset)
        .ok_or_else(invalid)?;
    if expected_offset != field_byte_offset
        || field_byte_offset
            .checked_add(u32::from(expected_shape.byte_size))
            .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
    {
        return Err(invalid());
    }
    let assigned_source = super::scalar_call::assign_known_unit_scalar_source(
        source,
        preceding_operations,
        &BTreeMap::new(),
    )
    .ok_or_else(invalid)?;
    if !matches!(
        assigned_source,
        AssignedUnitScalarArgumentSource::IntegerImmediate { .. }
    ) {
        return Err(invalid());
    }
    Ok(AssignedUnitOperation::StructuralScalarFieldStore {
        psi_operation,
        destination: destination.clone(),
        path: path.to_vec(),
        field,
        destination_placement: destination_placement.clone(),
        field_byte_offset,
        source: assigned_source,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_call(
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
    body: &TargetUnitBody,
    target: NativeTarget,
    psi_operation: OperationId,
    result: AbstractResult,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    scalar_arguments: &[TargetUnitScalarCallArgument],
    arguments: &[TargetStructuralArgument],
    claim_transfers: &[ClaimTransfer],
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[CrashRouteBucket],
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::StructuralScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let ScalarType::Integer(integer_type) = result.scalar_type else {
        return Err(invalid());
    };
    let result_shape = super::scalar_call::fixed_integer_shape(result.value, integer_type)
        .map_err(|_| invalid())?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_arguments
                .iter()
                .map(|argument| argument.placement.shape)
                .chain(arguments.iter().map(|argument| argument.shape))
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| invalid())?;
    if &expected_plan != call_plan
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape)
        || call_plan.parameters.len() != scalar_arguments.len() + arguments.len()
    {
        return Err(invalid());
    }
    let assigned_scalar_arguments = scalar_arguments
        .iter()
        .enumerate()
        .map(|(parameter_index, argument)| {
            if usize::try_from(argument.parameter_index) != Ok(parameter_index)
                || call_plan.parameters.get(parameter_index) != Some(&argument.placement)
                || argument.placement.shape
                    != super::scalar_call::fixed_integer_shape(
                        argument.source.source_value(),
                        argument.source.scalar_type(),
                    )
                    .map_err(|_| invalid())?
            {
                return Err(invalid());
            }
            super::scalar_call::validate_placement_registers(
                argument.source.source_value(),
                &argument.placement,
                target,
            )
            .map_err(|_| invalid())?;
            let source = super::scalar_call::assign_known_unit_scalar_source(
                argument.source,
                preceding_operations,
                assigned_scalar_homes,
            )
            .ok_or_else(invalid)?;
            Ok(AssignedUnitScalarCallArgument {
                parameter_index: argument.parameter_index,
                source,
                destination: super::scalar_call::assigned_unit_scalar_destination(
                    argument.source.source_value(),
                    &argument.placement,
                    target,
                )
                .map_err(|_| invalid())?,
            })
        })
        .collect::<Result<Vec<_>, AssignmentError>>()?;
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let free_whole_affine = attachment.is_none()
        && arguments.len() == body.parameters.len()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && body.parameters.iter().all(|parameter| {
            parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                && parameter.access == psi_terminal::StructuralAccess::Owned
                && parameter.projected_qualifications.is_empty()
        })
        && arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<BTreeSet<_>>()
            .len()
            == arguments.len();
    let copies = arguments
        .iter()
        .enumerate()
        .map(|(parameter_index, argument)| {
            let root = body
                .parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
                .ok_or_else(invalid)?;
            let (projected_type, projected_shape, projected_offset) =
                if argument.path.is_empty() && free_whole_affine {
                    (root.structural_type, root.shape, 0)
                } else {
                    resolve_field_path(root.structural_type, &argument.path, &declarations)
                        .ok_or_else(invalid)?
                };
            if (argument.path.is_empty() && !free_whole_affine)
                || (!argument.path.is_empty() && attachment.is_none())
                || (free_whole_affine && argument.access != psi_terminal::StructuralAccess::Owned)
                || argument.root_structural_type != root.structural_type
                || argument.structural_type != projected_type
                || argument.shape != projected_shape
                || argument.source_byte_offset != projected_offset
                || argument.source != root.placement
                || call_plan
                    .parameters
                    .get(parameter_index + scalar_arguments.len())
                    != Some(&argument.destination)
                || argument.destination.shape != argument.shape
                || argument.fixed_array_length.is_some()
                || argument.element_stride.is_some()
                || projected_offset
                    .checked_add(u32::from(projected_shape.byte_size))
                    .is_none_or(|end| end > u32::from(root.shape.byte_size))
            {
                return Err(invalid());
            }
            Ok(AssignedAggregateCopy {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: argument.root_structural_type,
                structural_type: argument.structural_type,
                shape: argument.shape,
                source_byte_offset: argument.source_byte_offset,
                fixed_array_length: argument.fixed_array_length,
                element_stride: argument.element_stride,
                source: argument.source.clone(),
                destination: argument.destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, AssignmentError>>()?;
    Ok(AssignedUnitOperation::StructuralScalarCall {
        psi_operation,
        result,
        callee,
        call_plan: call_plan.clone(),
        scalar_arguments: assigned_scalar_arguments,
        copies,
        claim_transfers: claim_transfers.to_vec(),
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}

pub(in crate::assignment::function) fn declaration_map(
    declarations: &[StructuralTypeDeclaration],
) -> Option<BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>> {
    let map = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    (map.len() == declarations.len()).then_some(map)
}

pub(super) fn resolve_field_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(StructuralTypeId, ValueShape, u32)> {
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let StructuralTypeShape::Record { fields } = &declarations.get(&structural_type)?.shape
        else {
            return None;
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for candidate in fields
            .iter()
            .filter(|candidate| physically_retained_field(candidate))
        {
            if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            let shape = field_shape(
                &candidate.field_type,
                declarations,
                &mut BTreeMap::new(),
                &mut Vec::new(),
            )?;
            local_offset = align(local_offset, u32::from(shape.alignment))?;
            if candidate.identity == *identity {
                let StructuralFieldType::Structural(nested) = candidate.field_type else {
                    return None;
                };
                selected = Some((nested, shape, local_offset));
                break;
            }
            local_offset = local_offset.checked_add(u32::from(shape.byte_size))?;
        }
        let (nested, shape, offset) = selected?;
        total_offset = total_offset.checked_add(offset)?;
        structural_type = nested;
        selected_shape = Some(shape);
    }
    Some((structural_type, selected_shape?, total_offset))
}

pub(in crate::assignment::function) fn direct_integer_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    scalar_type: psi_core::IntegerType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u32> {
    direct_scalar_field_offset(
        structural_type,
        field,
        ScalarType::Integer(scalar_type),
        declarations,
    )
}

pub(in crate::assignment::function) fn direct_scalar_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u32> {
    let StructuralTypeShape::Record { fields } = &declarations.get(&structural_type)?.shape else {
        return None;
    };
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| physically_retained_field(candidate))
    {
        if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
            continue;
        }
        let shape = field_shape(
            &candidate.field_type,
            declarations,
            &mut BTreeMap::new(),
            &mut Vec::new(),
        )?;
        offset = align(offset, u32::from(shape.alignment))?;
        if candidate.id == field {
            return (candidate.field_type == StructuralFieldType::Scalar(scalar_type))
                .then_some(offset);
        }
        offset = offset.checked_add(u32::from(shape.byte_size))?;
    }
    None
}

pub(in crate::assignment::function) fn scalar_field_offset_at_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u32> {
    let (field_owner, path_offset) = match path {
        [] => (structural_type, 0),
        [StructuralPathSegment::Field(identity)] if !identity.is_empty() => {
            let (nested, _, offset) = resolve_field_path(structural_type, path, declarations)?;
            (nested, offset)
        }
        _ => return None,
    };
    path_offset.checked_add(direct_scalar_field_offset(
        field_owner,
        field,
        scalar_type,
        declarations,
    )?)
}

fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    if let Some(shape) = cache.get(&structural_type) {
        return Some(*shape);
    }
    if active.contains(&structural_type) {
        return None;
    }
    active.push(structural_type);
    let shape = match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            let mut size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields
                .iter()
                .filter(|field| physically_retained_field(field))
            {
                if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                let field_shape = field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                size = align(size, u32::from(field_shape.alignment))?;
                size = size.checked_add(u32::from(field_shape.byte_size))?;
            }
            size = align(size, u32::from(alignment))?;
            ValueShape::integer(u16::try_from(size).ok()?, alignment)
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let element = structural_shape(*element, declarations, cache, active)?;
            let stride = align(u32::from(element.byte_size), u32::from(element.alignment))?;
            let size = u64::from(stride).checked_mul(*length)?;
            ValueShape::integer(u16::try_from(size).ok()?, element.alignment)
        }
        _ => return None,
    };
    active.pop();
    cache.insert(structural_type, shape);
    Some(shape)
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean) => Some(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Some(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Some(ValueShape::float(8)),
        StructuralFieldType::Structural(nested) => {
            structural_shape(*nested, declarations, cache, active)
        }
        _ => None,
    }
}

fn physically_retained_field(field: &psi_terminal::StructuralFieldDeclaration) -> bool {
    field.relevance != BindingRelevance::Erased
        && !matches!(field.field_type, StructuralFieldType::Erased { .. })
}

fn align(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}
