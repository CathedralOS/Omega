use omega_target::Architecture;

pub(crate) fn wire_decode_read_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
) -> usize {
    omega_instruction_selection::wire_decode_read_page_offset(architecture, buffer_offset)
}

pub(crate) fn wire_decode_ok_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    omega_instruction_selection::wire_decode_ok_page_offset(
        architecture,
        buffer_offset,
        read_offset,
    )
}

pub(crate) fn wire_decode_varint_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    zigzag: bool,
) -> usize {
    omega_instruction_selection::wire_decode_varint_target_page_offset(
        architecture,
        buffer_offset,
        buffer_length,
        read_offset,
        zigzag,
    )
}

pub(crate) fn wire_decode_byte_slice_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    predicate_mask: u8,
) -> usize {
    omega_instruction_selection::wire_decode_byte_slice_target_page_offset(
        architecture,
        buffer_offset,
        buffer_length,
        read_offset,
        predicate_mask,
    )
}

pub(crate) fn wire_decode_nested_end_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    omega_instruction_selection::wire_decode_nested_end_page_offset(
        architecture,
        buffer_offset,
        read_offset,
    )
}

pub(crate) fn wire_decode_repeated_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    zigzag: bool,
) -> usize {
    omega_instruction_selection::wire_decode_repeated_target_page_offset(
        architecture,
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        zigzag,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_decode_repeated_count_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    omega_instruction_selection::wire_decode_repeated_count_page_offset(
        architecture,
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        target_offset,
        byte_size,
        zigzag,
        range,
    )
}
