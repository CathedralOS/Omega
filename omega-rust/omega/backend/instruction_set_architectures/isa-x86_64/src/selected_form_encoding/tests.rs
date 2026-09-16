//! Selected form encoding tests.

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    SelectedInstructionKind, X86_64SelectedFormEncodingError, encode_x86_64_selected_form,
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form, x86_64_physical_register_model,
};
use crate::selected_form_encoding::decoding::{DecodedInstruction, decode_one};
use optimization_core::AcceptedObligationFactIdentity;
use register_model::validate_physical_register_model;
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::{MachineId, ObligationId};

fn alternative(family: MachineAlternativeFamily, variant: u32) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant }
}

fn exact_add() -> SelectedInstructionKind {
    SelectedInstructionKind::ExactAddI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    }
}

#[test]
fn saturating_subtract_binds_registers_condition_and_flag_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingSubtractU64;
    let key = alternative(MachineAlternativeFamily::SaturatingSubtractU64, 0);
    for left in ["rax", "rcx", "r8", "r15"] {
        for right in ["rax", "rcx", "r8", "r15"] {
            for destination in ["rax", "rcx", "r8", "r15"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands);
                if destination == left || destination == right {
                    assert!(
                        encoded.is_err(),
                        "early-clobber output cannot overlap either input"
                    );
                    continue;
                }
                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 13);
                let decoded = validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(
                    !decoded
                        .footprint()
                        .encoded
                        .implicit_unit_clobbers
                        .is_empty()
                );
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
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
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingAddU64;
    let key = alternative(MachineAlternativeFamily::SaturatingAddU64, 0);
    for left in ["rax", "rcx", "r8", "r15"] {
        for right in ["rax", "rcx", "r8", "r15"] {
            for destination in ["rax", "rcx", "r8", "r15"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands);
                if destination == left || destination == right {
                    assert!(
                        encoded.is_err(),
                        "early-clobber output cannot overlap either input"
                    );
                    continue;
                }
                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 19);
                let decoded = validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(
                    !decoded
                        .footprint()
                        .encoded
                        .implicit_unit_clobbers
                        .is_empty()
                );
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
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
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let operands = ["r8", "r9", "r10"].map(|name| physical.model().view_named(name).unwrap().id);
    // Independently assembled with Apple clang; not derived from this encoder.
    for (kind, family, bytes) in [
        (
            SelectedInstructionKind::SaturatingSubtractU64,
            MachineAlternativeFamily::SaturatingSubtractU64,
            vec![
                0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
            ],
        ),
        (
            SelectedInstructionKind::SaturatingAddU64,
            MachineAlternativeFamily::SaturatingAddU64,
            vec![
                0x4d, 0x89, 0xc2, 0x49, 0xf7, 0xd2, 0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x42, 0xd1, 0x4d,
                0x29, 0xca, 0x49, 0xf7, 0xd2,
            ],
        ),
    ] {
        let key = alternative(family, 0);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), bytes);
        validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &bytes).unwrap();
    }
}

