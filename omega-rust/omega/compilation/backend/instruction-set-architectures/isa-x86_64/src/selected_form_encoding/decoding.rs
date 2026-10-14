//! Decoding emitted instructions and validating them against the request.

use crate::selected_form_encoding::instruction_bytes::{integer_bits, u12};
use crate::selected_form_encoding::saturating_forms::{SaturatingForm, saturating_operation};
use crate::selected_form_encoding::trapping_forms::{
    DividendGuard, RangeCheck, TrappingArithmetic, TrappingShape, trapping_form,
};
use crate::selected_form_encoding::{X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint};
use register_model::RegisterViewId;
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    SaturatingCarrier, SaturatingOperation, SelectedInstructionKind, TrappingForm,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodedInstruction {
    Crash,
    SignedDivide {
        divisor: u8,
    },
    SignExtendDividend,
    JumpEqualShort {
        displacement: i8,
    },
    JumpNotEqualShort {
        displacement: i8,
    },
    /// `jmp rel8`: the skip over the divide block in the guarded wrapping
    /// division sequence.
    JumpShort {
        displacement: i8,
    },
    /// `jno rel8`: a Trapping form's skip over its trap when OF is clear.
    JumpNoOverflowShort {
        displacement: i8,
    },
    /// `jb rel8`: the shift-count check's skip when CF is set.
    JumpBelowShort {
        displacement: i8,
    },
    /// `jae rel8` (`jnc`): the u64 add and subtract skip when CF is clear.
    JumpAboveOrEqualShort {
        displacement: i8,
    },
    CompareSignedImmediate8 {
        register: u8,
        immediate: i8,
    },
    /// `cmp rax, imm32` in the RAX short form (`48 3d`), sign-extended.
    CompareRaxImmediate32 {
        immediate: i32,
    },
    /// `shr destination, count`: the C1 /5 logical right shift by an imm8.
    ShiftRightLogicalImmediate {
        destination: u8,
        count: u8,
    },
    UnsignedDivide {
        divisor: u8,
    },
    /// `mul multiplier`: the F7 /4 widening unsigned multiply of RAX into
    /// RDX:RAX.
    UnsignedMultiply {
        multiplier: u8,
    },
    Complement {
        destination: u8,
    },
    MoveOnBorrow {
        source: u8,
        destination: u8,
    },
    MoveOnGreater {
        source: u8,
        destination: u8,
    },
    MoveOnLess {
        source: u8,
        destination: u8,
    },
    /// `cmova destination, source`: the unsigned-greater select.
    MoveOnAbove {
        source: u8,
        destination: u8,
    },
    MoveOnOverflow {
        source: u8,
        destination: u8,
    },
    /// `sar destination, 63`.
    ArithmeticShiftRight63 {
        destination: u8,
    },
    /// `btc destination, 63`.
    ComplementBit63 {
        destination: u8,
    },
    SubtractWithBorrow {
        source: u8,
        destination: u8,
    },
    Or {
        source: u8,
        destination: u8,
    },
    BitwiseAnd {
        source: u8,
        destination: u8,
    },
    SetBoolean {
        condition: u8,
        destination: u8,
    },
    ZeroExtendU16 {
        source: u8,
        destination: u8,
    },
    SignExtendI8 {
        source: u8,
        destination: u8,
    },
    SignExtendI16 {
        source: u8,
        destination: u8,
    },
    SignExtendI32 {
        source: u8,
        destination: u8,
    },
    ZeroExtendU8 {
        source: u8,
        destination: u8,
    },
    ZeroExtendU32 {
        source: u8,
        destination: u8,
    },
    Materialize {
        destination: u8,
        value: u64,
    },
    Move {
        source: u8,
        destination: u8,
    },
    Test {
        register: u8,
    },
    Compare {
        left: u8,
        right: u8,
    },
    CompareImmediate {
        register: u8,
        immediate: u32,
    },
    Lea {
        destination: u8,
        base: u8,
        index: Option<u8>,
        displacement: i32,
    },
    Xor {
        source: u8,
        destination: u8,
    },
    Subtract {
        source: u8,
        destination: u8,
    },
    /// `imul destination, source`: the register-form two-operand multiply.
    Multiply {
        source: u8,
        destination: u8,
    },
    /// `shl destination, cl`: the D3 /4 variable-count left shift.
    ShiftLeftByCl {
        destination: u8,
    },
    /// `shr destination, cl`: the D3 /6 logical variable-count right shift.
    ShiftRightLogicalByCl {
        destination: u8,
    },
    /// `sar destination, cl`: the D3 /7 arithmetic variable-count right shift.
    ShiftRightArithmeticByCl {
        destination: u8,
    },
    Negate {
        destination: u8,
    },
    /// `movzx`/`mov` of the atomic cell into the observed output.
    AtomicLoad {
        destination: u8,
        base: u8,
        bits: u16,
    },
    /// `mov` of the incoming value into the atomic cell.
    AtomicStore {
        source: u8,
        base: u8,
        bits: u16,
    },
    /// `mfence`: the store-load fence completing the atomic store.
    MemoryFence,
    /// `xchg` between the tied in/out register and the atomic cell.
    AtomicExchange {
        register: u8,
        base: u8,
        bits: u16,
    },
    /// `lock xadd` between the tied in/out register and the atomic cell.
    AtomicLockedAdd {
        register: u8,
        base: u8,
        bits: u16,
    },
    /// `lock cmpxchg` of the replacement register against the atomic cell,
    /// with the expected input and observed output fixed in RAX.
    AtomicBitwise {
        operation: selected_instructions::AtomicOperation,
        source: u8,
        destination: u8,
        bits: u16,
    },
    AtomicLockedCompareExchange {
        register: u8,
        base: u8,
        bits: u16,
    },
    Add {
        source: u8,
        destination: u8,
    },
    Return,
    /// `jmp qword ptr [rip+disp32]`: the closed indirect form the import
    /// thunk emits. Not a selected instruction kind — image emission owns
    /// its production; PCC admission owns its replay.
    JumpIndirectRip {
        displacement: i32,
    },
    /// `lea destination, [rip+0]`: the canonical unresolved form of a
    /// byte-literal image address. Object construction owns the disp32.
    UnresolvedRipRelativeAddress {
        destination: u8,
    },
}

/// A `DecodedInstruction::Move` shorthand for the expected-sequence
/// tables.
fn mov(source: u8, destination: u8) -> DecodedInstruction {
    DecodedInstruction::Move {
        source,
        destination,
    }
}

