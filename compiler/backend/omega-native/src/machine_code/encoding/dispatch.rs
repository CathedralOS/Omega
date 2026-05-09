use crate::architecture;
use crate::machine_code::branch_distances::{
    byte_distance_to_case_end, byte_distance_to_case_leave, byte_distance_to_dispatch_loop_start,
    byte_distance_to_next_state_write_end,
};
use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;
use omega_core::diagnostics::Diagnostic;
use omega_machine_program::MachineInstruction;

pub(super) fn encode_dispatch_loop_enter(
    native_plan: &NativePlan,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_loop_enter(native_plan.target.architecture, entry_dispatch_index)
}

pub(super) fn encode_dispatch_case_enter(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_case_enter(
        native_plan.target.architecture,
        dispatch_index,
        byte_distance_to_case_end(machine_instructions, machine_instruction_index)?,
    )
}

pub(super) fn encode_dispatch_guard_compare_static(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_guard_compare_static(
        native_plan.target.architecture,
        byte_offset,
        byte_size,
        expected_value,
        byte_distance_to_next_state_write_end(machine_instructions, machine_instruction_index)?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_dispatch_state_write(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_state_write(
        native_plan.target.architecture,
        dispatch_index,
        byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
    )
}

pub(super) fn encode_dispatch_terminal_write(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_dispatch_state_write(
        native_plan,
        machine_instructions,
        machine_instruction_index,
        native_plan.runtime_dispatch_loop.terminal_dispatch_index,
    )
}

pub(super) fn encode_dispatch_case_leave(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_case_leave(
        native_plan.target.architecture,
        byte_distance_to_dispatch_loop_start(machine_instructions, machine_instruction_index)?,
    )
}
