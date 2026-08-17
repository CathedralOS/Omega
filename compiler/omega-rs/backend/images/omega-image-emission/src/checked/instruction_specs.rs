//! Reconstructs exact expected bytes and relocation recipes for compiler instructions.

use super::*;
use omega_target_operations::InstructionOperandLike;

mod control_entry;

pub(super) type CompilerInstructionSpec = (
    Option<usize>,
    Vec<u8>,
    u8,
    CompilerInstructionRelocationRecipe,
);

pub(super) fn expected_compiler_instruction_spec(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    function_instruction_count: usize,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<CompilerInstructionSpec, Diagnostic> {
    if let Some(spec) = control_entry::expected_control_entry_spec(
        architecture,
        code,
        function_instruction_count,
        kind.clone(),
    )? {
        return Ok(spec);
    }

    let (expected_position, expected_bytes, kind_tag, relocation_recipe): (
                    Option<usize>,
                    Vec<u8>,
                    u8,
                    CompilerInstructionRelocationRecipe,
                ) = match kind {
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyAtomic(
                        operation,
                    ) => {
                        let (bytes, validation_tag, address_sites) =
                            replay_compiler_atomic_operation(
                                architecture,
                                &code.runtime_value_operands,
                                operation,
                            )?;
                        (
                            None,
                            bytes,
                            validation_tag,
                            CompilerInstructionRelocationRecipe::DataAddressSites {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::ExitIndirectResultCopy {
                        source,
                        target,
                        byte_count,
                    } => {
                        let (source_offset, pointer_byte_offset) =
                            compiler_exit_indirect_result_copy_offsets(&source, &target)?;
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_copy_places(
                                    &source,
                                    &target,
                                    byte_count,
                                )?
                                .0,
                                Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_pointee(
                                    source_offset,
                                    pointer_byte_offset,
                                    0,
                                    byte_count,
                                )?,
                            },
                            20u8,
                            CompilerInstructionRelocationRecipe::PlaceCopy {
                                source,
                                target,
                                byte_count,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceCopy {
                        source,
                        target,
                        byte_count,
                    } => {
                        let shape = compiler_body_place_copy_shape(&source, &target)?;
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_copy_places(
                                    &source,
                                    &target,
                                    byte_count,
                                )?
                                .0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceCopyShape::Direct {
                                        source_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy(
                                        source_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToPointee {
                                        source_offset,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_pointee(
                                        source_offset,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromPointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed {
                                        descriptor_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_pointee_double_indexed_to_runtime_storage(
                                        descriptor_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target.region,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeePair {
                                        source_pointer_byte_offset,
                                        source_field_byte_offset,
                                        target_pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
                                        source_pointer_byte_offset,
                                        0,
                                        1,
                                        source_field_byte_offset,
                                        target_pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => match target.region {
                                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_with_index_region(
                                            descriptor_offset,
                                            index_region,
                                            index_offset,
                                            index_byte_size,
                                            element_byte_size,
                                            field_byte_offset,
                                            target_offset,
                                            byte_count,
                                        )?,
                                        omega_target_operations::RuntimeStorageRegion::Machine => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
                                            descriptor_offset,
                                            index_region,
                                            index_offset,
                                            index_byte_size,
                                            element_byte_size,
                                            field_byte_offset,
                                            target_offset,
                                            byte_count,
                                        )?,
                                    },
                                    CompilerBodyPlaceCopyShape::ToIndexed {
                                        source_offset,
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed(
                                        source_offset,
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                                        source_offset,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
                                        source.region,
                                        source_offset,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::IndexedToPointee {
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
                                        descriptor_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromFrameBaseIndexed {
                                        base_byte_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
                                        base_byte_offset,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToFrameBaseIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed {
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed(
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromMachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        index_offset,
                                        index_region,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToMachineIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                                        source_offset,
                                        base_byte_offset,
                                        index_offset,
                                        index_region,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::MachineIndexedToPointee {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_machine_indexed_to_runtime_pointee(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeeToMachineIndexed {
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_runtime_pointee_to_machine_indexed(
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee(
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed(
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_machine_double_indexed_to_runtime_pointee(
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        source_field_byte_offset,
                                        pointer_byte_offset,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed {
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_runtime_pointee_to_machine_double_indexed(
                                        pointer_byte_offset,
                                        source_field_byte_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                                        source.region,
                                        source_offset,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::MachineIndexedPair {
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
                                        source_base_byte_offset,
                                        source_index_offset,
                                        source_index_region,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_offset,
                                        target_index_region,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_frame_base_indexed_to_frame_base_indexed(
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::CrossRegionIndexedPair {
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_cross_region_indexed_pair(
                                        source.region,
                                        source_base_byte_offset,
                                        source_index_region,
                                        source_index_offset,
                                        source_index_byte_size,
                                        source_element_byte_size,
                                        source_field_byte_offset,
                                        target.region,
                                        target_base_byte_offset,
                                        target_index_region,
                                        target_index_offset,
                                        target_index_byte_size,
                                        target_element_byte_size,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair {
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_cross_region_double_indexed_pair(
                                        source.region,
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target.region,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed(
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair {
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_storage_copy_machine_double_indexed_to_machine_double_indexed(
                                        source_base_byte_offset,
                                        source_outer_index_region,
                                        source_outer_index_offset,
                                        source_outer_index_byte_size,
                                        source_outer_stride,
                                        source_inner_index_region,
                                        source_inner_index_offset,
                                        source_inner_index_byte_size,
                                        source_inner_stride,
                                        source_field_byte_offset,
                                        target_base_byte_offset,
                                        target_outer_index_region,
                                        target_outer_index_offset,
                                        target_outer_index_byte_size,
                                        target_outer_stride,
                                        target_inner_index_region,
                                        target_inner_index_offset,
                                        target_inner_index_byte_size,
                                        target_inner_stride,
                                        target_field_byte_offset,
                                        byte_count,
                                    )?,
                                    CompilerBodyPlaceCopyShape::General => {
                                        return Err(Diagnostic::error(
                                            "final aarch64 compiler-body place copy reached the x86-only general materializer class",
                                        ));
                                    }
                                },
                            },
                            21u8,
                            CompilerInstructionRelocationRecipe::PlaceCopy {
                                source,
                                target,
                                byte_count,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                        target,
                        value,
                        byte_size,
                    } => {
                        let shape = compiler_body_place_write_shape_with_cross_region_frame_base(&target)?;
                        (
                            None,
                            match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_place_integer_write(
                                    &target,
                                    value,
                                    byte_size,
                                )?
                                .0
                            }
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                omega_isa_aarch64::encode_runtime_machine_integer_write(
                                    byte_offset,
                                    byte_size,
                                    value,
                                )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_integer_write(
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_integer_write_with_index_region(
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_integer_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_integer_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_integer_write(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_integer_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed {
                                    descriptor_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_double_indexed_integer_write(
                                    descriptor_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    value,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::General => {
                                    return Err(Diagnostic::error(
                                        "final aarch64 compiler-body integer write reached the x86-only general materializer class",
                                    ));
                                }
                            },
                            },
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceIntegerWrite(target),
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceAddressWrite {
                        source,
                        target_offset,
                    } => (
                        None,
                        encode_compiler_place_address_write(architecture, &source, target_offset)?,
                        33u8,
                        CompilerInstructionRelocationRecipe::PlaceAddressWrite {
                            source,
                            target_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyConstantHostResult {
                        result_region,
                        result_offset,
                        result_byte_size,
                        value,
                    } => {
                        (
                            None,
                            match architecture {
                                Architecture::Aarch64 => {
                                    omega_isa_aarch64::encode_host_call_sequence_constant_result_from_operands(
                                        [
                                            omega_isa_aarch64::Aarch64CallOperand::RuntimeScalarInteger {
                                                byte_offset: result_offset,
                                                byte_count: result_byte_size,
                                            },
                                            omega_isa_aarch64::Aarch64CallOperand::ImmediateInteger(value),
                                        ]
                                        .into_iter(),
                                    )?
                                }
                                Architecture::X86_64 => {
                                    let operands = [
                                        omega_target_operations::InstructionOperand {
                                            kind: omega_target_operations::InstructionOperandKind::RuntimeScalarInteger {
                                                region: result_region,
                                                byte_offset: result_offset,
                                                byte_count: result_byte_size,
                                            },
                                        },
                                        omega_target_operations::InstructionOperand {
                                            kind: omega_target_operations::InstructionOperandKind::ImmediateInteger(value),
                                        },
                                    ];
                                    omega_isa_x86_64::encode_constant_result(&operands)?
                                }
                            },
                            34u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: result_region,
                                address_site: match architecture {
                                    Architecture::Aarch64 => 16,
                                    Architecture::X86_64 => 10,
                                },
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if !storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final immediate-import replay unexpectedly retained storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            43u8,
                            CompilerInstructionRelocationRecipe::ImmediateImport {
                                call_site,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final immediate-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            45u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) =
                            encode_float_parameter_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &plan,
                            )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final float-parameter import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            47u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final dereferenced-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            48u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final data-parameter import replay lost its address relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            49u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if address_sites.len() < 2
                            || !address_sites.iter().any(|(_, target)| {
                                matches!(target, OutboundCallRelocationTarget::Storage(_))
                            })
                        {
                            return Err(Diagnostic::error(
                                "final result-bearing data-parameter import replay lost its relocation roots",
                            ));
                        }
                        (
                            None,
                            bytes,
                            50u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            51u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            52u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            53u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored float import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            54u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            55u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            56u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) =
                            encode_authored_aggregate_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &data_symbols,
                                &plan,
                            )?;
                        (
                            None,
                            bytes,
                            57u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
                        operands,
                        data_symbols,
                        mechanism,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_indirect_call_replay(
                            architecture,
                            &operands,
                            &data_symbols,
                            &mechanism,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            76u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_open_create_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            58u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
                        target_region,
                        target_offset,
                        payload_offset,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            true,
                            target_offset,
                            payload_offset,
                            OutboundCallRelocationTarget::Storage(target_region),
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            59u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
                        source_region,
                        source_offset,
                        literal_symbol,
                        source_is_place,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let address_target = if source_is_place {
                            OutboundCallRelocationTarget::Storage(source_region)
                        } else {
                            OutboundCallRelocationTarget::Data(literal_symbol)
                        };
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            false,
                            source_offset,
                            0,
                            address_target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            60u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
                        buffer_symbol,
                        target_region,
                        target_offset,
                        byte_capacity,
                        target,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_line_read_replay(
                            architecture,
                            buffer_symbol,
                            target_region,
                            target_offset,
                            byte_capacity,
                            target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            61u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final storage-import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            44u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final result-bearing storage import replay lost its argument sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            46u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscall {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if !address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "no-result outbound syscall replay unexpectedly produced a result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            35u8,
                            CompilerInstructionRelocationRecipe::NoRelocations,
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "storage-argument outbound syscall replay lost its operand relocations",
                            ));
                        }
                        (
                            None,
                            bytes,
                            37u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            &operands,
                            &data_symbols,
                        )?;
                        if data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "data-argument outbound syscall replay lost its data-object relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            39u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "result-bearing outbound syscall replay lost its result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            36u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "result-bearing storage-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            38u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let Some((_, arguments)) = operands.split_first() else {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost its result operand",
                            ));
                        };
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            arguments,
                            &data_symbols,
                        )?;
                        if storage_sites.is_empty() || data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            40u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let Some((result_region, _, _)) = operands
                            .first()
                            .and_then(InstructionOperandLike::runtime_scalar_integer)
                        else {
                            return Err(Diagnostic::error(
                                "timespec-result syscall replay lost its semantic result storage",
                            ));
                        };
                        let (bytes, address_site) = encode_linux_timespec_result_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let address_site = match architecture {
                            Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                Diagnostic::error(
                                    "x86 timespec-result relocation precedes its address opcode",
                                )
                            })?,
                            Architecture::Aarch64 => address_site,
                        };
                        (
                            None,
                            bytes,
                            41u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: result_region,
                                address_site,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_site) = encode_linux_timespec_argument_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let relocation_recipe = match (
                            operands.first().and_then(InstructionOperandLike::runtime_scalar_integer),
                            address_site,
                        ) {
                            (Some((storage_region, _, _)), Some(address_site)) => {
                                CompilerInstructionRelocationRecipe::StaticStorage {
                                    storage_region,
                                    address_site: match architecture {
                                        Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                            Diagnostic::error(
                                                "x86 timespec-argument relocation precedes its address opcode",
                                            )
                                        })?,
                                        Architecture::Aarch64 => address_site,
                                    },
                                }
                            }
                            (None, None) => CompilerInstructionRelocationRecipe::NoRelocations,
                            _ => {
                                return Err(Diagnostic::error(
                                    "timespec-argument syscall replay retained inconsistent operand relocation evidence",
                                ));
                            }
                        };
                        (None, bytes, 42u8, relocation_recipe)
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite {
                        region,
                        base_byte_offset,
                        fragments,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_runtime_storage_bit_field_write(
                                base_byte_offset,
                                &fragments,
                                value,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_bit_field_write(
                                base_byte_offset,
                                &fragments,
                                value,
                            )?,
                        },
                        23u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
                        target,
                        literal,
                    } => {
                        let shape = compiler_body_place_bounded_buffer_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer write retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => {
                                    omega_isa_x86_64::encode_place_bounded_buffer_write(
                                        &target,
                                        &literal,
                                    )?
                                    .0
                                }
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                        omega_isa_aarch64::encode_runtime_machine_bounded_buffer_write(
                                            byte_offset,
                                            &literal,
                                        )?
                                    }
                                    CompilerBodyPlaceIntegerWriteShape::Pointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_pointee_bounded_buffer_write(
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_write(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_write(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_write(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_write(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    _ => unreachable!("aarch64 bounded-buffer shape checked above"),
                                },
                            },
                            24u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferWrite {
                                target,
                                literal,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
                        target,
                        literal,
                    } => {
                        let shape =
                            compiler_body_place_bounded_buffer_literal_append_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer literal append retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_bounded_buffer_literal_append(
                                    &target,
                                    &literal,
                                )?.0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
                                        omega_isa_aarch64::encode_place_bounded_buffer_literal_append(
                                            &target,
                                            &literal,
                                        )?.0
                                    }
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_literal_append(
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_literal_append(
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_literal_append(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_literal_append(
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        &literal,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::General => unreachable!(
                                        "aarch64 bounded-buffer literal-append shape checked above"
                                    ),
                                },
                            },
                            26u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferLiteralAppend {
                                target,
                                literal,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
                        target,
                        source,
                    } => {
                        let target_shape =
                            compiler_body_place_bounded_buffer_source_append_shape(&target)?;
                        let source_shape = compiler_body_place_integer_write_shape(&source)?;
                        if architecture == Architecture::Aarch64 && (!matches!(
                            target_shape,
                            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                        ) || !matches!(
                            source_shape,
                            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        )) {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body bounded-buffer source append retained an unsupported place",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_bounded_buffer_source_append(&target, &source)?.0,
                                Architecture::Aarch64 => encode_aarch64_bounded_buffer_source_append(
                                    &target,
                                    &source,
                                )?.0,
                            },
                            27u8,
                            CompilerInstructionRelocationRecipe::PlaceBoundedBufferSourceAppend {
                                target,
                                source,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite {
                        target,
                        data_symbol,
                        byte_length,
                    } => {
                        let shape = compiler_body_place_string_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body string write retained an unsupported target",
                            ));
                        }
                        let bytes = match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_string_write(
                                &target,
                                byte_length,
                            )?.0,
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                    omega_isa_aarch64::encode_runtime_machine_string_write(
                                        byte_offset,
                                        byte_length,
                                    )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_string_write(
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_string_write_with_index_region(
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_string_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_string_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_string_write_with_index_region(
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_string_write(
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_length,
                                )?,
                                _ => unreachable!("aarch64 string-write shape checked above"),
                            },
                        };
                        (
                            None,
                            bytes,
                            25u8,
                            CompilerInstructionRelocationRecipe::PlaceStringWrite {
                                target,
                                data_symbol,
                                byte_length,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireLiteralByteAppend {
                        out_region,
                        out_offset,
                        written_region,
                        written_offset,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_append_wire_literal_byte(
                                out_offset,
                                written_offset,
                                value,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_append_wire_literal_byte(
                                out_offset,
                                written_offset,
                                value,
                            )?,
                        },
                        65u8,
                        CompilerInstructionRelocationRecipe::WireLiteralByteAppend {
                            out_region,
                            written_region,
                            out_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireScalarVarintAppend {
                        source_region,
                        source_offset,
                        byte_size,
                        zigzag,
                        out_region,
                        out_offset,
                        written_region,
                        written_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_append_wire_scalar_varint(
                                source_region,
                                source_offset,
                                byte_size,
                                zigzag,
                                out_offset,
                                written_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_append_wire_scalar_varint(
                                source_region,
                                source_offset,
                                byte_size,
                                zigzag,
                                out_offset,
                                written_offset,
                            )?,
                        },
                        66u8,
                        CompilerInstructionRelocationRecipe::WireSourceAppend {
                            source_region,
                            out_region,
                            written_region,
                            out_offset,
                            written_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireTextBytesAppend {
                        source_region,
                        source_offset,
                        out_region,
                        out_offset,
                        out_length,
                        written_region,
                        written_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_append_wire_text_bytes(
                                source_region,
                                source_offset,
                                out_offset,
                                out_length,
                                written_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_append_wire_text_bytes(
                                source_region,
                                source_offset,
                                out_offset,
                                out_length,
                                written_offset,
                            )?,
                        },
                        69u8,
                        CompilerInstructionRelocationRecipe::WireSourceAppend {
                            source_region,
                            out_region,
                            written_region,
                            out_offset,
                            written_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireScalarSliceAppend {
                        source_region,
                        source_offset,
                        element_byte_size,
                        zigzag,
                        out_region,
                        out_offset,
                        out_length,
                        written_region,
                        written_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_append_wire_scalar_slice(
                                source_region,
                                source_offset,
                                element_byte_size,
                                zigzag,
                                out_offset,
                                out_length,
                                written_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_append_wire_scalar_slice(
                                source_region,
                                source_offset,
                                element_byte_size,
                                zigzag,
                                out_offset,
                                out_length,
                                written_offset,
                            )?,
                        },
                        70u8,
                        CompilerInstructionRelocationRecipe::WireSourceAppend {
                            source_region,
                            out_region,
                            written_region,
                            out_offset,
                            written_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintAppend {
                        source_region,
                        source_offset,
                        byte_size,
                        zigzag,
                        index,
                        count_region,
                        count_offset,
                        out_region,
                        out_offset,
                        written_region,
                        written_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_append_wire_repeated_scalar_varint(
                                source_region,
                                source_offset,
                                byte_size,
                                zigzag,
                                index,
                                count_region,
                                count_offset,
                                out_offset,
                                written_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_append_wire_repeated_scalar_varint(
                                source_region,
                                source_offset,
                                byte_size,
                                zigzag,
                                index,
                                count_region,
                                count_offset,
                                out_offset,
                                written_offset,
                            )?,
                        },
                        71u8,
                        CompilerInstructionRelocationRecipe::WireRepeatedScalarAppend {
                            source_region,
                            count_region,
                            out_region,
                            written_region,
                            out_offset,
                            written_offset,
                            count_offset,
                            index,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireExpectedByteRead {
                        buffer_region,
                        buffer_offset,
                        buffer_length,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        expected,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_expected_byte(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                expected,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_expected_byte(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                expected,
                            )?,
                        },
                        67u8,
                        CompilerInstructionRelocationRecipe::WireExpectedByteRead {
                            buffer_region,
                            read_region,
                            ok_region,
                            buffer_offset,
                            read_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireScalarVarintRead {
                        buffer_region,
                        buffer_offset,
                        buffer_length,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        target_region,
                        target_offset,
                        byte_size,
                        zigzag,
                        range,
                    } => {
                        let range = range.map(|range| {
                            omega_target_operations::WireScalarRange {
                                minimum: range.minimum,
                                maximum: range.maximum,
                                signed: range.signed,
                            }
                        });
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_scalar_varint(
                                    buffer_offset,
                                    buffer_length,
                                    read_offset,
                                    ok_offset,
                                    target_region,
                                    target_offset,
                                    byte_size,
                                    zigzag,
                                    range,
                                )?,
                                Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_scalar_varint(
                                    buffer_offset,
                                    buffer_length,
                                    read_offset,
                                    ok_offset,
                                    target_region,
                                    target_offset,
                                    byte_size,
                                    zigzag,
                                    range,
                                )?,
                            },
                            68u8,
                            CompilerInstructionRelocationRecipe::WireScalarVarintRead {
                                buffer_region,
                                read_region,
                                ok_region,
                                target_region,
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                zigzag,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireByteSliceRead {
                        buffer_region,
                        buffer_offset,
                        buffer_length,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        target_region,
                        target_offset,
                        predicate_mask,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_byte_slice(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                target_region,
                                target_offset,
                                predicate_mask,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_byte_slice(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                target_region,
                                target_offset,
                                predicate_mask,
                            )?,
                        },
                        72u8,
                        CompilerInstructionRelocationRecipe::WireByteSliceRead {
                            buffer_region,
                            read_region,
                            ok_region,
                            target_region,
                            buffer_offset,
                            buffer_length,
                            read_offset,
                            predicate_mask,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireNestedOpen {
                        buffer_region,
                        buffer_offset,
                        buffer_length,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        end_region,
                        end_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_nested_open(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                end_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_nested_open(
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                ok_offset,
                                end_offset,
                            )?,
                        },
                        73u8,
                        CompilerInstructionRelocationRecipe::WireNestedRead {
                            buffer_region,
                            read_region,
                            ok_region,
                            end_region,
                            buffer_offset,
                            read_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireNestedClose {
                        buffer_region,
                        buffer_offset,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        end_region,
                        end_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_nested_close(
                                buffer_offset,
                                read_offset,
                                ok_offset,
                                end_offset,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_nested_close(
                                buffer_offset,
                                read_offset,
                                ok_offset,
                                end_offset,
                            )?,
                        },
                        74u8,
                        CompilerInstructionRelocationRecipe::WireNestedRead {
                            buffer_region,
                            read_region,
                            ok_region,
                            end_region,
                            buffer_offset,
                            read_offset,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintRead {
                        buffer_region,
                        buffer_offset,
                        buffer_length,
                        read_region,
                        read_offset,
                        ok_region,
                        ok_offset,
                        end_region,
                        end_offset,
                        count_region,
                        count_offset,
                        target_region,
                        target_offset,
                        byte_size,
                        zigzag,
                        range,
                    } => {
                        let range = range.map(|range| {
                            omega_target_operations::WireScalarRange {
                                minimum: range.minimum,
                                maximum: range.maximum,
                                signed: range.signed,
                            }
                        });
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_read_wire_repeated_scalar_varint(
                                    buffer_offset,
                                    buffer_length,
                                    read_offset,
                                    ok_offset,
                                    end_offset,
                                    count_region,
                                    count_offset,
                                    target_region,
                                    target_offset,
                                    byte_size,
                                    zigzag,
                                    range,
                                )?,
                                Architecture::Aarch64 => omega_isa_aarch64::encode_read_wire_repeated_scalar_varint(
                                    buffer_offset,
                                    buffer_length,
                                    read_offset,
                                    ok_offset,
                                    end_offset,
                                    count_region,
                                    count_offset,
                                    target_region,
                                    target_offset,
                                    byte_size,
                                    zigzag,
                                    range,
                                )?,
                            },
                            75u8,
                            CompilerInstructionRelocationRecipe::WireRepeatedScalarRead {
                                buffer_region,
                                read_region,
                                ok_region,
                                end_region,
                                count_region,
                                target_region,
                                buffer_offset,
                                buffer_length,
                                read_offset,
                                end_offset,
                                target_offset,
                                byte_size,
                                zigzag,
                                range,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize {
                        buffer_symbol,
                        target,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize(
                                byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_buffer_materialize(&target)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize(
                                byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region(
                                descriptor_offset,
                                index_region,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_base_indexed(
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed(
                                base_byte_offset,
                                outer_index_offset,
                                outer_index_byte_size,
                                outer_stride,
                                inner_index_offset,
                                inner_index_byte_size,
                                inner_stride,
                                field_byte_offset,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body text-buffer materialization retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            28u8,
                            CompilerInstructionRelocationRecipe::TextBufferMaterialize {
                                buffer_symbol,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend {
                        buffer_symbol,
                        target,
                        literal,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append(
                                byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append_to_runtime_pointee(
                                pointer_byte_offset,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_literal_append(&target, &literal)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append(
                                0,
                                byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_pointee(
                                0,
                                pointer_byte_offset,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                                0,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_frame_base_indexed(
                                0,
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                &literal,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_literal_append_to_runtime_frame_base_double_indexed(
                                0,
                                base_byte_offset,
                                outer_index_offset,
                                outer_index_byte_size,
                                outer_stride,
                                inner_index_offset,
                                inner_index_byte_size,
                                inner_stride,
                                field_byte_offset,
                                &literal,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body text literal append retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            29u8,
                            CompilerInstructionRelocationRecipe::TextLiteralAppend {
                                buffer_symbol,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextStoredAppend {
                        buffer_symbol,
                        source_region,
                        source_offset,
                        target,
                    } => {
                        let shape = compiler_body_place_integer_write_shape(&target)?;
                        let bytes = match (architecture, shape) {
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append(
                                source_offset,
                                byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                                source_offset,
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::X86_64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_x86_64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                                source_offset,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (Architecture::X86_64, _) => {
                                omega_isa_x86_64::encode_place_text_stored_append(&target, source_offset)?.0
                            }
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append(
                                0,
                                source_offset,
                                byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                                0,
                                source_offset,
                                pointer_byte_offset,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    ..
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                                0,
                                source_offset,
                                descriptor_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region: _,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_base_indexed(
                                0,
                                source_offset,
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            )?,
                            (
                                Architecture::Aarch64,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                },
                            ) => omega_isa_aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_base_double_indexed(
                                0,
                                source_offset,
                                base_byte_offset,
                                outer_index_offset,
                                outer_index_byte_size,
                                outer_stride,
                                inner_index_offset,
                                inner_index_byte_size,
                                inner_stride,
                                field_byte_offset,
                            )?,
                            _ => {
                                return Err(Diagnostic::error(
                                    "final compiler-body stored-text append retained an unsupported target",
                                ));
                            }
                        };
                        (
                            None,
                            bytes,
                            30u8,
                            CompilerInstructionRelocationRecipe::TextStoredAppend {
                                buffer_symbol,
                                source_region,
                                target,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite {
                        buffer_symbol,
                        byte_offset,
                        literal,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_literal_segment_write(
                                    byte_offset,
                                    &literal,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_literal_segment_write(
                                    byte_offset,
                                    &literal,
                                )?
                            }
                        },
                        31u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextLiteral {
                            buffer_symbol,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend {
                        buffer_symbol,
                        buffer_offset,
                        source_region,
                        source_offset,
                        target_region,
                        target_offset,
                        length_delta,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_stored_suffix_append(
                                    buffer_offset,
                                    source_offset,
                                    target_offset,
                                    length_delta,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_stored_suffix_append(
                                    buffer_offset,
                                    source_offset,
                                    target_offset,
                                    length_delta,
                                )?
                            }
                        },
                        32u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextStoredSuffix {
                            buffer_symbol,
                            source_region,
                            target_region,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceBinaryWrite {
                        target,
                        byte_size,
                        left,
                        operator,
                        right,
                        is_float,
                        domain,
                        target_signed,
                    } => {
                        let shape = compiler_body_place_binary_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. },
                            )
                        {
                            return Err(Diagnostic::error(
                                "final compiler-body binary-write subset retained an unsupported target",
                            ));
                        }
                        let bytes = match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_binary_write(
                                &code.runtime_value_operands,
                                &target,
                                byte_size,
                                left,
                                operator,
                                right,
                                is_float,
                                domain,
                                target_signed,
                            )?.0,
                            Architecture::Aarch64 => match shape {
                                CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                                    omega_isa_aarch64::encode_runtime_storage_binary_write(
                                        &code.runtime_value_operands,
                                        byte_offset,
                                        byte_size,
                                        left,
                                        operator,
                                        right,
                                        is_float,
                                        domain,
                                        target_signed,
                                    )?
                                }
                                CompilerBodyPlaceIntegerWriteShape::Pointee {
                                    pointer_byte_offset,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_pointee_binary_write(
                                    &code.runtime_value_operands,
                                    pointer_byte_offset,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_indexed_binary_write_with_index_region(
                                    &code.runtime_value_operands,
                                    descriptor_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_frame_base_indexed_binary_write_with_index_region(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                    base_byte_offset,
                                    outer_index_region,
                                    outer_index_offset,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_region,
                                    inner_index_offset,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                } => omega_isa_aarch64::encode_runtime_machine_double_indexed_binary_write(
                                    &code.runtime_value_operands,
                                    base_byte_offset,
                                    outer_index_offset,
                                    outer_index_region,
                                    outer_index_byte_size,
                                    outer_stride,
                                    inner_index_offset,
                                    inner_index_region,
                                    inner_index_byte_size,
                                    inner_stride,
                                    field_byte_offset,
                                    byte_size,
                                    left,
                                    operator,
                                    right,
                                )?,
                                _ => unreachable!("binary-write shape checked above"),
                            },
                        };
                        (
                            None,
                            bytes,
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceBinaryWrite {
                                target,
                                left,
                                right,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyStorageConvertWrite {
                        target_region,
                        target_offset,
                        target_byte_size,
                        source,
                        source_byte_size,
                        source_is_float,
                        target_is_float,
                        source_signed,
                        target_signed,
                        trapping,
                        saturating,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_runtime_storage_convert(
                                &code.runtime_value_operands,
                                target_offset,
                                target_byte_size,
                                source,
                                source_byte_size,
                                source_is_float,
                                target_is_float,
                                source_signed,
                                target_signed,
                                trapping,
                                saturating,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_runtime_storage_convert(
                                &code.runtime_value_operands,
                                target_offset,
                                target_byte_size,
                                source,
                                source_byte_size,
                                source_is_float,
                                target_is_float,
                                source_signed,
                                target_signed,
                                trapping,
                                saturating,
                            )?,
                        },
                        22u8,
                        CompilerInstructionRelocationRecipe::StorageConvertWrite {
                            target_region,
                            source,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceConvertWrite {
                        target,
                        target_byte_size,
                        source,
                        source_byte_size,
                        source_is_float,
                        target_is_float,
                        source_signed,
                        target_signed,
                        trapping,
                        saturating,
                    } => {
                        let shape = compiler_body_place_convert_write_shape(&target)?;
                        if architecture == Architecture::Aarch64
                            && !matches!(
                                shape,
                                CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                                    | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                            )
                        {
                            return Err(Diagnostic::error(
                                "final aarch64 compiler-body place conversion retained an unsupported target",
                            ));
                        }
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => omega_isa_x86_64::encode_place_convert_write(
                                    &code.runtime_value_operands,
                                    &target,
                                    target_byte_size,
                                    source,
                                    source_byte_size,
                                    source_is_float,
                                    target_is_float,
                                    source_signed,
                                    target_signed,
                                    trapping,
                                    saturating,
                                )?.0,
                                Architecture::Aarch64 => match shape {
                                    CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } =>
                                        omega_isa_aarch64::encode_runtime_storage_convert(
                                            &code.runtime_value_operands,
                                            byte_offset,
                                            target_byte_size,
                                            source,
                                            source_byte_size,
                                            source_is_float,
                                            target_is_float,
                                            source_signed,
                                            target_signed,
                                            trapping,
                                            saturating,
                                        )?,
                                    CompilerBodyPlaceIntegerWriteShape::Pointee {
                                        pointer_byte_offset,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_pointee_convert_write(
                                        &code.runtime_value_operands,
                                        pointer_byte_offset,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        descriptor_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_indexed_convert_write_with_index_region(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                                        base_byte_offset,
                                        outer_index_region,
                                        outer_index_offset,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_region,
                                        inner_index_offset,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                    } => omega_isa_aarch64::encode_runtime_machine_double_indexed_convert_write(
                                        &code.runtime_value_operands,
                                        base_byte_offset,
                                        outer_index_offset,
                                        outer_index_region,
                                        outer_index_byte_size,
                                        outer_stride,
                                        inner_index_offset,
                                        inner_index_region,
                                        inner_index_byte_size,
                                        inner_stride,
                                        field_byte_offset,
                                        target_byte_size,
                                        source,
                                        source_byte_size,
                                        source_is_float,
                                        target_is_float,
                                        source_signed,
                                        target_signed,
                                        trapping,
                                        saturating,
                                    )?,
                                    _ => unreachable!("aarch64 place-convert shape checked above"),
                                },
                            },
                            22u8,
                            CompilerInstructionRelocationRecipe::PlaceConvertWrite {
                                target,
                                source,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchStateWrite {
                        dispatch_index,
                        case_leave_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?.to_vec(),
                        },
                        5u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchCaseLeave {
                        loop_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(loop_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(loop_byte_distance)?.to_vec(),
                        },
                        7u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchForwardBranchSkip {
                        branch_arms_end_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?.to_vec(),
                        },
                        6u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    _ => unreachable!("control/entry instruction specification was dispatched above"),
                };

    Ok((
        expected_position,
        expected_bytes,
        kind_tag,
        relocation_recipe,
    ))
}
