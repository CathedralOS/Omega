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
    validate_x86_64_selected_u64_less_than_branch_form,
};
use crate::selected_form_encoding::decoding::{DecodedInstruction, decode_one};
use crate::selected_form_encoding::saturating_forms::SaturatingForm;
use crate::selected_form_encoding::trapping_forms::{TrappingRow, TrappingShape};
use crate::x86_64_physical_register_model;
use optimization_core::AcceptedObligationFactIdentity;
use register_model::{
    RegisterViewId, ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use selected_instructions::{
    MachineSemanticKind, SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
};
use semantic_vocabulary::{IntegerSign, IntegerValue};
use semantic_vocabulary::{MachineId, ObligationId};

mod control;
mod saturating;
mod scalar;
mod trapping;

fn alternative(family: MachineAlternativeFamily, variant: u32) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant }
}

fn exact_add() -> SelectedInstructionKind {
    SelectedInstructionKind::ExactAddI64 {
        obligation: ObligationId::new(1).unwrap(),
        accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
    }
}

fn assert_every_byte_change_rejected(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    key: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
    context: &str,
) {
    for byte_position in 0..bytes.len() {
        let mut changed = bytes.to_vec();
        changed[byte_position] ^= 1;
        assert!(
            validate_x86_64_selected_form_encoding(physical, kind, key, operands, &changed)
                .is_err(),
            "{context} byte {byte_position}"
        );
    }
}

fn assert_every_bit_change_rejected(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    key: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
    context: &str,
) {
    for byte_position in 0..bytes.len() {
        for bit in 0..8 {
            let mut changed = bytes.to_vec();
            changed[byte_position] ^= 1 << bit;
            assert!(
                validate_x86_64_selected_form_encoding(physical, kind, key, operands, &changed)
                    .is_err(),
                "{context} byte {byte_position} bit {bit}"
            );
        }
    }
}

