//! Validates exact compiler relocation sets, symbols, and unchanged instruction bits.

use super::*;

pub(super) fn validate_compiler_place_string_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    data_symbol: &str,
    byte_length: usize,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Data,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let mut sites = Vec::new();
    match architecture {
        Architecture::X86_64 => {
            sites.push((0usize, ExpectedTarget::Data));
            let (_, target_sites) =
                omega_isa_x86_64::encode_place_string_write(&target, byte_length)?;
            for (offset, side) in target_sites.iter() {
                let region = match side {
                    omega_isa_x86_64::PlaceCopySide::Target => target.region,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                        .scaled_index_region()
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "string-write target index relocation has no retained index step",
                            )
                        })?,
                    omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                        .scaled_index_regions()
                        .nth(1)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "string-write second target index relocation has no retained index step",
                            )
                        })?,
                    _ => {
                        return Err(Diagnostic::error(
                            "string write retained an invalid source relocation site",
                        ));
                    }
                };
                sites.push((offset, ExpectedTarget::Storage(region)));
            }
        }
        Architecture::Aarch64 => match compiler_body_place_string_write_shape(&target)? {
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. } => {
                sites.push((0, ExpectedTarget::Data));
                sites.push((8, ExpectedTarget::Storage(target.region)));
            }
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
                        ExpectedTarget::Storage(index_region),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_frame_indexed_string_data_address_offset_with_index_region(
                        index_region,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        ExpectedTarget::Storage(
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_machine_double_indexed_string_data_address_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                sites.push((
                    0,
                    ExpectedTarget::Storage(
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ),
                ));
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_double_indexed_string_data_address_offset(
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        ExpectedTarget::Storage(index_region),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_frame_base_indexed_string_data_address_offset_with_index_region(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => {
                sites.push((0, ExpectedTarget::Storage(target.region)));
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        ExpectedTarget::Storage(
                            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                        ),
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_machine_indexed_string_data_address_offset_with_index_region(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    ExpectedTarget::Data,
                ));
            }
            _ => {
                return Err(Diagnostic::error(
                    "final aarch64 string-write relocation recipe retained an unsupported target",
                ));
            }
        },
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Data => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        data_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler string-write instruction #{selected_instruction_index} does not retain its exact data/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

pub(super) fn validate_compiler_text_buffer_materialize_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (
                omega_isa_x86_64::RUNTIME_TEXT_BUFFER_MATERIALIZE_TARGET_IMM_OFFSET,
                ExpectedTarget::Storage(target.region),
            ),
        ],
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_byte_size, ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site) =
                omega_isa_x86_64::encode_place_text_buffer_materialize(&target)?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("text materialization walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites
        }
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
            | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
        ) => {
            let mut sites = aarch64_bounded_buffer_write_relocation_sites(target)?
                .into_iter()
                .map(|(site, region)| (site, ExpectedTarget::Storage(region)))
                .collect::<Vec<_>>();
            sites.push((
                aarch64_text_buffer_materialize_buffer_address_offset(target)?,
                ExpectedTarget::Buffer,
            ));
            sites
        }
        _ => {
            return Err(Diagnostic::error(
                "final text-buffer materialization relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler text-buffer materialization instruction #{selected_instruction_index} does not retain its exact buffer/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

pub(super) fn validate_compiler_text_literal_append_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
        ],
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_INDEXED_LITERAL_APPEND_BUFFER_IMM_OFFSET,
                ExpectedTarget::Buffer,
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site) =
                omega_isa_x86_64::encode_place_text_literal_append(&target, b"")?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("literal append walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites
        }
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Direct { .. }
            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_indexed_literal_append_buffer_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
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
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_double_indexed_literal_append_buffer_address_offset(),
                ExpectedTarget::Buffer,
            ),
        ],
        _ => {
            return Err(Diagnostic::error(
                "final text literal-append relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler text literal-append instruction #{selected_instruction_index} does not retain its exact buffer/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

pub(super) fn validate_compiler_text_stored_append_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    target: omega_target_operations::Place,
    buffer_symbol: &str,
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> Result<Vec<usize>, Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget {
        Buffer,
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let sites = match (
        architecture,
        compiler_body_place_integer_write_shape(&target)?,
    ) {
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::Direct { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET,
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (Architecture::X86_64, CompilerBodyPlaceIntegerWriteShape::Pointee { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (10usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET,
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::X86_64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                index_byte_size, ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_buffer_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_source_imm_offset(
                    index_byte_size,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (Architecture::X86_64, _) => {
            let (_, encoded_sites, buffer_site, source_site) =
                omega_isa_x86_64::encode_place_text_stored_append(&target, 0)?;
            let mut sites = encoded_sites
                .iter()
                .map(|(site, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .expect("target index site implies an index"),
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .expect("second target index site implies two indices"),
                        _ => unreachable!("stored-text append walks only its target"),
                    };
                    (site, ExpectedTarget::Storage(region))
                })
                .collect::<Vec<_>>();
            sites.push((buffer_site, ExpectedTarget::Buffer));
            sites.push((source_site, ExpectedTarget::Storage(source_region)));
            sites
        }
        (Architecture::Aarch64, CompilerBodyPlaceIntegerWriteShape::Direct { .. }) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
            (28usize, ExpectedTarget::Storage(source_region)),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            },
        ) => vec![
            (0usize, ExpectedTarget::Buffer),
            (8usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_stored_place_pointee_source_address_offset(
                    pointer_byte_offset,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_indexed_stored_place_buffer_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_aarch64::runtime_text_indexed_stored_place_source_address_offset(
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
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
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_aarch64::runtime_text_frame_base_indexed_stored_place_source_address_offset(
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                ),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        (
            Architecture::Aarch64,
            CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
        ) => vec![
            (0usize, ExpectedTarget::Storage(target.region)),
            (
                omega_isa_aarch64::runtime_text_frame_base_double_indexed_stored_place_buffer_address_offset(),
                ExpectedTarget::Buffer,
            ),
            (
                omega_isa_aarch64::runtime_text_frame_base_double_indexed_stored_place_source_address_offset(),
                ExpectedTarget::Storage(source_region),
            ),
        ],
        _ => {
            return Err(Diagnostic::error(
                "final stored-text append relocation recipe retained an unsupported target",
            ));
        }
    };

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in &sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        buffer_symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler stored-text append instruction #{selected_instruction_index} does not retain its exact buffer/source/target relocation set"
        )));
    }
    Ok(sites.into_iter().map(|(site, _)| site).collect())
}

pub(super) fn validate_compiler_data_address_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, region) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                *region,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    *region,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    *region,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, region))| {
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} does not retain its exact operand-derived storage relocation set"
        )));
    }
    Ok(())
}

