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
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use optimization_core::AcceptedObligationFactIdentity;
use register_model::validate_physical_register_model;
use selected_instructions::{SaturatingCarrier, SaturatingOperation};
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
    let kind = SelectedInstructionKind::SaturatingSubtract {
        carrier: SaturatingCarrier::U64,
    };
    let key = alternative(
        MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
        0,
    );
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
    let kind = SelectedInstructionKind::SaturatingAdd {
        carrier: SaturatingCarrier::U64,
    };
    let key = alternative(
        MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
        0,
    );
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
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::U64,
            },
            MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
            vec![
                0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
            ],
        ),
        (
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::U64,
            },
            MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
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

const SATURATING_OPERATIONS: [SaturatingOperation; 3] = [
    SaturatingOperation::Add,
    SaturatingOperation::Subtract,
    SaturatingOperation::Divide,
];

fn saturating_kind(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
) -> SelectedInstructionKind {
    match operation {
        SaturatingOperation::Add => SelectedInstructionKind::SaturatingAdd { carrier },
        SaturatingOperation::Subtract => SelectedInstructionKind::SaturatingSubtract { carrier },
        SaturatingOperation::Divide => SelectedInstructionKind::SaturatingDivide {
            carrier,
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        },
    }
}

fn saturating_key(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
) -> MachineAlternativeKey {
    alternative(
        match operation {
            SaturatingOperation::Add => MachineAlternativeFamily::SaturatingAdd(carrier),
            SaturatingOperation::Subtract => MachineAlternativeFamily::SaturatingSubtract(carrier),
            SaturatingOperation::Divide => MachineAlternativeFamily::SaturatingDivide(carrier),
        },
        0,
    )
}

/// One register assignment per operand layout: the low assignment matches
/// the clang-assembled bytes below, the high assignment exercises REX.B/REX.R
/// on r8-r15. Division is pinned to `[rax, divisor, rax, rdx]`.
fn saturating_operand_names(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    high: bool,
) -> Vec<&'static str> {
    let form = SaturatingForm::of(operation, carrier);
    if form.is_division() {
        vec!["rax", if high { "r9" } else { "rsi" }, "rax", "rdx"]
    } else if form.operand_count() == 4 {
        if high {
            vec!["r8", "r11", "r10", "r9"]
        } else {
            vec!["rdi", "rsi", "rax", "rcx"]
        }
    } else if high {
        vec!["r8", "r9", "r10"]
    } else {
        vec!["rdi", "rsi", "rax"]
    }
}

fn saturating_operands(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    names: &[&str],
) -> Vec<register_model::RegisterViewId> {
    names
        .iter()
        .map(|name| physical.model().view_named(name).unwrap().id)
        .collect()
}

