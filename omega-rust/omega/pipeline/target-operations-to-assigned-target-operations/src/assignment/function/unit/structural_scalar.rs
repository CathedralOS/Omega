//! Independent custody replay for the bounded attached-Unit projected store
//! and discarded structural-scalar call lane.

mod layout;

use crate::assignment::shared::*;
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use semantic_vocabulary::{
    MachineId, OperationId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use std::collections::BTreeSet;
use target_operations::{
    AbstractResult, TargetStructuralArgument, TargetUnitBody, TargetUnitScalarCallArgument,
};
use terminal_psi::{
    ClaimTransfer, CrashRouteBucket, StructuralParameterDeclaration, StructuralPathSegment,
};

pub(in crate::assignment::function) use layout::{
    declaration_map, direct_scalar_field_offset, resolve_field_path, resolve_projection_path,
    scalar_field_offset_at_path, structural_value_shape,
};

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
    preceding_assigned_operations: &[AssignedUnitOperation],
    target: NativeTarget,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::StructuralScalarFieldStoreCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    if (destination.is_self && attachment != Some(destination.structural_type))
        || !matches!(
            destination.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        )
        || parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || &parameter.placement != destination_placement
        || parameter.shape != destination_placement.shape
    {
        return Err(invalid());
    }
    let source_scalar_type = source.scalar_type();
    let expected_shape = projected_store_scalar_shape(source.source_value(), source_scalar_type)
        .map_err(|_| invalid())?;
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let expected_offset = scalar_field_offset_at_path(
        destination.structural_type,
        path,
        field,
        source_scalar_type,
        &declarations,
    )
    .ok_or_else(invalid)?;
    if expected_offset != field_byte_offset
        || field_byte_offset
            .checked_add(u32::from(expected_shape.byte_size))
            .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
    {
        return Err(invalid());
    }
    let assigned_source = match source {
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type: source_type,
        } => {
            let scalar_parameter_index = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let scalar_parameter = body
                .scalar_parameters
                .get(scalar_parameter_index)
                .ok_or_else(invalid)?;
            let parameter_shapes = body
                .scalar_parameters
                .iter()
                .map(|parameter| {
                    let shape =
                        projected_store_scalar_shape(parameter.value, parameter.scalar_type)
                            .map_err(|_| invalid())?;
                    (parameter.placement.shape == shape)
                        .then_some(shape)
                        .ok_or_else(invalid)
                })
                .chain(body.parameters.iter().map(|parameter| Ok(parameter.shape)))
                .collect::<Result<Vec<_>, AssignmentError>>()?;
            let expected_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes,
                    result: None,
                },
            )
            .map_err(|_| invalid())?;
            let location = match scalar_parameter.placement.locations.as_slice() {
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] if *byte_size == expected_shape.byte_size => {
                    crate::assignment::placement::require_register_architecture(
                        source_value,
                        *register,
                        target.architecture,
                    )?;
                    AssignedScalarLocation::Register(*register)
                }
                [
                    ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] if *byte_size == expected_shape.byte_size => {
                    AssignedScalarLocation::IncomingStack {
                        byte_offset: *stack_byte_offset,
                    }
                }
                _ => return Err(invalid()),
            };
            if body.parameters.len() != 1
                || scalar_parameter.value != source_value
                || scalar_parameter.scalar_type != source_type
                || scalar_parameter.placement.shape != expected_shape
                || body.call_plan != expected_plan
                || body.call_plan.parameters.get(scalar_parameter_index)
                    != Some(&scalar_parameter.placement)
                || body.call_plan.parameters.get(body.scalar_parameters.len())
                    != Some(destination_placement)
            {
                return Err(invalid());
            }
            AssignedUnitScalarArgumentSource::Parameter {
                parameter_index,
                source_value,
                scalar_type: source_type,
                location,
            }
        }
        TargetUnitScalarArgumentSource::IntegerImmediate { .. }
        | TargetUnitScalarArgumentSource::BooleanImmediate { .. } => {
            let assigned = super::scalar_call::assign_known_unit_scalar_source(
                source,
                preceding_operations,
                &BTreeMap::new(),
            )
            .ok_or_else(invalid)?;
            if !matches!(
                (source, assigned),
                (
                    TargetUnitScalarArgumentSource::IntegerImmediate { .. },
                    AssignedUnitScalarArgumentSource::IntegerImmediate { .. },
                ) | (
                    TargetUnitScalarArgumentSource::BooleanImmediate { .. },
                    AssignedUnitScalarArgumentSource::BooleanImmediate { .. },
                )
            ) {
                return Err(invalid());
            }
            assigned
        }
        TargetUnitScalarArgumentSource::Home(home) => {
            let target_matches = preceding_operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::ScalarCall { result_home, .. }
                            if *result_home == home
                    )
                })
                .count();
            let assigned = preceding_assigned_operations
                .iter()
                .filter_map(|operation| match operation {
                    AssignedUnitOperation::ScalarCall { result_home, .. }
                        if result_home.defining_operation == home.defining_operation
                            && result_home.source_value == home.source_value
                            && result_home.scalar_type == home.scalar_type
                            && result_home.shape == home.shape =>
                    {
                        Some(*result_home)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [assigned] = assigned.as_slice() else {
                return Err(invalid());
            };
            if target_matches != 1
                || assigned.scalar_type != source_scalar_type
                || assigned.shape != expected_shape
            {
                return Err(invalid());
            }
            AssignedUnitScalarArgumentSource::Home(*assigned)
        }
    };
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

fn projected_store_scalar_shape(
    value: ValueId,
    scalar_type: ScalarType,
) -> Result<ValueShape, AssignmentError> {
    match scalar_type {
        ScalarType::Boolean => Ok(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => super::scalar_call::fixed_integer_shape(value, integer),
        ScalarType::IeeeFloat(_) => Err(AssignmentError::UnitScalarCallSourceMismatch(value)),
    }
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
    call_plan: &calling_conventions::CallPlan,
    scalar_arguments: &[TargetUnitScalarCallArgument],
    arguments: &[TargetStructuralArgument],
    claim_transfers: &[ClaimTransfer],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
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
                        match argument.source.scalar_type() {
                            ScalarType::Integer(integer) => integer,
                            _ => return Err(invalid()),
                        },
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
            parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && parameter.access == terminal_psi::StructuralAccess::Owned
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
                || (free_whole_affine && argument.access != terminal_psi::StructuralAccess::Owned)
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
        transport: super::scalar_transport::assign(
            call_plan,
            &assigned_scalar_arguments,
            target,
            super::scalar_transport::CallTransportKind::Mixed,
        )?,
        scalar_arguments: assigned_scalar_arguments,
        copies,
        claim_transfers: claim_transfers.to_vec(),
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}