pub(super) fn validate_compiler_immediate_import_relocation(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    expected_library: &str,
    expected_symbol: &str,
) -> Result<(), Diagnostic> {
    let actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    let (kind, width) = match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4usize),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4usize),
    };
    let symbol_matches = actual.first().is_some_and(|relocation| {
        compiler_import_symbol_matches(
            object,
            relocation.symbol_handle,
            expected_library,
            expected_symbol,
        )
    });
    let matches = actual.len() == 1
        && actual[0].offset == instruction_byte_offset + call_site
        && actual[0].kind == kind
        && actual[0].byte_width == width
        && actual[0].addend == 0
        && symbol_matches;
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler immediate-import instruction #{selected_instruction_index} does not retain its exact library/symbol call relocation"
        )));
    }
    Ok(())
}

pub(super) fn validate_compiler_internal_call_relocation(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    expected_target: omega_control_flow::MachineFunctionIdentity,
) -> Result<(), Diagnostic> {
    let actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    let (kind, width) = match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4usize),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4usize),
    };
    let expected_symbol = omega_object_file::object_function_symbol(object, expected_target)
        .map(|(handle, _)| handle);
    let matches = actual.len() == 1
        && expected_symbol.is_some()
        && actual[0].offset == instruction_byte_offset + call_site
        && actual[0].kind == kind
        && actual[0].byte_width == width
        && actual[0].addend == 0
        && Some(actual[0].symbol_handle) == expected_symbol;
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler internal-call instruction #{selected_instruction_index} does not retain its exact target-identity relocation",
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_compiler_storage_import_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    storage_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
    expected_library: &str,
    expected_symbol: &str,
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = vec![match architecture {
        Architecture::X86_64 => (
            instruction_byte_offset + call_site,
            RelocationKind::X86_64Relative32,
            4usize,
            None,
        ),
        Architecture::Aarch64 => (
            instruction_byte_offset + call_site,
            RelocationKind::Aarch64Branch26,
            4usize,
            None,
        ),
    }];
    for (site, region) in storage_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site,
                RelocationKind::Absolute64,
                8usize,
                Some(*region),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    Some(*region),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    Some(*region),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual.iter().zip(&expected).all(
            |(relocation, (offset, kind, width, storage_region))| {
                let target_matches = storage_region.map_or_else(
                    || {
                        compiler_import_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            expected_library,
                            expected_symbol,
                        )
                    },
                    |region| {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, region)
                    },
                );
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            },
        );
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler storage-import instruction #{selected_instruction_index} does not retain its exact call/storage relocation set"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_compiler_planned_import_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_site: usize,
    address_sites: &[(usize, OutboundCallRelocationTarget)],
    expected_locator: &omega_calling_conventions::HostImportLocator,
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = vec![match architecture {
        Architecture::X86_64 => (
            instruction_byte_offset + call_site,
            RelocationKind::X86_64Relative32,
            4usize,
            None,
        ),
        Architecture::Aarch64 => (
            instruction_byte_offset + call_site,
            RelocationKind::Aarch64Branch26,
            4usize,
            None,
        ),
    }];
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site,
                RelocationKind::Absolute64,
                8usize,
                Some(target),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    Some(target),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    Some(target),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = target.map_or_else(
                    || {
                        compiler_import_locator_matches(
                            object,
                            relocation.symbol_handle,
                            expected_locator,
                        )
                    },
                    |target| match target {
                        OutboundCallRelocationTarget::Storage(region) => {
                            compiler_storage_symbol_matches(
                                object,
                                relocation.symbol_handle,
                                *region,
                            )
                        }
                        OutboundCallRelocationTarget::Data(symbol) => {
                            compiler_data_object_symbol_matches(
                                object,
                                relocation.symbol_handle,
                                symbol,
                            )
                        }
                    },
                );
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler planned-import instruction #{selected_instruction_index} does not retain its exact call/data/storage relocation set"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_compiler_runtime_text_boundary_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    call_sites: &[(usize, std::sync::Arc<str>, std::sync::Arc<str>)],
    address_sites: &[(usize, OutboundCallRelocationTarget)],
) -> Result<(), Diagnostic> {
    enum ExpectedTarget<'target> {
        Import(&'target str, &'target str),
        Address(&'target OutboundCallRelocationTarget),
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, library, symbol) in call_sites {
        let kind = match architecture {
            Architecture::X86_64 => RelocationKind::X86_64Relative32,
            Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        };
        expected.push((
            instruction_byte_offset + site,
            kind,
            4usize,
            ExpectedTarget::Import(library, symbol),
        ));
    }
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                ExpectedTarget::Address(target),
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    ExpectedTarget::Address(target),
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    ExpectedTarget::Address(target),
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Import(library, symbol) => compiler_import_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        library,
                        symbol,
                    ),
                    ExpectedTarget::Address(OutboundCallRelocationTarget::Storage(region)) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                    ExpectedTarget::Address(OutboundCallRelocationTarget::Data(symbol)) => {
                        compiler_data_object_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            symbol,
                        )
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler runtime-text instruction #{selected_instruction_index} does not retain its exact call/address relocation set"
        )));
    }
    Ok(())
}