/// Every saturating form is checked byte for byte against an independent
/// assembly (Apple clang) for at least one low and one r8-r15 assignment;
/// forms that differ from a clang-assembled sibling only in the MOVABS bound
/// immediate are listed with that immediate substituted and say so.
fn independently_assembled_saturating_forms() -> Vec<(
    SaturatingOperation,
    SaturatingCarrier,
    Vec<&'static str>,
    Vec<u8>,
)> {
    let signed_narrow_low = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1, 0x48, 0xb9]);
        bytes.extend(minimum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4c, 0xc1]);
        bytes
    };
    let signed_narrow_high = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
        let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x49, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1, 0x49, 0xb9]);
        bytes.extend(minimum);
        bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4c, 0xd1]);
        bytes
    };
    let unsigned_narrow_low = |maximum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x01, 0xf0, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1]);
        bytes
    };
    let signed_narrow_divide_low = |maximum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x99, 0x48, 0xf7, 0xfe, 0x48, 0xba];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2]);
        bytes
    };
    let i8_maximum = [0x7f, 0, 0, 0, 0, 0, 0, 0];
    let i8_minimum = [0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let i16_maximum = [0xff, 0x7f, 0, 0, 0, 0, 0, 0];
    let i16_minimum = [0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let i32_maximum = [0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0];
    let i32_minimum = [0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff];
    let low4 = vec!["rdi", "rsi", "rax", "rcx"];
    let high4 = vec!["r8", "r11", "r10", "r9"];
    let low3 = vec!["rdi", "rsi", "rax"];
    let high3 = vec!["r8", "r9", "r10"];
    let low_divide = vec!["rax", "rsi", "rax", "rdx"];
    let high_divide = vec!["rax", "r9", "rax", "rdx"];
    let i64_overflow_select_low = |arithmetic: u8| {
        vec![
            0x48, 0x89, 0xf9, 0x48, 0xf7, 0xd1, 0x48, 0xc1, 0xf9, 0x3f, 0x48, 0x0f, 0xba, 0xf9,
            0x3f, 0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0x0f, 0x40, 0xc1,
        ]
    };
    let i64_overflow_select_high = |arithmetic: u8| {
        vec![
            0x4d, 0x89, 0xc1, 0x49, 0xf7, 0xd1, 0x49, 0xc1, 0xf9, 0x3f, 0x49, 0x0f, 0xba, 0xf9,
            0x3f, 0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x4d, 0x0f, 0x40, 0xd1,
        ]
    };
    let unsigned_subtract_low = vec![
        0x48, 0x39, 0xf7, 0x48, 0x89, 0xf8, 0x48, 0x0f, 0x42, 0xc6, 0x48, 0x29, 0xf0,
    ];
    let unsigned_subtract_high = vec![
        0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
    ];
    use SaturatingCarrier::*;
    use SaturatingOperation::*;
    vec![
        // clang: mov rax, rdi; add rax, rsi; movabs rcx, 0x7f; cmp rax, rcx;
        // cmovg rax, rcx; movabs rcx, -0x80; cmp rax, rcx; cmovl rax, rcx.
        (
            Add,
            I8,
            low4.clone(),
            signed_narrow_low(i8_maximum, i8_minimum, 0x01),
        ),
        // The i8 clang bytes with the i16 bound immediates substituted.
        (
            Add,
            I16,
            low4.clone(),
            signed_narrow_low(i16_maximum, i16_minimum, 0x01),
        ),
        (
            Add,
            I32,
            low4.clone(),
            signed_narrow_low(i32_maximum, i32_minimum, 0x01),
        ),
        // The i8 clang bytes with `sub` (0x29) in place of `add` (0x01).
        (
            Subtract,
            I8,
            low4.clone(),
            signed_narrow_low(i8_maximum, i8_minimum, 0x29),
        ),
        (
            Subtract,
            I16,
            low4.clone(),
            signed_narrow_low(i16_maximum, i16_minimum, 0x29),
        ),
        (
            Subtract,
            I32,
            low4.clone(),
            signed_narrow_low(i32_maximum, i32_minimum, 0x29),
        ),
        // The i64 clang high assignment (r8, r11, r10, r9) with the narrow
        // clamp sequence in place of the overflow select.
        (
            Add,
            I8,
            high4.clone(),
            signed_narrow_high(i8_maximum, i8_minimum, 0x01),
        ),
        (
            Subtract,
            I16,
            high4.clone(),
            signed_narrow_high(i16_maximum, i16_minimum, 0x29),
        ),
        // clang: mov rax, rdi; add rax, rsi; movabs rcx, 0xffffffff; cmp rax,
        // rcx; cmovg rax, rcx.
        (
            Add,
            U32,
            low4.clone(),
            unsigned_narrow_low([0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]),
        ),
        (
            Add,
            U8,
            low4.clone(),
            unsigned_narrow_low([0xff, 0, 0, 0, 0, 0, 0, 0]),
        ),
        (
            Add,
            U16,
            low4.clone(),
            unsigned_narrow_low([0xff, 0xff, 0, 0, 0, 0, 0, 0]),
        ),
        // The clang high assignment of the signed narrow add with the u16
        // bound and no lower clamp.
        (Add, U16, high4.clone(), {
            let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x01, 0xda, 0x49, 0xb9];
            bytes.extend([0xff, 0xff, 0, 0, 0, 0, 0, 0]);
            bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1]);
            bytes
        }),
        // clang: mov rcx, rdi; not rcx; sar rcx, 63; btc rcx, 63; mov rax,
        // rdi; add rax, rsi; cmovo rax, rcx.
        (Add, I64, low4.clone(), i64_overflow_select_low(0x01)),
        (Add, I64, high4.clone(), i64_overflow_select_high(0x01)),
        // clang: the i64 add with sub rax, rsi.
        (Subtract, I64, low4.clone(), i64_overflow_select_low(0x29)),
        (Subtract, I64, high4.clone(), i64_overflow_select_high(0x29)),
        // clang: cmp rdi, rsi; mov rax, rdi; cmovb rax, rsi; sub rax, rsi
        // for every unsigned carrier.
        (Subtract, U8, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U16, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U32, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U64, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U32, high3.clone(), unsigned_subtract_high.clone()),
        (Subtract, U64, high3.clone(), unsigned_subtract_high),
        // clang: the u64 complement/borrow-select add on r8, r9, r10.
        (
            Add,
            U64,
            high3,
            vec![
                0x4d, 0x89, 0xc2, 0x49, 0xf7, 0xd2, 0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x42, 0xd1, 0x4d,
                0x29, 0xca, 0x49, 0xf7, 0xd2,
            ],
        ),
        (
            Add,
            U64,
            low3,
            vec![
                0x48, 0x89, 0xf8, 0x48, 0xf7, 0xd0, 0x48, 0x39, 0xf0, 0x48, 0x0f, 0x42, 0xc6, 0x48,
                0x29, 0xf0, 0x48, 0xf7, 0xd0,
            ],
        ),
        // clang: div rsi / div r9 for every unsigned carrier.
        (Divide, U8, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
        (Divide, U64, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
        (Divide, U16, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
        (Divide, U32, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
        // clang: cqo; idiv rsi; movabs rdx, 0x7fff; cmp rax, rdx; cmovg rax, rdx.
        (
            Divide,
            I16,
            low_divide.clone(),
            signed_narrow_divide_low(i16_maximum),
        ),
        (
            Divide,
            I8,
            low_divide.clone(),
            signed_narrow_divide_low(i8_maximum),
        ),
        (
            Divide,
            I32,
            low_divide.clone(),
            signed_narrow_divide_low(i32_maximum),
        ),
        // clang: cqo; idiv r9; movabs rdx, 0x7fffffff; cmp rax, rdx; cmovg rax, rdx.
        (
            Divide,
            I32,
            high_divide.clone(),
            vec![
                0x48, 0x99, 0x49, 0xf7, 0xf9, 0x48, 0xba, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x00, 0x00,
                0x00, 0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2,
            ],
        ),
        // clang: cmp rsi, -1; sbb rdx, rdx; or rdx, rax; neg rdx; lea rdx,
        // [rax + 1]; cmovo rax, rdx; cqo; idiv rsi.
        (
            Divide,
            I64,
            low_divide,
            vec![
                0x48, 0x83, 0xfe, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7, 0xda, 0x48,
                0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x48, 0xf7, 0xfe,
            ],
        ),
        (
            Divide,
            I64,
            high_divide,
            vec![
                0x49, 0x83, 0xf9, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7, 0xda, 0x48,
                0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x49, 0xf7, 0xf9,
            ],
        ),
    ]
}

#[test]
fn saturating_forms_match_independent_assembler_for_every_carrier() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut covered = Vec::new();
    for (operation, carrier, names, expected) in independently_assembled_saturating_forms() {
        let kind = saturating_kind(operation, carrier);
        let key = saturating_key(operation, carrier);
        let operands = saturating_operands(&physical, &names);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(
            encoded.bytes(),
            expected,
            "{operation:?} {carrier:?} {names:?}"
        );
        assert_eq!(
            expected.len(),
            usize::from(SaturatingForm::of(operation, carrier).byte_count()),
            "{operation:?} {carrier:?}"
        );
        validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &expected).unwrap();
        let high = names.iter().any(|name| {
            name[1..]
                .chars()
                .all(|character| character.is_ascii_digit())
        });
        covered.push((
            operation,
            carrier,
            SaturatingForm::of(operation, carrier),
            high,
        ));
    }
    // Every carrier of every operation has an independently assembled form,
    // and every realization shape has a low and an r8-r15 assignment.
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            assert!(
                covered
                    .iter()
                    .any(|(o, c, ..)| (*o, *c) == (operation, carrier)),
                "{operation:?} {carrier:?} has no independently assembled bytes"
            );
        }
    }
    for form in [
        SaturatingForm::AddU64,
        SaturatingForm::SubtractUnsigned,
        SaturatingForm::DivideUnsigned,
        SaturatingForm::ClampSignedNarrow,
        SaturatingForm::ClampUnsignedNarrow,
        SaturatingForm::OverflowSelectI64,
        SaturatingForm::DivideSignedNarrow,
        SaturatingForm::DivideI64,
    ] {
        for high in [false, true] {
            assert!(
                covered.iter().any(|(_, _, f, h)| (*f, *h) == (form, high)),
                "{form:?} high={high} has no independently assembled bytes"
            );
        }
    }
}

