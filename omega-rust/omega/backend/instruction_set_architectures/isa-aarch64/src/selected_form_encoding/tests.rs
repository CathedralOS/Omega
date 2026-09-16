//! Tests for aarch64 selected form encoding.

use super::{
    Aarch64MovkPatch, Aarch64MovnSeed, Aarch64SelectedFormEncodingError,
    aarch64_shortest_movn_materialization_recipe,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_form, encode_aarch64_selected_i64_less_than_branch_form,
    encode_aarch64_selected_nonzero_branch_form, encode_aarch64_selected_u64_less_than_branch_form,
    encode_aarch64_shortest_movn_materialization,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_form_encoding, validate_aarch64_selected_i64_less_than_branch_form,
    validate_aarch64_selected_nonzero_branch_form,
    validate_aarch64_selected_u64_less_than_branch_form,
    validate_aarch64_shortest_movn_materialization,
};
use crate::aarch64_physical_register_model;
use crate::selected_form_encoding::decoding::DecodedWord;
use crate::selected_form_encoding::decoding::decode_words;
use optimization_core::AcceptedObligationFactIdentity;
use register_model::validate_physical_register_model;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedStackEffect;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId};

fn alternative(family: MachineAlternativeFamily) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant: 0 }
}

#[test]
fn saturating_subtract_binds_registers_condition_and_flag_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingSubtractU64;
    let key = alternative(MachineAlternativeFamily::SaturatingSubtractU64);
    for left in ["x0", "x1", "x9", "x29"] {
        for right in ["x0", "x1", "x9", "x29"] {
            for destination in ["x0", "x1", "x9", "x29"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands);

                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 8);
                let decoded = validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(!decoded.footprint().encoded.implicit_unit_defs.is_empty());
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "changed byte {byte}"
                    );
                }
            }
        }
    }
}

#[test]
fn saturating_add_binds_registers_condition_and_flag_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingAddU64;
    let key = alternative(MachineAlternativeFamily::SaturatingAddU64);
    for left in ["x0", "x1", "x9", "x29"] {
        for right in ["x0", "x1", "x9", "x29"] {
            for destination in ["x0", "x1", "x9", "x29"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands);

                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 8);
                let decoded = validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(!decoded.footprint().encoded.implicit_unit_defs.is_empty());
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "changed byte {byte}"
                    );
                }
            }
        }
    }
}

#[test]
fn saturation_high_register_bytes_match_independent_assembler() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let operands = ["x9", "x10", "x11"].map(|name| physical.model().view_named(name).unwrap().id);
    // Independently assembled with Apple clang; not derived from this encoder.
    for (kind, family, bytes) in [
        (
            SelectedInstructionKind::SaturatingSubtractU64,
            MachineAlternativeFamily::SaturatingSubtractU64,
            [0xeb0a012bu32, 0x9a9f216b]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
        (
            SelectedInstructionKind::SaturatingAddU64,
            MachineAlternativeFamily::SaturatingAddU64,
            [0xab0a012bu32, 0xda9f316b]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
    ] {
        let key = alternative(family);
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), bytes);
        validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &bytes).unwrap();
    }
}

