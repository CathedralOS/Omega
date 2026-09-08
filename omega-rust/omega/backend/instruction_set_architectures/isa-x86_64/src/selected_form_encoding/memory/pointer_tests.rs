use super::*;

#[test]
fn pointer_store_known_bytes_preserve_the_requested_width() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let operands = ["r12", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::Store,
        variant: 0,
    };
    for (byte_size, expected) in [
        (1, vec![0x45, 0x88, 0x8c, 0x24, 8, 0, 0, 0]),
        (2, vec![0x66, 0x45, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
        (4, vec![0x45, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
        (8, vec![0x4d, 0x89, 0x8c, 0x24, 8, 0, 0, 0]),
    ] {
        let kind = SelectedInstructionKind::Store {
            byte_offset: 8,
            byte_size,
        };
        let encoded =
            encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 8).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert!(
            validate_x86_64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &[operands[1], operands[0]],
                8,
                encoded.bytes()
            )
            .is_err()
        );
        for wrong_width in [1, 2, 4, 8] {
            if wrong_width != byte_size {
                assert!(
                    validate_x86_64_selected_memory_form(
                        &physical,
                        SelectedInstructionKind::Store {
                            byte_offset: 8,
                            byte_size: wrong_width
                        },
                        alternative,
                        &operands,
                        8,
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
        }
    }
    let address = encode_x86_64_selected_memory_form(
        &physical,
        SelectedInstructionKind::AddressOffset { byte_offset: 2 },
        MachineAlternativeKey {
            family: MachineAlternativeFamily::AddressOffset,
            variant: 0,
        },
        &operands,
        2,
    )
    .unwrap();
    assert_eq!(address.bytes(), [0x4d, 0x8d, 0x8c, 0x24, 2, 0, 0, 0]);
}

#[test]
fn pointer_stores_and_offsets_replay_every_bit_and_operand() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    for base_name in ["rax", "rbp", "rsi", "r8", "r12", "r15"] {
        for value_name in ["rax", "rbp", "rsi", "r8", "r12", "r15"] {
            let operands =
                [base_name, value_name].map(|name| physical.model().view_named(name).unwrap().id);
            for byte_size in [1, 2, 4, 8] {
                for byte_offset in [0, u32::from(byte_size), i32::MAX as u32] {
                    let kind = SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    };
                    check_replay(
                        &physical,
                        kind,
                        MachineAlternativeFamily::Store,
                        &operands,
                        byte_offset,
                    );
                }
            }
            for byte_offset in [0, 2, 4095] {
                check_replay(
                    &physical,
                    SelectedInstructionKind::AddressOffset { byte_offset },
                    MachineAlternativeFamily::AddressOffset,
                    &operands,
                    byte_offset,
                );
            }
        }
    }
}

fn check_replay(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    family: MachineAlternativeFamily,
    operands: &[RegisterViewId],
    displacement: u32,
) {
    let alternative = MachineAlternativeKey { family, variant: 0 };
    let encoded =
        encode_x86_64_selected_memory_form(physical, kind, alternative, operands, displacement)
            .unwrap();
    let store = matches!(kind, SelectedInstructionKind::Store { .. });
    assert_eq!(
        encoded.footprint().encoded.external_operand_reads,
        if store { vec![0, 1] } else { vec![0] }
    );
    assert_eq!(
        encoded.footprint().encoded.external_operand_writes,
        if store { vec![] } else { vec![1] }
    );
    assert_eq!(
        encoded.footprint().encoded.memory,
        if store {
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 }
        } else {
            MachineEncodedMemoryEffect::NoneV1
        }
    );
    for bit in 0..encoded.bytes().len() * 8 {
        let mut corrupt = encoded.bytes().to_vec();
        corrupt[bit / 8] ^= 1 << (bit % 8);
        assert!(
            validate_x86_64_selected_memory_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                &corrupt
            )
            .is_err(),
            "{kind:?} bit {bit}"
        );
    }
    for end in 0..encoded.bytes().len() {
        assert!(
            validate_x86_64_selected_memory_form(
                physical,
                kind,
                alternative,
                operands,
                displacement,
                &encoded.bytes()[..end]
            )
            .is_err()
        );
    }
    let mut trailing = encoded.bytes().to_vec();
    trailing.push(0);
    assert!(
        validate_x86_64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            displacement,
            &trailing
        )
        .is_err()
    );
    assert!(
        validate_x86_64_selected_memory_form(
            physical,
            kind,
            alternative,
            operands,
            displacement + 1,
            encoded.bytes()
        )
        .is_err()
    );
    assert!(
        encode_x86_64_selected_memory_form(
            physical,
            kind,
            MachineAlternativeKey { family, variant: 1 },
            operands,
            displacement
        )
        .is_err()
    );
    assert!(
        encode_x86_64_selected_memory_form(
            physical,
            kind,
            alternative,
            &operands[..1],
            displacement
        )
        .is_err()
    );
}

#[test]
fn pointer_stores_reject_unsupported_widths_and_offsets() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let operands = ["rax", "r9"].map(|name| physical.model().view_named(name).unwrap().id);
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::Store,
        variant: 0,
    };
    for (byte_size, byte_offset) in [(0, 0), (3, 0), (16, 0), (8, u32::MAX)] {
        assert!(
            encode_x86_64_selected_memory_form(
                &physical,
                SelectedInstructionKind::Store {
                    byte_offset,
                    byte_size
                },
                alternative,
                &operands,
                byte_offset
            )
            .is_err()
        );
    }
}
