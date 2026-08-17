//! Place guard, copy, write, conversion, and relocation replay regressions.

use super::*;

#[test]
fn place_guard_replay_uses_materializer_relocation_sites() {
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, StateGuardOperator};

    let target = NativeTarget::linux_x64();
    let mut object = ObjectPlan::with_capacity(target, 0, 2);
    let machine_symbol = object.layout.symbols.insert(SymbolPlan {
        name: "omega_machine_Main_storage".to_owned(),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: 64,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    let frame_symbol = object.layout.symbols.insert(SymbolPlan {
        name: omega_object_file::runtime_frame_storage_symbol_name(),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 64,
        size: 64,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    let mut place = Place::at(RuntimeStorageRegion::Machine, 16);
    assert!(place.push_step(PlaceStep::ScaledIndex {
        index_region: RuntimeStorageRegion::RuntimeFrame,
        index_offset: 8,
        index_byte_size: 4,
        element_byte_size: 4,
    }));
    let kind = CompilerInstructionValidationKind::PlaceValueGuard {
        place,
        byte_size: 4,
        expected_value: 7,
        failure_branch_distance: 12,
        operator: StateGuardOperator::Equal,
    };
    let sites = compiler_place_value_address_sites(omega_target::Architecture::X86_64, place, kind)
        .expect("place materializer sites");
    assert!(sites.len() >= 2);
    let mut relocations = RelocationPlan::with_target(target);
    for (site, region) in &sites {
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 19,
            },
            section: SectionKind::Text,
            offset: site + 2,
            byte_width: 8,
            symbol_handle: match region {
                RuntimeStorageRegion::Machine => machine_symbol,
                RuntimeStorageRegion::RuntimeFrame => frame_symbol,
            },
            addend: 0,
            kind: RelocationKind::Absolute64,
        });
    }
    validate_compiler_data_address_relocations(
        omega_target::Architecture::X86_64,
        &object,
        &relocations,
        19,
        0,
        &sites,
    )
    .expect("every materializer site should retain its place region");

    let (expected, _) =
        omega_isa_x86_64::encode_place_value_compare(&place, 4, 7, 12, StateGuardOperator::Equal)
            .expect("place guard bytes");
    let mut final_bytes = expected.clone();
    for (index, (site, _)) in sites.iter().enumerate() {
        final_bytes[site + 2..site + 10]
            .copy_from_slice(&(0x1000u64 + index as u64 * 0x100).to_le_bytes());
    }
    let site_offsets = sites.iter().map(|(offset, _)| *offset).collect::<Vec<_>>();
    assert!(compiler_instruction_non_relocation_bits_match(
        omega_target::Architecture::X86_64,
        &expected,
        &final_bytes,
        &site_offsets,
    ));
    final_bytes[0] ^= 0xff;
    assert!(!compiler_instruction_non_relocation_bits_match(
        omega_target::Architecture::X86_64,
        &expected,
        &final_bytes,
        &site_offsets,
    ));

    let missing = RelocationPlan::with_target(target);
    let diagnostic = validate_compiler_data_address_relocations(
        omega_target::Architecture::X86_64,
        &object,
        &missing,
        19,
        0,
        &sites,
    )
    .expect_err("missing place-derived relocations must reject");
    assert!(diagnostic.message.contains("operand-derived"));
}

#[test]
fn general_x86_place_copy_replay_uses_the_materializer_and_its_sites() {
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

    let direct_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 80);
    let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
    assert!(matches!(
        compiler_body_place_copy_shape(&direct_source, &target)
            .expect("classify closed frame-double write"),
        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. }
    ));
    let source = direct_source
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 88,
            index_byte_size: 8,
            element_byte_size: 8,
        })
        .expect("indexed source keeps the pair in the general class");
    assert!(matches!(
        compiler_body_place_copy_shape(&source, &target).expect("classify final place copy"),
        CompilerBodyPlaceCopyShape::General
    ));

    let (bytes, encoded_sites) =
        omega_isa_x86_64::encode_copy_places(&source, &target, 8).expect("general x86 place copy");
    assert!(!bytes.is_empty());
    let replay_sites =
        compiler_place_copy_address_sites(omega_target::Architecture::X86_64, source, target, 8)
            .expect("general x86 final relocation sites");
    let expected_sites = encoded_sites
        .iter()
        .map(|(offset, side)| {
            let region = match side {
                omega_isa_x86_64::PlaceCopySide::Source => source.region,
                omega_isa_x86_64::PlaceCopySide::Target => target.region,
                omega_isa_x86_64::PlaceCopySide::SourceIndex
                | omega_isa_x86_64::PlaceCopySide::SourceIndex2 => {
                    source.scaled_index_region().unwrap_or(source.region)
                }
                omega_isa_x86_64::PlaceCopySide::TargetIndex => {
                    target.scaled_index_region().expect("first target index")
                }
                omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                    .scaled_index_regions()
                    .nth(1)
                    .expect("second target index"),
            };
            (offset, region)
        })
        .collect::<Vec<_>>();
    assert_eq!(replay_sites, expected_sites);
}

