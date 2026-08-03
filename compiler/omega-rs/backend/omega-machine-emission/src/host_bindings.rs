use crate::MachineEmissionContext;
use omega_assigned_target_operations::SelectedInstructionKind;
use omega_calling_conventions::{HostBinding, HostOperationKey};

pub(super) fn host_binding<'plan>(
    input: MachineEmissionContext<'plan>,
    operation_key: HostOperationKey,
) -> Option<&'plan HostBinding> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}

pub(super) fn instruction_requires_float_control_restore(
    input: MachineEmissionContext<'_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    instruction
        .host_operation_key()
        .and_then(|operation_key| host_binding(input, operation_key))
        .is_some_and(|binding| binding.mechanism.requires_float_control_restore())
}
