use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn runtime_text_stored_suffix_source_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => {
            omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET
        }
    }
}

pub(crate) fn runtime_text_stored_suffix_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 52,
        Architecture::X86_64 => {
            omega_isa_x86_64::RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET
        }
    }
}

pub(crate) fn runtime_text_stored_place_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 28,
        Architecture::X86_64 => {
            omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET
        }
    }
}

pub(crate) fn runtime_text_stored_place_pointee_source_address_offset(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            16 + aarch64_load_data_offset_width(pointer_byte_offset, 8)
                + aarch64_add_constant_width(field_byte_offset)
                + aarch64_load_data_offset_width(8, 8)
                + 4
                + 4
        }
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET
        }
    }
}

pub(crate) fn runtime_text_stored_place_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => {
            omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_TARGET_IMM_OFFSET
        }
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 8
        }
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_buffer_imm_offset(
                index_byte_size,
            )
        }
    }
}

fn aarch64_load_data_offset_width(byte_offset: usize, byte_size: usize) -> usize {
    if aarch64_data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + aarch64_add_constant_width(byte_offset) + 4
    }
}

fn aarch64_data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

fn aarch64_add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        aarch64_unsigned_immediate_width(value as u64) + 4
    }
}

fn aarch64_unsigned_immediate_width(value: u64) -> usize {
    if value == 0 {
        return 4;
    }

    let mut width = 0;
    for halfword_index in 0..4 {
        if ((value >> (halfword_index * 16)) & 0xffff) != 0 {
            width += 4;
        }
    }

    width
}

pub(crate) fn runtime_text_indexed_stored_place_source_address_offset(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 24
        }
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
                index_byte_size,
            )
        }
    }
}