#[test]
fn wrapping_remainder_binds_signed_divide_msub_and_preserved_inputs() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::WrappingRemainderI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::WrappingRemainderI64);
    for names in [["x9", "x10", "x11"], ["x9", "x9", "x11"]] {
        let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes().len(), 8);
        assert_eq!(encoded.footprint().register_reads, operands[..2]);
        assert_eq!(encoded.footprint().register_writes, operands[2..]);
        assert_eq!(
            encoded.footprint().encoded,
            MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
        );
        assert!(!encoded.footprint().writes_nzcv);
        if names[1] == "x10" {
            assert_eq!(
                encoded.bytes(),
                [0x9aca_0d2b_u32, 0x9b0a_a56b]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
        }
        for byte_position in 0..encoded.bytes().len() {
            let mut changed = encoded.bytes().to_vec();
            changed[byte_position] ^= 1;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &changed,)
                    .is_err(),
                "mutated byte {byte_position}"
            );
        }
        for operand_position in 0..operands.len() {
            let mut changed = operands;
            changed[operand_position] = physical.model().view_named("x8").unwrap().id;
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &changed,
                    encoded.bytes(),
                )
                .is_err()
            );
        }
    }
    for names in [["x9", "x10", "x9"], ["x9", "x10", "x10"]] {
        let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
        assert!(encode_aarch64_selected_form(&physical, kind, key, &operands).is_err());
        // Even bytes matching the requested destructive alias must reject.
        let destination = if names[2] == "x9" { 9 } else { 10 };
        let bytes = [
            0x9aca_0d20 | destination,
            0x9b0a_a400 | (destination << 5) | destination,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        assert!(
            validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                .is_err()
        );
    }
}

#[test]
fn wrapping_remainder_decoded_arithmetic_covers_minimum_and_dividend_sign() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::WrappingRemainderI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::WrappingRemainderI64);
    let operands = ["x9", "x10", "x11"].map(|name| physical.model().view_named(name).unwrap().id);
    let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
    for (dividend, divisor, expected) in [
        (i64::MIN, -1, 0),
        (i64::MIN, 1, 0),
        (i64::MIN, 3, -2),
        (7, 2, 1),
        (-7, 2, -1),
        (7, -2, 1),
        (-7, -2, -1),
        (0, -1, 0),
    ] {
        let mut registers = [0_i64; 31];
        registers[9] = dividend;
        registers[10] = divisor;
        for instruction in decode_words(encoded.bytes()).unwrap() {
            match instruction {
                DecodedWord::SignedDivide {
                    dividend,
                    divisor,
                    destination,
                } => {
                    registers[destination as usize] =
                        registers[dividend as usize].wrapping_div(registers[divisor as usize]);
                }
                DecodedWord::MultiplySubtract {
                    left,
                    right,
                    minuend,
                    destination,
                } => {
                    registers[destination as usize] = registers[minuend as usize].wrapping_sub(
                        registers[left as usize].wrapping_mul(registers[right as usize]),
                    );
                }
                _ => panic!("unexpected remainder instruction"),
            }
        }
        assert_eq!(registers[11], expected, "{dividend} % {divisor}");
        assert_eq!((registers[9], registers[10]), (dividend, divisor));
    }
}

#[test]
fn exact_divide_binds_unsigned_opcode_and_registers() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ExactDivideU64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::ExactDivideU64);
    let names = ["x9", "x10", "x11"];
    let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
    let expected = 0x9aca092bu32.to_le_bytes().to_vec();

    let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
    assert_eq!(encoded.bytes(), expected);
    for byte in 0..expected.len() {
        let mut changed = expected.clone();
        changed[byte] ^= 1;
        assert!(
            validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                .is_err()
        );
    }
    for operand in 0..operands.len() {
        let mut changed = operands;
        changed[operand] = physical.model().view_named("x8").unwrap().id;
        assert!(
            validate_aarch64_selected_form_encoding(&physical, kind, key, &changed, &expected)
                .is_err()
        );
    }
}

