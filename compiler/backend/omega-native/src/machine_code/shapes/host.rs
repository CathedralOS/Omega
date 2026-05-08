use crate::abi::HostBindingMechanism;
use crate::architecture;
use crate::instructions::InstructionOperand;
use crate::machine_code::host_bindings::host_binding_mechanism;
use crate::machine_code::model::MachineInstructionKind;
use crate::machine_code::widths::host_call_sequence_width;
use crate::plan::NativePlan;

pub(super) fn host_operation_shape(
    native_plan: &NativePlan,
    capability: &str,
    operation: &str,
    operands: &[InstructionOperand],
) -> (MachineInstructionKind, usize) {
    let byte_width = match host_binding_mechanism(native_plan, capability, operation) {
        Some(HostBindingMechanism::Syscall { number, .. }) => {
            architecture::syscall_sequence_width(native_plan.target.architecture, operands, *number)
        }
        _ => host_call_sequence_width(native_plan.target.architecture, operands),
    };

    (
        MachineInstructionKind::HostCallSequence {
            capability: capability.to_owned(),
            operation: operation.to_owned(),
        },
        byte_width,
    )
}