#[test]
fn frame_double_indexed_to_pointee_replay_uses_one_frame_root() {
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

    let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 104,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 112,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
        .expect("all-frame double-indexed source");
    let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 120)
        .with_step(PlaceStep::Deref)
        .and_then(|place| place.with_step(PlaceStep::ConstOffset(8)))
        .expect("frame-held pointee target");

    assert!(matches!(
        compiler_body_place_copy_shape(&source, &target)
            .expect("classify final double-indexed pointee copy"),
        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
            base_byte_offset: 32,
            outer_index_offset: 104,
            inner_index_offset: 112,
            source_field_byte_offset: 4,
            pointer_byte_offset: 120,
            target_field_byte_offset: 8,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(omega_target::Architecture::Aarch64, source, target, 12,)
            .expect("final relocation sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );

    assert!(matches!(
        compiler_body_place_copy_shape(&target, &source)
            .expect("classify final reverse pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
            pointer_byte_offset: 120,
            source_field_byte_offset: 8,
            base_byte_offset: 32,
            outer_index_offset: 104,
            inner_index_offset: 112,
            target_field_byte_offset: 4,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(omega_target::Architecture::Aarch64, target, source, 12,)
            .expect("final reverse relocation sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );

    let cross_frame_double_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 152,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 160,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index frame double source");
    assert!(matches!(
        compiler_body_place_copy_shape(&cross_frame_double_source, &target)
            .expect("classify final mixed-index frame double pointee copy"),
        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
            outer_index_region: RuntimeStorageRegion::Machine,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            cross_frame_double_source.clone(),
            target.clone(),
            12,
        )
        .expect("final mixed-index frame double pointee sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&target, &cross_frame_double_source)
            .expect("classify final reverse mixed-index frame double pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
            outer_index_region: RuntimeStorageRegion::Machine,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            target.clone(),
            cross_frame_double_source,
            12,
        )
        .expect("final reverse mixed-index frame double pointee sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );

    let cross_frame_double_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 152,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 160,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index frame double direct source");
    let direct = Place::at(RuntimeStorageRegion::RuntimeFrame, 176);
    assert!(matches!(
        compiler_body_place_copy_shape(&cross_frame_double_source, &direct)
            .expect("classify final mixed-index frame double direct read"),
        CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
            outer_index_region: RuntimeStorageRegion::Machine,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            cross_frame_double_source.clone(),
            direct.clone(),
            12,
        )
        .expect("final mixed-index frame double direct-read sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (8, RuntimeStorageRegion::Machine),
            (52, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&direct, &cross_frame_double_source)
            .expect("classify final mixed-index frame double direct write"),
        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
            outer_index_region: RuntimeStorageRegion::Machine,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            direct,
            cross_frame_double_source,
            12,
        )
        .expect("final mixed-index frame double direct-write sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );

    let frame_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 200)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 144,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("all-frame indexed source");
    assert!(matches!(
        compiler_body_place_copy_shape(&frame_indexed_source, &target)
            .expect("classify final all-frame indexed pointee copy"),
        CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee {
            base_byte_offset: 200,
            index_offset: 144,
            pointer_byte_offset: 120,
            target_field_byte_offset: 8,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            frame_indexed_source.clone(),
            target.clone(),
            12,
        )
        .expect("final all-frame indexed pointee sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&target, &frame_indexed_source)
            .expect("classify final reverse all-frame indexed pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed {
            pointer_byte_offset: 120,
            source_field_byte_offset: 8,
            base_byte_offset: 200,
            index_offset: 144,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            target.clone(),
            frame_indexed_source,
            12,
        )
        .expect("final reverse all-frame indexed pointee sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );

    let cross_frame_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 208)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 152,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("machine-indexed frame source");
    assert!(matches!(
        compiler_body_place_copy_shape(&cross_frame_indexed_source, &target)
            .expect("classify final cross-region frame indexed pointee copy"),
        CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee {
            index_region: RuntimeStorageRegion::Machine,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            cross_frame_indexed_source.clone(),
            target.clone(),
            12,
        )
        .expect("cross-region frame indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&target, &cross_frame_indexed_source)
            .expect("classify final reverse cross-region frame indexed pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed {
            index_region: RuntimeStorageRegion::Machine,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            target.clone(),
            cross_frame_indexed_source,
            12,
        )
        .expect("reverse cross-region frame indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );

    let machine_source = Place::at(RuntimeStorageRegion::Machine, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 104,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 112,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index machine double source");
    assert!(matches!(
        compiler_body_place_copy_shape(&machine_source, &target)
            .expect("classify final machine double-indexed pointee copy"),
        CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee {
            base_byte_offset: 32,
            outer_index_region: RuntimeStorageRegion::Machine,
            outer_index_offset: 104,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            inner_index_offset: 112,
            source_field_byte_offset: 0,
            pointer_byte_offset: 120,
            target_field_byte_offset: 8,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            machine_source.clone(),
            target.clone(),
            12,
        )
        .expect("final machine double-indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&target, &machine_source)
            .expect("classify final reverse machine double-indexed pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed {
            pointer_byte_offset: 120,
            source_field_byte_offset: 8,
            base_byte_offset: 32,
            outer_index_region: RuntimeStorageRegion::Machine,
            outer_index_offset: 104,
            inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            inner_index_offset: 112,
            target_field_byte_offset: 0,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            target.clone(),
            machine_source.clone(),
            12,
        )
        .expect("final reverse machine double-indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
    let machine_indexed_source = Place::at(RuntimeStorageRegion::Machine, 200)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 144,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("frame-indexed machine source");
    assert!(matches!(
        compiler_body_place_copy_shape(&machine_indexed_source, &target)
            .expect("classify final machine indexed pointee copy"),
        CompilerBodyPlaceCopyShape::MachineIndexedToPointee {
            base_byte_offset: 200,
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 144,
            pointer_byte_offset: 120,
            target_field_byte_offset: 8,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            machine_indexed_source.clone(),
            target.clone(),
            12,
        )
        .expect("final machine indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
    assert!(matches!(
        compiler_body_place_copy_shape(&target, &machine_indexed_source)
            .expect("classify final reverse machine indexed pointee copy"),
        CompilerBodyPlaceCopyShape::PointeeToMachineIndexed {
            pointer_byte_offset: 120,
            source_field_byte_offset: 8,
            base_byte_offset: 200,
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 144,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            target.clone(),
            machine_indexed_source,
            12,
        )
        .expect("final reverse machine indexed pointee sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
    let machine_target = Place::at(RuntimeStorageRegion::Machine, 160)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 128,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 136,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index machine double target");
    assert!(matches!(
        compiler_body_place_copy_shape(&machine_source, &machine_target)
            .expect("classify final machine double-indexed pair"),
        CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair {
            source_outer_index_region: RuntimeStorageRegion::Machine,
            source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_inner_index_region: RuntimeStorageRegion::Machine,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            machine_source,
            machine_target,
            12,
        )
        .expect("final mixed-index machine double-pair sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
            (56, RuntimeStorageRegion::Machine),
            (64, RuntimeStorageRegion::RuntimeFrame),
        ]
    );

    let double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 128,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 136,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
        .expect("all-frame double-indexed target");
    assert!(matches!(
        compiler_body_place_copy_shape(&source, &double_target)
            .expect("classify final all-frame double-indexed pair"),
        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
            source_base_byte_offset: 32,
            source_outer_index_offset: 104,
            source_inner_index_offset: 112,
            target_base_byte_offset: 160,
            target_outer_index_offset: 128,
            target_inner_index_offset: 136,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            source,
            double_target,
            12,
        )
        .expect("final all-frame double-indexed-pair sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );

    let mixed_frame_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 144,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 152,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index frame double pair source");
    let mixed_frame_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 192)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 160,
            index_byte_size: 8,
            element_byte_size: 36,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 168,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("mixed-index frame double pair target");
    assert!(matches!(
        compiler_body_place_copy_shape(&mixed_frame_source, &mixed_frame_target)
            .expect("classify final mixed-index frame double pair"),
        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
            source_outer_index_region: RuntimeStorageRegion::Machine,
            source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_inner_index_region: RuntimeStorageRegion::Machine,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            mixed_frame_source,
            mixed_frame_target,
            12,
        )
        .expect("final mixed-index frame double-pair sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );

    let indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 104,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("all-frame indexed source");
    let indexed_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 112,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("all-frame indexed target");
    assert!(matches!(
        compiler_body_place_copy_shape(&indexed_source, &indexed_target)
            .expect("classify final all-frame indexed pair"),
        CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
            source_base_byte_offset: 32,
            source_index_region: RuntimeStorageRegion::RuntimeFrame,
            source_index_offset: 104,
            target_base_byte_offset: 160,
            target_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_index_offset: 112,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            indexed_source,
            indexed_target,
            12,
        )
        .expect("final all-frame indexed-pair sites"),
        vec![(0, RuntimeStorageRegion::RuntimeFrame)]
    );

    let mixed_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 104,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("mixed-index frame source");
    assert!(matches!(
        compiler_body_place_copy_shape(&mixed_indexed_source, &indexed_target)
            .expect("classify final mixed-index frame pair"),
        CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
            source_index_region: RuntimeStorageRegion::Machine,
            target_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            mixed_indexed_source,
            indexed_target,
            12,
        )
        .expect("final mixed-index frame-pair sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (12, RuntimeStorageRegion::Machine),
        ]
    );

    let cross_region_indexed_source = Place::at(RuntimeStorageRegion::Machine, 200)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 120,
            index_byte_size: 8,
            element_byte_size: 12,
        })
        .expect("cross-region indexed source");
    assert!(matches!(
        compiler_body_place_copy_shape(&cross_region_indexed_source, &mixed_indexed_source)
            .expect("classify final cross-region indexed pair"),
        CompilerBodyPlaceCopyShape::CrossRegionIndexedPair {
            source_index_region: RuntimeStorageRegion::RuntimeFrame,
            target_index_region: RuntimeStorageRegion::Machine,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            cross_region_indexed_source,
            mixed_indexed_source,
            12,
        )
        .expect("final cross-region indexed-pair sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );

    let cross_region_double_source = Place::at(RuntimeStorageRegion::Machine, 200)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 120,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 128,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("cross-region double-indexed source");
    let cross_region_double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 240)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 136,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 12,
            })
        })
        .expect("cross-region double-indexed target");
    assert!(matches!(
        compiler_body_place_copy_shape(&cross_region_double_source, &cross_region_double_target,)
            .expect("classify final cross-region double-indexed pair"),
        CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair {
            source_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
            source_inner_index_region: RuntimeStorageRegion::Machine,
            target_outer_index_region: RuntimeStorageRegion::Machine,
            target_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
            ..
        }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(
            omega_target::Architecture::Aarch64,
            cross_region_double_source,
            cross_region_double_target,
            12,
        )
        .expect("final cross-region double-indexed-pair sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
        ]
    );
}

#[test]
fn pointee_double_indexed_replay_uses_frame_root_and_one_shared_machine_site() {
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

    let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
        .with_step(PlaceStep::Deref)
        .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 24,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 32,
                index_byte_size: 8,
                element_byte_size: 2,
            })
        })
        .expect("pointee double-indexed source");
    let target = Place::at(RuntimeStorageRegion::Machine, 40);

    assert!(matches!(
        compiler_body_place_copy_shape(&source, &target)
            .expect("classify final pointee double-indexed copy"),
        CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed { .. }
    ));
    assert_eq!(
        compiler_place_copy_address_sites(omega_target::Architecture::Aarch64, source, target, 2,)
            .expect("pointee double-indexed copy sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (32, RuntimeStorageRegion::Machine),
        ]
    );
    assert_eq!(
        compiler_place_integer_write_address_sites(
            omega_target::Architecture::Aarch64,
            source,
            omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
                target: source,
                value: 17,
                byte_size: 2,
            },
        )
        .expect("pointee double-indexed integer-write sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (32, RuntimeStorageRegion::Machine),
        ]
    );
}

#[test]
fn general_x86_integer_write_replay_uses_the_materializer_and_its_sites() {
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion};

    let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .expect("cross-region inline frame target");
    assert!(matches!(
        compiler_body_place_integer_write_shape(&target).expect("classify final integer write"),
        CompilerBodyPlaceIntegerWriteShape::General
    ));

    let value = 7;
    let byte_size = 4;
    let (bytes, encoded_sites) =
        omega_isa_x86_64::encode_place_integer_write(&target, value, byte_size)
            .expect("general x86 integer write");
    assert!(!bytes.is_empty());
    let replay_sites = compiler_place_integer_write_address_sites(
        omega_target::Architecture::X86_64,
        target,
        omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite {
            target,
            value,
            byte_size,
        },
    )
    .expect("general x86 integer-write final relocation sites");
    let expected_sites = encoded_sites
        .iter()
        .map(|(offset, side)| {
            let region = match side {
                omega_isa_x86_64::PlaceCopySide::Target => target.region,
                omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                    .scaled_index_region()
                    .expect("general target index region"),
                omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                    .scaled_index_regions()
                    .nth(1)
                    .expect("general second target index region"),
                _ => panic!("integer-write materializer emitted a non-target site"),
            };
            (offset, region)
        })
        .collect::<Vec<_>>();
    assert_eq!(replay_sites, expected_sites);
}

#[test]
fn general_x86_binary_write_replay_uses_the_materializer_and_its_sites() {
    use omega_target_operations::{
        Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand, StateGuardOperator,
    };

    let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
    assert!(matches!(
        compiler_body_place_integer_write_shape(&target).expect("classify final binary write"),
        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
    ));

    let mut operands = psi_arena::Arena::new();
    let left = operands.insert(RuntimeValueOperand::Immediate(2));
    let right = operands.insert(RuntimeValueOperand::Immediate(3));
    let (bytes, encoded_sites) = omega_isa_x86_64::encode_place_binary_write(
        &operands,
        &target,
        4,
        left,
        StateGuardOperator::Add,
        right,
        false,
        psi_numerics::arithmetic::ArithmeticDomain::Exact,
        true,
    )
    .expect("general x86 binary write");
    assert!(!bytes.is_empty());

    let replay_sites = compiler_place_binary_write_address_sites(
        omega_target::Architecture::X86_64,
        &operands,
        target,
        left,
        right,
    )
    .expect("general x86 binary-write final relocation sites");
    let expected_sites = encoded_sites
        .iter()
        .map(|(offset, side)| {
            let region = match side {
                omega_isa_x86_64::PlaceCopySide::Target => target.region,
                omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                    .scaled_index_region()
                    .expect("general target index region"),
                omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                    .scaled_index_regions()
                    .nth(1)
                    .expect("general second target index region"),
                _ => panic!("binary-write materializer emitted a non-target site"),
            };
            (offset, region)
        })
        .collect::<Vec<_>>();
    assert_eq!(replay_sites, expected_sites);
}

#[test]
fn aarch64_composed_place_convert_relocation_sites_follow_each_address_recipe() {
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand};

    let mut operands = psi_arena::Arena::new();
    let source = operands.insert(RuntimeValueOperand::Storage {
        region: RuntimeStorageRegion::Machine,
        byte_offset: 96,
        byte_size: 4,
    });

    let direct = Place::at(RuntimeStorageRegion::Machine, 16);
    assert_eq!(
        compiler_place_convert_write_address_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            direct,
            source,
        )
        .expect("direct conversion sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::Machine)
        ]
    );

    let frame_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
        .with_step(PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 16,
            })
        })
        .expect("frame-indexed place");
    let frame_indexed_operand_start = omega_isa_aarch64::runtime_frame_indexed_operand_start_width(
        RuntimeStorageRegion::Machine,
        16,
        0,
    );
    assert_eq!(
        compiler_place_convert_write_address_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            frame_indexed,
            source,
        )
        .expect("frame-indexed conversion sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (32, RuntimeStorageRegion::Machine),
            (frame_indexed_operand_start, RuntimeStorageRegion::Machine),
        ]
    );

    let frame_base_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 56,
            index_byte_size: 8,
            element_byte_size: 16,
        })
        .expect("frame-base-indexed place");
    let frame_base_operand_start =
        omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width(48, 56, 8, 16, 0);
    assert_eq!(
        compiler_place_convert_write_address_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            frame_base_indexed,
            source,
        )
        .expect("frame-base-indexed conversion sites"),
        vec![
            (0, RuntimeStorageRegion::RuntimeFrame),
            (frame_base_operand_start, RuntimeStorageRegion::Machine),
        ]
    );

    let machine_double_indexed = Place::at(RuntimeStorageRegion::Machine, 64)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: 72,
            index_byte_size: 8,
            element_byte_size: 16,
        })
        .and_then(|place| {
            place.with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 80,
                index_byte_size: 8,
                element_byte_size: 4,
            })
        })
        .expect("machine-double-indexed place");
    let machine_double_operand_start =
        omega_isa_aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
            RuntimeStorageRegion::RuntimeFrame,
            RuntimeStorageRegion::Machine,
        );
    assert_eq!(
        compiler_place_convert_write_address_sites(
            omega_target::Architecture::Aarch64,
            &operands,
            machine_double_indexed,
            source,
        )
        .expect("machine-double-indexed conversion sites"),
        vec![
            (0, RuntimeStorageRegion::Machine),
            (8, RuntimeStorageRegion::RuntimeFrame),
            (machine_double_operand_start, RuntimeStorageRegion::Machine),
        ]
    );
}
