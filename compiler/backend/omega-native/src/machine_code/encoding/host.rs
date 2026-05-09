use crate::architecture;
use crate::instructions::InstructionOperand;
use crate::plan::NativePlan;
use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;

use crate::machine_code::host_bindings::host_binding_mechanism;

pub(super) fn encode_host_operation(
    native_plan: &NativePlan,
    capability: &str,
    operation: &str,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match host_binding_mechanism(native_plan, capability, operation) {
        Some(HostBindingMechanism::Syscall {
            number,
            number_register,
            supervisor_call,
            ..
        }) => architecture::encode_syscall_sequence(
            native_plan.target.architecture,
            operands,
            *number,
            *number_register,
            *supervisor_call,
        ),
        _ => architecture::encode_host_call_sequence(native_plan.target.architecture, operands),
    }
}
