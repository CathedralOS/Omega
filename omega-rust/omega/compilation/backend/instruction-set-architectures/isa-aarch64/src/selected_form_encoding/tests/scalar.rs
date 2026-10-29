//! Scalar-form tests: explicit call refusal, zero-extend and movn/movk
//! materializations, bitwise and wrapping alias custody, decoded
//! footprints, and the compare-immediate canonical form.

use super::super::ValidatedAarch64SelectedFormEncoding;
use super::*;
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};

fn check_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, bytes)
}

#[test]
fn scalar_call_is_explicitly_refused_before_encoding() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    assert_eq!(
        encode_aarch64_selected_form(
            &physical,
            SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    MachineId::new(1).unwrap()
                ),
            },
            alternative(MachineAlternativeFamily::ReturnUnit),
            &[],
        ),
        Err(Aarch64SelectedFormEncodingError::LayoutDependentForm),
    );
}

#[test]
fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for source in ["x0", "x1", "x9", "x16", "x29"] {
        for destination in ["x0", "x1", "x9", "x16", "x29"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU8;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU8);
            let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
            let copy = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64),
                &operands,
            )
            .unwrap();
            assert!(check_encoding(&physical, kind, key, &operands, copy.bytes()).is_err());
            for byte in 0..4 {
                let mut changed = encoded.bytes().to_vec();
                changed[byte] ^= 1;
                assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
            }
        }
    }
}

#[test]
fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for source in ["x0", "x1", "x9", "x16", "x29"] {
        for destination in ["x0", "x1", "x9", "x16", "x29"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU32;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU32);
            let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            // UBFM with immr=0, imms=31 selects exactly the low U32 bits.
            let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
            assert_eq!(word & 0xffff_fc00, 0xd340_7c00);
            check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
            let narrow = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::ZeroExtendU8,
                alternative(MachineAlternativeFamily::ZeroExtendU8),
                &operands,
            )
            .unwrap();
            assert!(check_encoding(&physical, kind, key, &operands, narrow.bytes()).is_err());
            let copy = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64),
                &operands,
            )
            .unwrap();
            assert!(check_encoding(&physical, kind, key, &operands, copy.bytes()).is_err());
            for byte in 0..4 {
                let mut changed = encoded.bytes().to_vec();
                changed[byte] ^= 1;
                assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
            }
        }
    }
}

#[test]
fn zero_seeded_materialization_is_canonical_and_decoded_independently() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    for (value, byte_count) in [
        (IntegerValue::Unsigned(0), 4),
        (IntegerValue::Unsigned(u64::MAX as u128), 16),
        (IntegerValue::Unsigned(0x1234_0000_5678_0000), 12),
        (IntegerValue::Unsigned(0x1234_5678_9abc_def0), 16),
    ] {
        let kind = SelectedInstructionKind::MaterializeI64 { value };
        let encoded = encode_aarch64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::MaterializeI64),
            &[x9],
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), byte_count);
        let mut corrupted = encoded.bytes().to_vec();
        corrupted[0] ^= 0x20;
        assert!(
            check_encoding(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::MaterializeI64),
                &[x9],
                &corrupted,
            )
            .is_err()
        );
    }
}

