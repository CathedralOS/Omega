//! Physical descriptor and result-home assignment for rebound dynamic calls.

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
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![initial_argument.shape],
            result: Some(result_requirement.shape),
        },
    )
    .map_err(|_| invalid())?;
    let source_matches = |argument: &omega_target_operations::TargetStructuralArgument,
                          source: &psi_terminal::StructuralArgument| {
        argument.place == source.place
            && argument.access == source.access
            && argument.path == source.path
    };
    if result.value != result_requirement.source_value
        || result.scalar_type != result_requirement.scalar_type
        || result_requirement.defining_operation != psi_operation
        || call_plan != &expected_call_plan
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
    let size = descriptor_abi.total_size();
    *next_scalar_home = next_scalar_home
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .ok_or_else(invalid)?;
    let descriptor_home_byte_offset = *next_scalar_home;
    *next_scalar_home = next_scalar_home.checked_add(size).ok_or_else(invalid)?;
    let result_home = allocate_unit_scalar_home(
        result_requirement,
        assigned_scalar_homes,
        next_scalar_home,
        invalid(),
    )?;
    let copy =
        |argument: &omega_target_operations::TargetStructuralArgument| AssignedAggregateCopy {
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
        };
    Ok(AssignedUnitOperation::DynamicScalarCall {
        psi_operation,
        result,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: call_plan.clone(),
        result_home,
        descriptor_abi,
        descriptor_home_byte_offset,
        initial_copy: copy(initial_argument),
        rebound_copy: copy(rebound_argument),
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
    })
}