fn assert_every_operand_substitution_rejected(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    key: MachineAlternativeKey,
    operands: &[RegisterViewId],
    substitute: RegisterViewId,
    bytes: &[u8],
    context: &str,
) {
    for operand_position in 0..operands.len() {
        let mut changed = operands.to_vec();
        changed[operand_position] = substitute;
        assert!(
            validate_x86_64_selected_form_encoding(physical, kind, key, &changed, bytes).is_err(),
            "{context} substituted operand {operand_position}"
        );
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
                assert_every_byte_change_rejected(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                    "changed",
                );
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
                assert_every_byte_change_rejected(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                    "changed",
                );
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
fn variable_shift_bytes_match_independent_assembler() {
    // D3 /digit shifts r/m64 by CL. The digit is the whole difference
    // between the three families, and encoder and decoder previously agreed
    // on /6 for the logical right shift, so a round trip could not catch it:
    // /6 is an undocumented SAL alias of /4 and shifts LEFT. SHR is /5.
    // Independently assembled; not derived from this encoder.
    //   mov r10, r8   -> 4d 89 c2
    //   shl r10, cl   -> 49 d3 e2   (D3 /4)
    //   shr r10, cl   -> 49 d3 ea   (D3 /5)
    //   sar r10, cl   -> 49 d3 fa   (D3 /7)
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let operands = ["r8", "rcx", "r10"].map(|name| physical.model().view_named(name).unwrap().id);
    for (kind, family, tail) in [
        (
            SelectedInstructionKind::WrappingShiftLeftI64,
            MachineAlternativeFamily::WrappingShiftLeftI64,
            0xe2u8,
        ),
        (
            SelectedInstructionKind::WrappingShiftRightU64,
            MachineAlternativeFamily::WrappingShiftRightU64,
            0xea,
        ),
        (
            SelectedInstructionKind::WrappingShiftRightI64,
            MachineAlternativeFamily::WrappingShiftRightI64,
            0xfa,
        ),
    ] {
        let bytes = vec![0x4d, 0x89, 0xc2, 0x49, 0xd3, tail];
        let key = alternative(family, 0);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), bytes, "{kind:?} encodes its own D3 digit");
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
        assert_every_byte_change_rejected(
            &physical,
            kind,
            key,
            &operands,
            encoded.bytes(),
            "mutated",
        );
        assert_every_operand_substitution_rejected(
            &physical,
            kind,
            key,
            &operands,
            physical.model().view_named("r8").unwrap().id,
            encoded.bytes(),
            "mutated",
        );
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

/// Narrow signed wrapping division shares the guarded i64 divide: normalized
/// narrow operands never reach the faulting i64::MIN / -1 pair, a narrow
/// MIN / -1 widens to -MIN, and the sign extension selection appends
/// truncates that quotient back to the wrapped carrier MIN.
#[test]
fn wrapping_divide_then_sign_extension_wraps_narrow_min_by_minus_one() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let view = |name: &str| physical.model().view_named(name).unwrap().id;
    let divide = encode_x86_64_selected_form(
        &physical,
        SelectedInstructionKind::WrappingDivideI64 {
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        },
        alternative(MachineAlternativeFamily::WrappingDivideI64, 0),
        &["rax", "r9", "rax", "rdx"].map(view),
    )
    .unwrap();
    for (extension, family, minimum, maximum) in [
        (
            Some(SelectedInstructionKind::SignExtendI8),
            MachineAlternativeFamily::SignExtendI8,
            i64::from(i8::MIN),
            i64::from(i8::MAX),
        ),
        (
            Some(SelectedInstructionKind::SignExtendI16),
            MachineAlternativeFamily::SignExtendI16,
            i64::from(i16::MIN),
            i64::from(i16::MAX),
        ),
        (
            Some(SelectedInstructionKind::SignExtendI32),
            MachineAlternativeFamily::SignExtendI32,
            i64::from(i32::MIN),
            i64::from(i32::MAX),
        ),
        (
            None,
            MachineAlternativeFamily::SignExtendI32,
            i64::MIN,
            i64::MAX,
        ),
    ] {
        let mut bytes = divide.bytes().to_vec();
        if let Some(extension) = extension {
            bytes.extend(
                encode_x86_64_selected_form(
                    &physical,
                    extension,
                    alternative(family, 0),
                    &["rax", "rax"].map(view),
                )
                .unwrap()
                .bytes(),
            );
        }
        for (dividend, divisor, quotient) in [
            (minimum, -1, minimum),
            (minimum, 1, minimum),
            (maximum, -1, -maximum),
            (minimum, 2, minimum / 2),
            (-7, 2, -3),
            (7, -2, -3),
            (0, -1, 0),
        ] {
            let mut registers = [0_i64; 16];
            registers[0] = dividend;
            registers[9] = divisor;
            let mut equal = false;
            let mut byte_position = 0;
            while byte_position < bytes.len() {
                let (instruction, length) = decode_one(&bytes[byte_position..]).unwrap();
                byte_position += length;
                match instruction {
                    DecodedInstruction::CompareSignedImmediate8 {
                        register,
                        immediate,
                    } => equal = registers[register as usize] == i64::from(immediate),
                    DecodedInstruction::JumpNotEqualShort { displacement } => {
                        if !equal {
                            byte_position = byte_position
                                .checked_add_signed(displacement as isize)
                                .unwrap();
                        }
                    }
                    DecodedInstruction::JumpShort { displacement } => {
                        byte_position = byte_position
                            .checked_add_signed(displacement as isize)
                            .unwrap();
                    }
                    DecodedInstruction::Negate { destination } => {
                        registers[destination as usize] =
                            registers[destination as usize].wrapping_neg();
                    }
                    DecodedInstruction::SignExtendDividend => registers[2] = registers[0] >> 63,
                    DecodedInstruction::SignedDivide { divisor } => {
                        let dividend =
                            (i128::from(registers[2]) << 64) | i128::from(registers[0] as u64);
                        let divisor = i128::from(registers[divisor as usize]);
                        registers[0] = i64::try_from(dividend / divisor)
                            .expect("a guarded IDIV never sees the faulting pair");
                        registers[2] = (dividend % divisor) as i64;
                    }
                    DecodedInstruction::SignExtendI8 {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] =
                            i64::from(registers[source as usize] as i8)
                    }
                    DecodedInstruction::SignExtendI16 {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] =
                            i64::from(registers[source as usize] as i16)
                    }
                    DecodedInstruction::SignExtendI32 {
                        source,
                        destination,
                    } => {
                        registers[destination as usize] =
                            i64::from(registers[source as usize] as i32)
                    }
                    other => panic!("unexpected wrapping divide instruction {other:?}"),
                }
            }
            assert_eq!(registers[0], quotient, "{dividend} / {divisor}");
            assert_eq!(registers[9], divisor);
        }
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
    assert_every_byte_change_rejected(&physical, kind, key, &operands, &expected, "changed");
    assert_every_operand_substitution_rejected(
        &physical,
        kind,
        key,
        &operands,
        physical.model().view_named("r8").unwrap().id,
        &expected,
        "changed",
    );
}

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
        SaturatingOperation::Remainder => SelectedInstructionKind::SaturatingRemainder {
            carrier,
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
        },
        SaturatingOperation::Multiply => SelectedInstructionKind::SaturatingMultiply { carrier },
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
            SaturatingOperation::Remainder => {
                MachineAlternativeFamily::SaturatingRemainder(carrier)
            }
            SaturatingOperation::Multiply => MachineAlternativeFamily::SaturatingMultiply(carrier),
        },
        0,
    )
}

