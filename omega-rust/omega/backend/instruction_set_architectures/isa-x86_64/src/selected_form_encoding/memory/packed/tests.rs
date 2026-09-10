//! Exact memory coverage and independent register/effect custody for packed forms.
use super::*;
use register_model::{RegisterOperandAccess, validate_physical_register_model};
use selected_instructions::{MachineSemanticKind, PackedByteWidth};

fn kind(load: bool, width: PackedByteWidth, byte_offset: u32) -> SelectedInstructionKind {
    if load {
        SelectedInstructionKind::LoadPacked { byte_offset, width }
    } else {
        SelectedInstructionKind::StorePacked { byte_offset, width }
    }
}

fn alternative(load: bool, width: PackedByteWidth) -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: if load {
            match width {
                PackedByteWidth::Three => MachineAlternativeFamily::LoadPacked3,
                PackedByteWidth::Five => MachineAlternativeFamily::LoadPacked5,
                PackedByteWidth::Six => MachineAlternativeFamily::LoadPacked6,
                PackedByteWidth::Seven => MachineAlternativeFamily::LoadPacked7,
            }
        } else {
            MachineAlternativeFamily::StorePacked
        },
        variant: 0,
    }
}

#[test]
fn packed_memory_replays_exact_bytes_widths_offsets_and_aliases() {
    let model = validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
    for names in [
        ["rdi", "rax", "r10"],
        ["r12", "r9", "r11"],
        ["r13", "r8", "rbx"],
    ] {
        let operands = names.map(|name| model.model().view_named(name).unwrap().id);
        for width in [
            PackedByteWidth::Three,
            PackedByteWidth::Five,
            PackedByteWidth::Six,
            PackedByteWidth::Seven,
        ] {
            for load in [false, true] {
                let kind = kind(load, width, 17);
                let alternative = alternative(load, width);
                let encoded = encode(&model, kind, alternative, &operands, 17).unwrap();
                validate(&model, kind, alternative, &operands, 17, encoded.bytes()).unwrap();
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(
                    encoded.footprint().register_writes,
                    if load {
                        vec![operands[1], operands[2]]
                    } else {
                        vec![operands[2]]
                    }
                );
                if load {
                    assert_eq!(
                        encoded.footprint().encoded.memory,
                        MachineEncodedMemoryEffect::ReadPointerV1 {
                            pointer_operand: 0,
                            byte_count: u16::from(width.byte_size()),
                        }
                    );
                }
                for position in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[position] ^= 1;
                    assert!(
                        validate(&model, kind, alternative, &operands, 17, &changed).is_err(),
                        "byte {position}"
                    );
                }
                assert!(
                    validate(&model, kind, alternative, &operands, 18, encoded.bytes()).is_err()
                );
                assert!(
                    validate(
                        &model,
                        kind,
                        alternative,
                        &operands,
                        17,
                        &encoded.bytes()[..encoded.bytes().len() - 1]
                    )
                    .is_err()
                );
                for pair in [(0, 2), (1, 2), (0, 1)] {
                    let mut changed = operands;
                    changed[pair.1] = changed[pair.0];
                    assert_eq!(
                        encode(&model, kind, alternative, &changed, 17).is_err(),
                        load || pair != (0, 1)
                    );
                }
            }
        }
    }
    let operands = ["rdi", "rax", "r10"].map(|name| model.model().view_named(name).unwrap().id);
    let width = PackedByteWidth::Three;
    let load = encode(
        &model,
        kind(true, width, 0),
        alternative(true, width),
        &operands,
        0,
    )
    .unwrap();
    assert_eq!(
        load.bytes(),
        &[
            0x48, 0x0f, 0xb6, 0x87, 0, 0, 0, 0, 0x4c, 0x0f, 0xb6, 0x97, 1, 0, 0, 0, 0x49, 0xc1,
            0xe2, 8, 0x4c, 0x09, 0xd0, 0x4c, 0x0f, 0xb6, 0x97, 2, 0, 0, 0, 0x49, 0xc1, 0xe2, 16,
            0x4c, 0x09, 0xd0,
        ]
    );
    for load in [false, true] {
        let last = i32::MAX as u32 - 2;
        assert!(
            encode(
                &model,
                kind(load, width, last),
                alternative(load, width),
                &operands,
                last
            )
            .is_ok()
        );
        for offset in [last + 1, u32::MAX] {
            assert!(
                encode(
                    &model,
                    kind(load, width, offset),
                    alternative(load, width),
                    &operands,
                    offset
                )
                .is_err()
            );
        }
        let three = encode(
            &model,
            kind(load, width, 0),
            alternative(load, width),
            &operands,
            0,
        )
        .unwrap();
        assert!(
            validate(
                &model,
                kind(load, PackedByteWidth::Five, 0),
                alternative(load, PackedByteWidth::Five),
                &operands,
                0,
                three.bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn packed_memory_constraints_and_effects_retain_early_writes_and_flags() {
    let model = validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
    let catalog = crate::x86_64_register_constraint_catalog(&model);
    let constraints =
        crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &model).unwrap();
    let effects =
        crate::x86_64_machine_effect_catalog(target::NativeTarget::linux_x64(), &constraints)
            .unwrap();
    for (key, load) in [
        (crate::X86_64_LOAD_PACKED, true),
        (crate::X86_64_STORE_PACKED, false),
    ] {
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap();
        assert_eq!(row.operands.len(), 3);
        assert_eq!(
            row.clobbers,
            model.model().view_named("rflags").unwrap().units
        );
        for (position, operand) in row.operands.iter().enumerate() {
            let writes = position == 2 || (load && position == 1);
            assert_eq!(
                operand.access,
                if writes {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                }
            );
            assert_eq!(operand.early_clobber, writes);
            assert!(operand.fixed_view.is_none());
        }
        for mutation in 0..3 {
            let mut changed = catalog.clone();
            let row = changed
                .constraints
                .iter_mut()
                .find(|row| row.key == key)
                .unwrap();
            match mutation {
                0 => row.operands[2].early_clobber = false,
                1 => row.clobbers.clear(),
                _ => row.operands[2].access = RegisterOperandAccess::Use,
            }
            assert!(crate::validate_x86_64_register_constraint_catalog(changed, &model).is_err());
        }
    }
    for (semantic, width) in [
        (MachineSemanticKind::LoadPacked3, 3),
        (MachineSemanticKind::LoadPacked5, 5),
        (MachineSemanticKind::LoadPacked6, 6),
        (MachineSemanticKind::LoadPacked7, 7),
    ] {
        let declaration = effects
            .declarations
            .iter()
            .find(|row| row.semantic == semantic)
            .unwrap();
        assert_eq!(
            declaration.alternatives[0].encoded.memory,
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: width
            }
        );
        assert_eq!(
            declaration.alternatives[0].encoded.external_operand_writes,
            [1, 2]
        );
        for mutation in 0..3 {
            let mut changed = effects.clone();
            let encoded = &mut changed
                .declarations
                .iter_mut()
                .find(|row| row.semantic == semantic)
                .unwrap()
                .alternatives[0]
                .encoded;
            match mutation {
                0 => encoded.implicit_unit_clobbers.clear(),
                1 => encoded.external_operand_writes.truncate(1),
                _ => {
                    encoded.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: 8,
                    }
                }
            }
            assert!(
                crate::validate_x86_64_machine_effect_catalog(
                    target::NativeTarget::linux_x64(),
                    &constraints,
                    changed
                )
                .is_err()
            );
        }
    }
}
