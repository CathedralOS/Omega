//! Exact narrow pointer reads reject opcode, operand and displacement substitutions.
use super::*;

#[test]
fn narrow_loads_decode_exact_width_full_result_and_every_encoded_bit() {
    let physical =
        register_model::validate_physical_register_model(crate::aarch64_physical_register_model())
            .unwrap();
    for width in [1_u16, 2] {
        let family = if width == 1 {
            MachineAlternativeFamily::Load8
        } else {
            MachineAlternativeFamily::Load16
        };
        let alternative = MachineAlternativeKey { family, variant: 0 };
        for displacement in [0, u32::from(width), 4095 * u32::from(width)] {
            let kind = if width == 1 {
                SelectedInstructionKind::Load8 {
                    byte_offset: displacement,
                }
            } else {
                SelectedInstructionKind::Load16 {
                    byte_offset: displacement,
                }
            };
            for base_name in ["x0", "x1", "x15", "x28"] {
                let base = physical.model().view_named(base_name).unwrap().id;
                let destination = physical.model().view_named("x2").unwrap().id;
                let operands = [base, destination];
                let encoded = encode_aarch64_selected_memory_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.footprint().register_reads, [base]);
                assert_eq!(encoded.footprint().register_writes, [destination]);
                assert_eq!(
                    encoded.footprint().encoded.memory,
                    MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: width
                    }
                );
                assert_eq!(
                    encoded.footprint().encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                assert_eq!(
                    encoded.footprint().encoded.trap,
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                );
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        encoded.bytes()
                    )
                    .unwrap(),
                    encoded
                );
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &changed
                        )
                        .is_err(),
                        "{kind:?}, {base_name}, bit {bit}"
                    );
                }
                for changed_operands in [
                    [destination, base],
                    [destination, destination],
                    [base, base],
                ] {
                    assert!(
                        validate_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &changed_operands,
                            displacement,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement + u32::from(width),
                        encoded.bytes()
                    )
                    .is_err()
                );
                let changed_kind = if width == 1 {
                    SelectedInstructionKind::Load16 {
                        byte_offset: displacement,
                    }
                } else {
                    SelectedInstructionKind::Load8 {
                        byte_offset: displacement,
                    }
                };
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        changed_kind,
                        alternative,
                        &operands,
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        MachineAlternativeKey { family, variant: 1 },
                        &operands,
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands[..1],
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
                for end in 0..encoded.bytes().len() {
                    assert!(
                        validate_aarch64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &encoded.bytes()[..end]
                        )
                        .is_err()
                    );
                }
                let mut trailing = encoded.bytes().to_vec();
                trailing.push(0);
                assert!(
                    validate_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        &trailing
                    )
                    .is_err()
                );
                // The complete address is read before the result is defined.
                assert!(
                    encode_aarch64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &[base, base],
                        displacement
                    )
                    .is_ok()
                );
            }
        }
    }
}

#[test]
fn narrow_loads_have_zero_extending_opcodes_and_reject_invalid_addresses() {
    let physical =
        register_model::validate_physical_register_model(crate::aarch64_physical_register_model())
            .unwrap();
    let operands = ["x1", "x2"].map(|name| physical.model().view_named(name).unwrap().id);
    for (kind, family, expected) in [
        (
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            MachineAlternativeFamily::Load8,
            vec![0x22, 0, 0x40, 0x39],
        ),
        (
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            MachineAlternativeFamily::Load16,
            vec![0x22, 0, 0x40, 0x79],
        ),
    ] {
        let alternative = MachineAlternativeKey { family, variant: 0 };
        let encoded =
            encode_aarch64_selected_memory_form(&physical, kind, alternative, &operands, 0)
                .unwrap();
        assert_eq!(encoded.bytes(), expected);
        let mut signed = expected.clone();
        signed[2] = 0x80;
        assert!(
            validate_aarch64_selected_memory_form(
                &physical,
                kind,
                alternative,
                &operands,
                0,
                &signed
            )
            .is_err()
        );
        for name in ["sp", "w1", "d0"] {
            let invalid = physical.model().view_named(name).unwrap().id;
            for changed in [[invalid, operands[1]], [operands[0], invalid]] {
                assert!(
                    encode_aarch64_selected_memory_form(&physical, kind, alternative, &changed, 0)
                        .is_err()
                );
            }
        }
    }
    for displacement in [1, 8192] {
        assert!(
            encode_aarch64_selected_memory_form(
                &physical,
                SelectedInstructionKind::Load16 {
                    byte_offset: displacement
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Load16,
                    variant: 0
                },
                &operands,
                displacement
            )
            .is_err()
        );
    }
    assert!(
        encode_aarch64_selected_memory_form(
            &physical,
            SelectedInstructionKind::Load8 { byte_offset: 4096 },
            MachineAlternativeKey {
                family: MachineAlternativeFamily::Load8,
                variant: 0
            },
            &operands,
            4096
        )
        .is_err()
    );
}
