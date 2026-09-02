//! Custody and ABI replay for ordinary calls carrying existential descriptors.

use crate::assignment::shared::*;
use omega_calling_conventions::{CallSignature, ValueLocation};
use psi_core::{OperationId, ScalarType};
use psi_terminal::{ClaimTransfer, CrashRouteBucket, StructuralPathSegment};

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    machine: MachineId,
    body: &omega_target_operations::TargetUnitBody,
    target: NativeTarget,
    psi_operation: OperationId,
    result: Option<omega_target_operations::AbstractResult>,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    result_home: Option<omega_target_operations::TargetUnitScalarHomeRequirement>,
    structural_arguments: &[omega_target_operations::TargetStructuralArgument],
    dynamic_arguments: &[TargetDynamicDescriptorArgument],
    claim_transfers: &[ClaimTransfer],
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[CrashRouteBucket],
    assigned_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    next_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::DynamicDescriptorCallArgumentMismatch {
        machine,
        operation: psi_operation,
    };
    if !structural_arguments.is_empty() || dynamic_arguments.is_empty() {
        return Err(invalid());
    }
    let result_shape = result
        .map(|result| match result.scalar_type {
            ScalarType::Boolean => Ok(ValueShape::integer(1, 1)),
            ScalarType::Integer(integer_type) => {
                super::scalar_call::fixed_integer_shape(result.value, integer_type)
                    .map_err(|_| invalid())
            }
            ScalarType::IeeeFloat(_) => Err(invalid()),
        })
        .transpose()?;
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let parameter_count = dynamic_arguments.len().checked_mul(2).ok_or_else(invalid)?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape; parameter_count],
            result: result_shape,
        },
    )
    .map_err(|_| invalid())?;
    if &expected_plan != call_plan
        || call_plan.result.as_ref().map(|placement| placement.shape) != result_shape
        || call_plan.parameters.len() != parameter_count
        || result.zip(result_home).is_some_and(|(result, home)| {
            home.defining_operation != psi_operation
                || home.source_value != result.value
                || home.scalar_type != result.scalar_type
                || Some(home.shape) != result_shape
        })
        || result.is_some() != result_home.is_some()
    {
        return Err(invalid());
    }

    let declarations =
        super::structural_scalar::declaration_map(&body.structural_types).ok_or_else(invalid)?;
    let dynamic_arguments = dynamic_arguments
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            if !argument
                .custody
                .has_complete_custody(machine, psi_operation, callee)
            {
                return Err(invalid());
            }
            let AbstractDynamicDescriptorSource::Rebound { rebound, .. } = &argument.custody.source
            else {
                return Err(invalid());
            };
            let root = body
                .parameters
                .iter()
                .find(|parameter| parameter.place == argument.instance.place)
                .ok_or_else(invalid)?;
            let (projected_type, projected_shape, projected_offset) =
                super::structural_scalar::resolve_field_path(
                    root.structural_type,
                    &argument.instance.path,
                    &declarations,
                )
                .ok_or_else(invalid)?;
            let instance_index = ordinal.checked_mul(2).ok_or_else(invalid)?;
            let instance_placement = call_plan
                .parameters
                .get(instance_index)
                .ok_or_else(invalid)?;
            let table_placement = call_plan
                .parameters
                .get(instance_index + 1)
                .ok_or_else(invalid)?;
            if argument.instance.path.is_empty()
                || argument
                    .instance
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                || argument.instance.place != rebound.source.place
                || argument.instance.access != rebound.source.access
                || argument.instance.path != rebound.source.path
                || argument.instance.root_structural_type != root.structural_type
                || argument.instance.structural_type != projected_type
                || argument.instance.shape != projected_shape
                || argument.instance.source_byte_offset != projected_offset
                || argument.instance.source != root.placement
                || &argument.instance.destination != instance_placement
                || &argument.table_destination != table_placement
                || instance_placement.shape != pointer_shape
                || table_placement.shape != pointer_shape
                || projected_offset
                    .checked_add(u32::from(projected_shape.byte_size))
                    .is_none_or(|end| end > u32::from(root.shape.byte_size))
            {
                return Err(invalid());
            }
            let instance_destination =
                exact_pointer_register(instance_placement, target).ok_or_else(invalid)?;
            let table_destination =
                exact_pointer_register(table_placement, target).ok_or_else(invalid)?;
            Ok(AssignedDynamicDescriptorArgument {
                custody: argument.custody.clone(),
                instance: AssignedDynamicDescriptorInstanceArgument {
                    place: argument.instance.place,
                    access: argument.instance.access,
                    path: argument.instance.path.clone(),
                    root_structural_type: argument.instance.root_structural_type,
                    structural_type: argument.instance.structural_type,
                    shape: argument.instance.shape,
                    source_byte_offset: argument.instance.source_byte_offset,
                    source: argument.instance.source.clone(),
                    destination: instance_destination,
                },
                table_destination,
            })
        })
        .collect::<Result<Vec<_>, AssignmentError>>()?;
    let assigned_result_home = result
        .zip(result_home)
        .map(|(result, result_home)| {
            super::scalar_call::allocate_unit_scalar_home(
                result_home,
                assigned_homes,
                next_home,
                AssignmentError::UnitScalarCallSourceMismatch(result.value),
            )
        })
        .transpose()?;

    Ok(match (result, assigned_result_home) {
        (Some(result), Some(result_home)) => {
            AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                psi_operation,
                result,
                callee,
                call_plan: call_plan.clone(),
                result_home,
                copies: Vec::new(),
                dynamic_arguments,
                claim_transfers: claim_transfers.to_vec(),
                requirement_obligations: requirement_obligations.to_vec(),
                crash_continuations: crash_continuations.to_vec(),
            }
        }
        (None, None) => AssignedUnitOperation::StructuralUnitCallWithDynamicArguments {
            psi_operation,
            callee,
            call_plan: call_plan.clone(),
            copies: Vec::new(),
            dynamic_arguments,
            claim_transfers: claim_transfers.to_vec(),
            requirement_obligations: requirement_obligations.to_vec(),
            crash_continuations: crash_continuations.to_vec(),
        },
        _ => return Err(invalid()),
    })
}

fn exact_pointer_register(
    placement: &omega_calling_conventions::ValuePlacement,
    target: NativeTarget,
) -> Option<MachineRegister> {
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    (usize::from(*byte_size) == target.pointer_size
        && register.architecture() == target.architecture)
        .then_some(*register)
}
