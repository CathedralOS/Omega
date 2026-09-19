//! Encoding one selected form: validating the request, resolving registers
//! and emitting the words for every operation family.

use crate::aarch64_physical_register_model;
use crate::saturating_forms::SaturatingRealization;
use crate::selected_form_encoding::copy_bytes;
use crate::selected_form_encoding::decoding::{decode_words, footprint, validate_decoded};
use crate::selected_form_encoding::float_bits;
use crate::selected_form_encoding::movn_materialization::append_canonical_materialization;
use crate::selected_form_encoding::{
    Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
};
use semantic_vocabulary::IntegerValue;

pub fn encode_aarch64_selected_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::encode(physical, alternative, operands);
    }
    if float_bits::is_transfer(kind) {
        return float_bits::encode(physical, kind, alternative, operands);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    let bytes = encode_unchecked(kind, &registers)?;
    validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, &bytes)
}

pub fn validate_aarch64_selected_form_encoding(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    if kind == SelectedInstructionKind::CopyBytes {
        return copy_bytes::validate(physical, alternative, operands, bytes);
    }
    if float_bits::is_transfer(kind) {
        return float_bits::validate(physical, kind, alternative, operands, bytes);
    }
    validate_request(physical, kind, alternative, operands)?;
    let registers = resolve_scalar_registers(physical, kind, operands)?;
    validate_return_home(kind, &registers)?;
    let decoded = decode_words(bytes)?;
    validate_decoded(kind, &registers, &decoded)?;
    let canonical = encode_unchecked(kind, &registers)?;
    if bytes != canonical {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(kind, operands),
    })
}

fn validate_request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count) = family_and_operand_count(kind)?;
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    Ok(())
}

fn family_and_operand_count(
    kind: SelectedInstructionKind,
) -> Result<(MachineAlternativeFamily, usize), Aarch64SelectedFormEncodingError> {
    Ok(match kind {
        SelectedInstructionKind::CompareI64Zero => (MachineAlternativeFamily::CompareI64Zero, 1),
        SelectedInstructionKind::CompareI64Immediate { .. } => {
            (MachineAlternativeFamily::CompareI64Immediate, 1)
        }
        SelectedInstructionKind::CompareI64 => (MachineAlternativeFamily::CompareI64, 2),
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (MachineAlternativeFamily::MaterializeI64, 1)
        }
        SelectedInstructionKind::CopyI64 => (MachineAlternativeFamily::CopyI64, 2),
        SelectedInstructionKind::MaterializeBooleanEqual => {
            (MachineAlternativeFamily::MaterializeBooleanEqual, 1)
        }
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            (MachineAlternativeFamily::MaterializeBooleanU64LessThan, 1)
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            (MachineAlternativeFamily::MaterializeBooleanI64LessThan, 1)
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => (
            MachineAlternativeFamily::MaterializeBooleanU64LessOrEqual,
            1,
        ),
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => (
            MachineAlternativeFamily::MaterializeBooleanI64LessOrEqual,
            1,
        ),
        SelectedInstructionKind::ZeroExtendU8 => (MachineAlternativeFamily::ZeroExtendU8, 2),
        SelectedInstructionKind::ZeroExtendU16 => (MachineAlternativeFamily::ZeroExtendU16, 2),
        SelectedInstructionKind::SignExtendI8 => (MachineAlternativeFamily::SignExtendI8, 2),
        SelectedInstructionKind::SignExtendI16 => (MachineAlternativeFamily::SignExtendI16, 2),
        SelectedInstructionKind::SignExtendI32 => (MachineAlternativeFamily::SignExtendI32, 2),
        SelectedInstructionKind::ZeroExtendU32 => (MachineAlternativeFamily::ZeroExtendU32, 2),
        SelectedInstructionKind::ByteViewAddress => (MachineAlternativeFamily::ByteViewAddress, 3),
        SelectedInstructionKind::ExactAddI64 { .. } => (MachineAlternativeFamily::ExactAddI64, 3),
        SelectedInstructionKind::WrappingAddI64 => (MachineAlternativeFamily::WrappingAddI64, 3),
        SelectedInstructionKind::BitwiseAndI64 => (MachineAlternativeFamily::BitwiseAndI64, 3),
        SelectedInstructionKind::BitwiseXorI64 => (MachineAlternativeFamily::BitwiseXorI64, 3),
        SelectedInstructionKind::SaturatingAdd { carrier } => (
            MachineAlternativeFamily::SaturatingAdd(carrier),
            SaturatingRealization::of_kind(kind)
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                .operand_count(),
        ),
        SelectedInstructionKind::SaturatingSubtract { carrier } => (
            MachineAlternativeFamily::SaturatingSubtract(carrier),
            SaturatingRealization::of_kind(kind)
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                .operand_count(),
        ),
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => (
            MachineAlternativeFamily::SaturatingDivide(carrier),
            SaturatingRealization::of_kind(kind)
                .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
                .operand_count(),
        ),
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            (MachineAlternativeFamily::ExactDivideU64, 3)
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            (MachineAlternativeFamily::WrappingRemainderI64, 3)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64, 3)
        }
        SelectedInstructionKind::ExactMultiplyI64 { .. } => {
            (MachineAlternativeFamily::ExactMultiplyI64, 3)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactAddI64Immediate, 2)
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (MachineAlternativeFamily::ExactSubtractI64Immediate, 2)
        }
        SelectedInstructionKind::ReturnScalar => (MachineAlternativeFamily::ReturnScalar, 1),
        SelectedInstructionKind::ReturnAggregate { fragment_count } => {
            if !(1..=2).contains(&fragment_count) {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            (
                MachineAlternativeFamily::ReturnAggregate,
                usize::from(fragment_count),
            )
        }
        SelectedInstructionKind::ReturnUnit => (MachineAlternativeFamily::ReturnUnit, 0),
        SelectedInstructionKind::ConditionalBranchNonZero => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::NormalizedForeignCall { .. }
        | SelectedInstructionKind::CallScalar { .. } => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    })
}

