use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackPrivateObjectStoreRequest,
    CallbackPrivateRelocationDemand, CallbackRegistrarArgumentBinding,
    CallbackRegistrarAssignedOperandBinding, CallbackRegistrarPhysicalDestination,
    CallbackRegistrarPhysicalDestinationKind, CallbackThunkPlan,
    replay_callback_private_object_store_requests,
};
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub(super) fn plan_callback_private_object_store_requests(
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
    assigned_bindings: &[CallbackRegistrarAssignedOperandBinding],
    object: &omega_object_file::ObjectPlan,
    entry_machine_name: &str,
) -> Result<Arc<[CallbackPrivateObjectStoreRequest]>, Diagnostic> {
    let mut requests = Vec::with_capacity(assigned_bindings.len());
    for (binding_index, binding) in assigned_bindings.iter().enumerate() {
        let CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. } =
            &binding.destination.kind
        else {
            return Err(store_error(
                binding_index,
                "direct callback parameters remain fenced by OWNER_QUESTIONS Q13",
            ));
        };
        let assigned_operand = assigned_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == binding.assigned_operand)
            .map(|(_, operand)| operand)
            .ok_or_else(|| store_error(binding_index, "assigned operand is missing"))?;
        let omega_target_operations::TargetInstructionOperandKind::RuntimeStorageAddress {
            region,
            byte_offset,
        } = assigned_operand.kind
        else {
            return Err(store_error(
                binding_index,
                "assigned operand is not the admitted RuntimeStorageAddress shape; DataAddress remains fenced",
            ));
        };
        let destination_offset = byte_offset
            .checked_add(layout_demand.offset)
            .ok_or_else(|| store_error(binding_index, "runtime-storage offset overflowed"))?;
        let (storage_symbol, storage_symbol_plan) = exact_storage_symbol(
            object,
            region,
            entry_machine_name,
            destination_offset,
            layout_demand.byte_size,
        )
        .ok_or_else(|| store_error(binding_index, "exact BSS storage symbol is missing"))?;
        let function_identity = binding.destination.binding.demand.function_identity;
        let (function_symbol, function_symbol_plan) =
            omega_object_file::object_function_symbol(object, function_identity).ok_or_else(
                || {
                    store_error(
                        binding_index,
                        "exact private callback function symbol is missing",
                    )
                },
            )?;
        let group_count = assigned_bindings
            .iter()
            .filter(|candidate| candidate.assigned_instruction == binding.assigned_instruction)
            .count();
        let group_ordinal = assigned_bindings[..binding_index]
            .iter()
            .filter(|candidate| candidate.assigned_instruction == binding.assigned_instruction)
            .count();
        let store_index = usize::try_from(binding.assigned_instruction.arena_index())
            .ok()
            .and_then(|registrar| registrar.checked_sub(group_count))
            .and_then(|first| first.checked_add(group_ordinal))
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| store_error(binding_index, "pre-registrar store position is invalid"))?;
        requests.push(CallbackPrivateObjectStoreRequest {
            assigned_binding_index: binding_index,
            assigned_binding: binding.clone(),
            storage_region: region,
            storage_base_offset: byte_offset,
            slot_offset: layout_demand.offset,
            destination_offset,
            byte_size: layout_demand.byte_size,
            alignment: layout_demand.alignment,
            storage_symbol,
            storage_symbol_plan: storage_symbol_plan.clone(),
            function_identity,
            function_symbol,
            function_symbol_plan: function_symbol_plan.clone(),
            abstract_store_instruction: psi_arena::Handle::from_parts(
                store_index,
                binding.abstract_instruction.generation(),
            ),
            target_store_instruction: psi_arena::Handle::from_parts(
                store_index,
                binding.target_instruction.generation(),
            ),
            assigned_store_instruction: psi_arena::Handle::from_parts(
                store_index,
                binding.assigned_instruction.generation(),
            ),
        });
    }

    replay_callback_private_object_store_requests(
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
        assigned_bindings,
        object,
        entry_machine_name,
        &requests,
    )
    .map_err(|error| Diagnostic::error(format!("callback object-store replay failed: {error}")))?;
    Ok(Arc::from(requests))
}

fn exact_storage_symbol<'object>(
    object: &'object omega_object_file::ObjectPlan,
    region: omega_target_operations::RuntimeStorageRegion,
    entry_machine_name: &str,
    destination_offset: usize,
    byte_size: usize,
) -> Option<(
    omega_object_file::ObjectSymbolHandle,
    &'object omega_object_file::SymbolPlan,
)> {
    let name = omega_object_file::storage_region_symbol_name(region, entry_machine_name);
    let mut matches = object.layout.symbols.iter().filter(|(_, symbol)| {
        symbol.name == name
            && symbol.kind == omega_object_file::SymbolKind::Object
            && symbol.section
                == omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Bss)
    });
    let (handle, symbol) = matches.next()?;
    let end = destination_offset.checked_add(byte_size)?;
    (matches.next().is_none() && end <= symbol.size).then_some((handle, symbol))
}

fn store_error(index: usize, message: &str) -> Diagnostic {
    Diagnostic::error(format!("callback private object store {index}: {message}"))
}

#[cfg(test)]
mod tests;
