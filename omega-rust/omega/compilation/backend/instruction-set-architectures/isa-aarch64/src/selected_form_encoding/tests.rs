//! Tests for aarch64 selected form encoding.
use crate::test_support::rejects_every_bit_flip;

use super::{
    Aarch64ConditionalBranchForm, Aarch64SelectedFormEncodingError,
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_conditional_branch_form, encode_aarch64_selected_form,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_conditional_branch_form, validate_aarch64_selected_form_encoding,
};
use crate::aarch64_physical_register_model;
use crate::saturating_forms::SaturatingRealization;
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
use selected_instructions::{SaturatingCarrier, SaturatingOperation};
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId};

mod control;
mod saturating;
mod scalar;

fn alternative(family: MachineAlternativeFamily) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant: 0 }
}

#[test]
fn scalar_operand_effects_cover_remainder_wrapping_and_bitwise_forms() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let obligation = ObligationId::new(1).unwrap();
    let accepted_fact = AcceptedObligationFactIdentity::from_bytes([3; 32]);
    for (kind, family, count) in [
        (
            SelectedInstructionKind::WrappingSubtractI64,
            MachineAlternativeFamily::WrappingSubtractI64,
            3,
        ),
        (
            SelectedInstructionKind::WrappingMultiplyI64,
            MachineAlternativeFamily::WrappingMultiplyI64,
            3,
        ),
        (
            SelectedInstructionKind::BitwiseOrI64,
            MachineAlternativeFamily::BitwiseOrI64,
            3,
        ),
        (
            SelectedInstructionKind::BitwiseNotI64,
            MachineAlternativeFamily::BitwiseNotI64,
            2,
        ),
        (
            SelectedInstructionKind::ExactRemainderU64 {
                obligation,
                accepted_fact,
            },
            MachineAlternativeFamily::ExactRemainderU64,
            3,
        ),
        (
            SelectedInstructionKind::WrappingDivideI64 {
                obligation,
                accepted_fact,
            },
            MachineAlternativeFamily::WrappingDivideI64,
            3,
        ),
        (
            SelectedInstructionKind::WrappingShiftLeftI64,
            MachineAlternativeFamily::WrappingShiftLeftI64,
            3,
        ),
        (
            SelectedInstructionKind::WrappingShiftRightI64,
            MachineAlternativeFamily::WrappingShiftRightI64,
            3,
        ),
        (
            SelectedInstructionKind::WrappingShiftRightU64,
            MachineAlternativeFamily::WrappingShiftRightU64,
            3,
        ),
        (
            SelectedInstructionKind::ExactShiftLeftI64 {
                obligation,
                accepted_fact,
            },
            MachineAlternativeFamily::ExactShiftLeftI64,
            3,
        ),
        (
            SelectedInstructionKind::ExactShiftRightI64 {
                obligation,
                accepted_fact,
            },
            MachineAlternativeFamily::ExactShiftRightI64,
            3,
        ),
        (
            SelectedInstructionKind::ExactShiftRightU64 {
                obligation,
                accepted_fact,
            },
            MachineAlternativeFamily::ExactShiftRightU64,
            3,
        ),
    ] {
        for names in [["x9", "x10", "x11"], ["x9", "x9", "x11"]] {
            let registers = names.map(|name| physical.model().view_named(name).unwrap().id);
            let operands = &registers[..count];
            let key = alternative(family);
            let encoded = encode_aarch64_selected_form(&physical, kind, key, operands).unwrap();
            let decoded = validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key,
                operands,
                encoded.bytes(),
            )
            .unwrap();
            assert_eq!(decoded.footprint().register_reads, operands[..count - 1]);
            assert_eq!(decoded.footprint().register_writes, operands[count - 1..]);
            assert_eq!(
                decoded.footprint().encoded,
                MachineEncodedEffects::fallthrough_v1(
                    (0..count as u16 - 1).collect(),
                    vec![count as u16 - 1],
                )
            );
            rejects_every_bit_flip(&physical, kind, key, operands, encoded.bytes());
        }
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