fn signed_saturating_i32_kinds() -> [(SelectedInstructionKind, MachineAlternativeFamily); 3] {
    [
        (
            SelectedInstructionKind::SaturatingAddI32,
            MachineAlternativeFamily::SaturatingAddI32,
        ),
        (
            SelectedInstructionKind::SaturatingSubtractI32,
            MachineAlternativeFamily::SaturatingSubtractI32,
        ),
        (
            SelectedInstructionKind::SaturatingDivideI32 {
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            MachineAlternativeFamily::SaturatingDivideI32,
        ),
    ]
}

#[test]
fn signed_saturating_i32_forms_match_independent_assembler_and_reject_mutation() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let operands =
        ["x9", "x10", "x11", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
    // Independently assembled with Apple clang; not derived from this encoder.
    let upper_clamp = [0xb240_7bec_u32, 0xeb0c_017f, 0x9a8b_c18b];
    let lower_clamp = [0xb261_83ec_u32, 0xeb0c_017f, 0x9a8b_b18b];
    for (kind, family) in signed_saturating_i32_kinds() {
        let key = alternative(family);
        let mut expected = vec![match kind {
            SelectedInstructionKind::SaturatingAddI32 => 0x8b0a_012b,
            SelectedInstructionKind::SaturatingSubtractI32 => 0xcb0a_012b,
            _ => 0x9aca_0d2b,
        }];
        expected.extend(upper_clamp);
        if !matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. }) {
            expected.extend(lower_clamp);
        }
        let expected = expected
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), expected, "{kind:?}");
        assert_eq!(encoded.footprint().register_reads, operands[..2]);
        assert_eq!(encoded.footprint().register_writes, operands[2..]);
        assert!(encoded.footprint().writes_nzcv);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2, 3]);
        for byte_position in 0..expected.len() {
            let mut changed = expected.clone();
            changed[byte_position] ^= 1;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                    .is_err(),
                "{kind:?} mutated byte {byte_position}"
            );
        }
        for operand_position in 0..operands.len() {
            let mut changed = operands;
            changed[operand_position] = physical.model().view_named("x8").unwrap().id;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &changed, &expected)
                    .is_err(),
                "{kind:?} substituted operand {operand_position}"
            );
        }
        // Both outputs are early-clobber: neither the result nor the bound
        // scratch may alias an input, and they may not alias each other.
        for names in [
            ["x9", "x10", "x11", "x9"],
            ["x9", "x10", "x11", "x10"],
            ["x9", "x10", "x11", "x11"],
            ["x9", "x10", "x9", "x12"],
            ["x9", "x10", "x10", "x12"],
        ] {
            let aliased = names.map(|name| physical.model().view_named(name).unwrap().id);
            assert!(
                encode_aarch64_selected_form(&physical, kind, key, &aliased).is_err(),
                "{kind:?} {names:?}"
            );
        }
        for (other_kind, other_family) in signed_saturating_i32_kinds() {
            if other_family != family {
                assert!(
                    validate_aarch64_selected_form_encoding(
                        &physical,
                        other_kind,
                        alternative(other_family),
                        &operands,
                        &expected,
                    )
                    .is_err(),
                    "{kind:?} bytes accepted as {other_kind:?}"
                );
            }
        }
    }
}

#[test]
fn signed_saturating_i32_decoded_arithmetic_clamps_every_carrier_edge() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let operands =
        ["x9", "x10", "x11", "x12"].map(|name| physical.model().view_named(name).unwrap().id);
    let inputs = [
        (i32::MAX, 1),
        (i32::MIN, -1),
        (i32::MIN, 1),
        (i32::MAX, i32::MAX),
        (i32::MIN, i32::MIN),
        (-7, 2),
        (7, -2),
        (40, 30),
        (0, -1),
        (i32::MAX, -1),
    ];
    for (kind, family) in signed_saturating_i32_kinds() {
        let encoded =
            encode_aarch64_selected_form(&physical, kind, alternative(family), &operands).unwrap();
        let decoded = decode_words(encoded.bytes()).unwrap();
        for (left, right) in inputs {
            let expected = match kind {
                SelectedInstructionKind::SaturatingAddI32 => left.saturating_add(right),
                SelectedInstructionKind::SaturatingSubtractI32 => left.saturating_sub(right),
                _ => left.saturating_div(right),
            };
            let mut registers = [0_i64; 31];
            registers[9] = i64::from(left);
            registers[10] = i64::from(right);
            let mut greater = false;
            let mut less = false;
            for word in &decoded {
                match *word {
                    DecodedWord::Add {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] =
                            registers[left as usize] + registers[right as usize];
                    }
                    DecodedWord::Subtract {
                        left,
                        right,
                        destination,
                    } => {
                        registers[destination as usize] =
                            registers[left as usize] - registers[right as usize];
                    }
                    DecodedWord::SignedDivide {
                        dividend,
                        divisor,
                        destination,
                    } => {
                        registers[destination as usize] =
                            registers[dividend as usize] / registers[divisor as usize];
                    }
                    DecodedWord::MaterializeI32Maximum { destination } => {
                        registers[destination as usize] = i64::from(i32::MAX);
                    }
                    DecodedWord::MaterializeI32Minimum { destination } => {
                        registers[destination as usize] = i64::from(i32::MIN);
                    }
                    DecodedWord::Compare { left, right } => {
                        greater = registers[left as usize] > registers[right as usize];
                        less = registers[left as usize] < registers[right as usize];
                    }
                    DecodedWord::SelectOnGreater {
                        source,
                        destination,
                    } => {
                        if greater {
                            registers[destination as usize] = registers[source as usize];
                        }
                    }
                    DecodedWord::SelectOnLess {
                        source,
                        destination,
                    } => {
                        if less {
                            registers[destination as usize] = registers[source as usize];
                        }
                    }
                    other => panic!("unexpected saturating word {other:?}"),
                }
            }
            assert_eq!(
                registers[11],
                i64::from(expected),
                "{kind:?} {left} {right}"
            );
            assert_eq!(
                (registers[9], registers[10]),
                (i64::from(left), i64::from(right))
            );
        }
    }
}