#[test]
fn saturating_forms_reject_every_mutation_substitution_and_sibling_carrier() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let substitute = physical.model().view_named("r14").unwrap().id;
    let mut canonical = Vec::new();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            for high in [false, true] {
                let kind = saturating_kind(operation, carrier);
                let key = saturating_key(operation, carrier);
                let operands = saturating_operands(
                    &physical,
                    &saturating_operand_names(operation, carrier, high),
                );
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                let form = SaturatingForm::of(operation, carrier);
                assert_eq!(encoded.bytes().len(), usize::from(form.byte_count()));
                let (reads, writes) = form.operand_reads_and_writes();
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, reads);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, writes);
                assert_eq!(
                    encoded.footprint().register_reads,
                    reads
                        .iter()
                        .map(|&position| operands[usize::from(position)])
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    encoded.footprint().register_writes,
                    writes
                        .iter()
                        .map(|&position| operands[usize::from(position)])
                        .collect::<Vec<_>>()
                );
                let rdx = physical.model().view_named("rdx").unwrap().units.clone();
                let clobbers = &encoded.footprint().encoded.implicit_unit_clobbers;
                assert!(!clobbers.is_empty());
                assert_eq!(
                    rdx.iter().all(|unit| clobbers.contains(unit)),
                    form.is_division(),
                    "{operation:?} {carrier:?} RDX clobber"
                );
                assert_eq!(
                    encoded.footprint().encoded.trap
                        == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    form.is_division()
                );
                for byte_position in 0..encoded.bytes().len() {
                    for bit in 0..8 {
                        let mut changed = encoded.bytes().to_vec();
                        changed[byte_position] ^= 1 << bit;
                        assert!(
                            validate_x86_64_selected_form_encoding(
                                &physical, kind, key, &operands, &changed
                            )
                            .is_err(),
                            "{operation:?} {carrier:?} high={high} byte {byte_position} bit {bit}"
                        );
                    }
                }
                for operand_position in 0..operands.len() {
                    let mut changed = operands.clone();
                    changed[operand_position] = substitute;
                    assert!(
                        validate_x86_64_selected_form_encoding(
                            &physical,
                            kind,
                            key,
                            &changed,
                            encoded.bytes()
                        )
                        .is_err(),
                        "{operation:?} {carrier:?} substituted operand {operand_position}"
                    );
                }
                if !high {
                    canonical.push((operation, carrier, kind, key, operands, encoded));
                }
            }
        }
    }
    // Bytes replayed under another carrier's kind (of any operation) are
    // rejected wherever that carrier's realization differs. The unsigned
    // subtract and unsigned divide are the only forms shared verbatim across
    // carriers, so those are the only replays that can be accepted here.
    for (operation, carrier, _, _, _, encoded) in &canonical {
        for (other_operation, other_carrier, other_kind, other_key, other_operands, other) in
            &canonical
        {
            if (operation, carrier) == (other_operation, other_carrier) {
                continue;
            }
            let accepted = validate_x86_64_selected_form_encoding(
                &physical,
                *other_kind,
                *other_key,
                other_operands,
                encoded.bytes(),
            )
            .is_ok();
            let shared = operation == other_operation
                && encoded.bytes() == other.bytes()
                && matches!(
                    SaturatingForm::of(*operation, *carrier),
                    SaturatingForm::SubtractUnsigned | SaturatingForm::DivideUnsigned
                );
            assert_eq!(
                accepted, shared,
                "{operation:?} {carrier:?} bytes replayed as {other_operation:?} {other_carrier:?}"
            );
        }
    }
}

