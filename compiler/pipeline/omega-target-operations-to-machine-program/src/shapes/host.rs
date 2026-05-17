use omega_machine_program::MachineInstructionKind;
pub(super) fn host_operation_kind(
    _operation_key: omega_target_operations::HostOperationKey,
) -> MachineInstructionKind {
    MachineInstructionKind::HostCallSequence
}
