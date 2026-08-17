//! Derives bit-field, bounded-buffer, wire, and text footprints.

use super::*;

pub(super) fn buffer_wire_text_footprint_parts(
    architecture: Architecture,
    _runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprintParts> {
    use omega_calling_conventions::MachineStateSet;
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let parts = match kind {
        CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_x86_64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_x86_64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_aarch64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_aarch64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
            target, ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_write_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_write_additional_machine_state(&target),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_write_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_write_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_write_additional_machine_state(),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
            target,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_literal_append_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_literal_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_literal_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_literal_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
            target,
            source,
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_source_append_register_writes(
                    &target, &source,
                ),
                omega_isa_x86_64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_source_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) || !matches!(
                    compiler_body_place_integer_write_shape(&source).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_source_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_source_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite { target, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                    omega_isa_x86_64::place_string_write_register_writes(&target),
                    omega_isa_x86_64::place_string_write_additional_machine_state(&target),
                ),
                Architecture::Aarch64 => {
                    if !matches!(
                        compiler_body_place_string_write_shape(&target).ok()?,
                        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                    ) {
                        return None;
                    }
                    (
                        BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                        omega_isa_aarch64::place_string_write_register_write_ceiling(),
                        omega_isa_aarch64::place_string_write_additional_machine_state(),
                    )
                }
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireLiteralByteAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                    omega_isa_x86_64::append_wire_literal_byte_clobbers(),
                    omega_isa_x86_64::append_wire_literal_byte_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                    omega_isa_aarch64::append_wire_literal_byte_clobbers(),
                    MachineStateSet::empty(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireScalarVarintAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                    omega_isa_x86_64::append_wire_scalar_varint_clobbers(),
                    omega_isa_x86_64::append_wire_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                    omega_isa_aarch64::append_wire_scalar_varint_clobbers(),
                    MachineStateSet::empty(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireTextBytesAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                    omega_isa_x86_64::append_wire_text_bytes_clobbers(),
                    omega_isa_x86_64::append_wire_text_bytes_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                    omega_isa_aarch64::append_wire_text_bytes_clobbers(),
                    omega_isa_aarch64::append_wire_text_bytes_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireScalarSliceAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                    omega_isa_x86_64::append_wire_scalar_slice_clobbers(),
                    omega_isa_x86_64::append_wire_scalar_slice_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                    omega_isa_aarch64::append_wire_scalar_slice_clobbers(),
                    omega_isa_aarch64::append_wire_scalar_slice_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintAppend {
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                omega_isa_x86_64::append_wire_repeated_scalar_varint_clobbers(),
                omega_isa_x86_64::append_wire_repeated_scalar_varint_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                omega_isa_aarch64::append_wire_repeated_scalar_varint_clobbers(),
                omega_isa_aarch64::append_wire_repeated_scalar_varint_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireExpectedByteRead {
            read_offset,
            ok_offset,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                omega_isa_x86_64::read_wire_expected_byte_clobbers(),
                omega_isa_x86_64::read_wire_expected_byte_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                omega_isa_aarch64::read_wire_expected_byte_clobbers(read_offset, ok_offset),
                omega_isa_aarch64::read_wire_expected_byte_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireScalarVarintRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                    omega_isa_x86_64::read_wire_scalar_varint_clobbers(),
                    omega_isa_x86_64::read_wire_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                    omega_isa_aarch64::read_wire_scalar_varint_clobbers(),
                    omega_isa_aarch64::read_wire_scalar_varint_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireByteSliceRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                    omega_isa_x86_64::read_wire_byte_slice_clobbers(),
                    omega_isa_x86_64::read_wire_byte_slice_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                    omega_isa_aarch64::read_wire_byte_slice_clobbers(),
                    omega_isa_aarch64::read_wire_byte_slice_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireNestedOpen { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                    omega_isa_x86_64::read_wire_nested_open_clobbers(),
                    omega_isa_x86_64::read_wire_nested_open_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                    omega_isa_aarch64::read_wire_nested_open_clobbers(),
                    omega_isa_aarch64::read_wire_nested_open_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireNestedClose { .. } => match architecture
        {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                omega_isa_x86_64::read_wire_nested_close_clobbers(),
                omega_isa_x86_64::read_wire_nested_close_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                omega_isa_aarch64::read_wire_nested_close_clobbers(),
                omega_isa_aarch64::read_wire_nested_close_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_clobbers(),
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_clobbers(),
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::place_text_buffer_materialize_additional_machine_state(
                        &target,
                    ),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_literal_append_register_writes(&target),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_stored_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_segment_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_segment_write_additional_machine_state(
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
            }
        }
        _ => return None,
    };
    Some(parts)
}