fn validate_return_home(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if matches!(kind, SelectedInstructionKind::ReturnScalar) && registers != [0] {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if let SelectedInstructionKind::ReturnAggregate { fragment_count } = kind
        && (!(1..=2).contains(&fragment_count)
            || registers != &([0, 1][..usize::from(fragment_count)]))
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

fn resolve_scalar_registers(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    // RET does not encode the returned register. The exact constraint row
    // retains its ABI class; accept only the canonical floating result home.
    if kind == SelectedInstructionKind::ReturnScalar
        && physical
            .model()
            .view_named("d0")
            .is_some_and(|view| operands == [view.id])
    {
        return Ok(vec![0]);
    }
    resolve_registers(physical, operands)
}

pub(crate) fn resolve_registers(
    physical: &ValidatedPhysicalRegisterModel,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    operands
        .iter()
        .map(|id| {
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == *id)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            let Some(index) = view.name.strip_prefix('x') else {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            };
            let index = index
                .parse::<u8>()
                .ok()
                .filter(|index| *index <= 30)
                .ok_or(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id))?;
            if view.bits != 64 || !view.allocatable {
                return Err(Aarch64SelectedFormEncodingError::UnknownOrNonGpr64View(*id));
            }
            Ok(index)
        })
        .collect()
}

pub(crate) fn integer_bits(value: IntegerValue) -> Result<u64, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => u64::try_from(value)
            .map_err(|_| Aarch64SelectedFormEncodingError::IntegerOutsideI64Bits),
    }
}

