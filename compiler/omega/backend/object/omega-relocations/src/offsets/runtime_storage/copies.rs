use crate::offsets::runtime_frame::runtime_frame_index_setup_width;
use omega_target::Architecture;

pub(crate) fn runtime_storage_copy_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(crate) fn runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        }
        // Start of the target-base `mov r15,imm64` in the Place
        // materializer's canonical shape: mov r14,imm64 (10) + index load (7)
        // + imul (7) + descriptor deref (7) + add (3); the planner adds the
        // +2 itself.
        Architecture::X86_64 => 27 + x86_unsigned_index_load_width(index_byte_size),
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
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                architecture,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_count,
            )
        }
        // Start of the target-base mov, which follows the source-address
        // sequence; the relocation planner adds the +2 immediate offset
        // itself. SINGLE-VALUE (byte_count 1|4|8) loads the element first, so
        // machine index: 10+7+7+3+7 = 34; a frame-resident index inserts
        // `mov r10,imm64` (10): 44. CHUNKED (record-view snapshot) puts the
        // target mov (r10) right after `add r15,rax`: 27 / 37.
        Architecture::X86_64 => {
            let frame_index =
                index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
            let width_delta = x86_unsigned_index_load_width(index_byte_size) - 7;
            if matches!(byte_count, 1 | 4 | 8) {
                if frame_index {
                    44 + width_delta
                } else {
                    34 + width_delta
                }
            } else if frame_index {
                37 + width_delta
            } else {
                27 + width_delta
            }
        }
    }
}

/// aarch64: byte offset of the SOURCE adrp (`adrp x20`) inside the store-side
/// `machine[i] = <machine source>` — the machine page-pair for the source value.
/// (x86_64 uses its own frame-source layout; unused there.)
pub(crate) fn runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
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
            omega_instruction_selection::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                architecture,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

fn x86_unsigned_index_load_width(index_byte_size: usize) -> usize {
    match index_byte_size {
        1 | 2 => 8,
        4 | 8 => 7,
        _ => 0,
    }
}

/// Start of the WRITE part's `mov r15,imm64` inside the dual-indexed copy
/// (`arr[i] = arr[j]`) -- the second machine-base relocation; the relocation
/// planner adds the +2 immediate offset itself. x86_64 only (aarch64 has no
/// dual-indexed encoder; emission rejects the instruction first).
pub(crate) fn runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
    architecture: Architecture,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
        architecture,
        source_index_region,
    )
}

/// Start of the frame-base `mov r10,imm64` for a FRAME-resident index inside
/// the dual-indexed copy: the source side sits after the opening machine mov
/// (+10); the target side after the read part + the write's machine mov.
pub(crate) fn runtime_storage_copy_machine_indexed_frame_index_offset(
    architecture: Architecture,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_side: bool,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_machine_indexed_frame_index_offset(
        architecture,
        source_index_region,
        target_side,
    )
}

/// Frame-base relocation start (pre-`+2`) inside the double-indexed read,
/// present only when an index is frame-resident.
pub(crate) fn runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
    architecture: Architecture,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
        architecture,
    )
}

/// Target-region relocation start (pre-`+2`) inside the double-indexed read.
pub(crate) fn runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
        architecture,
        outer_index_region,
        inner_index_region,
    )
}

/// Target-region relocation start (pre-`+2`) inside the frame-base
/// double-indexed read.
pub(crate) fn runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    omega_instruction_selection::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
        architecture,
        outer_index_region,
        inner_index_region,
    )
}

pub(crate) fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 17,
    }
}
