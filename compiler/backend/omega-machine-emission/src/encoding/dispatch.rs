use crate::MachineEmissionContext;
use crate::branch_distances::{
    byte_distance_to_case_end, byte_distance_to_case_leave, byte_distance_to_dispatch_loop_start,
    byte_distance_to_next_state_write_end,
};
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_operations::StateGuardOperator;

pub(super) fn encode_dispatch_loop_enter(
    input: MachineEmissionContext<'_>,
    entry_dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_loop_enter(input.target.architecture, entry_dispatch_index)
}

pub(super) fn encode_dispatch_case_enter(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_case_enter(
        input.target.architecture,
        dispatch_index,
        byte_distance_to_case_end(machine_instructions, machine_instruction_index)?,
    )
}

pub(super) fn encode_dispatch_guard_compare_static(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_guard_compare_static(
        input.target.architecture,
        byte_offset,
        byte_size,
        expected_value,
        byte_distance_to_next_state_write_end(machine_instructions, machine_instruction_index)?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_dispatch_state_write(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_state_write(
        input.target.architecture,
        dispatch_index,
        byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
    )
}

pub(super) fn encode_dispatch_terminal_write(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_dispatch_state_write(
        input,
        machine_instructions,
        machine_instruction_index,
        input.terminal_dispatch_index,
    )
}

pub(super) fn encode_dispatch_case_leave(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_dispatch_case_leave(
        input.target.architecture,
        byte_distance_to_dispatch_loop_start(machine_instructions, machine_instruction_index)?,
    )
}