pub(super) fn validate_compiler_outbound_syscall_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    address_sites: &[(usize, OutboundCallRelocationTarget)],
) -> Result<(), Diagnostic> {
    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);
    let mut expected = Vec::new();
    for (site, target) in address_sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    OutboundCallRelocationTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                    OutboundCallRelocationTarget::Data(symbol) => {
                        compiler_data_object_symbol_matches(
                            object,
                            relocation.symbol_handle,
                            symbol,
                        )
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler outbound syscall instruction #{selected_instruction_index} does not retain its exact data/storage relocation set"
        )));
    }
    Ok(())
}

pub(super) fn validate_compiler_runtime_text_relocations(
    architecture: Architecture,
    object: &omega_object_file::ObjectPlan,
    relocations: &RelocationPlan,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    buffer_symbol: &str,
    storage_sites: &[(usize, omega_target_operations::RuntimeStorageRegion)],
) -> Result<(), Diagnostic> {
    #[derive(Clone, Copy)]
    enum ExpectedTarget<'symbol> {
        Buffer(&'symbol str),
        Storage(omega_target_operations::RuntimeStorageRegion),
    }

    let mut actual = relocations
        .records()
        .filter_map(|(_, relocation)| {
            (relocation.section == SectionKind::Text
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index))
            .then_some(relocation)
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(|relocation| relocation.offset);

    let mut sites = vec![(0usize, ExpectedTarget::Buffer(buffer_symbol))];
    for (site, region) in storage_sites {
        sites.push((*site, ExpectedTarget::Storage(*region)));
    }
    let mut expected = Vec::new();
    for (site, target) in sites {
        match architecture {
            Architecture::X86_64 => expected.push((
                instruction_byte_offset + site + 2,
                RelocationKind::Absolute64,
                8usize,
                target,
            )),
            Architecture::Aarch64 => {
                expected.push((
                    instruction_byte_offset + site,
                    RelocationKind::Aarch64Page21,
                    4usize,
                    target,
                ));
                expected.push((
                    instruction_byte_offset + site + 4,
                    RelocationKind::Aarch64PageOffset12,
                    4usize,
                    target,
                ));
            }
        }
    }
    expected.sort_unstable_by_key(|(offset, _, _, _)| *offset);

    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(&expected)
            .all(|(relocation, (offset, kind, width, target))| {
                let target_matches = match target {
                    ExpectedTarget::Buffer(symbol) => compiler_data_object_symbol_matches(
                        object,
                        relocation.symbol_handle,
                        symbol,
                    ),
                    ExpectedTarget::Storage(region) => {
                        compiler_storage_symbol_matches(object, relocation.symbol_handle, *region)
                    }
                };
                relocation.offset == *offset
                    && relocation.kind == *kind
                    && relocation.byte_width == *width
                    && relocation.addend == 0
                    && target_matches
            });
    if !matches {
        return Err(Diagnostic::error(format!(
            "compiler runtime-text instruction #{selected_instruction_index} does not retain its exact buffer/storage relocation set"
        )));
    }
    Ok(())
}

fn compiler_data_object_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    expected_symbol: &str,
) -> bool {
    object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Object
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::Section(SectionKind::Data)
        && object.layout.symbols.get(symbol_handle).name == expected_symbol
        && object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.name == expected_symbol)
            .count()
            == 1
}

