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
        // The literal `mov r14,imm64` occupies bytes 0..10, so the frame base
        // `mov r15,imm64` begins at offset 10 (its immediate is relocated at +2).
        Architecture::X86_64 => 10,
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

pub(crate) fn runtime_frame_indexed_string_data_address_offset_with_index_region(
    architecture: Architecture,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
                + if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    8
                } else {
                    0
                }
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub(crate) fn runtime_frame_base_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_frame_base_indexed_string_data_address_offset(
                architecture,
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub(crate) fn runtime_frame_base_indexed_string_data_address_offset_with_index_region(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_frame_base_indexed_string_data_address_offset_with_index_region(
                architecture,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub(crate) fn runtime_machine_indexed_string_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_machine_indexed_string_runtime_frame_address_offset(
        architecture,
        base_byte_offset,
    )
}

pub(crate) fn runtime_machine_indexed_string_data_address_offset_with_index_region(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_machine_indexed_string_data_address_offset_with_index_region(
        architecture,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub(crate) fn runtime_machine_double_indexed_string_data_address_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    omega_instruction_selection::runtime_machine_double_indexed_string_data_address_offset(
        architecture,
        outer_index_region,
        inner_index_region,
    )
}

pub(crate) fn runtime_frame_base_double_indexed_string_data_address_offset(
    architecture: Architecture,
) -> usize {
    omega_instruction_selection::runtime_frame_base_double_indexed_string_data_address_offset(
        architecture,
    )
}
