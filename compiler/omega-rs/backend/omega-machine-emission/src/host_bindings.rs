use crate::MachineEmissionContext;
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
