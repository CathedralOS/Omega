use crate::InstructionSelectionInput;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};

pub(super) fn host_binding_mechanism<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    operation_key: HostOperationKey,
) -> Option<&'plan HostBindingMechanism> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| &binding.mechanism)
}
