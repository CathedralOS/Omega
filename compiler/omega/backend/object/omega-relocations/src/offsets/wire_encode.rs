use omega_target::Architecture;

pub(crate) fn wire_append_written_page_offset(
    architecture: Architecture,
    out_offset: usize,
) -> usize {
    omega_instruction_selection::wire_append_written_page_offset(architecture, out_offset)
}

pub(crate) fn wire_append_varint_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    omega_instruction_selection::wire_append_varint_source_page_offset(
        architecture,
        out_offset,
        written_offset,
    )
}

pub(crate) fn wire_append_repeated_count_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    omega_instruction_selection::wire_append_repeated_count_page_offset(
        architecture,
        out_offset,
        written_offset,
    )
}

pub(crate) fn wire_append_repeated_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
    count_offset: usize,
    index: u64,
) -> usize {
    omega_instruction_selection::wire_append_repeated_source_page_offset(
        architecture,
        out_offset,
        written_offset,
        count_offset,
        index,
    )
}
