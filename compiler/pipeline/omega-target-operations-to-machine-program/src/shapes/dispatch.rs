use omega_machine_program::MachineInstructionKind;
use omega_target_operations::StateGuardOperator;

pub(super) fn dispatch_loop_enter_kind(entry_dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchLoopEnter {
        entry_dispatch_index,
    }
}

pub(super) fn dispatch_case_enter_kind(dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchCaseEnter { dispatch_index }
}

pub(super) fn dispatch_guard_compare_static_kind(
    operator: StateGuardOperator,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::DispatchGuardCompareStatic {
        operator,
        byte_offset,
        byte_size,
        expected_value,
    }
}

pub(super) fn dispatch_state_write_kind(dispatch_index: u32) -> MachineInstructionKind {
    MachineInstructionKind::DispatchStateWrite { dispatch_index }
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