fn compiler_import_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    expected_library: &str,
    expected_symbol: &str,
) -> bool {
    object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Import
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::None
        && object.layout.symbols.get(symbol_handle).name == expected_symbol
        && object.layout.symbols.get(symbol_handle).import_library == expected_library
}

fn compiler_import_locator_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    locator: &omega_calling_conventions::HostImportLocator,
) -> bool {
    match locator {
        omega_calling_conventions::HostImportLocator::StringBackedBootstrap { library, symbol } => {
            compiler_import_symbol_matches(object, symbol_handle, library, symbol)
        }
        omega_calling_conventions::HostImportLocator::Normalized(locator) => {
            omega_object_file::object_symbol_handle_by_foreign_locator(object, locator)
                == symbol_handle
        }
    }
}

pub(super) fn compiler_storage_symbol_matches(
    object: &omega_object_file::ObjectPlan,
    symbol_handle: omega_object_file::ObjectSymbolHandle,
    storage_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    let symbol_name = omega_object_file::object_symbol_name(object, symbol_handle);
    let symbol_is_storage_object = object.layout.symbols.is_valid(symbol_handle)
        && object.layout.symbols.get(symbol_handle).kind == omega_object_file::SymbolKind::Object
        && object.layout.symbols.get(symbol_handle).section
            == omega_object_file::SymbolSection::Section(SectionKind::Bss);
    let expected_symbol = match storage_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            symbol_name == omega_object_file::runtime_frame_storage_symbol_name()
                && object
                    .layout
                    .symbols
                    .iter()
                    .filter(|(_, symbol)| {
                        symbol.name == omega_object_file::runtime_frame_storage_symbol_name()
                    })
                    .count()
                    == 1
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            symbol_name.starts_with("omega_machine_")
                && symbol_name.ends_with("_storage")
                && object
                    .layout
                    .symbols
                    .iter()
                    .filter(|(_, symbol)| {
                        symbol.name.starts_with("omega_machine_")
                            && symbol.name.ends_with("_storage")
                            && symbol.kind == omega_object_file::SymbolKind::Object
                            && symbol.section
                                == omega_object_file::SymbolSection::Section(SectionKind::Bss)
                    })
                    .count()
                    == 1
        }
    };
    symbol_is_storage_object && expected_symbol
}