/// One register assignment per operand layout: the low assignment matches
/// the clang-assembled bytes below, the high assignment exercises REX.B/REX.R
/// on r8-r15. Division and the u64 multiply are pinned to `[rax, right, rax,
/// rdx]`.
fn saturating_operand_names(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    high: bool,
) -> Vec<&'static str> {
    let form = SaturatingForm::of(operation, carrier);
    if form.is_fixed_rax_rdx() {
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

/// How one decoded form ends: by falling through its last byte with the
/// register file, or at a UD2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodedOutcome {
    Completed([u64; 16]),
    Trapped,
}

#[derive(Default, Clone, Copy)]
struct SaturatingFlags {
    overflow: Option<bool>,
    carry: Option<bool>,
    greater: Option<bool>,
    less: Option<bool>,
    zero: Option<bool>,
}

impl SaturatingFlags {
    /// Flags of an exact signed result compared with zero, plus the
    /// explicit overflow and carry the instruction computed. ZF follows the
    /// truncated 64-bit result, which is zero for i64::MIN + i64::MIN too.
    fn arithmetic(exact: i128, overflow: bool, carry: bool) -> Self {
        Self {
            overflow: Some(overflow),
            carry: Some(carry),
            greater: Some(exact > 0),
            less: Some(exact < 0),
            zero: Some(exact as u64 == 0),
        }
    }

    fn undefined() -> Self {
        Self::default()
    }
}

