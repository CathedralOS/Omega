//! Encodes retained place writes and derives their exact register and relocation sites.

use super::*;

pub(super) fn encode_compiler_place_address_write(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_place_address_write(source, target_offset)
            .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match compiler_body_place_address_write_shape(source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                omega_isa_aarch64::encode_runtime_storage_address_to_runtime_frame_write(
                    byte_offset,
                    target_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_pointee_address_to_runtime_frame_write(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                index_region,
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                omega_isa_aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    target_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
                base_byte_offset,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
            ),
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
            } => omega_isa_aarch64::encode_runtime_machine_double_indexed_address_to_runtime_frame_write(
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
            ),
            CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address row retained an unsupported source shape",
            )),
        },
    }
}

pub(super) fn compiler_place_address_write_register_writes(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<omega_calling_conventions::RegisterSet, Diagnostic> {
    match architecture {
        Architecture::X86_64 => Ok(omega_isa_x86_64::place_address_write_register_writes(source)),
        Architecture::Aarch64 => match compiler_body_place_address_write_shape(source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => Ok(
                omega_isa_aarch64::runtime_storage_address_to_runtime_frame_write_clobbers(
                    byte_offset,
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => Ok(
                omega_isa_aarch64::runtime_pointee_address_to_runtime_frame_write_clobbers(
                    pointer_byte_offset,
                    field_byte_offset,
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region, ..
            } => Ok(
                omega_isa_aarch64::runtime_frame_indexed_address_to_runtime_frame_write_clobbers(
                    index_region,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { index_region, .. } => Ok(
                omega_isa_aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers_with_index_region(
                    index_region,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. } => Ok(
                omega_isa_aarch64::runtime_machine_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => Ok(
                omega_isa_aarch64::runtime_frame_base_double_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. } => Ok(
                omega_isa_aarch64::runtime_machine_double_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address footprint retained an unsupported source shape",
            )),
        },
    }
}

pub(super) fn compiler_place_value_address_sites(
    architecture: Architecture,
    place: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::PlaceValueGuard {
        byte_size,
        expected_value,
        failure_branch_distance,
        operator,
        ..
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place-value validation recipe",
        ));
    };
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_place_value_compare(
                &place,
                byte_size,
                expected_value,
                failure_branch_distance,
                operator,
            )?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => place.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => place
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-value index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => place
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-value second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place-value recipe retained an invalid source relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => Ok(vec![(0, place.region)]),
    }
}

pub(super) fn compiler_place_integer_write_address_sites(
    architecture: Architecture,
    place: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
        target,
        value,
        byte_size,
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place integer-write validation recipe",
        ));
    };
    if target != place {
        return Err(Diagnostic::error(
            "final place integer-write relocation recipe changed its retained target",
        ));
    }
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) =
                omega_isa_x86_64::encode_place_integer_write(&place, value, byte_size)?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => place.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => place
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place integer-write index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => place
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place integer-write second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place integer-write recipe retained an invalid source relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => {
            let shape = compiler_body_place_write_shape_with_cross_region_frame_base(&place)?;
            let mut sites = vec![(0, place.region)];
            if let CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                ..
            } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                sites.push((
                    omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                ..
            } = shape
                && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } = shape
                && (outer_index_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ));
            }
            if let CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } = shape
                && (outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                    || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine)
            {
                sites.push((
                    omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    omega_target_operations::RuntimeStorageRegion::Machine,
                ));
            }
            Ok(sites)
        }
    }
}

