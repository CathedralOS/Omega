use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;
use omega_instruction_selection::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, return_width,
};
use omega_machine_program::MachineInstructionKind;

pub(super) fn dispatch_loop_enter_shape(
    native_plan: &NativePlan,
    entry_dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchLoopEnter {
            entry_dispatch_index,
        },
        dispatch_loop_enter_width(native_plan.target.architecture),
    )
}

pub(super) fn dispatch_case_enter_shape(
    native_plan: &NativePlan,
    dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchCaseEnter { dispatch_index },
        dispatch_case_enter_width(native_plan.target.architecture),
    )
}

pub(super) fn dispatch_guard_compare_static_shape(
    native_plan: &NativePlan,
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
        dispatch_guard_compare_static_width(native_plan.target.architecture),
    )
}

pub(super) fn dispatch_state_write_shape(
    native_plan: &NativePlan,
    dispatch_index: u32,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchStateWrite { dispatch_index },
        dispatch_state_write_width(native_plan.target.architecture),
    )
}

pub(super) fn dispatch_terminate_shape(
    native_plan: &NativePlan,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchTerminate {
            terminal_dispatch_index: native_plan.runtime_dispatch_loop.terminal_dispatch_index,
        },
        dispatch_state_write_width(native_plan.target.architecture),
    )
}

pub(super) fn dispatch_case_leave_shape(
    native_plan: &NativePlan,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::DispatchCaseLeave,
        dispatch_case_leave_width(native_plan.target.architecture),
    )
}

pub(super) fn return_shape(native_plan: &NativePlan) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::Return,
        return_width(native_plan.target.architecture),
    )
}
