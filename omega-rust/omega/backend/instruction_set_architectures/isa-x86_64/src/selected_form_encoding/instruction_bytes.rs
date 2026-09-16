//! Assembling REX, ModRM, register and LEA bytes for one selected form.

use crate::selected_form_encoding::X86_64SelectedFormEncodingError;
use selected_instructions::{MachineAlternativeKey, SelectedInstructionKind};
use semantic_vocabulary::IntegerValue;

pub(crate) fn integer_bits(value: IntegerValue) -> Result<u64, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Signed(value) => i64::try_from(value)
            .map(|value| value as u64)
            .map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits),
        IntegerValue::Unsigned(value) => {
            u64::try_from(value).map_err(|_| X86_64SelectedFormEncodingError::IntegerOutsideI64Bits)
        }
    }
}

pub(crate) fn u12(value: IntegerValue) -> Result<u32, X86_64SelectedFormEncodingError> {
    match value {
        IntegerValue::Unsigned(value) if value <= 4095 => Ok(value as u32),
        _ => Err(X86_64SelectedFormEncodingError::ImmediateOutsideU12),
    }
}

pub(crate) fn rex(register: u8, index: u8, base: u8) -> u8 {
    0x48 | ((register >> 3) << 2) | ((index >> 3) << 1) | (base >> 3)
}

pub(crate) fn modrm(mode: u8, register: u8, rm: u8) -> u8 {
    (mode << 6) | ((register & 7) << 3) | (rm & 7)
}

fn append_register_binary(bytes: &mut Vec<u8>, opcode: u8, source: u8, destination: u8) {
    bytes.extend([
        rex(source, 0, destination),
        opcode,
        modrm(3, source, destination),
    ]);
}

/// `movabs scratch, bound; cmp value, scratch; cmovcc value, scratch` with
/// i32::MAX selected on G, or i32::MIN selected on L.
fn append_i32_clamp(bytes: &mut Vec<u8>, value: u8, scratch: u8, upper: bool) {
    let (bound, condition) = if upper {
        (i64::from(i32::MAX), 0x4f)
    } else {
        (i64::from(i32::MIN), 0x4c)
    };
    bytes.extend([0x48 | (scratch >> 3), 0xb8 | (scratch & 7)]);
    bytes.extend((bound as u64).to_le_bytes());
    append_register_binary(bytes, 0x39, scratch, value);
    bytes.extend([
        rex(value, 0, scratch),
        0x0f,
        condition,
        modrm(3, value, scratch),
    ]);
}

fn append_lea_register(bytes: &mut Vec<u8>, left: u8, right: u8, destination: u8) {
    let (base, index) = if left & 7 == 5 && right & 7 != 5 {
        (right, left)
    } else {
        (left, right)
    };
    let needs_zero_displacement = base & 7 == 5;
    bytes.extend([
        rex(destination, index, base),
        0x8d,
        modrm(u8::from(needs_zero_displacement), destination, 4),
        ((index & 7) << 3) | (base & 7),
    ]);
    if needs_zero_displacement {
        bytes.push(0);
    }
}

fn append_lea_immediate(bytes: &mut Vec<u8>, base: u8, destination: u8, displacement: i32) {
    let use_disp8 = i8::try_from(displacement).is_ok();
    let uses_sib = base & 7 == 4;
    bytes.extend([
        rex(destination, 0, base),
        0x8d,
        modrm(
            if use_disp8 { 1 } else { 2 },
            destination,
            if uses_sib { 4 } else { base },
        ),
    ]);
    if uses_sib {
        bytes.push(0x20 | (base & 7));
    }
    if use_disp8 {
        bytes.push(displacement as i8 as u8);
    } else {
        bytes.extend(displacement.to_le_bytes());
    }
}