#[test]
fn wrapping_remainder_binds_guard_signed_division_scratch_and_aliases() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::WrappingRemainderI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::WrappingRemainderI64, 0);
    for divisor in ["rax", "rcx", "r9", "r15"] {
        let operands = ["rax", divisor, "rax", "rdx"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes().len(), 17);
        assert_eq!(encoded.footprint().register_reads, operands[..2]);
        assert_eq!(encoded.footprint().register_writes, operands[2..]);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(encoded.footprint().encoded.external_operand_writes, [2, 3]);
        assert!(encoded.footprint().writes_rflags);
        if divisor == "r9" {
            assert_eq!(
                encoded.bytes(),
                [
                    0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49, 0xf7,
                    0xf9, 0x48, 0x89, 0xd0,
                ]
            );
        }
        for byte_position in 0..encoded.bytes().len() {
            let mut changed = encoded.bytes().to_vec();
            changed[byte_position] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &changed,)
                    .is_err(),
                "mutated byte {byte_position}"
            );
        }
        for operand_position in 0..operands.len() {
            let mut changed = operands;
            changed[operand_position] = physical.model().view_named("r8").unwrap().id;
            assert!(
                validate_x86_64_selected_form_encoding(
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
    let invalid =
        ["rax", "rdx", "rax", "rdx"].map(|name| physical.model().view_named(name).unwrap().id);
    assert!(encode_x86_64_selected_form(&physical, kind, key, &invalid).is_err());
}

#[test]
fn wrapping_remainder_guard_skips_overflowing_quotient_and_keeps_dividend_sign() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::WrappingRemainderI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::WrappingRemainderI64, 0);
    let operands =
        ["rax", "r9", "rax", "rdx"].map(|name| physical.model().view_named(name).unwrap().id);
    let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
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
        // Execute only this decoded sequence in the test, with the byte
        // displacement selecting whether CQO and IDIV are reached.
        let mut registers = [0_i64; 16];
        registers[0] = dividend;
        registers[9] = divisor;
        registers[2] = 123;
        let mut equal = false;
        let mut byte_position = 0;
        while byte_position < encoded.bytes().len() {
            let (instruction, length) = decode_one(&encoded.bytes()[byte_position..]).unwrap();
            byte_position += length;
            match instruction {
                DecodedInstruction::Xor {
                    source,
                    destination,
                } => {
                    registers[destination as usize] ^= registers[source as usize];
                }
                DecodedInstruction::CompareSignedImmediate8 {
                    register,
                    immediate,
                } => {
                    equal = registers[register as usize] == i64::from(immediate);
                }
                DecodedInstruction::JumpEqualShort { displacement } => {
                    if equal {
                        byte_position = byte_position
                            .checked_add_signed(displacement as isize)
                            .unwrap();
                    }
                }
                DecodedInstruction::SignExtendDividend => {
                    registers[2] = registers[0] >> 63;
                }
                DecodedInstruction::SignedDivide { divisor } => {
                    let dividend =
                        (i128::from(registers[2]) << 64) | i128::from(registers[0] as u64);
                    let divisor = i128::from(registers[divisor as usize]);
                    let quotient =
                        i64::try_from(dividend / divisor).expect("IDIV quotient must fit");
                    registers[2] = (dividend % divisor) as i64;
                    registers[0] = quotient;
                }
                DecodedInstruction::Move {
                    source,
                    destination,
                } => {
                    registers[destination as usize] = registers[source as usize];
                }
                _ => panic!("unexpected remainder instruction"),
            }
        }
        assert_eq!(registers[0], expected, "{dividend} % {divisor}");
        assert_eq!(registers[9], divisor);
    }
}

#[test]
fn exact_divide_binds_unsigned_opcode_and_registers() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ExactDivideU64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let key = alternative(MachineAlternativeFamily::ExactDivideU64, 0);

    let names = ["rax", "r9", "rax", "rdx"];
    let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
    let expected = vec![0x49, 0xf7, 0xf1];

    let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
    assert_eq!(encoded.bytes(), expected);
    for byte in 0..expected.len() {
        let mut changed = expected.clone();
        changed[byte] ^= 1;
        assert!(
            validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                .is_err()
        );
    }
    for operand in 0..operands.len() {
        let mut changed = operands;
        changed[operand] = physical.model().view_named("r8").unwrap().id;
        assert!(
            validate_x86_64_selected_form_encoding(&physical, kind, key, &changed, &expected)
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

fn signed_saturating_i32_operands(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
) -> [register_model::RegisterViewId; 4] {
    if matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. }) {
        ["rax", "r9", "rax", "rdx"]
    } else {
        ["r9", "r10", "r11", "r12"]
    }
    .map(|name| physical.model().view_named(name).unwrap().id)
}

