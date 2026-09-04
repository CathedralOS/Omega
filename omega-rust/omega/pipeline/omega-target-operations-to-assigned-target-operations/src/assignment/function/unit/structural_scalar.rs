//! Independent custody replay for the bounded attached-Unit projected store
//! and discarded structural-scalar call lane.

mod layout;

use crate::assignment::shared::*;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use omega_target_operations::{
    AbstractResult, TargetStructuralArgument, TargetUnitBody, TargetUnitScalarCallArgument,
};
use psi_core::{MachineId, OperationId, ScalarType, StructuralFieldId, StructuralTypeId};
use psi_terminal::{
    ClaimTransfer, CrashRouteBucket, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeShape,
};
use std::collections::BTreeSet;

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
    {
        return Err(invalid());
    }
    let ScalarType::Integer(scalar_type) = source.scalar_type() else {
        return Err(invalid());
    };
    let expected_shape =
        super::scalar_call::fixed_integer_shape(source.source_value(), scalar_type)
            .map_err(|_| invalid())?;
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let expected_offset = scalar_field_offset_at_path(
        destination.structural_type,
        path,
        field,
        ScalarType::Integer(scalar_type),
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
                    let ScalarType::Integer(integer) = parameter.scalar_type else {
                        return Err(invalid());
                    };
                    let shape = super::scalar_call::fixed_integer_shape(parameter.value, integer)
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
                || body.scalar_parameters.len() != 1
                || scalar_parameter_index != 0
                || scalar_parameter.value != source_value
                || scalar_parameter.scalar_type != source_type
                || scalar_parameter.placement.shape != expected_shape
                || body.call_plan != expected_plan
                || body.call_plan.parameters.first() != Some(&scalar_parameter.placement)
                || body.call_plan.parameters.get(1) != Some(destination_placement)
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
        TargetUnitScalarArgumentSource::IntegerImmediate { .. } => {
            let assigned = super::scalar_call::assign_known_unit_scalar_source(
                source,
                preceding_operations,
                &BTreeMap::new(),
            )
            .ok_or_else(invalid)?;
            if !matches!(
                assigned,
                AssignedUnitScalarArgumentSource::IntegerImmediate { .. }
            ) {
                return Err(invalid());
            }
            assigned
        }
        _ => return Err(invalid()),
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

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_result_call(
    machine: MachineId,
    attachment: Option<StructuralTypeId>,
    body: &TargetUnitBody,
    target: NativeTarget,
    psi_operation: OperationId,
    result: &psi_terminal::StructuralOperationResult,
    callee: MachineId,
    callee_result: &psi_terminal::StructuralResultDeclaration,
    call_plan: &omega_calling_conventions::CallPlan,
    scalar_arguments: &[TargetUnitScalarCallArgument],
    arguments: &[TargetStructuralArgument],
    claim_transfers: &[ClaimTransfer],
    returned_claim_transfers: &[psi_terminal::StructuralResultClaimTransfer],
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[CrashRouteBucket],
    preceding_operations: &[TargetUnitOperation],
    assigned_scalar_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::StructuralScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let ([scalar_argument], [argument]) = (scalar_arguments, arguments) else {
        return Err(invalid());
    };
    let declarations = declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let Some(declaration) = declarations.get(&result.structural_type).copied() else {
        return Err(invalid());
    };
    let exact_record = matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields }
            if matches!(
                fields.as_slice(),
                [field]
                    if matches!(
                        field.field_type,
                        StructuralFieldType::Scalar(ScalarType::Integer(integer))
                            if integer.carrier() == psi_core::IntegerCarrier::Fixed
                                && integer.bits() == 64
                    )
            )
    );
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_argument.placement.shape, argument.shape],
            result: Some(argument.shape),
        },
    )
    .map_err(|_| invalid())?;
    let root = body
        .parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .ok_or_else(invalid)?;
    if attachment.is_some()
        || !exact_record
        || expected_plan != *call_plan
        || call_plan.parameters.as_slice()
            != [
                scalar_argument.placement.clone(),
                argument.destination.clone(),
            ]
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(argument.shape)
        || scalar_argument.parameter_index != 0
        || result.structural_type != callee_result.structural_type
        || result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || callee_result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !callee_result.qualifications.is_empty()
        || !callee_result.projected_qualifications.is_empty()
        || root.structural_type != argument.root_structural_type
        || root.structural_type != argument.structural_type
        || root.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || root.access != psi_terminal::StructuralAccess::Owned
        || !root.projected_qualifications.is_empty()
        || !argument.path.is_empty()
        || argument.access != psi_terminal::StructuralAccess::Owned
        || argument.shape != ValueShape::integer(8, 8)
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || argument.source != root.placement
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
    {
        return Err(invalid());
    }
    super::scalar_call::validate_placement_registers(
        scalar_argument.source.source_value(),
        &scalar_argument.placement,
        target,
    )
    .map_err(|_| invalid())?;
    let assigned_scalar_source = super::scalar_call::assign_known_unit_scalar_source(
        scalar_argument.source,
        preceding_operations,
        assigned_scalar_homes,
    )
    .ok_or_else(invalid)?;
    let assigned_scalar_argument = AssignedUnitScalarCallArgument {
        parameter_index: 0,
        source: assigned_scalar_source,
        destination: super::scalar_call::assigned_unit_scalar_destination(
            scalar_argument.source.source_value(),
            &scalar_argument.placement,
            target,
        )
        .map_err(|_| invalid())?,
    };
    Ok(AssignedUnitOperation::StructuralResultCall {
        psi_operation,
        result: result.clone(),
        callee,
        callee_result: callee_result.clone(),
        call_plan: call_plan.clone(),
        scalar_arguments: vec![assigned_scalar_argument],
        copies: vec![AssignedAggregateCopy {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: argument.root_structural_type,
            structural_type: argument.structural_type,
            shape: argument.shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: argument.source.clone(),
            destination: argument.destination.clone(),
        }],
        claim_transfers: Vec::new(),
        returned_claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    })
}
