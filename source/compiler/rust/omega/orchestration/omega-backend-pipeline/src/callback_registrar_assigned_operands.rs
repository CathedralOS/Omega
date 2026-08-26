use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackPrivateRelocationDemand,
    CallbackRegistrarArgumentBinding, CallbackRegistrarAssignedOperandBinding,
    CallbackRegistrarPhysicalDestination, CallbackThunkPlan,
    replay_callback_registrar_assigned_operand_bindings,
};
use psi_arena::Handle;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_callback_registrar_assigned_operand_bindings(
    target: omega_target::NativeTarget,
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &omega_platform_interface::HostCallPlan,
    boundaries: &omega_abstract_operations::AbstractBoundarySummary,
    argument_bindings: &[CallbackRegistrarArgumentBinding],
    layouts: &omega_layout::LayoutPlan,
    destinations: &[CallbackRegistrarPhysicalDestination],
    abstract_operations: &omega_abstract_operations::AbstractOperationPlan,
    target_operations: &omega_target_operations::TargetOperationPlan,
    assigned_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
) -> Result<Arc<[CallbackRegistrarAssignedOperandBinding]>, Diagnostic> {
    let mut bindings = Vec::with_capacity(destinations.len());
    for (destination_index, destination) in destinations.iter().enumerate() {
        let candidates = target_operations
            .code
            .instructions
            .iter()
            .filter_map(|(instruction, selected)| {
                let omega_target_operations::TargetOperationKind::HostOperation {
                    provenance: Some(provenance),
                    ..
                } = &selected.kind
                else {
                    return None;
                };
                let formals = provenance
                    .formal_operands
                    .iter()
                    .filter(|formal| {
                        provenance.occurrence == destination.binding.host_call
                            && formal.native_argument == destination.binding.native_argument
                            && formal.formal_ordinal == destination.formal_ordinal
                    })
                    .collect::<Vec<_>>();
                let [formal] = formals.as_slice() else {
                    return None;
                };
                Some((instruction, provenance, (*formal).clone()))
            })
            .collect::<Vec<_>>();
        let [(target_instruction, provenance, formal_operand)] = candidates.as_slice() else {
            return Err(Diagnostic::error(format!(
                "callback registrar destination {destination_index} resolves to {} selected formal operands; exactly one is required",
                candidates.len()
            )));
        };
        let abstract_instruction = Handle::from_parts(
            target_instruction.arena_index(),
            target_instruction.generation(),
        );
        let abstract_provenance = abstract_operations
            .code
            .instructions
            .iter()
            .find(|(handle, _)| *handle == abstract_instruction)
            .and_then(|(_, instruction)| match &instruction.kind {
                omega_abstract_operations::AbstractOperationKind::HostOperation {
                    provenance: Some(provenance),
                    ..
                } => Some(provenance.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar destination {destination_index} lost its abstract host-operation provenance"
                ))
            })?;
        let assigned_instruction = Handle::from_parts(
            target_instruction.arena_index(),
            target_instruction.generation(),
        );
        let assigned_operand = Handle::from_parts(
            formal_operand.operand.arena_index(),
            formal_operand.operand.generation(),
        );
        let target_operand = target_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == formal_operand.operand)
            .map(|(_, operand)| operand.clone())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar destination {destination_index} lost its target operand"
                ))
            })?;
        bindings.push(CallbackRegistrarAssignedOperandBinding {
            destination_index,
            destination: destination.clone(),
            abstract_instruction,
            target_instruction: *target_instruction,
            assigned_instruction,
            abstract_provenance,
            provenance: (*provenance).clone(),
            formal_operand: formal_operand.clone(),
            target_operand,
            assigned_operand,
        });
    }

    replay_callback_registrar_assigned_operand_bindings(
        target,
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts,
        destinations,
        abstract_operations,
        target_operations,
        assigned_operations,
        &bindings,
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "callback registrar assigned-operand replay failed: {error}"
        ))
    })?;
    Ok(Arc::from(bindings))
}

#[cfg(test)]
mod tests;