pub(super) fn compiler_instruction_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let mutable_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (site + 2..site + 10).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !mutable_mask == 0
        })
}

pub(super) fn compiler_instruction_import_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    call_site: usize,
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let call_mask = match architecture {
                Architecture::X86_64 if (call_site..call_site + 4).contains(&offset) => 0xff,
                Architecture::Aarch64 if (call_site..call_site + 4).contains(&offset) => {
                    [0xff, 0xff, 0xff, 0x03][offset - call_site]
                }
                _ => 0,
            };
            let address_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (*site..site + 8).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !(call_mask | address_mask) == 0
        })
}

pub(super) fn compiler_instruction_composite_non_relocation_bits_match(
    architecture: Architecture,
    expected: &[u8],
    final_bytes: &[u8],
    call_sites: &[usize],
    address_sites: &[usize],
) -> bool {
    if expected.len() != final_bytes.len() {
        return false;
    }
    expected
        .iter()
        .zip(final_bytes)
        .enumerate()
        .all(|(offset, (expected, final_byte))| {
            let call_mask = call_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (*site..site + 4).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xff, 0xff, 0xff, 0x03][offset - site]
                    }
                    _ => 0,
                }
            });
            let address_mask = address_sites.iter().fold(0u8, |mask, site| {
                mask | match architecture {
                    Architecture::X86_64 if (site + 2..site + 10).contains(&offset) => 0xff,
                    Architecture::Aarch64 if (*site..site + 4).contains(&offset) => {
                        [0xe0, 0xff, 0xff, 0x60][offset - site]
                    }
                    Architecture::Aarch64 if (site + 4..site + 8).contains(&offset) => {
                        [0x00, 0xfc, 0x3f, 0x00][offset - site - 4]
                    }
                    _ => 0,
                }
            });
            (expected ^ final_byte) & !(call_mask | address_mask) == 0
        })
}

