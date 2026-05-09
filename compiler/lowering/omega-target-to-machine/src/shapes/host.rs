use crate::TargetToMachineInput;
use crate::host_bindings::host_binding_mechanism;
use omega_calling_conventions::HostBindingMechanism;
use omega_instruction_selection as architecture;
use omega_instruction_selection::host_call_sequence_width;
use omega_machine_program::MachineInstructionKind;
use omega_target_program::InstructionOperand;

pub(super) fn host_operation_shape(
    input: TargetToMachineInput<'_>,
    capability: &str,
    operation: &str,
    operands: &[InstructionOperand],
) -> (MachineInstructionKind, usize) {
    let byte_width = match host_binding_mechanism(input, capability, operation) {
        Some(HostBindingMechanism::Syscall { number, .. }) => {
            architecture::syscall_sequence_width(input.target.architecture, operands, *number)
        }
        _ => host_call_sequence_width(input.target.architecture, operands),
    };

    (
        MachineInstructionKind::HostCallSequence {
            capability: capability.to_owned(),
            operation: operation.to_owned(),
        },
        byte_width,
    )
}