#[test]
fn signed_saturating_i32_forms_match_independent_assembler_and_reject_mutation() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    // Independently assembled with Apple clang; not derived from this encoder.
    let upper_clamp = [
        0x49, 0xbc, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x4d, 0x39, 0xe3, 0x4d, 0x0f,
        0x4f, 0xdc,
    ];
    let lower_clamp = [
        0x49, 0xbc, 0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0x4d, 0x39, 0xe3, 0x4d, 0x0f,
        0x4c, 0xdc,
    ];
    for (kind, family) in signed_saturating_i32_kinds() {
        let key = alternative(family, 0);
        let operands = signed_saturating_i32_operands(&physical, kind);
        let expected: Vec<u8> = match kind {
            SelectedInstructionKind::SaturatingAddI32 => [0x4d, 0x89, 0xcb, 0x4d, 0x01, 0xd3]
                .into_iter()
                .chain(upper_clamp)
                .chain(lower_clamp)
                .collect(),
            SelectedInstructionKind::SaturatingSubtractI32 => [0x4d, 0x89, 0xcb, 0x4d, 0x29, 0xd3]
                .into_iter()
                .chain(upper_clamp)
                .chain(lower_clamp)
                .collect(),
            _ => vec![
                0x48, 0x99, 0x49, 0xf7, 0xf9, 0x48, 0xba, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x00, 0x00,
                0x00, 0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2,
            ],
        };
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), expected, "{kind:?}");
        assert!(encoded.footprint().writes_rflags);
        if matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. }) {
            // Division keeps the explicit RDX input that CQO discards.
            assert_eq!(
                encoded.footprint().register_reads,
                [operands[0], operands[1], operands[3]]
            );
            assert_eq!(encoded.footprint().register_writes, [operands[2]]);
            assert_eq!(
                encoded.footprint().encoded.external_operand_reads,
                [0, 1, 3]
            );
            assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
        } else {
            assert_eq!(encoded.footprint().register_reads, operands[..2]);
            assert_eq!(encoded.footprint().register_writes, operands[2..]);
            assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
            assert_eq!(encoded.footprint().encoded.external_operand_writes, [2, 3]);
        }
        assert!(
            !encoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty()
        );
        for byte_position in 0..expected.len() {
            let mut changed = expected.clone();
            changed[byte_position] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                    .is_err(),
                "{kind:?} mutated byte {byte_position}"
            );
        }
        for operand_position in 0..operands.len() {
            let mut changed = operands;
            changed[operand_position] = physical.model().view_named("r8").unwrap().id;
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &changed, &expected)
                    .is_err(),
                "{kind:?} substituted operand {operand_position}"
            );
        }
        for (other_kind, other_family) in signed_saturating_i32_kinds() {
            if other_family != family {
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        other_kind,
                        alternative(other_family, 0),
                        &signed_saturating_i32_operands(&physical, other_kind),
                        &expected,
                    )
                    .is_err(),
                    "{kind:?} bytes accepted as {other_kind:?}"
                );
            }
        }
    }
    // Add and subtract accumulate in the result and clamp through the
    // scratch, so neither may alias an input and they may not alias each other.
    for (kind, family) in [
        (
            SelectedInstructionKind::SaturatingAddI32,
            MachineAlternativeFamily::SaturatingAddI32,
        ),
        (
            SelectedInstructionKind::SaturatingSubtractI32,
            MachineAlternativeFamily::SaturatingSubtractI32,
        ),
    ] {
        let key = alternative(family, 0);
        for names in [
            ["r9", "r10", "r9", "r12"],
            ["r9", "r10", "r10", "r12"],
            ["r9", "r10", "r11", "r9"],
            ["r9", "r10", "r11", "r10"],
            ["r9", "r10", "r11", "r11"],
        ] {
            let aliased = names.map(|name| physical.model().view_named(name).unwrap().id);
            assert!(
                encode_x86_64_selected_form(&physical, kind, key, &aliased).is_err(),
                "{kind:?} {names:?}"
            );
        }
    }
    let divide = signed_saturating_i32_kinds()[2].0;
    for names in [
        ["rax", "rdx", "rax", "rdx"],
        ["rcx", "r9", "rax", "rdx"],
        ["rax", "r9", "rcx", "rdx"],
        ["rax", "r9", "rax", "rcx"],
    ] {
        let invalid = names.map(|name| physical.model().view_named(name).unwrap().id);
        assert!(
            encode_x86_64_selected_form(
                &physical,
                divide,
                alternative(MachineAlternativeFamily::SaturatingDivideI32, 0),
                &invalid,
            )
            .is_err(),
            "{names:?}"
        );
    }
}

