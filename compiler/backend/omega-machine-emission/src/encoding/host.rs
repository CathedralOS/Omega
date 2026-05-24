use crate::MachineEmissionContext;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_assigned_target_operations::InstructionOperand;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;

use crate::host_bindings::host_binding_mechanism;

pub(super) fn encode_host_operation(
    input: MachineEmissionContext<'_>,
    operation_key: HostOperationKey,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match host_binding_mechanism(input, operation_key) {
        Some(HostBindingMechanism::Syscall {
            number,
            number_register,
            supervisor_call,
            ..
        }) => architecture::encode_syscall_sequence(
            input.target.architecture,
            operands,
            *number,
            *number_register,
            *supervisor_call,
        ),
        _ => architecture::encode_host_call_sequence(
            input.target.architecture,
            operation_key,
            operands,
        ),
    }
}
