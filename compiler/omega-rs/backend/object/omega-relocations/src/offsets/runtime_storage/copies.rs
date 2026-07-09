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
        // Start of the target-base `mov r15,imm64`: mov r14,imm64 (10)
        // + index load (7) + imul (7) + descriptor deref (7) + add (3)
        // + element load (7); the planner adds the +2 itself.
        Architecture::X86_64 => 41,
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
        // Start of the target-base `mov r15,imm64`: mov r14,imm64 (10)
        // + descriptor deref (7) + element load (7); planner adds +2.
        Architecture::X86_64 => 24,
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
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                architecture,
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        // Start of the target-base `mov r15,imm64`, which follows the
        // source-address sequence; the relocation planner adds the +2
        // immediate offset itself. Machine index: 10+7+7+3+7 = 34. A
        // frame-resident index inserts `mov r10,imm64` (10) + reads the index
        // off r10 (same 7 bytes): 44.
        Architecture::X86_64 => {
            if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                44
            } else {
                34
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
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
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

/// Start of the second `mov r15,imm64` (the machine base) inside the
/// FRAME-SOURCE variant of the storage->machine-indexed write; the relocation
/// planner adds the +2 immediate offset itself. x86_64 only.
pub(crate) fn runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        // mov r15,imm64 (10) + load rax (7) precede it.
        Architecture::X86_64 => 17,
    }
}

/// Start of the `mov r10,imm64` (the frame base for a FRAME-resident INDEX)
/// inside the storage->machine-indexed write; the relocation planner adds the
/// +2 immediate offset itself. x86_64 only (aarch64 relocates the frame index
/// via the shared read-side offset in its own record branch).
pub(crate) fn runtime_storage_copy_to_runtime_machine_indexed_frame_index_base_offset(
    architecture: Architecture,
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        // mov r15,imm64 (10) + load rax (7) precede it; a FRAME source adds
        // its machine re-load `mov r15,imm64` (10) in between.
        Architecture::X86_64 => {
            if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                27
            } else {
                17
            }
        }
    }
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
) -> usize {
    omega_instruction_selection::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
        architecture,
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