fn execute_decoded(bytes: &[u8], mut registers: [u64; 16]) -> DecodedOutcome {
    let mut flags = SaturatingFlags::default();
    let mut boundaries = vec![0];
    let signed = |value: u64| i128::from(value as i64);
    let subtract_flags = |left: u64, right: u64| {
        let exact = signed(left) - signed(right);
        SaturatingFlags::arithmetic(exact, i64::try_from(exact).is_err(), left < right)
    };
    let mut position = 0;
    while position < bytes.len() {
        position += decode_one(&bytes[position..]).unwrap().1;
        boundaries.push(position);
    }
    let mut byte_position = 0;
    let jump = |byte_position: &mut usize, displacement: i8, taken: bool| {
        if taken {
            *byte_position = byte_position
                .checked_add_signed(isize::from(displacement))
                .unwrap();
            assert!(
                boundaries.contains(byte_position),
                "a jump lands on an instruction boundary of the form"
            );
        }
    };
    while byte_position < bytes.len() {
        let (instruction, length) = decode_one(&bytes[byte_position..]).unwrap();
        byte_position += length;
        match instruction {
            DecodedInstruction::Crash => return DecodedOutcome::Trapped,
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
            DecodedInstruction::CompareRaxImmediate32 { immediate } => {
                flags = subtract_flags(registers[0], immediate as i64 as u64);
            }
            DecodedInstruction::Test { register } => {
                // TEST clears OF and CF and sets ZF and SF from the value.
                flags = SaturatingFlags::arithmetic(
                    signed(registers[usize::from(register)]),
                    false,
                    false,
                );
            }
            DecodedInstruction::SignExtendI8 {
                source,
                destination,
            } => {
                registers[usize::from(destination)] =
                    i64::from(registers[usize::from(source)] as i8) as u64;
            }
            DecodedInstruction::SignExtendI16 {
                source,
                destination,
            } => {
                registers[usize::from(destination)] =
                    i64::from(registers[usize::from(source)] as i16) as u64;
            }
            DecodedInstruction::SignExtendI32 {
                source,
                destination,
            } => {
                registers[usize::from(destination)] =
                    i64::from(registers[usize::from(source)] as i32) as u64;
            }
            DecodedInstruction::ShiftRightLogicalImmediate { destination, count } => {
                // A nonzero immediate count defines ZF, SF, and CF from the
                // result; OF is undefined for counts other than one.
                let value = registers[usize::from(destination)];
                assert!((1..64).contains(&count));
                registers[usize::from(destination)] = value >> count;
                flags = SaturatingFlags {
                    carry: Some((value >> (count - 1)) & 1 == 1),
                    zero: Some(value >> count == 0),
                    ..SaturatingFlags::undefined()
                };
            }
            DecodedInstruction::ShiftLeftByCl { destination }
            | DecodedInstruction::ShiftRightLogicalByCl { destination }
            | DecodedInstruction::ShiftRightArithmeticByCl { destination } => {
                // The 64-bit forms mask CL to six bits; a zero count leaves
                // every flag unchanged, and no Trapping form reads the flags
                // a nonzero count defines, so they are left undefined.
                let count = registers[1] & 63;
                let value = registers[usize::from(destination)];
                registers[usize::from(destination)] = match instruction {
                    DecodedInstruction::ShiftLeftByCl { .. } => value << count,
                    DecodedInstruction::ShiftRightLogicalByCl { .. } => value >> count,
                    _ => ((value as i64) >> count) as u64,
                };
                if count != 0 {
                    flags = SaturatingFlags::undefined();
                }
            }
            DecodedInstruction::JumpNotEqualShort { displacement } => {
                let taken = !flags.zero.expect("JNE reads a defined ZF");
                jump(&mut byte_position, displacement, taken);
            }
            DecodedInstruction::JumpNoOverflowShort { displacement } => {
                let taken = !flags.overflow.expect("JNO reads a defined OF");
                jump(&mut byte_position, displacement, taken);
            }
            DecodedInstruction::JumpBelowShort { displacement } => {
                let taken = flags.carry.expect("JB reads a defined CF");
                jump(&mut byte_position, displacement, taken);
            }
            DecodedInstruction::JumpAboveOrEqualShort { displacement } => {
                let taken = !flags.carry.expect("JAE reads a defined CF");
                jump(&mut byte_position, displacement, taken);
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
            DecodedInstruction::MoveOnAbove {
                source,
                destination,
            } => {
                // CMOVA reads CF = 0 and ZF = 0; ZF is set exactly when the
                // compared difference is neither positive nor negative.
                let carry = flags.carry.expect("CMOVA reads a defined CF");
                let nonzero = flags.greater.expect("CMOVA reads a defined ZF")
                    || flags.less.expect("CMOVA reads a defined ZF");
                if !carry && nonzero {
                    registers[usize::from(destination)] = registers[usize::from(source)];
                }
            }
            DecodedInstruction::Multiply {
                source,
                destination,
            } => {
                // IMUL sets OF and CF when the signed product is truncated
                // and leaves SF and ZF undefined.
                let (left, right) = (
                    registers[usize::from(destination)],
                    registers[usize::from(source)],
                );
                let exact = signed(left) * signed(right);
                registers[usize::from(destination)] = left.wrapping_mul(right);
                let truncated = i64::try_from(exact).is_err();
                flags = SaturatingFlags {
                    overflow: Some(truncated),
                    carry: Some(truncated),
                    ..SaturatingFlags::undefined()
                };
            }
            DecodedInstruction::UnsignedMultiply { multiplier } => {
                // MUL sets OF and CF when the RDX high half is nonzero.
                let product =
                    u128::from(registers[0]) * u128::from(registers[usize::from(multiplier)]);
                registers[0] = product as u64;
                registers[2] = (product >> 64) as u64;
                let high = registers[2] != 0;
                flags = SaturatingFlags {
                    overflow: Some(high),
                    carry: Some(high),
                    ..SaturatingFlags::undefined()
                };
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
            DecodedInstruction::Xor {
                source,
                destination,
            } => {
                registers[usize::from(destination)] ^= registers[usize::from(source)];
                flags = SaturatingFlags::arithmetic(
                    signed(registers[usize::from(destination)]),
                    false,
                    false,
                );
            }
            DecodedInstruction::JumpEqualShort { displacement } => {
                let taken = flags.zero.expect("JE reads a defined ZF");
                jump(&mut byte_position, displacement, taken);
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
            other => panic!("unexpected decoded instruction {other:?}"),
        }
    }
    DecodedOutcome::Completed(registers)
}
