use super::*;
use selected_instructions::{MachineSemanticKind, MachineSizeKnowledge};

fn case(
    load: bool,
    width: PackedByteWidth,
    byte_offset: u32,
) -> (
    SelectedInstructionKind,
    MachineAlternativeKey,
    MachineSemanticKind,
) {
    let semantic = if load {
        match width {
            PackedByteWidth::Three => MachineSemanticKind::LoadPacked3,
            PackedByteWidth::Five => MachineSemanticKind::LoadPacked5,
            PackedByteWidth::Six => MachineSemanticKind::LoadPacked6,
            PackedByteWidth::Seven => MachineSemanticKind::LoadPacked7,
        }
    } else {
        MachineSemanticKind::StorePacked
    };
    (
        if load {
            SelectedInstructionKind::LoadPacked { byte_offset, width }
        } else {
            SelectedInstructionKind::StorePacked { byte_offset, width }
        },
        MachineAlternativeKey {
            family: semantic.into(),
            variant: 0,
        },
        semantic,
    )
}

#[test]
fn packed_forms_access_exact_bytes_and_declare_all_scratch_writes() {
    let physical =
        register_model::validate_physical_register_model(aarch64_physical_register_model())
            .unwrap();
    let constraints = crate::validate_aarch64_register_constraint_catalog(
        crate::aarch64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let operands = ["x3", "x7", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
    for target in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
        for width in [
            PackedByteWidth::Three,
            PackedByteWidth::Five,
            PackedByteWidth::Six,
            PackedByteWidth::Seven,
        ] {
            for load in [true, false] {
                for offset in [0, 4096 - u32::from(width.byte_size())] {
                    let (kind, alternative, semantic) = case(load, width, offset);
                    let encoded = encode_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        offset,
                    )
                    .unwrap();
                    let effects = &encoded.footprint().encoded;
                    let row = catalog
                        .declarations
                        .iter()
                        .find(|row| row.semantic == semantic)
                        .unwrap();
                    assert_eq!(effects, &row.alternatives[0].encoded);
                    assert_eq!(
                        encoded.bytes().len(),
                        (usize::from(width.byte_size()) * 2 - usize::from(load)) * 4
                    );
                    if load {
                        assert_eq!(
                            row.alternatives[0].size,
                            MachineSizeKnowledge::ExactBytes(
                                (u16::from(width.byte_size()) * 2 - 1) * 4
                            )
                        );
                    }
                    assert_eq!(
                        effects.external_operand_writes,
                        if load { vec![1, 2] } else { vec![2] }
                    );
                    assert_eq!(
                        effects.external_operand_reads,
                        if load { vec![0] } else { vec![0, 1] }
                    );
                    assert!(effects.implicit_unit_clobbers.is_empty());
                    assert!(effects.implicit_unit_defs.is_empty());
                    assert!(!encoded.footprint().writes_nzcv);
                    let accesses = encoded
                        .bytes()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|bytes| u32::from_le_bytes(*bytes))
                        .filter(|word| {
                            word & 0xffc0_0000 == if load { 0x3940_0000 } else { 0x3900_0000 }
                        })
                        .map(|word| (word >> 10) & 4095)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        accesses,
                        (offset..offset + u32::from(width.byte_size())).collect::<Vec<_>>()
                    );
                    assert_eq!(
                        validate_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            offset,
                            encoded.bytes()
                        )
                        .unwrap(),
                        encoded
                    );
                }
            }
        }
    }
}

#[test]
fn packed_forms_reject_width_offset_alias_and_instruction_mutations() {
    let physical =
        register_model::validate_physical_register_model(aarch64_physical_register_model())
            .unwrap();
    let operands = ["x3", "x7", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
    for load in [true, false] {
        let (kind, alternative, _) = case(load, PackedByteWidth::Three, 16);
        let encoded =
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 16)
                .unwrap();
        for bit in 0..encoded.bytes().len() * 8 {
            let mut changed = encoded.bytes().to_vec();
            changed[bit / 8] ^= 1 << (bit % 8);
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    16,
                    &changed
                )
                .is_err(),
                "changed bit {bit}"
            );
        }
        for width in [
            PackedByteWidth::Five,
            PackedByteWidth::Six,
            PackedByteWidth::Seven,
        ] {
            let (other_kind, other_alternative, _) = case(load, width, 16);
            assert!(
                validate_aarch64_selected_memory_form(
                    &physical,
                    other_kind,
                    other_alternative,
                    &operands,
                    16,
                    encoded.bytes()
                )
                .is_err()
            );
        }
        for offset in [4094, u32::MAX] {
            let (kind, alternative, _) = case(load, PackedByteWidth::Three, offset);
            assert!(
                encode_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    offset
                )
                .is_err()
            );
        }
        assert!(
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 17)
                .is_err()
        );
        assert!(
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands[..2], 16)
                .is_err()
        );
        for alias in [0, 1] {
            let mut changed = operands;
            changed[2] = changed[alias];
            assert!(
                encode_aarch64_selected_memory_form(&physical, kind, alternative, &changed, 16)
                    .is_err()
            );
        }
        let mut changed = operands;
        changed[1] = changed[0];
        assert_eq!(
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &changed, 16)
                .is_err(),
            load
        );
        let mut changed = operands;
        changed[2] = physical.model().view_named("d0").unwrap().id;
        assert!(
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &changed, 16)
                .is_err()
        );
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                16,
                &encoded.bytes()[..encoded.bytes().len() - 4]
            )
            .is_err()
        );
    }
}

#[test]
fn packed_constraints_require_early_clobbers_and_effects_reject_scratch_omission() {
    let physical =
        register_model::validate_physical_register_model(aarch64_physical_register_model())
            .unwrap();
    let catalog = crate::aarch64_register_constraint_catalog(&physical);
    for (key, load) in [
        (crate::AARCH64_LOAD_PACKED, true),
        (crate::AARCH64_STORE_PACKED, false),
    ] {
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap();
        assert_eq!(row.operands.len(), 3);
        assert_eq!(row.operands[1].early_clobber, load);
        assert!(row.operands[2].early_clobber);
        assert!(row.clobbers.is_empty());
        let mut changed = catalog.clone();
        changed
            .constraints
            .iter_mut()
            .find(|row| row.key == key)
            .unwrap()
            .operands[2]
            .early_clobber = false;
        assert!(crate::validate_aarch64_register_constraint_catalog(changed, &physical).is_err());
    }
    let constraints =
        crate::validate_aarch64_register_constraint_catalog(catalog, &physical).unwrap();
    for target in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut effects = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
        effects
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::StorePacked)
            .unwrap()
            .alternatives[0]
            .encoded
            .external_operand_writes
            .clear();
        assert!(
            crate::validate_aarch64_machine_effect_catalog(target, &constraints, effects).is_err()
        );
    }
}