#[test]
fn bitwise_xor_preserves_exact_eor_registers_aliases_and_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let registers = [("x3", 3_u32), ("x9", 9), ("x20", 20)]
        .map(|(name, number)| (physical.model().view_named(name).unwrap().id, number));
    let kind = SelectedInstructionKind::BitwiseXorI64;
    let alternative = alternative(MachineAlternativeFamily::BitwiseXorI64);
    for (left, left_number) in registers {
        for (right, right_number) in registers {
            for (output, output_number) in registers {
                let operands = [left, right, output];
                let expected =
                    0xca00_0000 | (right_number << 16) | (left_number << 5) | output_number;
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, alternative, &operands).unwrap();
                assert_eq!(encoded.bytes(), expected.to_le_bytes());
                let footprint = encoded.footprint();
                assert_eq!(footprint.register_reads, [left, right]);
                assert_eq!(footprint.register_writes, [output]);
                assert!(!footprint.writes_nzcv);
                assert_eq!(footprint.encoded.external_operand_reads, [0, 1]);
                assert_eq!(footprint.encoded.external_operand_writes, [2]);
                assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
                // Operand substitution, AND/ORR/EON, 32-bit width and shifted EOR
                // all differ from the selected exact unshifted EOR64 operation.
                for mask in [
                    1_u32,
                    1 << 5,
                    1 << 16,
                    1 << 30,
                    1 << 29,
                    1 << 21,
                    1 << 31,
                    1 << 10,
                ] {
                    assert!(
                        check_encoding(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &(expected ^ mask).to_le_bytes(),
                        )
                        .is_err(),
                        "mutation {mask:#x}"
                    );
                }
                assert!(
                    check_encoding(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &encoded.bytes()[..3],
                    )
                    .is_err()
                );
                let duplicated = [encoded.bytes(), encoded.bytes()].concat();
                assert!(
                    check_encoding(&physical, kind, alternative, &operands, &duplicated,).is_err()
                );
            }
        }
    }
    let operands = registers.map(|(view, _)| view);
    let wrong_alternative = self::alternative(MachineAlternativeFamily::BitwiseAndI64);
    assert!(encode_aarch64_selected_form(&physical, kind, wrong_alternative, &operands).is_err());
}

