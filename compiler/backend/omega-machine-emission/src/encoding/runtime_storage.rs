use crate::MachineEmissionContext;
use crate::branch_distances::byte_distance_to_next_runtime_write_end;
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_operations::StateGuardOperator;

pub(super) fn encode_runtime_storage_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_compare(
        input.target.architecture,
        left_offset,
        right_offset,
        byte_size,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_storage_value_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_value_compare(
        input.target.architecture,
        byte_offset,
        byte_size,
        expected_value,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_machine_integer_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_integer_write(
        input.target.architecture,
        byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_machine_string_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_string_write(
        input.target.architecture,
        byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_storage_copy(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy(
        input.target.architecture,
        source_offset,
        target_offset,
        byte_count,
    )
}
