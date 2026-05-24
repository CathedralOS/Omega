use omega_machine_instructions::MachineInstructionKind;

pub(super) fn host_operation_kind(
    _operation_key: omega_assigned_target_operations::HostOperationKey,
) -> MachineInstructionKind {
    MachineInstructionKind::HostCallSequence
}