#[test]
fn signed_saturating_i32_decoded_arithmetic_clamps_every_carrier_edge() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
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
        let operands = signed_saturating_i32_operands(&physical, kind);
        let encoded =
            encode_x86_64_selected_form(&physical, kind, alternative(family, 0), &operands)
                .unwrap();
        let (left_home, right_home, result_home) =
            if matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. }) {
                (0, 9, 0)
            } else {
                (9, 10, 11)
            };
        for (left, right) in inputs {
            let expected = match kind {
                SelectedInstructionKind::SaturatingAddI32 => left.saturating_add(right),
                SelectedInstructionKind::SaturatingSubtractI32 => left.saturating_sub(right),
                _ => left.saturating_div(right),
            };
            let mut registers = [0_i64; 16];
            registers[left_home] = i64::from(left);
            registers[right_home] = i64::from(right);
            let mut greater = false;
            let mut less = false;
            let mut byte_position = 0;
            while byte_position < encoded.bytes().len() {
                let (instruction, length) = decode_one(&encoded.bytes()[byte_position..]).unwrap();
                byte_position += length;
                match instruction {
                    DecodedInstruction::Move {
                        source,
                        destination,
                    } => registers[destination as usize] = registers[source as usize],
                    DecodedInstruction::Add {
                        source,
                        destination,
                    } => registers[destination as usize] += registers[source as usize],
                    DecodedInstruction::Subtract {
                        source,
                        destination,
                    } => registers[destination as usize] -= registers[source as usize],
                    DecodedInstruction::SignExtendDividend => registers[2] = registers[0] >> 63,
                    DecodedInstruction::SignedDivide { divisor } => {
                        let dividend =
                            (i128::from(registers[2]) << 64) | i128::from(registers[0] as u64);
                        let divisor = i128::from(registers[divisor as usize]);
                        registers[2] = (dividend % divisor) as i64;
                        registers[0] =
                            i64::try_from(dividend / divisor).expect("IDIV quotient must fit");
                    }
                    DecodedInstruction::Materialize { destination, value } => {
                        registers[destination as usize] = value as i64;
                    }
                    DecodedInstruction::Compare { left, right } => {
                        greater = registers[left as usize] > registers[right as usize];
                        less = registers[left as usize] < registers[right as usize];
                    }
                    DecodedInstruction::MoveOnGreater {
                        source,
                        destination,
                    } => {
                        if greater {
                            registers[destination as usize] = registers[source as usize];
                        }
                    }
                    DecodedInstruction::MoveOnLess {
                        source,
                        destination,
                    } => {
                        if less {
                            registers[destination as usize] = registers[source as usize];
                        }
                    }
                    other => panic!("unexpected saturating instruction {other:?}"),
                }
            }
            assert_eq!(
                registers[result_home],
                i64::from(expected),
                "{kind:?} {left} {right}"
            );
            assert_eq!(registers[right_home], i64::from(right));
        }
    }
}

#[test]
fn scalar_call_is_explicitly_refused_before_encoding() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    assert_eq!(
        encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::CallScalar {
                callee: MachineId::new(1).unwrap(),
            },
            alternative(MachineAlternativeFamily::ReturnUnit, 0),
            &[],
        ),
        Err(X86_64SelectedFormEncodingError::LayoutDependentForm),
    );
}

#[test]
fn zero_extend_u8_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
        for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU8;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU8, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 4);
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let copy = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
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
                    validate_x86_64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn zero_extend_u32_binds_width_registers_and_is_not_a_copy() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for source in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
        for destination in ["rax", "rcx", "rsi", "rdi", "r8", "r15"] {
            let operands = [
                physical.model().view_named(source).unwrap().id,
                physical.model().view_named(destination).unwrap().id,
            ];
            let kind = SelectedInstructionKind::ZeroExtendU32;
            let key = alternative(MachineAlternativeFamily::ZeroExtendU32, 0);
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_eq!(encoded.bytes().len(), 3);
            // A 32-bit MOV writes the full parent register with zero upper bits.
            assert_eq!(encoded.bytes()[0] & 0xf8, 0x40);
            assert_eq!(encoded.bytes()[1], 0x89);
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            let narrow = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::ZeroExtendU8,
                alternative(MachineAlternativeFamily::ZeroExtendU8, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    narrow.bytes()
                )
                .is_err()
            );
            let copy = encode_x86_64_selected_form(
                &physical,
                SelectedInstructionKind::CopyI64,
                alternative(MachineAlternativeFamily::CopyI64, 0),
                &operands,
            )
            .unwrap();
            assert!(
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    copy.bytes()
                )
                .is_err()
            );
            for byte in 0..3 {
                let mut changed = encoded.bytes().to_vec();
                changed[byte] ^= 1;
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn r12_is_a_valid_rex_extended_sib_index() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let r12 = physical.model().view_named("r12").unwrap().id;
    let rax = physical.model().view_named("rax").unwrap().id;
    let encoded = encode_x86_64_selected_form(
        &physical,
        exact_add(),
        alternative(MachineAlternativeFamily::ExactAddI64, 0),
        &[r12, r12, rax],
    )
    .unwrap();
    assert_eq!(encoded.bytes(), [0x4b, 0x8d, 0x04, 0x24]);
}

