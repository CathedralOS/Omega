use crate::MachineEmissionContext;
use crate::branch_distances::{
    byte_distance_to_next_runtime_write_end,
    byte_distance_to_next_runtime_write_end_from_branch_offset,
    byte_distances_to_next_runtime_machine_write_end,
};
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_instruction_selection::runtime_text_storage_compare_width;
use omega_target_operations::RuntimeTextReadSource;
use omega_target_operations::StateGuardOperator;

pub(super) fn encode_runtime_text_literal_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_compare(
        input.target.architecture,
        literal,
        byte_distances_to_next_runtime_machine_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
            literal,
        )?,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
    )
}

pub(super) fn encode_runtime_text_storage_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    source_offset: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_storage_compare(
        input.target.architecture,
        source_offset,
        byte_distance_to_next_runtime_write_end_from_branch_offset(
            input,
            machine_instructions,
            machine_instruction_index,
            40,
        )?,
        byte_distance_to_next_runtime_write_end_from_branch_offset(
            input,
            machine_instructions,
            machine_instruction_index,
            runtime_text_storage_compare_width(input.target.architecture).saturating_sub(4),
        )?,
        operator == StateGuardOperator::NotEqual,
    )
}

pub(super) fn encode_runtime_text_literal_write(
    input: MachineEmissionContext<'_>,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_write(input.target.architecture, literal)
}

pub(super) fn encode_runtime_text_literal_segment_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_segment_write(
        input.target.architecture,
        byte_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_stored_suffix_append(
    input: MachineEmissionContext<'_>,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_suffix_append(
        input.target.architecture,
        buffer_offset,
        source_offset,
        target_offset,
        length_delta,
    )
}

pub(super) fn encode_runtime_text_stored_place_append(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_place_append(
        input.target.architecture,
        0,
        source_offset,
        target_offset,
    )
}

pub(super) fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_place_append_to_runtime_pointee(
        input.target.architecture,
        0,
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
    )
}

pub(super) fn encode_runtime_text_literal_append(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_append(
        input.target.architecture,
        0,
        target_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_literal_append_to_runtime_pointee(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_append_to_runtime_pointee(
        input.target.architecture,
        0,
        pointer_byte_offset,
        field_byte_offset,
        literal,
    )
}

pub(super) fn encode_runtime_text_buffer_materialize(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_buffer_materialize(input.target.architecture, target_offset)
}

pub(super) fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_buffer_materialize_to_runtime_pointee(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
    )
}

pub(super) fn encode_runtime_text_line_read(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_line_read(
        input.target.architecture,
        target_offset,
        byte_capacity,
        source,
    )
}
