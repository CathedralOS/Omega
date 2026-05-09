use crate::TargetToMachineInput;
use omega_instruction_selection::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, return_width,
};
use omega_machine_program::MachineInstructionKind;
use omega_target_program::StateGuardOperator;

pub(super) fn dispatch_loop_enter_shape(
    input: TargetToMachineInput<'_>,
    entry_dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchLoopEnter {
            entry_dispatch_index,
        },
        dispatch_loop_enter_width(input.target.architecture),
    )
}

pub(super) fn dispatch_case_enter_shape(
    input: TargetToMachineInput<'_>,
    dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchCaseEnter { dispatch_index },
        dispatch_case_enter_width(input.target.architecture),
    )
}

pub(super) fn dispatch_guard_compare_static_shape(
    input: TargetToMachineInput<'_>,
    operator: StateGuardOperator,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchGuardCompareStatic {
            operator,
            byte_offset,
            byte_size,
            expected_value,
        },
        dispatch_guard_compare_static_width(input.target.architecture),
    )
}

pub(super) fn dispatch_state_write_shape(
    input: TargetToMachineInput<'_>,
    dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchStateWrite { dispatch_index },
        dispatch_state_write_width(input.target.architecture),
    )
}

pub(super) fn dispatch_terminate_shape(
    input: TargetToMachineInput<'_>,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchTerminate {
            terminal_dispatch_index: input.terminal_dispatch_index,
        },
        dispatch_state_write_width(input.target.architecture),
    )
}

pub(super) fn dispatch_case_leave_shape(
    input: TargetToMachineInput<'_>,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchCaseLeave,
        dispatch_case_leave_width(input.target.architecture),
    )
}

pub(super) fn return_shape(input: TargetToMachineInput<'_>) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::Return,
        return_width(input.target.architecture),
    )
}
