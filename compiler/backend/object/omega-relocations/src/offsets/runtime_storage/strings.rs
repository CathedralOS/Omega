use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn string_descriptor_machine_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(crate) fn string_descriptor_pointee_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_frame_indexed_string_data_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        }
        // x86_64 lays the {ptr,len} store after a fixed 34-byte indexed-address
        // prefix; the literal-pointer `mov r15,imm64` begins there.
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            34
        }
    }
}

pub(crate) fn runtime_machine_indexed_string_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_machine_indexed_string_runtime_frame_address_offset(
                architecture,
                base_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_machine_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_machine_indexed_string_data_address_offset(
                architecture,
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}
