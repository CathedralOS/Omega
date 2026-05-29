use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn runtime_text_buffer_materialize_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_indexed_buffer_materialize_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 12
        }
        Architecture::X86_64 => 8,
    }
}