#[test]
fn scalar_call_is_explicitly_refused_before_encoding() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    assert_eq!(
        encode_aarch64_selected_form(
            &physical,
            SelectedInstructionKind::CallScalar {
                callee: MachineId::new(1).unwrap(),
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

#[test]
fn ret_x30_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap().id;
    let x1 = physical.model().view_named("x1").unwrap().id;
    let x30 = physical.model().view_named("x30").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let kind = SelectedInstructionKind::ReturnScalar;
    let alternative = alternative(MachineAlternativeFamily::ReturnScalar);
    let encoded = encode_aarch64_selected_form(&physical, kind, alternative, &[x0]).unwrap();

    assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
    assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
    assert_eq!(encoded.footprint().encoded.implicit_unit_uses, x30.units);
    assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::NoneV1
    );
    assert_eq!(
        encoded.footprint().encoded.stack,
        MachineEncodedStackEffect::UnchangedV1
    );
    assert_eq!(
        encoded.footprint().encoded.trap,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    );
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { target: x30.id }
    );
    assert!(encode_aarch64_selected_form(&physical, kind, alternative, &[x1]).is_err());
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            alternative,
            &[x0],
            &0xd65f_03a0_u32.to_le_bytes()
        )
        .is_err()
    );
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            alternative,
            &[x0],
            &[0xc0, 0x03, 0x5f, 0xd6, 0xc0, 0x03, 0x5f, 0xd6]
        )
        .is_err()
    );
}

#[test]
fn unit_return_is_a_distinct_zero_operand_ret_x30() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ReturnUnit;
    let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit);
    let encoded = encode_aarch64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

    assert_eq!(encoded.bytes(), [0xc0, 0x03, 0x5f, 0xd6]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert!(matches!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnIndirectRegisterV1 { .. }
    ));
    assert!(
        encode_aarch64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ReturnScalar),
            &[]
        )
        .is_err()
    );
}

#[test]
fn nonzero_branch_has_exact_instruction_relative_imm19_and_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero);
    for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
        let encoded =
            encode_aarch64_selected_nonzero_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(encoded.bytes().len(), 4);
        assert_eq!(encoded.bytes()[0] & 0x1f, 1);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_selected_nonzero_branch_form(&physical, alternative, displacement)
                .is_err()
        );
    }
    assert!(
        validate_aarch64_selected_nonzero_branch_form(
            &physical,
            alternative,
            0,
            &0x5400_0000_u32.to_le_bytes()
        )
        .is_err()
    );
    assert!(
        validate_aarch64_selected_nonzero_branch_form(&physical, alternative, 0, &[1, 0, 0, 0, 0])
            .is_err()
    );
}