#[test]
fn wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let views = ["x3", "x9", "x20"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::WrappingAddI64;
    let key = alternative(MachineAlternativeFamily::WrappingAddI64);
    for left in views {
        for right in views {
            for output in views {
                let operands = [left, right, output];
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
                check_encoding(&physical, kind, key, &operands, encoded.bytes()).unwrap();
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                let mut changed = encoded.bytes().to_vec();
                changed[0] ^= 1;
                assert!(check_encoding(&physical, kind, key, &operands, &changed).is_err());
                assert!(
                    encode_aarch64_selected_form(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64),
                        &operands
                    )
                    .is_err()
                );
                assert!(
                    check_encoding(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64),
                        &operands,
                        encoded.bytes()
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn bitwise_and_preserves_aliases_and_rejects_opcode_corruption() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let views = ["x3", "x9", "x20"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::BitwiseAndI64;
    let alternative = alternative(MachineAlternativeFamily::BitwiseAndI64);
    for left in views {
        for right in views {
            for output in views {
                let operands = [left, right, output];
                let encoded =
                    encode_aarch64_selected_form(&physical, kind, alternative, &operands).unwrap();
                assert_eq!(encoded.bytes().len(), 4);
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                for mask in [1_u32, 1 << 5, 1 << 16, 1 << 29] {
                    let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
                    let corrupted = (word ^ mask).to_le_bytes();
                    assert!(
                        check_encoding(&physical, kind, alternative, &operands, &corrupted)
                            .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_forms_report_exact_decoded_footprints() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let views = ["x3", "x4", "x5"].map(|name| physical.model().view_named(name).unwrap().id);
    let fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]);
    let cases = [
        (
            SelectedInstructionKind::CopyI64,
            MachineAlternativeFamily::CopyI64,
            2,
        ),
        (
            SelectedInstructionKind::CompareI64Zero,
            MachineAlternativeFamily::CompareI64Zero,
            1,
        ),
        (
            SelectedInstructionKind::CompareI64,
            MachineAlternativeFamily::CompareI64,
            2,
        ),
        (
            SelectedInstructionKind::ExactAddI64 {
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: fact,
            },
            MachineAlternativeFamily::ExactAddI64,
            3,
        ),
        (
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
                obligation: ObligationId::new(2).unwrap(),
                accepted_fact: fact,
            },
            MachineAlternativeFamily::ExactAddI64Immediate,
            2,
        ),
        (
            SelectedInstructionKind::ExactSubtractI64 {
                obligation: ObligationId::new(3).unwrap(),
                accepted_fact: fact,
            },
            MachineAlternativeFamily::ExactSubtractI64,
            3,
        ),
        (
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(5),
                obligation: ObligationId::new(4).unwrap(),
                accepted_fact: fact,
            },
            MachineAlternativeFamily::ExactSubtractI64Immediate,
            2,
        ),
        (
            SelectedInstructionKind::ExactMultiplyI64 {
                obligation: ObligationId::new(5).unwrap(),
                accepted_fact: fact,
            },
            MachineAlternativeFamily::ExactMultiplyI64,
            3,
        ),
    ];
    for (kind, family, count) in cases {
        let encoded =
            encode_aarch64_selected_form(&physical, kind, alternative(family), &views[..count])
                .unwrap();
        assert_eq!(encoded.bytes().len(), 4);
        if matches!(
            kind,
            SelectedInstructionKind::ExactSubtractI64Immediate { .. }
        ) {
            assert_eq!(encoded.bytes(), [0x64, 0x14, 0x00, 0xd1]);
            assert!(!encoded.footprint().writes_nzcv);
            assert!(encoded.footprint().encoded.implicit_unit_defs.is_empty());
        }
    }
    let compare = encode_aarch64_selected_form(
        &physical,
        SelectedInstructionKind::CompareI64,
        alternative(MachineAlternativeFamily::CompareI64),
        &views[..2],
    )
    .unwrap();
    assert_eq!(compare.bytes(), [0x7f, 0x00, 0x04, 0xeb]);
    assert_eq!(compare.footprint().register_reads, views[..2]);
    assert!(compare.footprint().register_writes.is_empty());
    assert!(compare.footprint().writes_nzcv);
    assert_eq!(compare.footprint().encoded.external_operand_reads, [0, 1]);
    // `mul x5, x3, x4` is `madd x5, x3, x4, xzr`: flag-transparent with the
    // ordinary three-operand read/write footprint.
    let multiply = encode_aarch64_selected_form(
        &physical,
        SelectedInstructionKind::ExactMultiplyI64 {
            obligation: ObligationId::new(5).unwrap(),
            accepted_fact: fact,
        },
        alternative(MachineAlternativeFamily::ExactMultiplyI64),
        &views,
    )
    .unwrap();
    assert_eq!(multiply.bytes(), [0x65, 0x7c, 0x04, 0x9b]);
    assert_eq!(multiply.footprint().register_reads, views[..2]);
    assert_eq!(multiply.footprint().register_writes, views[2..]);
    assert!(!multiply.footprint().writes_nzcv);
    assert_eq!(multiply.footprint().encoded.external_operand_reads, [0, 1]);
    assert_eq!(multiply.footprint().encoded.external_operand_writes, [2]);
}

#[test]
fn compare_i64_immediate_is_subs_xzr_imm12_with_exact_footprint() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x3 = physical.model().view_named("x3").unwrap().id;
    let x12 = physical.model().view_named("x12").unwrap().id;
    let alternative = alternative(MachineAlternativeFamily::CompareI64Immediate);
    for (operand, immediate, expected) in [
        (x3, 5, [0x7f, 0x14, 0x00, 0xf1]),
        (x3, 4095, [0x7f, 0xfc, 0x3f, 0xf1]),
        (x12, 4095, [0x9f, 0xfd, 0x3f, 0xf1]),
    ] {
        let kind = SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
        };
        let encoded =
            encode_aarch64_selected_form(&physical, kind, alternative, &[operand]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(encoded.footprint().register_reads, [operand]);
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(encoded.footprint().writes_nzcv);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0]);
        assert!(
            encoded
                .footprint()
                .encoded
                .external_operand_writes
                .is_empty()
        );
        assert!(!encoded.footprint().encoded.implicit_unit_defs.is_empty());
        assert!(check_encoding(&physical, kind, alternative, &[operand], encoded.bytes()).is_ok());
        for mask in [1_u32, 1 << 5, 1 << 10, 1 << 22, 1 << 30] {
            let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
            let corrupted = (word ^ mask).to_le_bytes();
            assert!(check_encoding(&physical, kind, alternative, &[operand], &corrupted).is_err());
        }
    }
    // `subs xzr, xN, #0` decodes as the shared `CompareZero` word; the
    // zero immediate still validates for this kind.
    let zero = SelectedInstructionKind::CompareI64Immediate {
        immediate: IntegerValue::Unsigned(0),
    };
    let encoded = encode_aarch64_selected_form(&physical, zero, alternative, &[x3]).unwrap();
    assert_eq!(encoded.bytes(), [0x7f, 0x00, 0x00, 0xf1]);
    assert!(check_encoding(&physical, zero, alternative, &[x3], encoded.bytes()).is_ok());
    for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
        let kind = SelectedInstructionKind::CompareI64Immediate { immediate };
        assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x3]).is_err());
    }
}
