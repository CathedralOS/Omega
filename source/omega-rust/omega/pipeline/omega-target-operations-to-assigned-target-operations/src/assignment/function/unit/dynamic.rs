//! Physical descriptor and optional result-home assignment for rebound dynamic calls.

use super::scalar_call::allocate_unit_scalar_home;
use crate::assignment::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    machine: MachineId,
    target: NativeTarget,
    psi_operation: OperationId,
    result: omega_target_operations::AbstractResult,
    dynamic_dispatch: &omega_target_operations::AbstractReboundDynamicDispatch,
    call_plan: &omega_calling_conventions::CallPlan,
    result_requirement: omega_target_operations::TargetUnitScalarHomeRequirement,
    initial_argument: &omega_target_operations::TargetStructuralArgument,
    rebound_argument: &omega_target_operations::TargetStructuralArgument,
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[psi_terminal::CrashRouteBucket],
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    next_scalar_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::DynamicScalarCallCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    if result.value != result_requirement.source_value
        || result.scalar_type != result_requirement.scalar_type
        || result_requirement.defining_operation != psi_operation
    {
        return Err(invalid());
    }
    let assigned = assign_dynamic_call(
        machine,
        target,
        psi_operation,
        dynamic_dispatch,
        call_plan,
        Some(result_requirement.shape),
        initial_argument,
        rebound_argument,
        next_scalar_home,
        &invalid,
    )?;
    let result_home = allocate_unit_scalar_home(
        result_requirement,
        assigned_scalar_homes,
        next_scalar_home,
        invalid(),
    )?;
    Ok(AssignedUnitOperation::DynamicScalarCall {
        psi_operation,
        result,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: call_plan.clone(),
        result_home,
        descriptor_abi: assigned.descriptor_abi,
        descriptor_home_byte_offset: assigned.descriptor_home_byte_offset,
        initial_copy: assigned.initial_copy,
        rebound_copy: assigned.rebound_copy,
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_unit(
    machine: MachineId,
    target: NativeTarget,
    psi_operation: OperationId,
    dynamic_dispatch: &omega_target_operations::AbstractReboundDynamicDispatch,
    call_plan: &omega_calling_conventions::CallPlan,
    initial_argument: &omega_target_operations::TargetStructuralArgument,
    rebound_argument: &omega_target_operations::TargetStructuralArgument,
    requirement_obligations: &[psi_core::ObligationId],
    crash_continuations: &[psi_terminal::CrashRouteBucket],
    next_scalar_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::DynamicUnitCallCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let assigned = assign_dynamic_call(
        machine,
        target,
        psi_operation,
        dynamic_dispatch,
        call_plan,
        None,
        initial_argument,
        rebound_argument,
        next_scalar_home,
        &invalid,
    )?;
    Ok(AssignedUnitOperation::DynamicUnitCall {
        psi_operation,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: call_plan.clone(),
        descriptor_abi: assigned.descriptor_abi,
        descriptor_home_byte_offset: assigned.descriptor_home_byte_offset,
        initial_copy: assigned.initial_copy,
        rebound_copy: assigned.rebound_copy,
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}

struct AssignedDynamicCall {
    descriptor_abi: AssignedDynamicTraitDescriptorAbi,
    descriptor_home_byte_offset: u32,
    initial_copy: AssignedAggregateCopy,
    rebound_copy: AssignedAggregateCopy,
}

#[allow(clippy::too_many_arguments)]
fn assign_dynamic_call(
    machine: MachineId,
    target: NativeTarget,
    psi_operation: OperationId,
    dynamic_dispatch: &omega_target_operations::AbstractReboundDynamicDispatch,
    call_plan: &omega_calling_conventions::CallPlan,
    result_shape: Option<omega_calling_conventions::ValueShape>,
    initial_argument: &omega_target_operations::TargetStructuralArgument,
    rebound_argument: &omega_target_operations::TargetStructuralArgument,
    next_home: &mut u32,
    invalid: &impl Fn() -> AssignmentError,
) -> Result<AssignedDynamicCall, AssignmentError> {
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![initial_argument.shape],
            result: result_shape,
        },
    )
    .map_err(|_| invalid())?;
    let source_matches = |argument: &omega_target_operations::TargetStructuralArgument,
                          source: &psi_terminal::StructuralArgument| {
        argument.place == source.place
            && argument.access == source.access
            && argument.path == source.path
    };
    if call_plan != &expected_call_plan
        || call_plan.result.as_ref().map(|placement| placement.shape) != result_shape
        || call_plan.parameters.as_slice() != std::slice::from_ref(&initial_argument.destination)
        || initial_argument.destination != rebound_argument.destination
        || initial_argument.shape != rebound_argument.shape
        || initial_argument.structural_type != rebound_argument.structural_type
        || !source_matches(initial_argument, &dynamic_dispatch.initial.source)
        || !source_matches(rebound_argument, &dynamic_dispatch.rebound.source)
        || !dynamic_dispatch.has_complete_application_custody(machine, psi_operation)
    {
        return Err(invalid());
    }
    let (descriptor_abi, descriptor_home_byte_offset) =
        assign_descriptor(target, next_home, invalid)?;
    Ok(AssignedDynamicCall {
        descriptor_abi,
        descriptor_home_byte_offset,
        initial_copy: assigned_copy(initial_argument),
        rebound_copy: assigned_copy(rebound_argument),
    })
}

fn assign_descriptor(
    target: NativeTarget,
    next_home: &mut u32,
    invalid: &impl Fn() -> AssignmentError,
) -> Result<(AssignedDynamicTraitDescriptorAbi, u32), AssignmentError> {
    let runtime_descriptor =
        omega_runtime_abi::build_runtime_abi_plan(target).dynamic_trait_descriptor();
    let descriptor_abi = AssignedDynamicTraitDescriptorAbi::new(
        u32::try_from(runtime_descriptor.instance_offset()).map_err(|_| invalid())?,
        u32::try_from(runtime_descriptor.table_offset()).map_err(|_| invalid())?,
        u32::try_from(runtime_descriptor.word_size()).map_err(|_| invalid())?,
        u32::try_from(runtime_descriptor.total_size()).map_err(|_| invalid())?,
        u32::try_from(runtime_descriptor.align()).map_err(|_| invalid())?,
    );
    let alignment = descriptor_abi.align();
    *next_home = next_home
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .ok_or_else(invalid)?;
    let descriptor_home_byte_offset = *next_home;
    *next_home = next_home
        .checked_add(descriptor_abi.total_size())
        .ok_or_else(invalid)?;
    Ok((descriptor_abi, descriptor_home_byte_offset))
}

fn assigned_copy(
    argument: &omega_target_operations::TargetStructuralArgument,
) -> AssignedAggregateCopy {
    AssignedAggregateCopy {
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
    }
}