pub(super) fn compiler_place_address_write_address_sites(
    architecture: Architecture,
    source: omega_target_operations::Place,
    target_offset: usize,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            let (bytes, sites) =
                omega_isa_x86_64::encode_place_address_write(&source, target_offset)?;
            let mut address_sites = sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => source.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => source
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-address index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => source
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-address second index relocation has no retained index step"))?,
                        _ => return Err(Diagnostic::error("place-address recipe retained an invalid source-side relocation site")),
                    };
                    Ok((offset, region))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            address_sites.push((
                bytes.len().checked_sub(17).ok_or_else(|| {
                    Diagnostic::error("place-address encoder omitted its target frame store")
                })?,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            ));
            Ok(address_sites)
        }
        Architecture::Aarch64 => match compiler_body_place_address_write_shape(&source)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => Ok(vec![
                (0, source.region),
                (
                    omega_isa_aarch64::runtime_storage_address_to_runtime_frame_target_frame_offset(
                        byte_offset,
                    ),
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => Ok(vec![(
                0,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )]),
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                ..
            } => {
                let mut sites = vec![(
                    0,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                )];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } => {
                let mut sites = vec![(
                    0,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                )];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                let mut sites = vec![(0, omega_target_operations::RuntimeStorageRegion::Machine)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                                base_byte_offset,
                            ),
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ));
                }
                sites.push((
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                Ok(sites)
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => Ok(vec![(
                0,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            )]),
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => Ok(vec![
                (0, omega_target_operations::RuntimeStorageRegion::Machine),
                (
                    omega_isa_aarch64::runtime_machine_double_indexed_address_frame_base_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
                "final aarch64 place-address recipe retained an unsupported source",
            )),
        },
    }
}

pub(super) fn compiler_place_binary_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target: omega_target_operations::Place,
    left: omega_target_operations::RuntimeValueOperandHandle,
    right: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
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
            "final compiler-body binary-write relocation recipe retained an unsupported target",
        ));
    }
    let operand_start = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::place_binary_operand_start_width(&target),
        Architecture::Aarch64 => match shape {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. } => 8,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_pointee_operand_start_width(
                pointer_byte_offset,
                field_byte_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                omega_isa_aarch64::runtime_frame_indexed_integer_write_width(
                    element_byte_size,
                    field_byte_offset,
                    0,
                ) + usize::from(
                    index_region == omega_target_operations::RuntimeStorageRegion::Machine,
                ) * 8
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                omega_isa_aarch64::runtime_frame_base_double_indexed_binary_left_operand_offset()
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_machine_indexed_integer_write_width(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                0,
            ),
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                outer_index_region,
                inner_index_region,
            ),
            _ => unreachable!("binary-write shape checked above"),
        },
    };
    let mut sites = vec![(0, target.region)];
    if architecture == Architecture::X86_64 {
        sites.extend(omega_isa_x86_64::place_binary_index_base_positions(&target));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            ..
        } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        sites.push((
            omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                base_byte_offset,
            ),
            index_region,
        ));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            ..
        } = shape
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        sites.push((
            omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                base_byte_offset,
            ),
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        ));
    }
    if architecture == Architecture::Aarch64
        && let CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            outer_index_region,
            inner_index_region,
            ..
        } = shape
        && (outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            || inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
    {
        sites.push((
            omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        ));
    }
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        left,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    let right_gap = match architecture {
        Architecture::X86_64 => omega_isa_x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
        Architecture::Aarch64 => 0,
    };
    let right_offset = operand_start
        + compiler_runtime_value_operand_width(architecture, operands, left)?
        + right_gap;
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        right,
        right_offset,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

pub(super) fn compiler_storage_convert_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target_region: omega_target_operations::RuntimeStorageRegion,
    source: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let operand_start = match architecture {
        Architecture::X86_64 => 10,
        Architecture::Aarch64 => 8,
    };
    let mut sites = vec![(0, target_region)];
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        source,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

pub(super) fn compiler_place_convert_write_address_sites(
    architecture: Architecture,
    operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    target: omega_target_operations::Place,
    source: omega_target_operations::RuntimeValueOperandHandle,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let mut sites = vec![(0, target.region)];
    let operand_start = match architecture {
        Architecture::X86_64 => {
            sites.extend(omega_isa_x86_64::place_binary_index_base_positions(&target));
            omega_isa_x86_64::place_binary_operand_start_width(&target)
        }
        Architecture::Aarch64 => match compiler_body_place_convert_write_shape(&target)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. } => 8,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => omega_isa_aarch64::runtime_pointee_operand_start_width(
                pointer_byte_offset,
                field_byte_offset,
            ),
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                omega_isa_aarch64::runtime_frame_indexed_operand_start_width(
                    index_region,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        omega_target_operations::RuntimeStorageRegion::Machine,
                    ));
                }
                omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                omega_isa_aarch64::runtime_frame_base_double_indexed_convert_operand_offset()
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                            omega_isa_aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(
                                base_byte_offset,
                            ),
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ));
                }
                omega_isa_aarch64::runtime_machine_indexed_integer_write_width(
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    0,
                )
            }
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                }
                omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                    outer_index_region,
                    inner_index_region,
                )
            }
            _ => {
                return Err(Diagnostic::error(
                    "final aarch64 compiler-body place-convert relocation recipe retained an unsupported target",
                ));
            }
        },
    };
    let mut visiting = Vec::new();
    collect_compiler_runtime_value_address_sites(
        architecture,
        operands,
        source,
        operand_start,
        &mut visiting,
        &mut sites,
    )?;
    Ok(sites)
}

