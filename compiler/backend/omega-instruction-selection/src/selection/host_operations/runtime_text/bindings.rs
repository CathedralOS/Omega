use crate::InstructionSelectionInput;
use omega_calling_conventions::HostBindingMechanism;

pub(super) fn host_binding_mechanism<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBindingMechanism> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| &binding.mechanism)
}
