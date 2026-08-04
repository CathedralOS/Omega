use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn runtime_text_buffer_materialize_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        // The buffer `mov r14, imm64` (10 bytes) precedes the target-region
        // `mov r15, imm64`, so its relocated immediate sits at offset 10.
        Architecture::X86_64 => omega_isa_x86_64::RUNTIME_TEXT_BUFFER_MATERIALIZE_TARGET_IMM_OFFSET,
    }
}

pub(crate) fn runtime_text_indexed_buffer_materialize_buffer_address_offset(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 12
        }
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_buffer_imm_offset(
                index_byte_size,
            )
        }
    }
}
