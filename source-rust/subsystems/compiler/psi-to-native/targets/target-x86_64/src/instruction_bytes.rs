//! Decode one x86-64 instruction and retain its operands for the calling verifier.
use selected_instructions::AtomicOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedInstruction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodedInstruction {
    Crash,
    PrivilegedStop,
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
    JumpShort {
        displacement: i8,
    },
    JumpNoOverflowShort {
        displacement: i8,
    },
    JumpBelowShort {
        displacement: i8,
    },
    JumpAboveOrEqualShort {
        displacement: i8,
    },
    CompareSignedImmediate8 {
        register: u8,
        immediate: i8,
    },
    CompareRaxImmediate32 {
        immediate: i32,
    },
    ShiftRightLogicalImmediate {
        destination: u8,
        count: u8,
    },
    UnsignedDivide {
        divisor: u8,
    },
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
    MoveOnAbove {
        source: u8,
        destination: u8,
    },
    MoveOnOverflow {
        source: u8,
        destination: u8,
    },
    ArithmeticShiftRight63 {
        destination: u8,
    },
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
    Multiply {
        source: u8,
        destination: u8,
    },
    ShiftLeftByCl {
        destination: u8,
    },
    ShiftRightLogicalByCl {
        destination: u8,
    },
    ShiftRightArithmeticByCl {
        destination: u8,
    },
    Negate {
        destination: u8,
    },
    AtomicLoad {
        destination: u8,
        base: u8,
        bits: u16,
    },
    AtomicStore {
        source: u8,
        base: u8,
        bits: u16,
    },
    MemoryFence,
    AtomicExchange {
        register: u8,
        base: u8,
        bits: u16,
    },
    AtomicLockedAdd {
        register: u8,
        base: u8,
        bits: u16,
    },
    AtomicBitwise {
        operation: AtomicOperation,
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
    JumpIndirectRip {
        displacement: i32,
    },
    UnresolvedRipRelativeAddress {
        destination: u8,
    },
}

fn modrm_source_destination(modrm: u8, rex: u8) -> (u8, u8) {
    (
        (modrm & 7) | ((rex & 1) << 3),
        ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
    )
}

pub fn decode_one(bytes: &[u8]) -> Result<(DecodedInstruction, usize), MalformedInstruction> {
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
    if bytes.starts_with(&crate::PRIVILEGED_STOP_BYTES) {
        return Ok((
            DecodedInstruction::PrivilegedStop,
            crate::PRIVILEGED_STOP_BYTES.len(),
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
    let (&rex, rest) = bytes.split_first().ok_or(MalformedInstruction)?;
    if !(0x48..=0x4f).contains(&rex) {
        return Err(MalformedInstruction);
    }
    let (&opcode, _) = rest.split_first().ok_or(MalformedInstruction)?;
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
            .ok_or(MalformedInstruction)?;
        return Ok((
            DecodedInstruction::Materialize {
                destination: (opcode & 7) | (rex_b << 3),
                value: immediate,
            },
            10,
        ));
    }
    let modrm = *bytes.get(2).ok_or(MalformedInstruction)?;
    let mode = modrm >> 6;
    let reg = ((modrm >> 3) & 7) | (rex_r << 3);
    let rm_low = modrm & 7;
    let rm = rm_low | (rex_b << 3);
    if opcode == 0x81 {
        if mode != 3 || reg != 7 {
            return Err(MalformedInstruction);
        }
        let immediate = bytes
            .get(3..7)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(MalformedInstruction)?;
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
            return Err(MalformedInstruction);
        }
        let decoded = match opcode {
            0x89 => DecodedInstruction::Move {
                source: reg,
                destination: rm,
            },
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
            _ => return Err(MalformedInstruction),
        };
        return Ok((decoded, 3));
    }
    if opcode != 0x8d || mode == 3 {
        return Err(MalformedInstruction);
    }
    let mut length = 3;
    let (base, index) = if rm_low == 4 {
        let sib = *bytes.get(length).ok_or(MalformedInstruction)?;
        length += 1;
        if sib >> 6 != 0 {
            return Err(MalformedInstruction);
        }
        let index_low = (sib >> 3) & 7;
        let base_low = sib & 7;
        if mode == 0 && base_low == 5 {
            return Err(MalformedInstruction);
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
            return Err(MalformedInstruction);
        }
        (rm, None)
    };
    let displacement = match mode {
        0 => 0,
        1 => {
            let value = *bytes.get(length).ok_or(MalformedInstruction)?;
            length += 1;
            i32::from(value as i8)
        }
        2 => {
            let value = bytes
                .get(length..length + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map(i32::from_le_bytes)
                .ok_or(MalformedInstruction)?;
            length += 4;
            value
        }
        _ => return Err(MalformedInstruction),
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
                0x20 => AtomicOperation::FetchAnd,
                0x08 => AtomicOperation::FetchOr,
                0x30 => AtomicOperation::FetchXor,
                _ => unreachable!(),
            };
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
        0xf6 | 0xf7 if modrm >> 6 == 3 && (modrm >> 3) & 7 == 3 => Some((
            DecodedInstruction::Negate {
                destination: (modrm & 7) | ((rex & 1) << 3),
            },
            cursor + 2,
        )),
        _ => None,
    }
}