#[test]
fn compare_i64_is_canonical_register_cmp_with_exact_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let rbx = physical.model().view_named("rbx").unwrap().id;
    let r8 = physical.model().view_named("r8").unwrap().id;
    let r9 = physical.model().view_named("r9").unwrap().id;
    let alternative = alternative(MachineAlternativeFamily::CompareI64, 0);
    for (operands, expected) in [
        ([rax, rbx], [0x48, 0x39, 0xd8]),
        ([r8, r9], [0x4d, 0x39, 0xc8]),
        ([rax, rax], [0x48, 0x39, 0xc0]),
    ] {
        let encoded = encode_x86_64_selected_form(
            &physical,
            SelectedInstructionKind::CompareI64,
            alternative,
            &operands,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(encoded.footprint().register_reads, operands);
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
    }
}

#[test]
fn compare_i64_immediate_is_canonical_cmp_imm32_with_exact_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let alternative = alternative(MachineAlternativeFamily::CompareI64Immediate, 0);
    for (operand, immediate, expected) in [
        (rax, 0, [0x48, 0x81, 0xf8, 0x00, 0x00, 0x00, 0x00]),
        (rax, 4095, [0x48, 0x81, 0xf8, 0xff, 0x0f, 0x00, 0x00]),
        (r12, 4095, [0x49, 0x81, 0xfc, 0xff, 0x0f, 0x00, 0x00]),
    ] {
        let kind = SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
        };
        let encoded =
            encode_x86_64_selected_form(&physical, kind, alternative, &[operand]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert_eq!(encoded.footprint().register_reads, [operand]);
        assert!(encoded.footprint().register_writes.is_empty());
        assert!(encoded.footprint().writes_rflags);
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0]);
        assert!(
            encoded
                .footprint()
                .encoded
                .external_operand_writes
                .is_empty()
        );
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[operand],
                encoded.bytes()
            )
            .is_ok()
        );
        for byte in 0..encoded.bytes().len() {
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[byte] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(
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
    for immediate in [IntegerValue::Unsigned(4096), IntegerValue::Signed(-1)] {
        let kind = SelectedInstructionKind::CompareI64Immediate { immediate };
        assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).is_err());
    }
}

#[test]
fn scalar_sizes_and_subtraction_alias_partitions_are_exact() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
    let materialize = encode_x86_64_selected_form(
        &physical,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(-1),
        },
        alternative(MachineAlternativeFamily::MaterializeI64, 0),
        &views[..1],
    )
    .unwrap();
    assert_eq!(materialize.bytes().len(), 10);
    for (homes, variant, size) in [
        ([views[0], views[0], views[0]], 0, 3),
        ([views[0], views[1], views[0]], 1, 3),
        ([views[0], views[1], views[1]], 2, 6),
        ([views[0], views[1], views[2]], 3, 6),
    ] {
        let kind = SelectedInstructionKind::ExactSubtractI64 {
            obligation: ObligationId::new(2).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
        };
        let encoded = encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
            &homes,
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), size);
        let mut corrupted = encoded.bytes().to_vec();
        corrupted[1] ^= 1;
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative(MachineAlternativeFamily::ExactSubtractI64, variant),
                &homes,
                &corrupted,
            )
            .is_err()
        );
    }
}