pub(crate) fn u12(value: IntegerValue) -> Result<u16, Aarch64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u16),
        _ => Err(Aarch64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

fn encode_unchecked(
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let mut words = Vec::new();
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            append_canonical_materialization(&mut words, registers[0], integer_bits(value)?);
        }
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            let condition: u32 = match kind {
                SelectedInstructionKind::MaterializeBooleanEqual => 0,
                SelectedInstructionKind::MaterializeBooleanU64LessThan => 3,
                SelectedInstructionKind::MaterializeBooleanI64LessThan => 11,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 9,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 13,
                _ => unreachable!("Boolean condition arm"),
            };
            // CSET Xd, condition is CSINC Xd, XZR, XZR, inverse(condition).
            words.push(0x9a9f_07e0 | ((condition ^ 1) << 12) | u32::from(registers[0]));
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            words.push(0xd340_1c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            words.push(0xd340_3c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::SignExtendI8 => {
            words.push(0x9340_1c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::SignExtendI16 => {
            words.push(0x9340_3c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::SignExtendI32 => {
            words.push(0x9340_7c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            words.push(0xd340_7c00 | (u32::from(registers[0]) << 5) | u32::from(registers[1]));
        }
        SelectedInstructionKind::CopyI64 => {
            words.push(0xaa00_03e0 | (u32::from(registers[0]) << 16) | u32::from(registers[1]));
        }
        SelectedInstructionKind::CompareI64Zero => {
            words.push(0xf100_001f | (u32::from(registers[0]) << 5));
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            // `cmp xN, #imm12` is `subs xzr, xN, #imm12` with LSL #0.
            words.push(
                0xf100_001f | (u32::from(u12(immediate)?) << 10) | (u32::from(registers[0]) << 5),
            );
        }
        SelectedInstructionKind::CompareI64 => {
            words.push(
                0xeb00_001f | (u32::from(registers[1]) << 16) | (u32::from(registers[0]) << 5),
            );
        }
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. } => {
            words.push(
                0x8b00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::BitwiseAndI64 => {
            words.push(
                0x8a00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::SaturatingAdd { .. }
        | SelectedInstructionKind::SaturatingSubtract { .. }
        | SelectedInstructionKind::SaturatingDivide { .. } => {
            append_saturating(&mut words, kind, registers)?;
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            words.push(
                0x9ac0_0800
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            if registers[2] == registers[0] || registers[2] == registers[1] {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            words.push(
                0x9ac0_0c00
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
            // MSUB uses the original dividend, including when SDIV wraps MIN / -1.
            words.push(
                0x9b00_8000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 10)
                    | (u32::from(registers[2]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::BitwiseXorI64 => {
            words.push(
                0xca00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            words.push(
                0x9100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            words.push(
                0xcb00_0000
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::ExactMultiplyI64 { .. } => {
            // `MUL Xd, Xn, Xm` is `MADD Xd, Xn, Xm, XZR`: the 0x9b00 base with
            // the addend field fixed to 31.
            words.push(
                0x9b00_7c00
                    | (u32::from(registers[1]) << 16)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[2]),
            );
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            words.push(
                0xd100_0000
                    | (u32::from(u12(immediate)?) << 10)
                    | (u32::from(registers[0]) << 5)
                    | u32::from(registers[1]),
            );
        }
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => words.push(0xd65f_03c0),
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::NormalizedForeignCall { .. }
        | SelectedInstructionKind::CallScalar { .. } => {
            return Err(Aarch64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// The single-word `orr xd, xzr, #bound` materialization of each carrier
/// bound, with the destination register cleared. Every saturating bound is
/// a valid bitmask immediate, so no bound needs a `movz`/`movk` sequence;
/// the words were assembled independently by Apple clang.
pub(crate) const BOUND_WORDS: [(u64, u32); 11] = [
    (i8::MAX as u64, 0xb240_1be0),
    (i8::MIN as i64 as u64, 0xb279_e3e0),
    (i16::MAX as u64, 0xb240_3be0),
    (i16::MIN as i64 as u64, 0xb271_c3e0),
    (i32::MAX as u64, 0xb240_7be0),
    (i32::MIN as i64 as u64, 0xb261_83e0),
    (i64::MAX as u64, 0xb240_fbe0),
    (i64::MIN as u64, 0xb241_03e0),
    (u8::MAX as u64, 0xb240_1fe0),
    (u16::MAX as u64, 0xb240_3fe0),
    (u32::MAX as u64, 0xb240_7fe0),
];

fn bound_word(bound: u64, destination: u8) -> Result<u32, Aarch64SelectedFormEncodingError> {
    BOUND_WORDS
        .iter()
        .find(|(value, _)| *value == bound)
        .map(|(_, word)| word | u32::from(destination))
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
}

/// `csel value, scratch, value, cond`: replace the value by the scratch when
/// the preceding compare set the condition.
fn select(value: u8, scratch: u8, condition: u32) -> u32 {
    0x9a80_0000
        | (u32::from(value) << 16)
        | (condition << 12)
        | (u32::from(scratch) << 5)
        | u32::from(value)
}

fn three_address(opcode: u32, left: u8, right: u8, destination: u8) -> u32 {
    opcode | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination)
}

/// Every saturating realization; see `SaturatingRealization` for why each
/// carrier class takes its shape. The four-operand forms require the result
/// and the bound scratch to be distinct from the inputs and from each other
/// (both are early-clobber outputs) because the scratch is written before
/// the inputs are consumed and the clamp reads both.
fn append_saturating(
    words: &mut Vec<u32>,
    kind: SelectedInstructionKind,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let realization = SaturatingRealization::of_kind(kind)
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let (left, right, value) = (registers[0], registers[1], registers[2]);
    if realization.operand_count() == 4
        && (registers[2..]
            .iter()
            .any(|late| registers[..2].contains(late))
            || registers[2] == registers[3])
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    match realization {
        SaturatingRealization::AddU64 => {
            words.push(three_address(0xab00_0000, left, right, value));
            words.push(0xda9f_3000 | (u32::from(value) << 5) | u32::from(value));
        }
        SaturatingRealization::SubtractUnsigned => {
            words.push(three_address(0xeb00_0000, left, right, value));
            words.push(0x9a9f_2000 | (u32::from(value) << 5) | u32::from(value));
        }
        SaturatingRealization::DivideUnsigned => {
            words.push(three_address(0x9ac0_0800, left, right, value));
        }
        SaturatingRealization::ClampNarrow { operation, .. } => {
            let scratch = registers[3];
            words.push(three_address(
                match operation {
                    selected_instructions::SaturatingOperation::Add => 0x8b00_0000,
                    selected_instructions::SaturatingOperation::Subtract => 0xcb00_0000,
                    selected_instructions::SaturatingOperation::Divide => 0x9ac0_0c00,
                },
                left,
                right,
                value,
            ));
            // The maximum is compared first (select on GT), then the minimum
            // (select on LT) for the signed add and subtract.
            for (index, bound) in realization.clamp_bounds().into_iter().enumerate() {
                words.push(bound_word(bound, scratch)?);
                words.push(0xeb00_001f | (u32::from(scratch) << 16) | (u32::from(value) << 5));
                words.push(select(value, scratch, if index == 0 { 0xc } else { 0xb }));
            }
        }
        SaturatingRealization::OverflowI64 { subtract } => {
            let scratch = registers[3];
            words.push(0x937f_fc00 | (u32::from(left) << 5) | u32::from(scratch));
            words.push(0xd240_f800 | (u32::from(scratch) << 5) | u32::from(scratch));
            words.push(three_address(
                if subtract { 0xeb00_0000 } else { 0xab00_0000 },
                left,
                right,
                value,
            ));
            words.push(select(value, scratch, 0x6));
        }
        SaturatingRealization::DivideI64 => {
            let scratch = registers[3];
            words.push(three_address(0x9ac0_0c00, left, right, value));
            words.push(bound_word(i64::MIN as u64, scratch)?);
            words.push(0xeb00_001f | (u32::from(scratch) << 16) | (u32::from(value) << 5));
            words.push(0xba41_0800 | (u32::from(right) << 5));
            words.push(bound_word(i64::MAX as u64, scratch)?);
            words.push(select(value, scratch, 0x0));
        }
    }
    Ok(())
}
