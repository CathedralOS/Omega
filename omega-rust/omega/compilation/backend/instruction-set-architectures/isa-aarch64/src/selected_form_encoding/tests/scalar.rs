//! Scalar-form tests: explicit call refusal, zero-extend and movn/movk
//! materializations, bitwise and wrapping alias custody, decoded
//! footprints, and the compare-immediate canonical form.

use super::*;

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
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let copy = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64),
                &operands,
            )
            .unwrap();
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    copy.bytes()
                )
                .is_err()
            );
            for byte in 0..4 {
                let mut changed = encoded.bytes().to_vec();
                changed[byte] ^= 1;
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
            }
        }
    }
}

fn movn(register: u8, halfword: u8, immediate: u16) -> [u8; 4] {
    (0x9280_0000 | (u32::from(halfword) << 21) | (u32::from(immediate) << 5) | u32::from(register))
        .to_le_bytes()
}

fn movk(register: u8, halfword: u8, immediate: u16) -> [u8; 4] {
    (0xf280_0000 | (u32::from(halfword) << 21) | (u32::from(immediate) << 5) | u32::from(register))
        .to_le_bytes()
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
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let narrow = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::ZeroExtendU8,
                alternative(MachineAlternativeFamily::ZeroExtendU8),
                &operands,
            )
            .unwrap();
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    narrow.bytes()
                )
                .is_err()
            );
            let copy = encode_aarch64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64),
                &operands,
            )
            .unwrap();
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    copy.bytes()
                )
                .is_err()
            );
            for byte in 0..4 {
                let mut changed = encoded.bytes().to_vec();
                changed[byte] ^= 1;
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
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
            validate_aarch64_selected_form_encoding(
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
fn shortest_movn_materialization_shrinks_all_ones_and_high_ones() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    for (value, baseline_bytes, movn_bytes, seed, patches) in [
        (
            IntegerValue::Unsigned(u64::MAX as u128),
            16,
            4,
            Aarch64MovnSeed {
                halfword: 0,
                immediate: 0,
            },
            vec![],
        ),
        (
            IntegerValue::Unsigned(0xffff_ffff_0000_0000),
            12,
            8,
            Aarch64MovnSeed {
                halfword: 0,
                immediate: u16::MAX,
            },
            vec![Aarch64MovkPatch {
                halfword: 1,
                immediate: 0,
            }],
        ),
    ] {
        let recipe = aarch64_shortest_movn_materialization_recipe(value).unwrap();
        assert_eq!(recipe.seed(), seed);
        assert_eq!(recipe.patches(), patches);
        assert_eq!(recipe.baseline_byte_count(), baseline_bytes);
        assert_eq!(recipe.encoded_byte_count(), movn_bytes);

        let baseline = encode_aarch64_selected_form(
            &physical,
            SelectedInstructionKind::MaterializeI64 { value },
            alternative(MachineAlternativeFamily::MaterializeI64),
            &[x9],
        )
        .unwrap();
        let encoded = encode_aarch64_shortest_movn_materialization(&physical, x9, value).unwrap();
        assert_eq!(baseline.bytes().len(), baseline_bytes);
        assert_eq!(encoded.bytes().len(), movn_bytes);
        assert!(encoded.bytes().len() < baseline.bytes().len());
        assert_eq!(encoded.footprint().register_writes, [x9]);
        validate_aarch64_shortest_movn_materialization(&physical, x9, value, encoded.bytes())
            .unwrap();
    }
}

#[test]
fn movn_recipe_rejects_zero_small_and_equal_length_baselines() {
    for value in [
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(1),
        IntegerValue::Unsigned(0x5678_ffff_0000_1234),
    ] {
        assert_eq!(
            aarch64_shortest_movn_materialization_recipe(value),
            Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink)
        );
    }

    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    let zero_movn = [
        movn(9, 0, u16::MAX),
        movk(9, 1, 0),
        movk(9, 2, 0),
        movk(9, 3, 0),
    ]
    .concat();
    assert_eq!(
        validate_aarch64_shortest_movn_materialization(
            &physical,
            x9,
            IntegerValue::Unsigned(0),
            &zero_movn,
        ),
        Err(Aarch64SelectedFormEncodingError::MovnMaterializationDoesNotShrink)
    );
}

#[test]
fn movn_recipe_chooses_the_lowest_seed_among_minimum_count_recipes() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    let value = IntegerValue::Unsigned(0xffff_ffff_0000_0000);
    let recipe = aarch64_shortest_movn_materialization_recipe(value).unwrap();
    assert_eq!(recipe.seed().halfword(), 0);

    // Seeding halfword one has the same instruction count and reconstructs
    // the same bits, but it is not the canonical lowest seed.
    let noncanonical = [movn(9, 1, u16::MAX), movk(9, 0, 0)].concat();
    assert!(
        validate_aarch64_shortest_movn_materialization(&physical, x9, value, &noncanonical,)
            .is_err()
    );
}