#[test]
fn saturating_forms_pin_outputs_away_from_inputs_and_division_to_rax_rdx() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            let kind = saturating_kind(operation, carrier);
            let key = saturating_key(operation, carrier);
            let form = SaturatingForm::of(operation, carrier);
            let invalid: Vec<Vec<&str>> = if form.is_division() {
                vec![
                    vec!["rax", "rdx", "rax", "rdx"],
                    vec!["rcx", "r9", "rax", "rdx"],
                    vec!["rax", "r9", "rcx", "rdx"],
                    vec!["rax", "r9", "rax", "rcx"],
                ]
            } else if form.operand_count() == 4 {
                // The result accumulates and the scratch holds the bound (or
                // the saturated value) while both inputs are still live.
                vec![
                    vec!["r9", "r10", "r9", "r12"],
                    vec!["r9", "r10", "r10", "r12"],
                    vec!["r9", "r10", "r11", "r9"],
                    vec!["r9", "r10", "r11", "r10"],
                    vec!["r9", "r10", "r11", "r11"],
                ]
            } else {
                vec![vec!["r9", "r10", "r9"], vec!["r9", "r10", "r10"]]
            };
            for names in invalid {
                let operands = saturating_operands(&physical, &names);
                assert!(
                    encode_x86_64_selected_form(&physical, kind, key, &operands).is_err(),
                    "{operation:?} {carrier:?} {names:?}"
                );
            }
            let wrong_count = saturating_operand_names(operation, carrier, false);
            let mut too_few = saturating_operands(&physical, &wrong_count);
            too_few.pop();
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &too_few),
                Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
            );
        }
    }
}