#[test]
fn wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::WrappingAddI64;
    let key = alternative(MachineAlternativeFamily::WrappingAddI64, 0);
    for left in views {
        for right in views {
            for output in views {
                let operands = [left, right, output];
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert!(!encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                let mut changed = encoded.bytes().to_vec();
                changed[1] ^= 1;
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical, kind, key, &operands, &changed
                    )
                    .is_err()
                );
                assert!(
                    encode_x86_64_selected_form(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64, 0),
                        &operands
                    )
                    .is_err()
                );
                assert!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        alternative(MachineAlternativeFamily::ExactAddI64, 0),
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
fn bitwise_operations_preserve_aliases_and_reject_opcode_corruption() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "r9", "r12"].map(|name| physical.model().view_named(name).unwrap().id);
    for (kind, family, expected_opcode) in [
        (
            SelectedInstructionKind::BitwiseAndI64,
            MachineAlternativeFamily::BitwiseAndI64,
            0x21,
        ),
        (
            SelectedInstructionKind::BitwiseXorI64,
            MachineAlternativeFamily::BitwiseXorI64,
            0x31,
        ),
    ] {
        let alternative = alternative(family, 0);
        for left in views {
            for right in views {
                for output in views {
                    let operands = [left, right, output];
                    let encoded =
                        encode_x86_64_selected_form(&physical, kind, alternative, &operands)
                            .unwrap();
                    assert_eq!(
                        encoded.bytes().len(),
                        if output == left || output == right {
                            3
                        } else {
                            6
                        }
                    );
                    assert!(encoded.footprint().writes_rflags);
                    assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
                    assert_eq!(encoded.footprint().encoded.external_operand_writes, [2]);
                    let mut corrupted = encoded.bytes().to_vec();
                    let opcode = corrupted.len() - 2;
                    assert_eq!(corrupted[opcode], expected_opcode);
                    // Another valid bitwise opcode must not satisfy this operation.
                    corrupted[opcode] = if expected_opcode == 0x21 { 0x31 } else { 0x21 };
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &corrupted
                        )
                        .is_err()
                    );
                    corrupted = encoded.bytes().to_vec();
                    let register_byte = corrupted.len() - 1;
                    corrupted[register_byte] ^= 1;
                    assert!(
                        validate_x86_64_selected_form_encoding(
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
fn immediate_lea_uses_declared_size_extremes() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let fact = AcceptedObligationFactIdentity::from_bytes([5; 32]);
    for (base, immediate, size) in [(rax, 0, 4), (rax, 4095, 7), (r12, 0, 5), (r12, 4095, 8)] {
        let kind = SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
            obligation: ObligationId::new(3).unwrap(),
            accepted_fact: fact,
        };
        let encoded = encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactAddI64Immediate, 0),
            &[base, rax],
        )
        .unwrap();
        assert_eq!(encoded.bytes().len(), size);
    }
}

#[test]
fn subtract_immediate_lea_is_negative_canonical_and_flag_transparent() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let r12 = physical.model().view_named("r12").unwrap().id;
    let fact = AcceptedObligationFactIdentity::from_bytes([6; 32]);
    for (base, immediate, expected) in [
        (rax, 0, vec![0x48, 0x8d, 0x40, 0x00]),
        (rax, 128, vec![0x48, 0x8d, 0x40, 0x80]),
        (rax, 129, vec![0x48, 0x8d, 0x80, 0x7f, 0xff, 0xff, 0xff]),
        (
            r12,
            4095,
            vec![0x49, 0x8d, 0x84, 0x24, 0x01, 0xf0, 0xff, 0xff],
        ),
    ] {
        let kind = SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: IntegerValue::Unsigned(immediate),
            obligation: ObligationId::new(4).unwrap(),
            accepted_fact: fact,
        };
        let alternative = alternative(MachineAlternativeFamily::ExactSubtractI64Immediate, 0);
        let encoded =
            encode_x86_64_selected_form(&physical, kind, alternative, &[base, rax]).unwrap();
        assert_eq!(encoded.bytes(), expected);
        assert!(!encoded.footprint().writes_rflags);
        assert!(
            encoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty()
        );
        let mut wrong_sign = expected;
        *wrong_sign.last_mut().unwrap() ^= 0x80;
        assert!(
            validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[base, rax],
                &wrong_sign,
            )
            .is_err()
        );
    }
}

#[test]
fn near_return_is_exact_and_separates_abi_result_custody_from_encoded_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap().id;
    let rbx = physical.model().view_named("rbx").unwrap().id;
    let rsp = physical.model().view_named("rsp").unwrap();
    let rip = physical.model().view_named("rip").unwrap();
    let kind = SelectedInstructionKind::ReturnScalar;
    let alternative = alternative(MachineAlternativeFamily::ReturnScalar, 0);
    let encoded = encode_x86_64_selected_form(&physical, kind, alternative, &[rax]).unwrap();

    assert_eq!(encoded.bytes(), [0xc3]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert_eq!(encoded.footprint().encoded.external_operand_reads, []);
    assert_eq!(encoded.footprint().encoded.external_operand_writes, []);
    assert_eq!(encoded.footprint().encoded.implicit_unit_uses, rsp.units);
    let mut expected_defs = rsp.units.clone();
    expected_defs.extend(&rip.units);
    expected_defs.sort_unstable();
    expected_defs.dedup();
    assert_eq!(
        encoded.footprint().encoded.implicit_unit_defs,
        expected_defs
    );
    assert_eq!(
        encoded.footprint().encoded.memory,
        MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: rsp.id,
            byte_count: 8,
        }
    );
    assert_eq!(
        encoded.footprint().encoded.stack,
        MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: rsp.id,
            byte_count: 8,
        }
    );
    assert_eq!(
        encoded.footprint().encoded.trap,
        MachineEncodedTrapBehavior::MayArchitecturalFaultV1
    );
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnFromActivationStackV1
    );
    assert!(encode_x86_64_selected_form(&physical, kind, alternative, &[rbx]).is_err());
    assert!(
        validate_x86_64_selected_form_encoding(&physical, kind, alternative, &[rax], &[0xc2, 0, 0])
            .is_err()
    );
    assert!(
        validate_x86_64_selected_form_encoding(&physical, kind, alternative, &[rax], &[0xc3, 0xc3])
            .is_err()
    );
}

