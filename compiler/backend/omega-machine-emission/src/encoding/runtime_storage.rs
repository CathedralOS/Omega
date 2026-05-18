use crate::MachineEmissionContext;
use crate::branch_distances::byte_distance_to_next_runtime_write_end;
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_operations::{RuntimeValueOperandHandle, StateGuardOperator};

pub(super) fn encode_runtime_value_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_value_compare(
        input.target.architecture,
        &input.instructions.runtime_value_operands,
        left,
        right,
        byte_size,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator,
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

pub(super) fn encode_runtime_pointee_integer_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_integer_write(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_storage_binary_write(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_binary_write(
        input.target.architecture,
        &input.instructions.runtime_value_operands,
        target_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

pub(super) fn encode_runtime_pointee_binary_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_binary_write(
        input.target.architecture,
        &input.instructions.runtime_value_operands,
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

pub(super) fn encode_runtime_frame_indexed_integer_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_frame_indexed_integer_write(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_frame_indexed_binary_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_frame_indexed_binary_write(
        input.target.architecture,
        &input.instructions.runtime_value_operands,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
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

pub(super) fn encode_runtime_pointee_string_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_string_write(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
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

pub(super) fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_to_runtime_frame_indexed(
        input.target.architecture,
        source_offset,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_frame_indexed(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_to_runtime_pointee(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_to_runtime_pointee(
        input.target.architecture,
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
        byte_count,
    )
}
