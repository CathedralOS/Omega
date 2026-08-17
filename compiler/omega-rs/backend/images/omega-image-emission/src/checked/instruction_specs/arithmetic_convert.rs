//! Reconstructs binary arithmetic and scalar conversion write specifications.

use super::*;

pub(super) fn expected_arithmetic_convert_spec(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Option<CompilerInstructionSpec>, Diagnostic> {
    let spec: CompilerInstructionSpec = match kind {
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
        _ => return Ok(None),
    };

    Ok(Some(spec))
}