#[test]
fn u64_less_than_branch_is_exact_b_lo_imm19_with_flag_control_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let nzcv = physical.model().view_named("nzcv").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan);
    for (displacement, expected) in [
        (-4, [0xe3, 0xff, 0xff, 0x54]),
        (0, [0x03, 0x00, 0x00, 0x54]),
        (4, [0x23, 0x00, 0x00, 0x54]),
    ] {
        let encoded =
            encode_aarch64_selected_u64_less_than_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert!(nzcv.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert!(pc.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
    }
    for displacement in [-1_048_576, 1_048_572] {
        assert!(
            encode_aarch64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .is_ok()
        );
    }
    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_selected_u64_less_than_branch_form(
                &physical,
                alternative,
                displacement,
            )
            .is_err()
        );
    }
    for bytes in [
        0x5400_0002_u32.to_le_bytes(),
        0x5400_0009_u32.to_le_bytes(),
        0x1400_0003_u32.to_le_bytes(),
    ] {
        assert_eq!(
            validate_aarch64_selected_u64_less_than_branch_form(&physical, alternative, 0, &bytes,),
            Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_aarch64_selected_u64_less_than_branch_form(
            &physical,
            alternative,
            0,
            &0x5400_0023_u32.to_le_bytes(),
        ),
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    );
}

#[test]
fn i64_less_than_branch_is_exact_b_lt_imm19() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan);
    for (displacement, expected) in [
        (-4, [0xeb, 0xff, 0xff, 0x54]),
        (0, [0x0b, 0x00, 0x00, 0x54]),
        (4, [0x2b, 0x00, 0x00, 0x54]),
    ] {
        let encoded =
            encode_aarch64_selected_i64_less_than_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    assert_eq!(
        validate_aarch64_selected_i64_less_than_branch_form(
            &physical,
            alternative,
            0,
            &0x5400_0003_u32.to_le_bytes(),
        ),
        Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
    );
}

#[test]
fn fused_cbnz_is_exact_rejects_nearby_opcodes_and_does_not_read_nzcv() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let x0 = physical.model().view_named("x0").unwrap();
    let x30 = physical.model().view_named("x30").unwrap();
    let pc = physical.model().view_named("pc").unwrap();
    let nzcv = physical.model().view_named("nzcv").unwrap();

    for (source, register) in [(x0.id, 0_u8), (x30.id, 30_u8)] {
        for displacement in [-1_048_576, -4, 0, 4, 1_048_572] {
            let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                source,
                displacement,
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            assert_eq!(encoded.bytes()[0] & 0x1f, register);
            assert_eq!(encoded.footprint().register_reads, [source]);
            assert!(encoded.footprint().register_writes.is_empty());
            assert!(!encoded.footprint().writes_nzcv);
            assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
            assert_eq!(encoded.footprint().encoded.implicit_unit_uses, pc.units);
            assert_eq!(encoded.footprint().encoded.implicit_unit_defs, pc.units);
            assert!(
                encoded
                    .footprint()
                    .encoded
                    .implicit_unit_uses
                    .iter()
                    .all(|unit| !nzcv.units.contains(unit))
            );
        }
    }

    for displacement in [-1_048_580, 1_048_576, 2] {
        assert!(
            encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                displacement,
            )
            .is_err()
        );
    }
    for word in [
        0xb400_0000_u32, // 64-bit CBZ
        0x3500_0000_u32, // 32-bit CBNZ
        0xb500_001e_u32, // wrong source register
    ] {
        assert!(
            validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                &physical,
                x0.id,
                0,
                &word.to_le_bytes(),
            )
            .is_err()
        );
    }
    assert!(
        validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
            &physical,
            x0.id,
            0,
            &[0, 0, 0, 0, 0],
        )
        .is_err()
    );
}
