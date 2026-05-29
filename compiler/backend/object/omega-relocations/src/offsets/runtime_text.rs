use super::runtime_frame::runtime_frame_index_setup_width;
use omega_calling_conventions::HostBindingMechanism;
use omega_target::Architecture;

pub(crate) fn runtime_text_stored_suffix_source_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_stored_suffix_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 52,
        Architecture::X86_64 => 16,
    }
}

pub(crate) fn runtime_text_stored_place_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 28,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_stored_place_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_literal_append_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_buffer_materialize_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_indexed_literal_append_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 4
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_indexed_stored_place_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 8
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_text_indexed_stored_place_source_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 24
        }
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

pub(crate) fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_target_address_offset(architecture, binding)
}

pub(crate) fn runtime_text_line_read_import_call_offset(
    architecture: Architecture,
    selected_text_offset: usize,
) -> usize {
    selected_text_offset
        + omega_instruction_selection::runtime_text_line_read_import_call_offset(architecture)
}