#[test]
fn unit_return_is_a_distinct_zero_operand_near_return() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::ReturnUnit;
    let return_alternative = alternative(MachineAlternativeFamily::ReturnUnit, 0);
    let encoded = encode_x86_64_selected_form(&physical, kind, return_alternative, &[]).unwrap();

    assert_eq!(encoded.bytes(), [0xc3]);
    assert!(encoded.footprint().register_reads.is_empty());
    assert!(encoded.footprint().register_writes.is_empty());
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ReturnFromActivationStackV1
    );
    assert!(
        encode_x86_64_selected_form(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ReturnScalar, 0),
            &[]
        )
        .is_err()
    );
}

#[test]
fn near_nonzero_branch_has_exact_end_relative_displacement_and_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
    for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
        let encoded =
            encode_x86_64_selected_nonzero_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(&encoded.bytes()[..2], [0x0f, 0x85]);
        assert_eq!(
            i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
            displacement as i32
        );
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
    }
    assert!(
        encode_x86_64_selected_nonzero_branch_form(&physical, alternative, i64::from(i32::MAX) + 1)
            .is_err()
    );
    assert!(
        validate_x86_64_selected_nonzero_branch_form(
            &physical,
            alternative,
            0,
            &[0x0f, 0x84, 0, 0, 0, 0]
        )
        .is_err()
    );
    assert!(
        validate_x86_64_selected_nonzero_branch_form(
            &physical,
            alternative,
            0,
            &[0x0f, 0x85, 0, 0, 0, 0, 0]
        )
        .is_err()
    );
}

#[test]
fn short_nonzero_branch_has_exact_signed_rel8_bounds_and_near_footprint() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);
    let near = encode_x86_64_selected_nonzero_branch_form(&physical, alternative, 0).unwrap();

    for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
        let encoded =
            encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(encoded.bytes(), [0x75, encoded_displacement]);
        assert_eq!(encoded.footprint(), near.footprint());
    }

    for displacement in [-129, 128] {
        assert_eq!(
            encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative, displacement,),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                alternative,
                displacement,
                &[0x75, 0],
            ),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
    }
}

#[test]
fn short_nonzero_branch_validation_rejects_every_noncanonical_form() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let canonical_alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 0);

    for bytes in [&[0x74, 0][..], &[0x75][..], &[0x75, 0, 0][..]] {
        assert_eq!(
            validate_x86_64_selected_short_nonzero_branch_form(
                &physical,
                canonical_alternative,
                0,
                bytes,
            ),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_x86_64_selected_short_nonzero_branch_form(
            &physical,
            canonical_alternative,
            0,
            &[0x75, 1],
        ),
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    );

    let wrong_alternative = alternative(MachineAlternativeFamily::ConditionalBranchNonZero, 1);
    assert_eq!(
        encode_x86_64_selected_short_nonzero_branch_form(&physical, wrong_alternative, 0,),
        Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
    );
    assert_eq!(
        validate_x86_64_selected_short_nonzero_branch_form(
            &physical,
            alternative(MachineAlternativeFamily::ReturnScalar, 0),
            0,
            &[0x75, 0],
        ),
        Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
    );

    let mut forged = x86_64_physical_register_model();
    forged.views[0].name = "forged.rax".into();
    let forged = validate_physical_register_model(forged).unwrap();
    assert_eq!(
        encode_x86_64_selected_short_nonzero_branch_form(&forged, canonical_alternative, 0,),
        Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)
    );
}