pub(super) fn aarch64_bounded_buffer_write_relocation_sites(
    target: omega_target_operations::Place,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    use omega_target_operations::RuntimeStorageRegion;

    let mut sites = vec![(0, target.region)];
    match compiler_body_place_bounded_buffer_write_shape(&target)? {
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {}
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            ..
        } => {
            if index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                        base_byte_offset,
                    ),
                    index_region,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed { index_region, .. } => {
            if index_region == RuntimeStorageRegion::Machine {
                sites.push((
                    omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                    index_region,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            ..
        } => {
            if index_region == RuntimeStorageRegion::RuntimeFrame {
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(
                        base_byte_offset,
                    ),
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            outer_index_region,
            inner_index_region,
            ..
        } => {
            if outer_index_region == RuntimeStorageRegion::RuntimeFrame
                || inner_index_region == RuntimeStorageRegion::RuntimeFrame
            {
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                    RuntimeStorageRegion::RuntimeFrame,
                ));
            }
        }
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {}
        CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
        | CompilerBodyPlaceIntegerWriteShape::General => {
            return Err(Diagnostic::error(
                "final aarch64 bounded-buffer write retained an unsupported target",
            ));
        }
    }
    Ok(sites)
}

pub(super) fn encode_aarch64_bounded_buffer_source_append(
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<(Vec<u8>, omega_isa_aarch64::BoundedBufferPlaceSites), Diagnostic> {
    if !matches!(
        compiler_body_place_integer_write_shape(source)?,
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
    ) {
        return Err(Diagnostic::error(
            "final aarch64 bounded-buffer source append retained an unsupported source",
        ));
    }
    match compiler_body_place_bounded_buffer_source_append_shape(target)? {
        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
            omega_isa_aarch64::encode_place_bounded_buffer_source_append(target, source)
        }
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_frame_indexed_bounded_buffer_source_append(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_machine_indexed_bounded_buffer_source_append(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
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
        } => omega_isa_aarch64::encode_runtime_machine_double_indexed_bounded_buffer_source_append(
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
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        } => omega_isa_aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_source_append(
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            source,
        ),
        CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed { .. }
        | CompilerBodyPlaceIntegerWriteShape::General => Err(Diagnostic::error(
            "final aarch64 bounded-buffer source append retained an unsupported target",
        )),
    }
}

pub(super) fn aarch64_text_buffer_materialize_buffer_address_offset(
    target: omega_target_operations::Place,
) -> Result<usize, Diagnostic> {
    let total_width = match compiler_body_place_integer_write_shape(&target)? {
        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            index_region,
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region_width(
            index_region,
            element_byte_size,
            field_byte_offset,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region: _,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_indexed_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
            omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_width()
        }
        _ => {
            return Err(Diagnostic::error(
                "final aarch64 indexed text-buffer materialization retained an unsupported target",
            ));
        }
    };
    total_width.checked_sub(40).ok_or_else(|| {
        Diagnostic::error("aarch64 text-buffer materialization width underflowed its fixed tail")
    })
}
