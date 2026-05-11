use omega_calling_conventions::HostOperationKey;
use omega_machine_program::MachineInstructionKind;

pub(super) fn host_operation_kind(operation_key: HostOperationKey) -> MachineInstructionKind {
    MachineInstructionKind::HostCallSequence { operation_key }
}