pub(crate) fn decode_all(
    bytes: &[u8],
) -> Result<Vec<DecodedInstruction>, X86_64SelectedFormEncodingError> {
    if bytes.is_empty() {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut decoded = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (instruction, length) = decode_one(&bytes[offset..])?;
        decoded.push(instruction);
        offset = offset
            .checked_add(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    }
    Ok(decoded)
}

/// Register-direct modrm operand pairing: `source` is the r/m field
/// extended by REX.B, `destination` the reg field extended by REX.R.
fn modrm_source_destination(modrm: u8, rex: u8) -> (u8, u8) {
    (
        (modrm & 7) | ((rex & 1) << 3),
        ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
    )
}

pub(crate) fn decode_one(
    bytes: &[u8],
) -> Result<(DecodedInstruction, usize), X86_64SelectedFormEncodingError> {
    if let [0x48, 0x99, ..] = bytes {
        return Ok((DecodedInstruction::SignExtendDividend, 2));
    }
    if let [0x74, displacement, ..] = bytes {
        return Ok((
            DecodedInstruction::JumpEqualShort {
                displacement: *displacement as i8,
            },
            2,
        ));
    }
    if let [0x75, displacement, ..] = bytes {
        return Ok((
            DecodedInstruction::JumpNotEqualShort {
                displacement: *displacement as i8,
            },
            2,
        ));
    }
    if let [0xeb, displacement, ..] = bytes {
        return Ok((
            DecodedInstruction::JumpShort {
                displacement: *displacement as i8,
            },
            2,
        ));
    }
    if let [opcode @ (0x71..=0x73), displacement, ..] = bytes {
        let displacement = *displacement as i8;
        return Ok((
            match *opcode {
                0x71 => DecodedInstruction::JumpNoOverflowShort { displacement },
                0x72 => DecodedInstruction::JumpBelowShort { displacement },
                _ => DecodedInstruction::JumpAboveOrEqualShort { displacement },
            },
            2,
        ));
    }
    if let [0x48, 0x3d, i0, i1, i2, i3, ..] = bytes {
        return Ok((
            DecodedInstruction::CompareRaxImmediate32 {
                immediate: i32::from_le_bytes([*i0, *i1, *i2, *i3]),
            },
            6,
        ));
    }
    if let [rex, 0xc1, modrm, count, ..] = bytes
        && rex & !1 == 0x48
        && modrm & 0xf8 == 0xe8
    {
        return Ok((
            DecodedInstruction::ShiftRightLogicalImmediate {
                destination: (modrm & 7) | ((rex & 1) << 3),
                count: *count,
            },
            4,
        ));
    }
    if let [rex, 0x83, modrm, immediate, ..] = bytes
        && rex & !1 == 0x48
        && modrm & 0xf8 == 0xf8
    {
        return Ok((
            DecodedInstruction::CompareSignedImmediate8 {
                register: (modrm & 7) | ((rex & 1) << 3),
                immediate: *immediate as i8,
            },
            4,
        ));
    }
    if let [rex, 0x0f, opcode, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && matches!(*opcode, 0x40 | 0x42 | 0x47 | 0x4c | 0x4f)
        && modrm & 0xc0 == 0xc0
    {
        let source = (modrm & 7) | ((rex & 1) << 3);
        let destination = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
        return Ok((
            match opcode {
                0x40 => DecodedInstruction::MoveOnOverflow {
                    source,
                    destination,
                },
                0x42 => DecodedInstruction::MoveOnBorrow {
                    source,
                    destination,
                },
                0x47 => DecodedInstruction::MoveOnAbove {
                    source,
                    destination,
                },
                0x4c => DecodedInstruction::MoveOnLess {
                    source,
                    destination,
                },
                _ => DecodedInstruction::MoveOnGreater {
                    source,
                    destination,
                },
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xaf, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::Multiply {
                source,
                destination,
            },
            4,
        ));
    }
    if let [rex, 0xc1, modrm, 63, ..] = bytes
        && rex & !1 == 0x48
        && modrm & 0xf8 == 0xf8
    {
        return Ok((
            DecodedInstruction::ArithmeticShiftRight63 {
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xba, modrm, 63, ..] = bytes
        && rex & !1 == 0x48
        && modrm & 0xf8 == 0xf8
    {
        return Ok((
            DecodedInstruction::ComplementBit63 {
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            5,
        ));
    }
    if let [rex, 0x0f, opcode, modrm, ..] = bytes
        && rex & !1 == 0x40
        && matches!(*opcode, 0x92 | 0x94 | 0x96 | 0x9c | 0x9e)
        && modrm & 0xf8 == 0xc0
    {
        return Ok((
            DecodedInstruction::SetBoolean {
                condition: opcode & 0x0f,
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xb6, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::ZeroExtendU8 {
                source,
                destination,
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xb7, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::ZeroExtendU16 {
                source,
                destination,
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbe, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::SignExtendI8 {
                source,
                destination,
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbf, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::SignExtendI16 {
                source,
                destination,
            },
            4,
        ));
    }
    if let [rex, 0x63, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        let (source, destination) = modrm_source_destination(*modrm, *rex);
        return Ok((
            DecodedInstruction::SignExtendI32 {
                source,
                destination,
            },
            3,
        ));
    }
    if let [rex, 0x89, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::ZeroExtendU32 {
                source: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            3,
        ));
    }
    if let [0xff, 0x25, d0, d1, d2, d3, ..] = bytes {
        return Ok((
            DecodedInstruction::JumpIndirectRip {
                displacement: i32::from_le_bytes([*d0, *d1, *d2, *d3]),
            },
            6,
        ));
    }
    if bytes.starts_with(&[0x0f, 0x0b]) {
        return Ok((DecodedInstruction::Crash, 2));
    }
    if bytes.first() == Some(&0xc3) {
        return Ok((DecodedInstruction::Return, 1));
    }
    if let Some(decoded) = decode_atomic(bytes) {
        return Ok(decoded);
    }
    let (&rex, rest) = bytes
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    if !(0x48..=0x4f).contains(&rex) {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let (&opcode, _) = rest
        .split_first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let rex_r = (rex >> 2) & 1;
    let rex_x = (rex >> 1) & 1;
    let rex_b = rex & 1;
    if opcode == 0x8d
        && rex_x == 0
        && rex_b == 0
        && let Some([modrm, 0, 0, 0, 0]) = bytes
            .get(2..7)
            .and_then(|field| <[u8; 5]>::try_from(field).ok())
        && modrm & 0xc7 == 0x05
    {
        return Ok((
            DecodedInstruction::UnresolvedRipRelativeAddress {
                destination: ((modrm >> 3) & 7) | (rex_r << 3),
            },
            7,
        ));
    }
    if (0xb8..=0xbf).contains(&opcode) {
        let immediate = bytes
            .get(2..10)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        return Ok((
            DecodedInstruction::Materialize {
                destination: (opcode & 7) | (rex_b << 3),
                value: immediate,
            },
            10,
        ));
    }
    let modrm = *bytes
        .get(2)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let mode = modrm >> 6;
    let reg = ((modrm >> 3) & 7) | (rex_r << 3);
    let rm_low = modrm & 7;
    let rm = rm_low | (rex_b << 3);
    if opcode == 0x81 {
        if mode != 3 || reg != 7 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let immediate = bytes
            .get(3..7)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        return Ok((
            DecodedInstruction::CompareImmediate {
                register: rm,
                immediate,
            },
            7,
        ));
    }
    if matches!(
        opcode,
        0x89 | 0x85 | 0x39 | 0x31 | 0x29 | 0x01 | 0x21 | 0x09 | 0x19 | 0xf7 | 0xd3
    ) {
        if mode != 3 || bytes.len() < 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let decoded = match opcode {
            0x89 => mov(reg, rm),
            0x85 if reg == rm => DecodedInstruction::Test { register: reg },
            0x39 => DecodedInstruction::Compare {
                left: rm,
                right: reg,
            },
            0x31 => DecodedInstruction::Xor {
                source: reg,
                destination: rm,
            },
            0x29 => DecodedInstruction::Subtract {
                source: reg,
                destination: rm,
            },
            0x21 => DecodedInstruction::BitwiseAnd {
                source: reg,
                destination: rm,
            },
            0x09 => DecodedInstruction::Or {
                source: reg,
                destination: rm,
            },
            0x19 => DecodedInstruction::SubtractWithBorrow {
                source: reg,
                destination: rm,
            },
            0x01 => DecodedInstruction::Add {
                source: reg,
                destination: rm,
            },
            0xf7 if reg == 2 && rex_x == 0 => DecodedInstruction::Complement { destination: rm },
            0xf7 if reg == 4 && rex_x == 0 => {
                DecodedInstruction::UnsignedMultiply { multiplier: rm }
            }
            0xf7 if reg == 6 && rex_x == 0 => DecodedInstruction::UnsignedDivide { divisor: rm },
            0xf7 if reg == 7 && rex_x == 0 => DecodedInstruction::SignedDivide { divisor: rm },
            0xf7 if (modrm >> 3) & 7 == 3 => DecodedInstruction::Negate { destination: rm },
            0xd3 if reg == 4 => DecodedInstruction::ShiftLeftByCl { destination: rm },
            0xd3 if reg == 5 => DecodedInstruction::ShiftRightLogicalByCl { destination: rm },
            0xd3 if reg == 7 => DecodedInstruction::ShiftRightArithmeticByCl { destination: rm },
            _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
        };
        return Ok((decoded, 3));
    }
    if opcode != 0x8d || mode == 3 {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut length = 3;
    let (base, index) = if rm_low == 4 {
        let sib = *bytes
            .get(length)
            .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
        length += 1;
        if sib >> 6 != 0 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        let index_low = (sib >> 3) & 7;
        let base_low = sib & 7;
        if mode == 0 && base_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (
            base_low | (rex_b << 3),
            if index_low == 4 && rex_x == 0 {
                None
            } else {
                Some(index_low | (rex_x << 3))
            },
        )
    } else {
        if mode == 0 && rm_low == 5 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
        }
        (rm, None)
    };
    let displacement = match mode {
        0 => 0,
        1 => {
            let value = *bytes
                .get(length)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 1;
            i32::from(value as i8)
        }
        2 => {
            let value = bytes
                .get(length..length + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
            length += 4;
            value
        }
        _ => return Err(X86_64SelectedFormEncodingError::MalformedEncoding),
    };
    Ok((
        DecodedInstruction::Lea {
            destination: reg,
            base,
            index,
            displacement,
        },
        length,
    ))
}

/// Decode atomic constituent instructions: an optional operand-size prefix, an
/// optional LOCK prefix, an optional REX, then the opcode and the
/// `[base + disp32]` cell. Every byte is checked against the emitted shape;
/// a sequence that only shares a prefix stays `None` for the generic arms.
fn decode_atomic(bytes: &[u8]) -> Option<(DecodedInstruction, usize)> {
    let mut cursor = 0;
    let operand_sized = bytes.first() == Some(&0x66);
    if operand_sized {
        cursor += 1;
    }
    let locked = bytes.get(cursor) == Some(&0xf0);
    if locked {
        cursor += 1;
    }
    let rex = match bytes.get(cursor) {
        Some(&byte) if (0x40..=0x4f).contains(&byte) => {
            cursor += 1;
            byte
        }
        _ => 0x40,
    };
    let opcode = *bytes.get(cursor)?;
    if opcode == 0x0f {
        let second = *bytes.get(cursor + 1)?;
        // `mfence` closes the atomic store sequence.
        if second == 0xae {
            return (bytes.get(cursor + 2) == Some(&0xf0))
                .then_some((DecodedInstruction::MemoryFence, cursor + 3));
        }
        cursor += 2;
        let modrm = *bytes.get(cursor)?;
        cursor += 1;
        if modrm >> 6 != 2 {
            return None;
        }
        let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
        let base = (modrm & 7) | ((rex & 1) << 3);
        if modrm & 7 == 4 {
            let sib = *bytes.get(cursor)?;
            cursor += 1;
            if sib != 0x24 {
                return None;
            }
        }
        if !matches!(bytes.get(cursor..cursor + 4), Some(&[0, 0, 0, 0])) {
            return None;
        }
        cursor += 4;
        let wide = rex & 8 != 0;
        let bits = |eight: bool| -> u16 {
            if eight {
                8
            } else if operand_sized {
                16
            } else if wide {
                64
            } else {
                32
            }
        };
        let instruction = match second {
            0xb6 if !locked => DecodedInstruction::AtomicLoad {
                destination: register,
                base,
                bits: 8,
            },
            0xb7 if !locked => DecodedInstruction::AtomicLoad {
                destination: register,
                base,
                bits: 16,
            },
            0xb0 if locked => DecodedInstruction::AtomicLockedCompareExchange {
                register,
                base,
                bits: 8,
            },
            0xb1 if locked => DecodedInstruction::AtomicLockedCompareExchange {
                register,
                base,
                bits: bits(false),
            },
            0xc0 if locked => DecodedInstruction::AtomicLockedAdd {
                register,
                base,
                bits: 8,
            },
            0xc1 if locked => DecodedInstruction::AtomicLockedAdd {
                register,
                base,
                bits: bits(false),
            },
            _ => return None,
        };
        return Some((instruction, cursor));
    }
    let modrm = *bytes.get(cursor + 1)?;
    match opcode {
        0x20 | 0x21 | 0x08 | 0x09 | 0x30 | 0x31 if !locked && modrm >> 6 == 3 => {
            let operation = match opcode & !1 {
                0x20 => selected_instructions::AtomicOperation::FetchAnd,
                0x08 => selected_instructions::AtomicOperation::FetchOr,
                0x30 => selected_instructions::AtomicOperation::FetchXor,
                _ => unreachable!(),
            };
            // Existing 64-bit scalar operations keep their ordinary decoded
            // vocabulary. The retry validator accepts that vocabulary too.
            if rex & 8 != 0 && opcode & 1 != 0 && !operand_sized {
                return None;
            }
            Some((
                DecodedInstruction::AtomicBitwise {
                    operation,
                    source: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
                    destination: (modrm & 7) | ((rex & 1) << 3),
                    bits: if opcode & 1 == 0 {
                        8
                    } else if operand_sized {
                        16
                    } else {
                        32
                    },
                },
                cursor + 2,
            ))
        }
        // `movzx`/`mov` into the observed output and `mov`/`xchg` against
        // the cell share the `[base + disp32]` operand.
        0x8b | 0x88 | 0x89 | 0x86 | 0x87 if modrm >> 6 == 2 => {
            let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
            let base = (modrm & 7) | ((rex & 1) << 3);
            let mut length = cursor + 2;
            if modrm & 7 == 4 {
                if bytes.get(length) != Some(&0x24) {
                    return None;
                }
                length += 1;
            }
            if !matches!(bytes.get(length..length + 4), Some(&[0, 0, 0, 0])) {
                return None;
            }
            length += 4;
            let wide = rex & 8 != 0;
            let bits = |eight: bool| -> u16 {
                if eight {
                    8
                } else if operand_sized {
                    16
                } else if wide {
                    64
                } else {
                    32
                }
            };
            let instruction = match opcode {
                0x8b => DecodedInstruction::AtomicLoad {
                    destination: register,
                    base,
                    bits: if wide { 64 } else { 32 },
                },
                0x88 => DecodedInstruction::AtomicStore {
                    source: register,
                    base,
                    bits: 8,
                },
                0x89 => DecodedInstruction::AtomicStore {
                    source: register,
                    base,
                    bits: bits(false),
                },
                0x86 => DecodedInstruction::AtomicExchange {
                    register,
                    base,
                    bits: 8,
                },
                _ => DecodedInstruction::AtomicExchange {
                    register,
                    base,
                    bits: bits(false),
                },
            };
            Some((instruction, length))
        }
        // `neg` in the fetch-sub prefix.
        0xf6 | 0xf7 if modrm >> 6 == 3 && (modrm >> 3) & 7 == 3 => Some((
            DecodedInstruction::Negate {
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            cursor + 2,
        )),
        _ => None,
    }
}

/// The divide/remainder custody layout: dividend halves zeroed at
/// registers 0 and 2, the raw divisor at 1, and the scratch pair
/// holding quotient and remainder across registers 1 and 3.
fn divide_register_layout(registers: &[u8]) -> bool {
    registers[0] == 0 && registers[2] == 0 && registers[3] == 2 && registers[1] != 2
}

pub(crate) fn validate_decoded(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
    decoded: &[DecodedInstruction],
) -> Result<(), X86_64SelectedFormEncodingError> {
    let valid = match kind {
        SelectedInstructionKind::PlacedScalarAccess {
            form, byte_offset, ..
        } => {
            let ordinary = form
                .ordinary_kind(byte_offset)
                .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            return validate_decoded(
                ordinary,
                selected_instructions::MachineAlternativeKey {
                    family: (if form.write {
                        selected_instructions::MachineSemanticKind::Store
                    } else {
                        match form.bytes {
                            1 => selected_instructions::MachineSemanticKind::Load8,
                            2 => selected_instructions::MachineSemanticKind::Load16,
                            4 => selected_instructions::MachineSemanticKind::Load32,
                            _ => selected_instructions::MachineSemanticKind::Load64,
                        }
                    })
                    .into(),
                    variant: alternative.variant,
                },
                registers,
                decoded,
            );
        }
        SelectedInstructionKind::AtomicAccess { form } => {
            let base = registers[0];
            let bits = match form.width {
                selected_instructions::AtomicWidth::Bits8 => 8,
                selected_instructions::AtomicWidth::Bits16 => 16,
                selected_instructions::AtomicWidth::Bits32 => 32,
                selected_instructions::AtomicWidth::Bits64 => 64,
            };
            match form.operation {
                selected_instructions::AtomicOperation::Load => {
                    decoded
                        == [DecodedInstruction::AtomicLoad {
                            destination: registers[1],
                            base,
                            bits,
                        }]
                }
                selected_instructions::AtomicOperation::Store => {
                    decoded
                        == [
                            DecodedInstruction::AtomicStore {
                                source: registers[1],
                                base,
                                bits,
                            },
                            DecodedInstruction::MemoryFence,
                        ]
                }
                selected_instructions::AtomicOperation::Swap => {
                    decoded
                        == [
                            mov(registers[1], registers[2]),
                            DecodedInstruction::AtomicExchange {
                                register: registers[2],
                                base,
                                bits,
                            },
                        ]
                }
                selected_instructions::AtomicOperation::FetchAdd => {
                    decoded
                        == [
                            mov(registers[1], registers[2]),
                            DecodedInstruction::AtomicLockedAdd {
                                register: registers[2],
                                base,
                                bits,
                            },
                        ]
                }
                selected_instructions::AtomicOperation::FetchSub => {
                    decoded
                        == [
                            mov(registers[1], registers[2]),
                            DecodedInstruction::Negate {
                                destination: registers[2],
                            },
                            DecodedInstruction::AtomicLockedAdd {
                                register: registers[2],
                                base,
                                bits,
                            },
                        ]
                }
                selected_instructions::AtomicOperation::CompareExchange => {
                    decoded
                        == [
                            DecodedInstruction::AtomicLockedCompareExchange {
                                register: registers[2],
                                base,
                                bits,
                            },
                            mov(0, registers[3]),
                        ]
                }
                operation @ (selected_instructions::AtomicOperation::FetchAnd
                | selected_instructions::AtomicOperation::FetchOr
                | selected_instructions::AtomicOperation::FetchXor) => {
                    let source = registers[1];
                    let destination = registers[2];
                    let bitwise = if bits == 64 {
                        match operation {
                            selected_instructions::AtomicOperation::FetchAnd => {
                                DecodedInstruction::BitwiseAnd {
                                    source,
                                    destination,
                                }
                            }
                            selected_instructions::AtomicOperation::FetchOr => {
                                DecodedInstruction::Or {
                                    source,
                                    destination,
                                }
                            }
                            selected_instructions::AtomicOperation::FetchXor => {
                                DecodedInstruction::Xor {
                                    source,
                                    destination,
                                }
                            }
                            _ => unreachable!(),
                        }
                    } else {
                        DecodedInstruction::AtomicBitwise {
                            operation,
                            source,
                            destination,
                            bits,
                        }
                    };
                    // Count instruction lengths independently of the encoder:
                    // MOV is 3, bitwise is prefix + opcode/ModRM, CMPXCHG is
                    // LOCK + optional width/REX + 0F opcode/ModRM + SIB/disp32.
                    let width_prefix = usize::from(bits == 16);
                    let bit_rex =
                        usize::from(bits == 8 || bits == 64 || source >= 8 || destination >= 8);
                    let cmp_rex =
                        usize::from(bits == 8 || bits == 64 || destination >= 8 || base >= 8);
                    let retry_length = 3
                        + width_prefix
                        + bit_rex
                        + 2
                        + 1
                        + width_prefix
                        + cmp_rex
                        + 3
                        + usize::from(base & 7 == 4)
                        + 4
                        + 2;
                    decoded
                        == [
                            DecodedInstruction::AtomicLoad {
                                destination: 0,
                                base,
                                bits,
                            },
                            DecodedInstruction::Move {
                                source: 0,
                                destination,
                            },
                            bitwise,
                            DecodedInstruction::AtomicLockedCompareExchange {
                                register: destination,
                                base,
                                bits,
                            },
                            DecodedInstruction::JumpNotEqualShort {
                                displacement: -(retry_length as i8),
                            },
                            DecodedInstruction::Move {
                                source: 0,
                                destination,
                            },
                        ]
                }
            }
        }
        SelectedInstructionKind::NearestIeeeFloatArithmetic { .. }
        | SelectedInstructionKind::FloatResultPolicy { .. } => false,
        SelectedInstructionKind::AtomicFence => {
            registers.is_empty() && decoded == [DecodedInstruction::MemoryFence]
        }
        SelectedInstructionKind::Crash => {
            registers.is_empty() && decoded == [DecodedInstruction::Crash]
        }
        SelectedInstructionKind::MaterializeI64 { value } if alternative.variant == 1 => {
            integer_bits(value)? == 0
                && decoded
                    == [DecodedInstruction::Xor {
                        source: registers[0],
                        destination: registers[0],
                    }]
        }
        SelectedInstructionKind::MaterializeI64 { value } => {
            decoded
                == [DecodedInstruction::Materialize {
                    destination: registers[0],
                    value: integer_bits(value)?,
                }]
        }
        SelectedInstructionKind::ImmutableImageAddress { .. } => {
            decoded
                == [DecodedInstruction::UnresolvedRipRelativeAddress {
                    destination: registers[0],
                }]
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
            decoded
                == [
                    DecodedInstruction::SetBoolean {
                        condition,
                        destination: registers[0],
                    },
                    DecodedInstruction::ZeroExtendU8 {
                        source: registers[0],
                        destination: registers[0],
                    },
                ]
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            decoded
                == [DecodedInstruction::ZeroExtendU8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            decoded
                == [DecodedInstruction::ZeroExtendU16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI8 => {
            decoded
                == [DecodedInstruction::SignExtendI8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI16 => {
            decoded
                == [DecodedInstruction::SignExtendI16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI32 => {
            decoded
                == [DecodedInstruction::SignExtendI32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            decoded
                == [DecodedInstruction::ZeroExtendU32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::CopyI64 => decoded == [mov(registers[0], registers[1])],
        SelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedInstruction::Test {
                    register: registers[0],
                }]
        }
        SelectedInstructionKind::CompareI64 => {
            decoded
                == [DecodedInstruction::Compare {
                    left: registers[0],
                    right: registers[1],
                }]
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            decoded
                == [DecodedInstruction::CompareImmediate {
                    register: registers[0],
                    immediate: u12(immediate)?,
                }]
        }
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. } => {
            matches!(decoded, [DecodedInstruction::Lea { destination, base, index: Some(index), displacement: 0 }]
                if *destination == registers[2]
                    && ((*base == registers[0] && *index == registers[1])
                        || (*base == registers[1] && *index == registers[0])))
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [DecodedInstruction::UnsignedDivide {
                        divisor: registers[1],
                    }]
        }
        SelectedInstructionKind::ExactRemainderU64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [
                        DecodedInstruction::Xor {
                            source: 2,
                            destination: 2,
                        },
                        DecodedInstruction::UnsignedDivide {
                            divisor: registers[1],
                        },
                        mov(2, 0),
                    ]
        }
        SelectedInstructionKind::WrappingDivideI64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [
                        DecodedInstruction::CompareSignedImmediate8 {
                            register: registers[1],
                            immediate: -1,
                        },
                        DecodedInstruction::JumpNotEqualShort { displacement: 5 },
                        DecodedInstruction::Negate { destination: 0 },
                        DecodedInstruction::JumpShort { displacement: 5 },
                        DecodedInstruction::SignExtendDividend,
                        DecodedInstruction::SignedDivide {
                            divisor: registers[1],
                        },
                    ]
        }
        SelectedInstructionKind::ExactDivideI64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [
                        DecodedInstruction::SignExtendDividend,
                        DecodedInstruction::SignedDivide {
                            divisor: registers[1],
                        },
                    ]
        }
        SelectedInstructionKind::ExactRemainderI64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [
                        DecodedInstruction::SignExtendDividend,
                        DecodedInstruction::SignedDivide {
                            divisor: registers[1],
                        },
                        mov(2, 0),
                    ]
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            divide_register_layout(registers)
                && decoded
                    == [
                        DecodedInstruction::Xor {
                            source: 2,
                            destination: 2,
                        },
                        DecodedInstruction::CompareSignedImmediate8 {
                            register: registers[1],
                            immediate: -1,
                        },
                        DecodedInstruction::JumpEqualShort { displacement: 5 },
                        DecodedInstruction::SignExtendDividend,
                        DecodedInstruction::SignedDivide {
                            divisor: registers[1],
                        },
                        mov(2, 0),
                    ]
        }
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            saturating_matches(SaturatingOperation::Add, carrier, registers, decoded)
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            saturating_matches(SaturatingOperation::Subtract, carrier, registers, decoded)
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            saturating_matches(SaturatingOperation::Divide, carrier, registers, decoded)
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            saturating_matches(SaturatingOperation::Remainder, carrier, registers, decoded)
        }
        SelectedInstructionKind::SaturatingMultiply { carrier } => {
            saturating_matches(SaturatingOperation::Multiply, carrier, registers, decoded)
        }
        SelectedInstructionKind::TrappingInteger { form } => {
            TrappingShape::of(form).accepts_registers(registers)
                && decoded == expected_trapping(form, registers)
        }
        SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseOrI64
        | SelectedInstructionKind::BitwiseXorI64 => {
            let operation = |source, destination| match kind {
                SelectedInstructionKind::BitwiseXorI64 => DecodedInstruction::Xor {
                    source,
                    destination,
                },
                SelectedInstructionKind::BitwiseOrI64 => DecodedInstruction::Or {
                    source,
                    destination,
                },
                _ => DecodedInstruction::BitwiseAnd {
                    source,
                    destination,
                },
            };
            if registers[2] == registers[0] {
                decoded == [operation(registers[1], registers[2])]
            } else if registers[2] == registers[1] {
                decoded == [operation(registers[0], registers[2])]
            } else {
                decoded
                    == [
                        mov(registers[0], registers[2]),
                        operation(registers[1], registers[2]),
                    ]
            }
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedInstruction::Lea {
                    destination: registers[1],
                    base: registers[0],
                    index: None,
                    displacement: i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                }]
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            decoded
                == [DecodedInstruction::Lea {
                    destination: registers[1],
                    base: registers[0],
                    index: None,
                    displacement: -i32::try_from(u12(immediate)?).expect("u12 fits i32"),
                }]
        }
        SelectedInstructionKind::BitwiseNotI64 => {
            if registers[1] == registers[0] {
                decoded
                    == [DecodedInstruction::Complement {
                        destination: registers[1],
                    }]
            } else {
                decoded
                    == [
                        mov(registers[0], registers[1]),
                        DecodedInstruction::Complement {
                            destination: registers[1],
                        },
                    ]
            }
        }
        SelectedInstructionKind::ExactSubtractI64 { .. }
        | SelectedInstructionKind::WrappingSubtractI64 => match alternative.variant {
            0 => {
                decoded
                    == [DecodedInstruction::Xor {
                        source: registers[2],
                        destination: registers[2],
                    }]
            }
            1 => {
                decoded
                    == [DecodedInstruction::Subtract {
                        source: registers[1],
                        destination: registers[2],
                    }]
            }
            2 => {
                decoded
                    == [
                        DecodedInstruction::Negate {
                            destination: registers[2],
                        },
                        DecodedInstruction::Add {
                            source: registers[0],
                            destination: registers[2],
                        },
                    ]
            }
            3 => {
                decoded
                    == [
                        mov(registers[0], registers[2]),
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
            }
            _ => false,
        },
        SelectedInstructionKind::ExactMultiplyI64 { .. }
        | SelectedInstructionKind::WrappingMultiplyI64 => match alternative.variant {
            0 => {
                decoded
                    == [DecodedInstruction::Multiply {
                        source: registers[2],
                        destination: registers[2],
                    }]
            }
            1 => {
                decoded
                    == [DecodedInstruction::Multiply {
                        source: registers[1],
                        destination: registers[2],
                    }]
            }
            2 => {
                decoded
                    == [DecodedInstruction::Multiply {
                        source: registers[0],
                        destination: registers[2],
                    }]
            }
            3 => {
                decoded
                    == [
                        mov(registers[0], registers[2]),
                        DecodedInstruction::Multiply {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
            }
            _ => false,
        },
        SelectedInstructionKind::WrappingShiftLeftI64
        | SelectedInstructionKind::WrappingShiftRightI64
        | SelectedInstructionKind::WrappingShiftRightU64
        | selected_instructions::exact_shift_i64_pair_pattern!()
        | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
            // The count source must resolve to the pinned RCX view; the
            // realization is the unconditional value copy into the result
            // followed by the in-place variable-count shift.
            let shift = match kind {
                SelectedInstructionKind::WrappingShiftLeftI64
                | SelectedInstructionKind::ExactShiftLeftI64 { .. } => {
                    DecodedInstruction::ShiftLeftByCl {
                        destination: registers[2],
                    }
                }
                SelectedInstructionKind::WrappingShiftRightU64
                | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
                    DecodedInstruction::ShiftRightLogicalByCl {
                        destination: registers[2],
                    }
                }
                _ => DecodedInstruction::ShiftRightArithmeticByCl {
                    destination: registers[2],
                },
            };
            registers[1] == 1 && decoded == [mov(registers[0], registers[2]), shift]
        }
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => decoded == [DecodedInstruction::Return],
        selected_instructions::layout_dependent_selected_transfers!()
        | selected_instructions::dedicated_selected_forms!() => false,
    };
    if valid {
        Ok(())
    } else {
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

/// Whether `decoded` is exactly the realization of one saturating operation
/// on one carrier for these registers, including the form's register pins.
fn saturating_matches(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    registers: &[u8],
    decoded: &[DecodedInstruction],
) -> bool {
    let form = SaturatingForm::of(operation, carrier);
    form.accepts_registers(registers)
        && decoded == expected_saturating(operation, carrier, registers)
}

/// The decoded instruction sequence `encode_unchecked` emits for one
/// saturating operation and carrier, spelled independently of the byte
/// assembler so a replayed encoding is checked instruction by instruction.
fn expected_saturating(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    registers: &[u8],
) -> Vec<DecodedInstruction> {
    let (left, right, result) = (registers[0], registers[1], registers[2]);
    let arithmetic = |source, destination| {
        if operation == SaturatingOperation::Add {
            DecodedInstruction::Add {
                source,
                destination,
            }
        } else {
            DecodedInstruction::Subtract {
                source,
                destination,
            }
        }
    };
    let clamp_with = |value, scratch, bound_bits, select: DecodedInstruction| {
        [
            DecodedInstruction::Materialize {
                destination: scratch,
                value: bound_bits,
            },
            DecodedInstruction::Compare {
                left: value,
                right: scratch,
            },
            select,
        ]
    };
    let clamp = |value, scratch, bound_bits, upper| {
        clamp_with(
            value,
            scratch,
            bound_bits,
            if upper {
                DecodedInstruction::MoveOnGreater {
                    source: scratch,
                    destination: value,
                }
            } else {
                DecodedInstruction::MoveOnLess {
                    source: scratch,
                    destination: value,
                }
            },
        )
    };
    match SaturatingForm::of(operation, carrier) {
        SaturatingForm::SubtractUnsigned => vec![
            DecodedInstruction::Compare { left, right },
            mov(left, result),
            DecodedInstruction::MoveOnBorrow {
                source: right,
                destination: result,
            },
            DecodedInstruction::Subtract {
                source: right,
                destination: result,
            },
        ],
        SaturatingForm::AddU64 => vec![
            mov(left, result),
            DecodedInstruction::Complement {
                destination: result,
            },
            DecodedInstruction::Compare {
                left: result,
                right,
            },
            DecodedInstruction::MoveOnBorrow {
                source: right,
                destination: result,
            },
            DecodedInstruction::Subtract {
                source: right,
                destination: result,
            },
            DecodedInstruction::Complement {
                destination: result,
            },
        ],
        form @ (SaturatingForm::ClampSignedNarrow | SaturatingForm::ClampUnsignedNarrow) => {
            let scratch = registers[3];
            let mut expected = vec![mov(left, result), arithmetic(right, result)];
            expected.extend(clamp(result, scratch, carrier.maximum_bits(), true));
            if form == SaturatingForm::ClampSignedNarrow {
                expected.extend(clamp(result, scratch, carrier.minimum_bits(), false));
            }
            expected
        }
        SaturatingForm::OverflowSelectI64 => {
            let scratch = registers[3];
            vec![
                mov(left, scratch),
                DecodedInstruction::Complement {
                    destination: scratch,
                },
                DecodedInstruction::ArithmeticShiftRight63 {
                    destination: scratch,
                },
                DecodedInstruction::ComplementBit63 {
                    destination: scratch,
                },
                mov(left, result),
                arithmetic(right, result),
                DecodedInstruction::MoveOnOverflow {
                    source: scratch,
                    destination: result,
                },
            ]
        }
        SaturatingForm::DivideUnsigned => {
            vec![DecodedInstruction::UnsignedDivide { divisor: right }]
        }
        SaturatingForm::DivideSignedNarrow => {
            let mut expected = vec![
                DecodedInstruction::SignExtendDividend,
                DecodedInstruction::SignedDivide { divisor: right },
            ];
            expected.extend(clamp(0, 2, carrier.maximum_bits(), true));
            expected
        }
        SaturatingForm::DivideI64 => vec![
            DecodedInstruction::CompareSignedImmediate8 {
                register: right,
                immediate: -1,
            },
            DecodedInstruction::SubtractWithBorrow {
                source: 2,
                destination: 2,
            },
            DecodedInstruction::Or {
                source: 0,
                destination: 2,
            },
            DecodedInstruction::Negate { destination: 2 },
            DecodedInstruction::Lea {
                destination: 2,
                base: 0,
                index: None,
                displacement: 1,
            },
            DecodedInstruction::MoveOnOverflow {
                source: 2,
                destination: 0,
            },
            DecodedInstruction::SignExtendDividend,
            DecodedInstruction::SignedDivide { divisor: right },
        ],
        SaturatingForm::RemainderUnsigned => vec![
            DecodedInstruction::Xor {
                source: 2,
                destination: 2,
            },
            DecodedInstruction::UnsignedDivide { divisor: right },
            mov(2, result),
        ],
        SaturatingForm::RemainderSigned => vec![
            DecodedInstruction::Xor {
                source: 2,
                destination: 2,
            },
            DecodedInstruction::CompareSignedImmediate8 {
                register: right,
                immediate: -1,
            },
            DecodedInstruction::JumpEqualShort { displacement: 5 },
            DecodedInstruction::SignExtendDividend,
            DecodedInstruction::SignedDivide { divisor: right },
            mov(2, result),
        ],
        form @ (SaturatingForm::MultiplySignedNarrow | SaturatingForm::MultiplyUnsignedNarrow) => {
            let scratch = registers[3];
            let mut expected = vec![
                mov(left, result),
                DecodedInstruction::Multiply {
                    source: right,
                    destination: result,
                },
            ];
            if form == SaturatingForm::MultiplySignedNarrow {
                expected.extend(clamp(result, scratch, carrier.maximum_bits(), true));
                expected.extend(clamp(result, scratch, carrier.minimum_bits(), false));
            } else {
                expected.extend(clamp_with(
                    result,
                    scratch,
                    carrier.maximum_bits(),
                    DecodedInstruction::MoveOnAbove {
                        source: scratch,
                        destination: result,
                    },
                ));
            }
            expected
        }
        SaturatingForm::MultiplyI64 => {
            let scratch = registers[3];
            vec![
                mov(left, scratch),
                DecodedInstruction::Xor {
                    source: right,
                    destination: scratch,
                },
                DecodedInstruction::Complement {
                    destination: scratch,
                },
                DecodedInstruction::ArithmeticShiftRight63 {
                    destination: scratch,
                },
                DecodedInstruction::ComplementBit63 {
                    destination: scratch,
                },
                mov(left, result),
                DecodedInstruction::Multiply {
                    source: right,
                    destination: result,
                },
                DecodedInstruction::MoveOnOverflow {
                    source: scratch,
                    destination: result,
                },
            ]
        }
        SaturatingForm::MultiplyU64 => vec![
            DecodedInstruction::UnsignedMultiply { multiplier: right },
            DecodedInstruction::SubtractWithBorrow {
                source: 2,
                destination: 2,
            },
            DecodedInstruction::Or {
                source: 2,
                destination: 0,
            },
        ],
    }
}

/// The decoded instruction sequence `encode_unchecked` emits for one
/// Trapping form, spelled independently of the byte assembler: every check
/// is a conditional jump over the two-byte UD2 (`Crash`), and the signed
/// divisor test jumps over the whole dividend guard.
fn expected_trapping(form: TrappingForm, registers: &[u8]) -> Vec<DecodedInstruction> {
    let trap_unless = |jump: DecodedInstruction| [jump, DecodedInstruction::Crash];
    let trap_unless_equal = trap_unless(DecodedInstruction::JumpEqualShort { displacement: 2 });
    let trap_on_overflow = trap_unless(DecodedInstruction::JumpNoOverflowShort { displacement: 2 });
    let range_check = |check: RangeCheck, value: u8, scratch: u8| {
        let mut expected = match check {
            RangeCheck::SignExtension { bits } => {
                let (source, destination) = (value, scratch);
                vec![
                    match bits {
                        8 => DecodedInstruction::SignExtendI8 {
                            source,
                            destination,
                        },
                        16 => DecodedInstruction::SignExtendI16 {
                            source,
                            destination,
                        },
                        _ => DecodedInstruction::SignExtendI32 {
                            source,
                            destination,
                        },
                    },
                    DecodedInstruction::Compare {
                        left: scratch,
                        right: value,
                    },
                ]
            }
            RangeCheck::HighBits { shift } => vec![
                mov(value, scratch),
                DecodedInstruction::ShiftRightLogicalImmediate {
                    destination: scratch,
                    count: shift,
                },
            ],
        };
        expected.extend(trap_unless_equal);
        expected
    };
    let arithmetic = |arithmetic: TrappingArithmetic| {
        let (left, right, result) = (registers[0], registers[1], registers[2]);
        vec![
            mov(left, result),
            match arithmetic {
                TrappingArithmetic::Add => DecodedInstruction::Add {
                    source: right,
                    destination: result,
                },
                TrappingArithmetic::Subtract => DecodedInstruction::Subtract {
                    source: right,
                    destination: result,
                },
                TrappingArithmetic::Multiply => DecodedInstruction::Multiply {
                    source: right,
                    destination: result,
                },
            },
        ]
    };
    // `cmp rcx, width; jb; ud2; mov result, value` before every shift, and
    // the carrier's sign-preserving right shift.
    let shift_prefix = || {
        let mut expected = vec![DecodedInstruction::CompareSignedImmediate8 {
            register: registers[1],
            immediate: form.carrier.bits() as i8,
        }];
        expected.extend(trap_unless(DecodedInstruction::JumpBelowShort {
            displacement: 2,
        }));
        expected.push(mov(registers[0], registers[2]));
        expected
    };
    let shift_right = |destination| {
        if form.carrier.is_signed() {
            DecodedInstruction::ShiftRightArithmeticByCl { destination }
        } else {
            DecodedInstruction::ShiftRightLogicalByCl { destination }
        }
    };
    match TrappingShape::of(form) {
        TrappingShape::OverflowFlag(operation) => {
            let mut expected = arithmetic(operation);
            expected.extend(trap_on_overflow);
            expected
        }
        TrappingShape::CarryFlag(operation) => {
            let mut expected = arithmetic(operation);
            expected.extend(trap_unless(DecodedInstruction::JumpAboveOrEqualShort {
                displacement: 2,
            }));
            expected
        }
        TrappingShape::NarrowRange(operation, check) => {
            let mut expected = arithmetic(operation);
            expected.extend(range_check(check, registers[2], registers[3]));
            expected
        }
        TrappingShape::MultiplyHighHalf => {
            let mut expected = vec![DecodedInstruction::UnsignedMultiply {
                multiplier: registers[1],
            }];
            expected.extend(trap_on_overflow);
            expected
        }
        TrappingShape::Division { guard, remainder } => {
            let divisor = registers[1];
            let mut expected = vec![DecodedInstruction::Test { register: divisor }];
            expected.extend(trap_unless(DecodedInstruction::JumpNotEqualShort {
                displacement: 2,
            }));
            let minimum_trap = |compare, skip, jump| {
                let mut guard = vec![
                    DecodedInstruction::CompareSignedImmediate8 {
                        register: divisor,
                        immediate: -1,
                    },
                    DecodedInstruction::JumpNotEqualShort { displacement: skip },
                    compare,
                ];
                guard.extend(trap_unless(jump));
                guard.extend([
                    DecodedInstruction::SignExtendDividend,
                    DecodedInstruction::SignedDivide { divisor },
                ]);
                guard
            };
            expected.extend(match guard {
                None => vec![
                    DecodedInstruction::Xor {
                        source: 2,
                        destination: 2,
                    },
                    DecodedInstruction::UnsignedDivide { divisor },
                ],
                Some(DividendGuard::Minimum8(minimum)) => minimum_trap(
                    DecodedInstruction::CompareSignedImmediate8 {
                        register: 0,
                        immediate: minimum,
                    },
                    8,
                    DecodedInstruction::JumpNotEqualShort { displacement: 2 },
                ),
                Some(DividendGuard::Minimum32(minimum)) => minimum_trap(
                    DecodedInstruction::CompareRaxImmediate32 { immediate: minimum },
                    10,
                    DecodedInstruction::JumpNotEqualShort { displacement: 2 },
                ),
                Some(DividendGuard::DecrementOverflows) => minimum_trap(
                    DecodedInstruction::CompareSignedImmediate8 {
                        register: 0,
                        immediate: 1,
                    },
                    8,
                    DecodedInstruction::JumpNoOverflowShort { displacement: 2 },
                ),
            });
            if remainder {
                expected.push(mov(2, 0));
            }
            expected
        }
        TrappingShape::ShiftLeftRoundTrip { .. } => {
            let (value, result, scratch) = (registers[0], registers[2], registers[3]);
            let mut expected = shift_prefix();
            expected.extend([
                DecodedInstruction::ShiftLeftByCl {
                    destination: result,
                },
                mov(result, scratch),
                shift_right(scratch),
                DecodedInstruction::Compare {
                    left: scratch,
                    right: value,
                },
            ]);
            expected.extend(trap_unless_equal);
            expected
        }
        TrappingShape::ShiftLeftRange { check, .. } => {
            let mut expected = shift_prefix();
            expected.push(DecodedInstruction::ShiftLeftByCl {
                destination: registers[2],
            });
            expected.extend(range_check(check, registers[2], registers[3]));
            expected
        }
        TrappingShape::ShiftRight { .. } => {
            let mut expected = shift_prefix();
            expected.push(shift_right(registers[2]));
            expected
        }
        TrappingShape::Convert(check) => {
            let (operand, result, scratch) = (registers[0], registers[1], registers[2]);
            let mut expected = check
                .map(|check| range_check(check, operand, scratch))
                .unwrap_or_default();
            expected.push(mov(operand, result));
            expected
        }
    }
}

/// Every Trapping form reads its row's inputs, defines its result and
/// scratch, clobbers RFLAGS, and may stop at its inline UD2.
fn trapping_footprint(
    form: TrappingForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let shape = TrappingShape::of(form);
    let (reads, writes) = shape.operand_reads_and_writes();
    X86_64SelectedFormFootprint {
        register_reads: reads
            .iter()
            .map(|&position| operands[usize::from(position)])
            .collect(),
        register_writes: writes
            .iter()
            .map(|&position| operands[usize::from(position)])
            .collect(),
        writes_rflags: true,
        encoded: shape.encoded_effects(),
    }
}

/// Every saturating form reads and writes the positions its shape declares,
/// clobbers RFLAGS, and every division additionally clobbers RDX and keeps
/// the architectural fault behaviour of unsigned division. The u64 multiply
/// shares the division pins but defines RDX as an operand and never faults.
fn saturating_footprint(
    form: SaturatingForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let physical = crate::x86_64_physical_register_model_ref();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let (reads, writes) = form.operand_reads_and_writes();
    let register_reads = reads
        .iter()
        .map(|&position| operands[position as usize])
        .collect();
    let register_writes = writes
        .iter()
        .map(|&position| operands[position as usize])
        .collect();
    let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
    encoded.implicit_unit_clobbers = units("rflags");
    if form.is_division() {
        // The quotient forms read an explicit RDX input the divider still
        // writes through, so RDX is an implicit clobber there; the remainder
        // forms declare RDX as an output operand instead.
        if !matches!(
            form,
            SaturatingForm::RemainderUnsigned | SaturatingForm::RemainderSigned
        ) {
            encoded.implicit_unit_clobbers.extend(units("rdx"));
            encoded.implicit_unit_clobbers.sort_unstable();
            encoded.implicit_unit_clobbers.dedup();
        }
        encoded.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    }
    X86_64SelectedFormFootprint {
        register_reads,
        register_writes,
        writes_rflags: true,
        encoded,
    }
}

pub(crate) fn footprint(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    if let Some((operation, carrier)) = saturating_operation(kind) {
        return saturating_footprint(SaturatingForm::of(operation, carrier), operands);
    }
    if let Some(form) = trapping_form(kind) {
        return trapping_footprint(form, operands);
    }
    if let SelectedInstructionKind::NearestIeeeFloatArithmetic { form } = kind {
        return super::ieee_float_arithmetic::footprint(form, operands);
    }
    if let SelectedInstructionKind::AtomicAccess { form } = kind {
        return super::atomic::footprint(form, operands);
    }
    let (reads, writes, writes_rflags) = match kind {
        SelectedInstructionKind::PlacedScalarAccess { form, .. } => {
            if form.write {
                (vec![operands[0], operands[1]], vec![], false)
            } else {
                (vec![operands[0]], vec![operands[1]], false)
            }
        }
        SelectedInstructionKind::FloatResultPolicy { form } => {
            return super::float_result_policy::footprint(form, operands);
        }
        SelectedInstructionKind::NearestIeeeFloatArithmetic { .. } => {
            unreachable!("float arithmetic footprint returned above")
        }
        SelectedInstructionKind::AtomicAccess { .. } => {
            unreachable!("atomic footprint returned above")
        }
        SelectedInstructionKind::AtomicFence | SelectedInstructionKind::Crash => {
            (Vec::new(), Vec::new(), false)
        }
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            (vec![], vec![operands[0]], false)
        }
        // The optimizer-selected `xor r64, r64` zero idiom writes RFLAGS.
        SelectedInstructionKind::MaterializeI64 { .. } => {
            (vec![], vec![operands[0]], alternative.variant == 1)
        }
        SelectedInstructionKind::ImmutableImageAddress { .. } => (vec![], vec![operands[0]], false),
        SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::ZeroExtendU32 => (vec![operands[0]], vec![operands[1]], false),
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CompareI64Immediate { .. } => (vec![operands[0]], vec![], true),
        SelectedInstructionKind::ExactDivideU64 { .. } => (
            vec![operands[0], operands[1], operands[3]],
            vec![operands[2]],
            true,
        ),
        selected_instructions::exact_wrapping_divrem_set_pattern!(..) => (
            vec![operands[0], operands[1]],
            vec![operands[2], operands[3]],
            true,
        ),
        SelectedInstructionKind::CompareI64 => (vec![operands[0], operands[1]], vec![], true),
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        // `not` never defines flags; only the guarding copy may precede it.
        SelectedInstructionKind::BitwiseNotI64 => (vec![operands[0]], vec![operands[1]], false),
        selected_instructions::exact_add_or_sub_pair_pattern!(..) => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. }
        | SelectedInstructionKind::WrappingSubtractI64
            if alternative.variant == 0 =>
        {
            (vec![], vec![operands[2]], true)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. }
        | SelectedInstructionKind::WrappingSubtractI64 => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        // `imul` reads its destination operand even when every operand shares
        // one view, so no variant collapses the reads the way `xor` does.
        SelectedInstructionKind::ExactMultiplyI64 { .. }
        | SelectedInstructionKind::WrappingMultiplyI64 => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        // The single shift form reads the value and the pinned RCX count and
        // writes the early-clobber result; SHx by CL writes RFLAGS.
        SelectedInstructionKind::WrappingShiftLeftI64
        | SelectedInstructionKind::WrappingShiftRightI64
        | SelectedInstructionKind::WrappingShiftRightU64
        | selected_instructions::exact_shift_i64_pair_pattern!()
        | SelectedInstructionKind::ExactShiftRightU64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseOrI64
        | SelectedInstructionKind::BitwiseXorI64 => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => (vec![], vec![], false),
        selected_instructions::layout_dependent_selected_transfers!()
        | selected_instructions::dedicated_selected_forms!() => (vec![], vec![], false),
        selected_instructions::saturating_add_or_subtract_pattern!()
        | SelectedInstructionKind::SaturatingDivide { .. }
        | SelectedInstructionKind::SaturatingRemainder { .. }
        | SelectedInstructionKind::SaturatingMultiply { .. } => {
            unreachable!("saturating forms handled above")
        }
        SelectedInstructionKind::TrappingInteger { .. } => {
            unreachable!("trapping forms handled above")
        }
    };
    let physical = crate::x86_64_physical_register_model_ref();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if kind == SelectedInstructionKind::AtomicFence {
        MachineEncodedEffects::atomic_fence_v1()
    } else if kind == SelectedInstructionKind::Crash {
        selected_instructions::MachineEncodedEffects::crash_v1(&units("rip"))
    } else if let SelectedInstructionKind::AtomicAccess { .. } = kind {
        unreachable!("atomic footprint returned above");
    } else if matches!(
        kind,
        SelectedInstructionKind::ReturnScalar
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit
    ) {
        let stack_pointer = physical.view_named("rsp").unwrap().id;
        let mut defs = units("rsp");
        defs.extend(units("rip"));
        defs.sort_unstable();
        defs.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: units("rsp"),
            implicit_unit_defs: defs,
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                stack_pointer,
                byte_count: 8,
            },
            stack: MachineEncodedStackEffect::PopBytesV1 {
                stack_pointer,
                byte_count: 8,
            },
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
        }
    } else if matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    ) {
        let mut uses = units("rflags");
        uses.extend(units("rip"));
        uses.sort_unstable();
        uses.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: uses,
            implicit_unit_defs: units("rip"),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        }
    } else {
        let mut effects = MachineEncodedEffects::fallthrough_v1(
            match kind {
                SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                | SelectedInstructionKind::MaterializeI64 { .. }
                | SelectedInstructionKind::ImmutableImageAddress { .. } => vec![],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::CompareI64Immediate { .. }
                | selected_instructions::exact_add_or_sub_pair_pattern!(..)
                | SelectedInstructionKind::BitwiseNotI64 => vec![0],
                SelectedInstructionKind::CompareI64 => vec![0, 1],
                SelectedInstructionKind::ExactDivideU64 { .. } => vec![0, 1, 3],
                selected_instructions::exact_wrapping_divrem_set_pattern!(..) => vec![0, 1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. } => vec![0, 1],
                SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::WrappingSubtractI64
                    if alternative.variant == 0 =>
                {
                    vec![]
                }
                SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::WrappingSubtractI64
                | SelectedInstructionKind::ExactMultiplyI64 { .. }
                | SelectedInstructionKind::WrappingMultiplyI64
                | SelectedInstructionKind::WrappingShiftLeftI64
                | SelectedInstructionKind::WrappingShiftRightI64
                | SelectedInstructionKind::WrappingShiftRightU64
                | selected_instructions::exact_shift_i64_pair_pattern!()
                | SelectedInstructionKind::ExactShiftRightU64 { .. }
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseOrI64
                | SelectedInstructionKind::BitwiseXorI64 => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                | SelectedInstructionKind::MaterializeI64 { .. }
                | SelectedInstructionKind::ImmutableImageAddress { .. } => vec![0],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | selected_instructions::exact_add_or_sub_pair_pattern!(..)
                | SelectedInstructionKind::BitwiseNotI64 => vec![1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactMultiplyI64 { .. }
                | SelectedInstructionKind::WrappingMultiplyI64
                | SelectedInstructionKind::WrappingShiftLeftI64
                | SelectedInstructionKind::WrappingShiftRightI64
                | SelectedInstructionKind::WrappingShiftRightU64
                | selected_instructions::exact_shift_i64_pair_pattern!()
                | SelectedInstructionKind::ExactShiftRightU64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::WrappingSubtractI64 => vec![2],
                SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseOrI64
                | SelectedInstructionKind::BitwiseXorI64 => vec![2],
                SelectedInstructionKind::CompareI64Zero => vec![],
                SelectedInstructionKind::ExactDivideU64 { .. } => vec![2],
                selected_instructions::exact_wrapping_divrem_set_pattern!(..) => vec![2, 3],
                SelectedInstructionKind::CompareI64 => vec![],
                SelectedInstructionKind::CompareI64Immediate { .. } => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if matches!(
            kind,
            SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::CompareI64
                | SelectedInstructionKind::CompareI64Immediate { .. }
        ) {
            effects.implicit_unit_defs = units("rflags");
        }
        if matches!(
            kind,
            SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::WrappingSubtractI64
                | SelectedInstructionKind::ExactMultiplyI64 { .. }
                | SelectedInstructionKind::WrappingMultiplyI64
                | SelectedInstructionKind::WrappingShiftLeftI64
                | SelectedInstructionKind::WrappingShiftRightI64
                | SelectedInstructionKind::WrappingShiftRightU64
                | selected_instructions::exact_shift_i64_pair_pattern!()
                | SelectedInstructionKind::ExactShiftRightU64 { .. }
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseOrI64
                | SelectedInstructionKind::BitwiseXorI64
        ) {
            effects.implicit_unit_clobbers = units("rflags");
        }
        if matches!(
            kind,
            SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
        ) {
            effects.implicit_unit_uses = units("rflags");
        }
        if matches!(kind, SelectedInstructionKind::ExactDivideU64 { .. }) {
            effects.implicit_unit_clobbers = units("rdx");
            effects.implicit_unit_clobbers.extend(units("rflags"));
            effects.implicit_unit_clobbers.sort_unstable();
            effects.implicit_unit_clobbers.dedup();
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        }
        if matches!(
            kind,
            selected_instructions::exact_wrapping_divrem_set_pattern!(..)
        ) {
            effects.implicit_unit_clobbers = units("rflags");
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        }
        if writes_rflags && matches!(kind, SelectedInstructionKind::MaterializeI64 { .. }) {
            effects.implicit_unit_clobbers = units("rflags");
        }
        effects
    };
    X86_64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_rflags,
        encoded,
    }
}
