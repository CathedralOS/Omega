use crate::offsets::runtime_frame::{add_constant_width, runtime_frame_index_setup_width};
use omega_target::Architecture;

pub(crate) fn runtime_storage_copy_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let source_offset = element_index
                .saturating_mul(element_byte_size)
                .saturating_add(field_byte_offset);
            12 + add_constant_width(source_offset)
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                architecture,
                base_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                architecture,
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 17,
    }
}