/// The RFLAGS bits the saturating forms read. `None` is an architecturally
/// undefined flag (after SAR, BTC, or a division), so a form that consumed it
/// would panic here instead of passing by accident.
#[derive(Default, Clone, Copy)]
struct SaturatingFlags {
    overflow: Option<bool>,
    carry: Option<bool>,
    greater: Option<bool>,
    less: Option<bool>,
}

impl SaturatingFlags {
    /// Flags of an exact signed result compared with zero, plus the
    /// explicit overflow and carry the instruction computed.
    fn arithmetic(exact: i128, overflow: bool, carry: bool) -> Self {
        Self {
            overflow: Some(overflow),
            carry: Some(carry),
            greater: Some(exact > 0),
            less: Some(exact < 0),
        }
    }

    fn undefined() -> Self {
        Self::default()
    }
}

/// Executes one decoded saturating form over a 16-register file, returning
/// the register file. Register values are 64-bit patterns.
fn execute_saturating(bytes: &[u8], mut registers: [u64; 16]) -> [u64; 16] {
    let mut flags = SaturatingFlags::default();
    let signed = |value: u64| i128::from(value as i64);
    let subtract_flags = |left: u64, right: u64| {
        let exact = signed(left) - signed(right);
        SaturatingFlags::arithmetic(exact, i64::try_from(exact).is_err(), left < right)
    };
    let mut byte_position = 0;
    while byte_position < bytes.len() {
        let (instruction, length) = decode_one(&bytes[byte_position..]).unwrap();
        byte_position += length;
        match instruction {
            DecodedInstruction::Move {
                source,
                destination,
            } => registers[usize::from(destination)] = registers[usize::from(source)],
            DecodedInstruction::Materialize { destination, value } => {
                registers[usize::from(destination)] = value;
            }
            DecodedInstruction::Add {
                source,
                destination,
            } => {
                let (left, right) = (
                    registers[usize::from(destination)],
                    registers[usize::from(source)],
                );
                let exact = signed(left) + signed(right);
                registers[usize::from(destination)] = left.wrapping_add(right);
                flags = SaturatingFlags::arithmetic(
                    exact,
                    i64::try_from(exact).is_err(),
                    left.checked_add(right).is_none(),
                );
            }
            DecodedInstruction::Subtract {
                source,
                destination,
            } => {
                let (left, right) = (
                    registers[usize::from(destination)],
                    registers[usize::from(source)],
                );
                registers[usize::from(destination)] = left.wrapping_sub(right);
                flags = subtract_flags(left, right);
            }
            DecodedInstruction::Compare { left, right } => {
                flags = subtract_flags(registers[usize::from(left)], registers[usize::from(right)]);
            }
            DecodedInstruction::CompareSignedImmediate8 {
                register,
                immediate,
            } => {
                flags = subtract_flags(registers[usize::from(register)], immediate as i64 as u64);
            }
            DecodedInstruction::Complement { destination } => {
                registers[usize::from(destination)] = !registers[usize::from(destination)];
            }
            DecodedInstruction::Negate { destination } => {
                let value = registers[usize::from(destination)];
                registers[usize::from(destination)] = value.wrapping_neg();
                flags = subtract_flags(0, value);
            }
            DecodedInstruction::SubtractWithBorrow {
                source,
                destination,
            } => {
                let (left, right) = (
                    registers[usize::from(destination)],
                    registers[usize::from(source)],
                );
                let borrow = u64::from(flags.carry.expect("SBB reads a defined CF"));
                let exact = signed(left) - signed(right) - i128::from(borrow);
                registers[usize::from(destination)] = left.wrapping_sub(right).wrapping_sub(borrow);
                flags = SaturatingFlags::arithmetic(
                    exact,
                    i64::try_from(exact).is_err(),
                    u128::from(left) < u128::from(right) + u128::from(borrow),
                );
            }
            DecodedInstruction::Or {
                source,
                destination,
            } => {
                registers[usize::from(destination)] |= registers[usize::from(source)];
                flags = SaturatingFlags::arithmetic(
                    signed(registers[usize::from(destination)]),
                    false,
                    false,
                );
            }
            DecodedInstruction::ArithmeticShiftRight63 { destination } => {
                let value = registers[usize::from(destination)];
                registers[usize::from(destination)] = ((value as i64) >> 63) as u64;
                flags = SaturatingFlags {
                    carry: Some((value >> 62) & 1 == 1),
                    ..SaturatingFlags::undefined()
                };
            }
            DecodedInstruction::ComplementBit63 { destination } => {
                let value = registers[usize::from(destination)];
                registers[usize::from(destination)] = value ^ (1 << 63);
                flags = SaturatingFlags {
                    carry: Some(value >> 63 == 1),
                    ..SaturatingFlags::undefined()
                };
            }
            DecodedInstruction::Lea {
                destination,
                base,
                index: None,
                displacement,
            } => {
                registers[usize::from(destination)] =
                    registers[usize::from(base)].wrapping_add(displacement as i64 as u64);
            }
            DecodedInstruction::MoveOnOverflow {
                source,
                destination,
            } => {
                if flags.overflow.expect("CMOVO reads a defined OF") {
                    registers[usize::from(destination)] = registers[usize::from(source)];
                }
            }
            DecodedInstruction::MoveOnBorrow {
                source,
                destination,
            } => {
                if flags.carry.expect("CMOVB reads a defined CF") {
                    registers[usize::from(destination)] = registers[usize::from(source)];
                }
            }
            DecodedInstruction::MoveOnGreater {
                source,
                destination,
            } => {
                if flags.greater.expect("CMOVG reads defined ZF/SF/OF") {
                    registers[usize::from(destination)] = registers[usize::from(source)];
                }
            }
            DecodedInstruction::MoveOnLess {
                source,
                destination,
            } => {
                if flags.less.expect("CMOVL reads defined SF/OF") {
                    registers[usize::from(destination)] = registers[usize::from(source)];
                }
            }
            DecodedInstruction::SignExtendDividend => {
                registers[2] = ((registers[0] as i64) >> 63) as u64;
            }
            DecodedInstruction::SignedDivide { divisor } => {
                let dividend = (i128::from(registers[2] as i64) << 64) | i128::from(registers[0]);
                let divisor = i128::from(registers[usize::from(divisor)] as i64);
                assert_ne!(divisor, 0, "IDIV by zero faults");
                let quotient = i64::try_from(dividend / divisor)
                    .expect("IDIV faults when the quotient does not fit");
                registers[2] = (dividend % divisor) as i64 as u64;
                registers[0] = quotient as u64;
                flags = SaturatingFlags::undefined();
            }
            DecodedInstruction::UnsignedDivide { divisor } => {
                let dividend = (u128::from(registers[2]) << 64) | u128::from(registers[0]);
                let divisor = u128::from(registers[usize::from(divisor)]);
                assert_ne!(divisor, 0, "DIV by zero faults");
                let quotient = u64::try_from(dividend / divisor)
                    .expect("DIV faults when the quotient does not fit");
                registers[2] = (dividend % divisor) as u64;
                registers[0] = quotient;
                flags = SaturatingFlags::undefined();
            }
            other => panic!("unexpected saturating instruction {other:?}"),
        }
    }
    registers
}

