use crate::MachineEmissionContext;
use crate::branch_distances::{
    byte_distance_to_next_runtime_write_end, byte_distances_to_next_runtime_machine_write_end,
};
use crate::host_bindings::host_binding_mechanism;
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_operations::RuntimeTextReadSource;

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
            input.target.architecture,
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

pub(super) fn encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
        input.target.architecture,
        0,
        source_offset,
        descriptor_offset,
        index_offset,
        element_byte_size,
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

pub(super) fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_literal_append_to_runtime_frame_indexed(
        input.target.architecture,
        0,
        descriptor_offset,
        index_offset,
        element_byte_size,
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

pub(super) fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )
}

pub(super) fn encode_runtime_text_line_read(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
) -> Result<Vec<u8>, Diagnostic> {
    let RuntimeTextReadSource::HostOperation { operation_key } = source;
    let Some(binding) = host_binding_mechanism(input, *operation_key) else {
        return Err(Diagnostic::error(format!(
            "missing host binding for runtime text read operation {}.{}",
            operation_key.capability_name(),
            operation_key.operation_name()
        )));
    };
    architecture::encode_runtime_text_line_read(
        input.target.architecture,
        target_offset,
        byte_capacity,
        binding,
    )
}
