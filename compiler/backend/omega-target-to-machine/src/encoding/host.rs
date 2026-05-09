use crate::TargetToMachineInput;
use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_program::InstructionOperand;

use crate::host_bindings::host_binding_mechanism;

pub(super) fn encode_host_operation(
    input: TargetToMachineInput<'_>,
    capability: &str,
    operation: &str,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match host_binding_mechanism(input, capability, operation) {
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
        _ => architecture::encode_host_call_sequence(input.target.architecture, operands),
    }
}
