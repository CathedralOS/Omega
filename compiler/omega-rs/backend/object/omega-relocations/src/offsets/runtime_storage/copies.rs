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
        // Start of the `mov r10,imm64` runtime-frame base (the second 10-byte
        // `mov`); the relocation planner adds the +2 immediate offset itself.
        // (Only used for a frame-resident index, which the x86_64 encoder does
        // not emit yet.)
        Architecture::X86_64 => 10,
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
        // Start of the second `mov r15,imm64` (the target base), which follows
        // the 34-byte source-address sequence (10+7+7+3+7); the relocation
        // planner adds the +2 immediate offset itself.
        Architecture::X86_64 => 34,
    }
}

/// Start of the WRITE part's `mov r15,imm64` inside the dual-indexed copy
/// (`arr[i] = arr[j]`) -- the second machine-base relocation; the relocation
/// planner adds the +2 immediate offset itself. x86_64 only (aarch64 has no
/// dual-indexed encoder; emission rejects the instruction first).
pub(crate) fn runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        // The 34-byte read part (10+7+7+3+7) precedes it.
        Architecture::X86_64 => 34,
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
