use crate::TargetToMachineInput;
use crate::host_bindings::host_binding_mechanism;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_instruction_selection as architecture;
use omega_instruction_selection::host_call_sequence_width;
use omega_machine_program::MachineInstructionKind;
use omega_target_program::InstructionOperand;

pub(super) fn host_operation_shape(
    input: TargetToMachineInput<'_>,
    operation_key: HostOperationKey,
    operands: &[InstructionOperand],
) -> (MachineInstructionKind, usize) {
    let byte_width = match host_binding_mechanism(input, operation_key) {
        Some(HostBindingMechanism::Syscall { number, .. }) => {
            architecture::syscall_sequence_width(input.target.architecture, operands, *number)
        }
        _ => host_call_sequence_width(input.target.architecture, operands),
    };

    (
        MachineInstructionKind::HostCallSequence { operation_key },
        byte_width,
    )
}
