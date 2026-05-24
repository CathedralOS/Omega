use omega_machine_instructions::MachineInstructionKind;

pub(super) fn dispatch_loop_enter_kind(_entry_dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchLoopEnter
}

pub(super) fn dispatch_case_enter_kind(_dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchCaseEnter
}

pub(super) fn dispatch_guard_compare_static_kind(
    _operator: omega_assigned_target_operations::StateGuardOperator,
    _byte_offset: usize,
    _byte_size: usize,
    _expected_value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::DispatchGuardCompareStatic
}

pub(super) fn dispatch_state_write_kind(_dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchStateWrite
}

pub(super) fn dispatch_terminate_kind() -> MachineInstructionKind {
    MachineInstructionKind::DispatchTerminate
}

pub(super) fn dispatch_case_leave_kind() -> MachineInstructionKind {
    MachineInstructionKind::DispatchCaseLeave
}

pub(super) fn return_kind() -> MachineInstructionKind {
    MachineInstructionKind::Return
}