#[cfg(test)]
mod internal_call_tests {
    use super::*;
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_object_file::{
        FunctionSymbolPlan, NormalizedImportPlan, ObjectPlan, RelocationOrigin, RelocationRecord,
        SymbolKind, SymbolPlan, SymbolSection,
    };
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
    };
    use psi_symbols::SymbolHandle;

    fn identity(state: u32) -> MachineFunctionIdentity {
        MachineFunctionIdentity::source(StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        })
    }

    fn exact_call_fixture(
        target: NativeTarget,
        callee: MachineFunctionIdentity,
        instruction_offset: usize,
    ) -> (ObjectPlan, RelocationPlan) {
        let mut object = ObjectPlan::with_capacities(target, 0, 1, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "__omega_exact_callee".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 32,
            size: 4,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: callee,
            symbol,
        });
        let (call_site, kind) = match target.architecture {
            Architecture::X86_64 => (1, RelocationKind::X86_64Relative32),
            Architecture::Aarch64 => (0, RelocationKind::Aarch64Branch26),
        };
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: symbol,
                selected_instruction_index: 9,
            },
            section: SectionKind::Text,
            offset: instruction_offset + call_site,
            byte_width: 4,
            symbol_handle: symbol,
            addend: 0,
            kind,
        });
        (object, relocations)
    }

    #[test]
    fn final_internal_calls_require_one_exact_identity_relocation_on_each_architecture() {
        let callee = identity(2);
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let (mut object, mut relocations) = exact_call_fixture(target, callee, 12);
            let call_site = usize::from(target.architecture == Architecture::X86_64);
            validate_compiler_internal_call_relocation(
                target.architecture,
                &object,
                &relocations,
                9,
                12,
                call_site,
                callee,
            )
            .expect("exact target-identity relocation");

            let wrong_target = validate_compiler_internal_call_relocation(
                target.architecture,
                &object,
                &relocations,
                9,
                12,
                call_site,
                identity(3),
            )
            .expect_err("a nearby identity must not satisfy final validation");
            assert!(wrong_target.message.contains("exact target-identity"));

            let (_, duplicate) = relocations.records().next().expect("call relocation");
            relocations.push_record(duplicate.clone());
            let duplicate = validate_compiler_internal_call_relocation(
                target.architecture,
                &object,
                &relocations,
                9,
                12,
                call_site,
                callee,
            )
            .expect_err("duplicate call relocations must reject");
            assert!(duplicate.message.contains("exact target-identity"));

            object.layout.function_symbols.insert(FunctionSymbolPlan {
                identity: callee,
                symbol: object.layout.entry_symbol,
            });
            let duplicate_binding = validate_compiler_internal_call_relocation(
                target.architecture,
                &object,
                &RelocationPlan::with_target(target),
                9,
                12,
                call_site,
                callee,
            )
            .expect_err("duplicate identity bindings must reject");
            assert!(duplicate_binding.message.contains("exact target-identity"));
        }
    }

    #[test]
    fn final_internal_call_replay_rejects_non_relocation_opcode_tampering() {
        assert!(compiler_instruction_import_non_relocation_bits_match(
            Architecture::X86_64,
            &[0xe8, 0, 0, 0, 0],
            &[0xe8, 1, 2, 3, 4],
            1,
            &[],
        ));
        assert!(!compiler_instruction_import_non_relocation_bits_match(
            Architecture::X86_64,
            &[0xe8, 0, 0, 0, 0],
            &[0xe9, 1, 2, 3, 4],
            1,
            &[],
        ));
        assert!(compiler_instruction_import_non_relocation_bits_match(
            Architecture::Aarch64,
            &[0, 0, 0, 0x94],
            &[1, 2, 3, 0x97],
            0,
            &[],
        ));
        assert!(!compiler_instruction_import_non_relocation_bits_match(
            Architecture::Aarch64,
            &[0, 0, 0, 0x94],
            &[1, 2, 3, 0x93],
            0,
            &[],
        ));
    }

    #[test]
    fn planned_import_replay_joins_the_exact_normalized_locator_atomically() {
        let target = NativeTarget::windows_x64();
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"raw\xff.dll".to_vec(),
                ordinal: 17,
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid PE locator");
        let mutated = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"raw\xff.dll".to_vec(),
                ordinal: 18,
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid mutated PE locator");
        let mut object = ObjectPlan::with_capacity(target, 0, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "diagnostic-only".into(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
            import_library: String::new(),
        });
        object.layout.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.clone(),
        });
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: symbol,
                selected_instruction_index: 7,
            },
            section: SectionKind::Text,
            offset: 13,
            byte_width: 4,
            symbol_handle: symbol,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });

        validate_compiler_planned_import_relocations(
            Architecture::X86_64,
            &object,
            &relocations,
            7,
            12,
            1,
            &[],
            &omega_calling_conventions::HostImportLocator::Normalized(locator.clone()),
        )
        .expect("exact normalized relocation join");
        let error = validate_compiler_planned_import_relocations(
            Architecture::X86_64,
            &object,
            &relocations,
            7,
            12,
            1,
            &[],
            &omega_calling_conventions::HostImportLocator::Normalized(mutated),
        )
        .expect_err("ordinal mutation must reject");
        assert!(
            error
                .message
                .contains("exact call/data/storage relocation set")
        );

        object.layout.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.clone(),
        });
        assert!(
            validate_compiler_planned_import_relocations(
                Architecture::X86_64,
                &object,
                &relocations,
                7,
                12,
                1,
                &[],
                &omega_calling_conventions::HostImportLocator::Normalized(locator),
            )
            .is_err(),
            "ambiguous object locator rows must fail closed"
        );
    }
}
