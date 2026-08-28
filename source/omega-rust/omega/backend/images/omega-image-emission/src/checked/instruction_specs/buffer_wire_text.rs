//! Reconstructs bit-field, bounded-buffer, wire, and text specifications.

use super::*;

pub(super) fn expected_buffer_wire_text_spec(
    architecture: Architecture,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Option<CompilerInstructionSpec>, Diagnostic> {
    let spec: CompilerInstructionSpec = match kind {
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
        _ => return Ok(None),
    };

    Ok(Some(spec))
}
