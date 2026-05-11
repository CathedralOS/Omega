use omega_machine_program::MachineInstructionKind;
use omega_target_operations::HostOperationKey;

pub(super) fn host_operation_kind(operation_key: HostOperationKey) -> MachineInstructionKind {
    MachineInstructionKind::HostCallSequence { operation_key }
}