pub(crate) fn encode_unchecked(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
) -> Result<Vec<u8>, X86_64SelectedFormEncodingError> {
    let mut bytes = Vec::new();
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            bytes.extend([0x48 | (registers[0] >> 3), 0xb8 | (registers[0] & 7)]);
            bytes.extend(integer_bits(value)?.to_le_bytes());
        }
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            let condition = match kind {
                SelectedInstructionKind::MaterializeBooleanEqual => 4,
                SelectedInstructionKind::MaterializeBooleanU64LessThan => 2,
                SelectedInstructionKind::MaterializeBooleanI64LessThan => 12,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 6,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 14,
                _ => unreachable!("Boolean condition arm"),
            };
            let destination = registers[0];
            // Always carry REX so low-byte views never name AH/CH/DH/BH.
            // MOVZX into the 32-bit view then defines all 64 result bits.
            bytes.extend([
                0x40 | (destination >> 3),
                0x0f,
                0x90 | condition,
                0xc0 | (destination & 7),
                0x40 | ((destination >> 3) << 2) | (destination >> 3),
                0x0f,
                0xb6,
                0xc0 | ((destination & 7) << 3) | (destination & 7),
            ]);
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            bytes.extend([
                0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xb6,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            bytes.extend([
                0x40 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xb7,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI8 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xbe,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI16 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x0f,
                0xbf,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::SignExtendI32 => {
            bytes.extend([
                0x48 | ((registers[1] >> 3) << 2) | (registers[0] >> 3),
                0x63,
                0xc0 | ((registers[1] & 7) << 3) | (registers[0] & 7),
            ]);
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            bytes.extend([
                0x40 | ((registers[0] >> 3) << 2) | (registers[1] >> 3),
                0x89,
                0xc0 | ((registers[0] & 7) << 3) | (registers[1] & 7),
            ]);
        }
        SelectedInstructionKind::CopyI64 => {
            append_register_binary(&mut bytes, 0x89, registers[0], registers[1]);
        }
        SelectedInstructionKind::CompareI64Zero => {
            append_register_binary(&mut bytes, 0x85, registers[0], registers[0]);
        }
        SelectedInstructionKind::CompareI64 => {
            append_register_binary(&mut bytes, 0x39, registers[1], registers[0]);
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            bytes.extend([rex(0, 0, registers[0]), 0x81, modrm(3, 7, registers[0])]);
            bytes.extend(u12(immediate)?.to_le_bytes());
        }
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. } => {
            append_lea_register(&mut bytes, registers[0], registers[1], registers[2]);
        }
        SelectedInstructionKind::SaturatingSubtractU64 => {
            if registers[2] == registers[0] || registers[2] == registers[1] {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            append_register_binary(&mut bytes, 0x39, registers[1], registers[0]);
            append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
            bytes.extend([
                rex(registers[2], 0, registers[1]),
                0x0f,
                0x42,
                modrm(3, registers[2], registers[1]),
            ]);
            append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
        }
        SelectedInstructionKind::SaturatingAddU64 => {
            if registers[2] == registers[0] || registers[2] == registers[1] {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            // min(left + right, MAX) = ~max((MAX - left) - right, 0).
            append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
            bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 2, registers[2])]);
            append_register_binary(&mut bytes, 0x39, registers[1], registers[2]);
            bytes.extend([
                rex(registers[2], 0, registers[1]),
                0x0f,
                0x42,
                modrm(3, registers[2], registers[1]),
            ]);
            append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
            bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 2, registers[2])]);
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2 {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 6, registers[1])]);
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2 {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            // Initialize the exceptional result before testing -1: IDIV would
            // otherwise fault for MIN / -1 even though its remainder is zero.
            append_register_binary(&mut bytes, 0x31, registers[3], registers[3]);
            bytes.extend([
                rex(0, 0, registers[1]),
                0x83,
                modrm(3, 7, registers[1]),
                0xff,
            ]);
            bytes.extend([0x74, 5, 0x48, 0x99]);
            bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 7, registers[1])]);
            append_register_binary(&mut bytes, 0x89, registers[3], registers[2]);
        }
        SelectedInstructionKind::SaturatingAddI32
        | SelectedInstructionKind::SaturatingSubtractI32 => {
            // The early-clobber result accumulates the exact 64-bit sum of two
            // sign-normalized i32 carriers; the early-clobber scratch holds
            // each bound for the CMP/CMOV clamp.
            if registers[2..]
                .iter()
                .any(|late| registers[..2].contains(late))
                || registers[2] == registers[3]
            {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
            let opcode = if kind == SelectedInstructionKind::SaturatingAddI32 {
                0x01
            } else {
                0x29
            };
            append_register_binary(&mut bytes, opcode, registers[1], registers[2]);
            append_i32_clamp(&mut bytes, registers[2], registers[3], true);
            append_i32_clamp(&mut bytes, registers[2], registers[3], false);
        }
        SelectedInstructionKind::SaturatingDivideI32 { .. } => {
            if registers[0] != 0 || registers[2] != 0 || registers[3] != 2 || registers[1] == 2 {
                return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
            }
            // The 64-bit quotient of sign-normalized i32 carriers cannot fault;
            // only i32::MIN / -1 exceeds the carrier and is clamped through RDX,
            // whose explicit input value CQO discards.
            bytes.extend([0x48, 0x99]);
            bytes.extend([rex(0, 0, registers[1]), 0xf7, modrm(3, 7, registers[1])]);
            append_i32_clamp(&mut bytes, registers[2], registers[3], true);
        }
        SelectedInstructionKind::BitwiseAndI64 | SelectedInstructionKind::BitwiseXorI64 => {
            // Both operations commute, so either input may already own the
            // output register. A distinct output needs one non-destructive copy.
            let opcode = if kind == SelectedInstructionKind::BitwiseXorI64 {
                0x31
            } else {
                0x21
            };
            if registers[2] == registers[0] {
                append_register_binary(&mut bytes, opcode, registers[1], registers[2]);
            } else if registers[2] == registers[1] {
                append_register_binary(&mut bytes, opcode, registers[0], registers[2]);
            } else {
                append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                append_register_binary(&mut bytes, opcode, registers[1], registers[2]);
            }
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            append_lea_immediate(
                &mut bytes,
                registers[0],
                registers[1],
                i32::try_from(u12(immediate)?).expect("u12 fits i32"),
            );
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            append_lea_immediate(
                &mut bytes,
                registers[0],
                registers[1],
                -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
            );
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
            0 => append_register_binary(&mut bytes, 0x31, registers[2], registers[2]),
            1 => append_register_binary(&mut bytes, 0x29, registers[1], registers[2]),
            2 => {
                bytes.extend([rex(0, 0, registers[2]), 0xf7, modrm(3, 3, registers[2])]);
                append_register_binary(&mut bytes, 0x01, registers[0], registers[2]);
            }
            3 => {
                append_register_binary(&mut bytes, 0x89, registers[0], registers[2]);
                append_register_binary(&mut bytes, 0x29, registers[1], registers[2]);
            }
            _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
        },
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => bytes.push(0xc3),
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
        SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::CopyBytes
        | SelectedInstructionKind::Load8 { .. }
        | SelectedInstructionKind::Load16 { .. }
        | SelectedInstructionKind::LoadPacked { .. }
        | SelectedInstructionKind::StorePacked { .. }
        | SelectedInstructionKind::Load32 { .. }
        | SelectedInstructionKind::Load64 { .. }
        | SelectedInstructionKind::Store { .. }
        | SelectedInstructionKind::AddressOffset { .. }
        | SelectedInstructionKind::Store64 { .. }
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::HostedReadByte { .. }
        | SelectedInstructionKind::HostedWriteByteI32 { .. }
        | SelectedInstructionKind::FrameAddress { .. }
        | SelectedInstructionKind::CallUnit { .. }
        | SelectedInstructionKind::CallAggregate { .. }
        | SelectedInstructionKind::CallScalar { .. } => {
            return Err(X86_64SelectedFormEncodingError::LayoutDependentForm);
        }
    }
    Ok(bytes)
}