/// Rust's `saturating_*` for the carrier's concrete type, as the normalized
/// 64-bit register pattern the scalar transport expects.
fn saturating_reference(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    left: i128,
    right: i128,
) -> u64 {
    macro_rules! reference {
        ($type:ty) => {{
            let left = <$type>::try_from(left).unwrap();
            let right = <$type>::try_from(right).unwrap();
            let result = match operation {
                SaturatingOperation::Add => left.saturating_add(right),
                SaturatingOperation::Subtract => left.saturating_sub(right),
                SaturatingOperation::Divide => left.saturating_div(right),
            };
            // Truncating the lossless i128 widening to 64 bits yields the
            // sign- or zero-normalized register pattern for every carrier.
            i128::from(result) as u64
        }};
    }
    match carrier {
        SaturatingCarrier::I8 => reference!(i8),
        SaturatingCarrier::I16 => reference!(i16),
        SaturatingCarrier::I32 => reference!(i32),
        SaturatingCarrier::I64 => reference!(i64),
        SaturatingCarrier::U8 => reference!(u8),
        SaturatingCarrier::U16 => reference!(u16),
        SaturatingCarrier::U32 => reference!(u32),
        SaturatingCarrier::U64 => reference!(u64),
    }
}

#[test]
fn saturating_decoded_arithmetic_clamps_every_carrier_edge() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            let minimum = i128::from(carrier.minimum_bits() as i64);
            let maximum = i128::from(carrier.maximum_bits());
            let in_range = |value: i128| (minimum..=maximum).contains(&value);
            // MAX + 1, MIN - 1, MAX + MAX, MIN + MIN, 0 - 1, MIN / -1, and
            // in-range controls; pairs outside the carrier are skipped, as is
            // a zero divisor, which no carrier licenses.
            let inputs = [
                (maximum, 1),
                (minimum, -1),
                (maximum, maximum),
                (minimum, minimum),
                (minimum, 1),
                (maximum, -1),
                (minimum, maximum),
                (maximum, minimum),
                (0, 1),
                (0, -1),
                (1, 2),
                (2, 7),
                (7, 2),
                (-7, 2),
                (7, -2),
                (maximum, 0),
                (maximum, 2),
                (minimum, 2),
                (1, maximum),
                (maximum / 2, maximum / 2 + 1),
            ];
            let mut checked = 0;
            for high in [false, true] {
                let names = saturating_operand_names(operation, carrier, high);
                let operands = saturating_operands(&physical, &names);
                let encoded = encode_x86_64_selected_form(
                    &physical,
                    saturating_kind(operation, carrier),
                    saturating_key(operation, carrier),
                    &operands,
                )
                .unwrap();
                let registers =
                    crate::selected_form_encoding::request_validation::resolve_registers(
                        &physical, &operands,
                    )
                    .unwrap();
                let (left_home, right_home, result_home) = (
                    usize::from(registers[0]),
                    usize::from(registers[1]),
                    usize::from(registers[2]),
                );
                for (left, right) in inputs {
                    if !in_range(left)
                        || !in_range(right)
                        || (operation == SaturatingOperation::Divide && right == 0)
                    {
                        continue;
                    }
                    let mut file = [0_u64; 16];
                    file[left_home] = left as u64;
                    file[right_home] = right as u64;
                    let file = execute_saturating(encoded.bytes(), file);
                    assert_eq!(
                        file[result_home],
                        saturating_reference(operation, carrier, left, right),
                        "{operation:?} {carrier:?} high={high} {left} {right}"
                    );
                    assert_eq!(file[right_home], right as u64, "{operation:?} {carrier:?}");
                    checked += 1;
                }
            }
            assert!(
                checked >= 16,
                "{operation:?} {carrier:?} checked only {checked} pairs"
            );
        }
    }
}

