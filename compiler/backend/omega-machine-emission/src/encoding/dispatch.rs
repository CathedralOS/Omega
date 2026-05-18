use crate::MachineEmissionContext;
use crate::branch_distances::byte_distance_to_next_state_write_end;
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_operations::StateGuardOperator;

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
        operator,
    )
}