#[test]
fn movn_recipe_is_bit_pattern_canonical_across_signedness() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    for (signed, unsigned) in [
        (
            IntegerValue::Signed(-1),
            IntegerValue::Unsigned(u64::MAX as u128),
        ),
        (
            IntegerValue::Signed(-4_294_967_296),
            IntegerValue::Unsigned(0xffff_ffff_0000_0000),
        ),
    ] {
        assert_eq!(
            aarch64_shortest_movn_materialization_recipe(signed).unwrap(),
            aarch64_shortest_movn_materialization_recipe(unsigned).unwrap()
        );
        assert_eq!(
            encode_aarch64_shortest_movn_materialization(&physical, x9, signed)
                .unwrap()
                .bytes(),
            encode_aarch64_shortest_movn_materialization(&physical, x9, unsigned)
                .unwrap()
                .bytes()
        );
    }
}

#[test]
fn movn_validation_rejects_opcode_destination_value_recipe_and_order_corruption() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x9 = physical.model().view_named("x9").unwrap().id;
    let x10 = physical.model().view_named("x10").unwrap().id;
    let value = IntegerValue::Unsigned(0xffff_0003_0002_0001);
    let encoded = encode_aarch64_shortest_movn_materialization(&physical, x9, value).unwrap();
    assert_eq!(encoded.bytes().len(), 12);

    let rejects = |bytes: &[u8]| {
        assert!(
            validate_aarch64_shortest_movn_materialization(&physical, x9, value, bytes,).is_err()
        );
    };

    let mut wrong_opcode = encoded.bytes().to_vec();
    wrong_opcode[..4].copy_from_slice(&0xd280_0009_u32.to_le_bytes());
    rejects(&wrong_opcode);

    let mut wrong_destination = encoded.bytes().to_vec();
    wrong_destination[0] = (wrong_destination[0] & !0x1f) | 10;
    rejects(&wrong_destination);
    assert!(
        validate_aarch64_shortest_movn_materialization(&physical, x10, value, encoded.bytes(),)
            .is_err()
    );

    let all_ones = IntegerValue::Unsigned(u64::MAX as u128);
    for corrupted in [movn(9, 1, 0).to_vec(), movn(9, 0, 1).to_vec()] {
        assert!(
            validate_aarch64_shortest_movn_materialization(&physical, x9, all_ones, &corrupted,)
                .is_err()
        );
    }

    let mut reversed_patches = encoded.bytes().to_vec();
    let first_patch = reversed_patches[4..8].to_vec();
    let second_patch = reversed_patches[8..12].to_vec();
    reversed_patches[4..8].copy_from_slice(&second_patch);
    reversed_patches[8..12].copy_from_slice(&first_patch);
    rejects(&reversed_patches);

    let mut wrong_immediate = encoded.bytes().to_vec();
    wrong_immediate[4] ^= 0x20;
    rejects(&wrong_immediate);

    let mut redundant_patch = encode_aarch64_shortest_movn_materialization(
        &physical,
        x9,
        IntegerValue::Unsigned(u64::MAX as u128),
    )
    .unwrap()
    .bytes()
    .to_vec();
    redundant_patch.extend_from_slice(&movk(9, 1, u16::MAX));
    assert!(
        validate_aarch64_shortest_movn_materialization(
            &physical,
            x9,
            IntegerValue::Unsigned(u64::MAX as u128),
            &redundant_patch,
        )
        .is_err()
    );

    assert!(
        validate_aarch64_shortest_movn_materialization(
            &physical,
            x9,
            IntegerValue::Unsigned(0xffff_0003_0002_0000),
            encoded.bytes(),
        )
        .is_err()
    );
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
                        validate_aarch64_selected_form_encoding(
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
                    validate_aarch64_selected_form_encoding(
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
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &duplicated,
                    )
                    .is_err()
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
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert!(!encoded.footprint().writes_nzcv);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                let mut changed = encoded.bytes().to_vec();
                changed[0] ^= 1;
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
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
                    validate_aarch64_selected_form_encoding(
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
                        validate_aarch64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &corrupted
                        )
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
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[operand],
                encoded.bytes()
            )
            .is_ok()
        );
        for mask in [1_u32, 1 << 5, 1 << 10, 1 << 22, 1 << 30] {
            let word = u32::from_le_bytes(encoded.bytes().try_into().unwrap());
            let corrupted = (word ^ mask).to_le_bytes();
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[operand],
                    &corrupted
                )
                .is_err()
            );
        }
    }
    // `subs xzr, xN, #0` decodes as the shared `CompareZero` word; the
    // zero immediate still validates for this kind.
    let zero = SelectedInstructionKind::CompareI64Immediate {
        immediate: IntegerValue::Unsigned(0),
    };
    let encoded = encode_aarch64_selected_form(&physical, zero, alternative, &[x3]).unwrap();
    assert_eq!(encoded.bytes(), [0x7f, 0x00, 0x00, 0xf1]);
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            zero,
            alternative,
            &[x3],
            encoded.bytes()
        )
        .is_ok()
    );
    for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
        let kind = SelectedInstructionKind::CompareI64Immediate { immediate };
        assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x3]).is_err());
    }
}