#[test]
fn near_u64_less_than_branch_is_exact_jb_rel32_with_flag_control_effects() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rflags = physical.model().view_named("rflags").unwrap();
    let rip = physical.model().view_named("rip").unwrap();
    let alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
    for displacement in [i64::from(i32::MIN), -6, 0, 6, i64::from(i32::MAX)] {
        let encoded =
            encode_x86_64_selected_u64_less_than_branch_form(&physical, alternative, displacement)
                .unwrap();
        assert_eq!(&encoded.bytes()[..2], [0x0f, 0x82]);
        assert_eq!(
            i32::from_le_bytes(encoded.bytes()[2..].try_into().unwrap()),
            displacement as i32
        );
        assert!(encoded.footprint().register_reads.is_empty());
        assert!(encoded.footprint().register_writes.is_empty());
        assert_eq!(
            encoded.footprint().encoded.control,
            MachineEncodedControlEffect::ConditionalRelativeBranchV1
        );
        assert!(rflags.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert!(rip.units.iter().all(|unit| {
            encoded
                .footprint()
                .encoded
                .implicit_unit_uses
                .contains(unit)
        }));
        assert_eq!(encoded.footprint().encoded.implicit_unit_defs, rip.units);
    }
    assert_eq!(
        encode_x86_64_selected_u64_less_than_branch_form(
            &physical,
            alternative,
            i64::from(i32::MAX) + 1,
        ),
        Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI32)
    );
    for bytes in [
        &[0x0f, 0x83, 0, 0, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0, 0, 0][..],
    ] {
        assert_eq!(
            validate_x86_64_selected_u64_less_than_branch_form(&physical, alternative, 0, bytes,),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
}

#[test]
fn short_u64_less_than_branch_is_exact_jb_rel8_and_rejects_near_forms() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let near_alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 0);
    let short_alternative = alternative(MachineAlternativeFamily::ConditionalBranchU64LessThan, 1);
    let near =
        encode_x86_64_selected_u64_less_than_branch_form(&physical, near_alternative, 0).unwrap();
    for (displacement, encoded_displacement) in [(-128, 0x80), (127, 0x7f)] {
        let encoded = encode_x86_64_selected_u64_less_than_branch_form(
            &physical,
            short_alternative,
            displacement,
        )
        .unwrap();
        assert_eq!(encoded.bytes(), [0x72, encoded_displacement]);
        assert_eq!(encoded.footprint(), near.footprint());
    }
    for displacement in [-129, 128] {
        assert_eq!(
            encode_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                displacement,
            ),
            Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
        );
    }
    for bytes in [
        &[0x73, 0][..],
        &[0x72][..],
        &[0x72, 0, 0][..],
        &[0x0f, 0x82, 0, 0, 0, 0][..],
    ] {
        assert_eq!(
            validate_x86_64_selected_u64_less_than_branch_form(
                &physical,
                short_alternative,
                0,
                bytes,
            ),
            Err(X86_64SelectedFormEncodingError::MalformedEncoding)
        );
    }
    assert_eq!(
        validate_x86_64_selected_u64_less_than_branch_form(
            &physical,
            short_alternative,
            0,
            &[0x72, 1],
        ),
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    );
}

#[test]
fn signed_less_than_branch_uses_exact_near_and_short_jl_forms() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let near = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 0);
    let short = alternative(MachineAlternativeFamily::ConditionalBranchI64LessThan, 1);

    let encoded = encode_x86_64_selected_i64_less_than_branch_form(&physical, near, -6).unwrap();
    assert_eq!(encoded.bytes(), [0x0f, 0x8c, 0xfa, 0xff, 0xff, 0xff]);
    assert_eq!(
        encoded.footprint().encoded.control,
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
    );
    let encoded = encode_x86_64_selected_i64_less_than_branch_form(&physical, short, -2).unwrap();
    assert_eq!(encoded.bytes(), [0x7c, 0xfe]);
    assert_eq!(
        validate_x86_64_selected_i64_less_than_branch_form(&physical, short, 0, &[0x72, 0],),
        Err(X86_64SelectedFormEncodingError::MalformedEncoding)
    );
    assert_eq!(
        encode_x86_64_selected_i64_less_than_branch_form(&physical, short, 128),
        Err(X86_64SelectedFormEncodingError::BranchDisplacementOutsideI8)
    );
}
