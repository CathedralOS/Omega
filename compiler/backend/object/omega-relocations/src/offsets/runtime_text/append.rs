use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn runtime_text_stored_suffix_source_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET,
    }
}

pub(crate) fn runtime_text_stored_suffix_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 52,
        Architecture::X86_64 => omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET,
    }
}

pub(crate) fn runtime_text_stored_place_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 28,
        Architecture::X86_64 => omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET,
    }
}

pub(crate) fn runtime_text_stored_place_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_TARGET_IMM_OFFSET,
    }
}

pub(crate) fn runtime_text_literal_append_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        // x86_64 loads the descriptor base via the second `mov r-, imm64`, whose
        // immediate sits 10 bytes in (insert_data_address adds the +2 itself).
        Architecture::X86_64 => 10,
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
        // x86_64 appends after the same fixed 34-byte indexed-address prefix; the
        // buffer-pointer `mov r15,imm64` begins there.
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            34
        }
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