#[test]
fn saturating_i64_divide_guard_only_rewrites_the_faulting_dividend() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let operands = saturating_operands(&physical, &["rax", "rsi", "rax", "rdx"]);
    let encoded = encode_x86_64_selected_form(
        &physical,
        saturating_kind(SaturatingOperation::Divide, SaturatingCarrier::I64),
        saturating_key(SaturatingOperation::Divide, SaturatingCarrier::I64),
        &operands,
    )
    .unwrap();
    for (dividend, divisor, quotient, remainder) in [
        (i64::MIN, -1, i64::MAX, 0),
        (i64::MIN, 1, i64::MIN, 0),
        (i64::MIN, 2, i64::MIN / 2, 0),
        (i64::MAX, -1, -i64::MAX, 0),
        (i64::MIN + 1, -1, i64::MAX, 0),
        (-1, -1, 1, 0),
        (7, -2, -3, 1),
        (-7, 2, -3, -1),
    ] {
        let mut file = [0_u64; 16];
        file[0] = dividend as u64;
        file[6] = divisor as u64;
        let file = execute_saturating(encoded.bytes(), file);
        assert_eq!(file[0] as i64, quotient, "{dividend} / {divisor}");
        assert_eq!(file[2] as i64, remainder, "{dividend} % {divisor}");
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
fn exact_multiply_alias_partitions_encode_and_decode() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let views = ["rax", "rbx", "rcx"].map(|name| physical.model().view_named(name).unwrap().id);
    let kind = SelectedInstructionKind::ExactMultiplyI64 {
        obligation: ObligationId::new(2).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([4; 32]),
    };
    for (homes, variant, size) in [
        ([views[0], views[0], views[0]], 0, 4),
        ([views[0], views[1], views[0]], 1, 4),
        ([views[0], views[1], views[1]], 2, 4),
        ([views[0], views[1], views[2]], 3, 7),
    ] {
        let key = alternative(MachineAlternativeFamily::ExactMultiplyI64, variant);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &homes).unwrap();
        assert_eq!(encoded.bytes().len(), size);
        let decoded =
            validate_x86_64_selected_form_encoding(&physical, kind, key, &homes, encoded.bytes())
                .unwrap();
        // Even the fully-aliased `imul r, r` reads both logical operands: the
        // multiply consumes its in-place destination input.
        assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
        assert!(
            !decoded
                .footprint()
                .encoded
                .implicit_unit_clobbers
                .is_empty(),
            "imul clobbers rflags"
        );
        for byte in 0..encoded.bytes().len() {
            let mut corrupted = encoded.bytes().to_vec();
            corrupted[byte] ^= 1;
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &homes, &corrupted,)
                    .is_err(),
                "changed byte {byte}"
            );
        }
    }
    // A mismatched variant cannot validate the alias partition it skips.
    let homes = [views[0], views[1], views[2]];
    let encoded = encode_x86_64_selected_form(
        &physical,
        kind,
        alternative(MachineAlternativeFamily::ExactMultiplyI64, 3),
        &homes,
    )
    .unwrap();
    assert!(
        validate_x86_64_selected_form_encoding(
            &physical,
            kind,
            alternative(MachineAlternativeFamily::ExactMultiplyI64, 0),
            &homes,
            encoded.bytes(),
        )
        .is_err()
    );
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
