//! Decoding emitted instructions and validating them against the request.

use crate::selected_form_encoding::instruction_bytes::{integer_bits, u12};
use crate::selected_form_encoding::{X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint};
use crate::x86_64_physical_register_model;
use register_model::RegisterViewId;
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    SelectedInstructionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodedInstruction {
    SignedDivide {
        divisor: u8,
    },
    SignExtendDividend,
    JumpEqualShort {
        displacement: i8,
    },
    CompareSignedImmediate8 {
        register: u8,
        immediate: i8,
    },
    UnsignedDivide {
        divisor: u8,
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
    Negate {
        destination: u8,
    },
    Add {
        source: u8,
        destination: u8,
    },
    Return,
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
        && matches!(*opcode, 0x42 | 0x4c | 0x4f)
        && modrm & 0xc0 == 0xc0
    {
        let source = (modrm & 7) | ((rex & 1) << 3);
        let destination = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
        return Ok((
            match opcode {
                0x42 => DecodedInstruction::MoveOnBorrow {
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
        return Ok((
            DecodedInstruction::ZeroExtendU8 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xb7, modrm, ..] = bytes
        && rex & !0x05 == 0x40
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::ZeroExtendU16 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbe, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI8 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x0f, 0xbf, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI16 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
            },
            4,
        ));
    }
    if let [rex, 0x63, modrm, ..] = bytes
        && rex & !0x05 == 0x48
        && modrm & 0xc0 == 0xc0
    {
        return Ok((
            DecodedInstruction::SignExtendI32 {
                source: (modrm & 7) | ((rex & 1) << 3),
                destination: ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3),
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
    if bytes.first() == Some(&0xc3) {
        return Ok((DecodedInstruction::Return, 1));
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
        0x89 | 0x85 | 0x39 | 0x31 | 0x29 | 0x01 | 0x21 | 0xf7
    ) {
        if mode != 3 || bytes.len() < 3 {
            return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
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
            0x01 => DecodedInstruction::Add {
                source: reg,
                destination: rm,
            },
            0xf7 if reg == 2 && rex_x == 0 => DecodedInstruction::Complement { destination: rm },
            0xf7 if reg == 6 && rex_x == 0 => DecodedInstruction::UnsignedDivide { divisor: rm },
            0xf7 if reg == 7 && rex_x == 0 => DecodedInstruction::SignedDivide { divisor: rm },
            0xf7 if (modrm >> 3) & 7 == 3 => DecodedInstruction::Negate { destination: rm },
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

pub(crate) fn validate_decoded(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    registers: &[u8],
    decoded: &[DecodedInstruction],
) -> Result<(), X86_64SelectedFormEncodingError> {
    let valid = match kind {
        SelectedInstructionKind::MaterializeI64 { value } => {
            decoded
                == [DecodedInstruction::Materialize {
                    destination: registers[0],
                    value: integer_bits(value)?,
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
        SelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedInstruction::Move {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
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
        SelectedInstructionKind::SaturatingSubtractU64 => {
            registers[2] != registers[0]
                && registers[2] != registers[1]
                && decoded
                    == [
                        DecodedInstruction::Compare {
                            left: registers[0],
                            right: registers[1],
                        },
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
                        DecodedInstruction::MoveOnBorrow {
                            source: registers[1],
                            destination: registers[2],
                        },
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
        }
        SelectedInstructionKind::SaturatingAddU64 => {
            registers[2] != registers[0]
                && registers[2] != registers[1]
                && decoded
                    == [
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
                        DecodedInstruction::Complement {
                            destination: registers[2],
                        },
                        DecodedInstruction::Compare {
                            left: registers[2],
                            right: registers[1],
                        },
                        DecodedInstruction::MoveOnBorrow {
                            source: registers[1],
                            destination: registers[2],
                        },
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                        DecodedInstruction::Complement {
                            destination: registers[2],
                        },
                    ]
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            registers[0] == 0
                && registers[2] == 0
                && registers[3] == 2
                && registers[1] != 2
                && decoded
                    == [DecodedInstruction::UnsignedDivide {
                        divisor: registers[1],
                    }]
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            registers[0] == 0
                && registers[2] == 0
                && registers[3] == 2
                && registers[1] != 2
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
                        DecodedInstruction::Move {
                            source: 2,
                            destination: 0,
                        },
                    ]
        }
        SelectedInstructionKind::SaturatingAddI32
        | SelectedInstructionKind::SaturatingSubtractI32
        | SelectedInstructionKind::SaturatingDivideI32 { .. } => {
            let (value, scratch) = (registers[2], registers[3]);
            let clamp = |bound: i64, select: fn(u8, u8) -> DecodedInstruction| {
                [
                    DecodedInstruction::Materialize {
                        destination: scratch,
                        value: bound as u64,
                    },
                    DecodedInstruction::Compare {
                        left: value,
                        right: scratch,
                    },
                    select(scratch, value),
                ]
            };
            let upper = clamp(i64::from(i32::MAX), |source, destination| {
                DecodedInstruction::MoveOnGreater {
                    source,
                    destination,
                }
            });
            let lower = clamp(i64::from(i32::MIN), |source, destination| {
                DecodedInstruction::MoveOnLess {
                    source,
                    destination,
                }
            });
            if let SelectedInstructionKind::SaturatingDivideI32 { .. } = kind {
                let mut expected = vec![
                    DecodedInstruction::SignExtendDividend,
                    DecodedInstruction::SignedDivide {
                        divisor: registers[1],
                    },
                ];
                expected.extend(upper);
                registers[0] == 0
                    && value == 0
                    && scratch == 2
                    && registers[1] != 2
                    && decoded == expected
            } else {
                let mut expected = vec![
                    DecodedInstruction::Move {
                        source: registers[0],
                        destination: value,
                    },
                    if kind == SelectedInstructionKind::SaturatingAddI32 {
                        DecodedInstruction::Add {
                            source: registers[1],
                            destination: value,
                        }
                    } else {
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: value,
                        }
                    },
                ];
                expected.extend(upper);
                expected.extend(lower);
                !registers[..2].contains(&value)
                    && !registers[..3].contains(&scratch)
                    && decoded == expected
            }
        }
        SelectedInstructionKind::BitwiseAndI64 | SelectedInstructionKind::BitwiseXorI64 => {
            let operation = |source, destination| {
                if kind == SelectedInstructionKind::BitwiseXorI64 {
                    DecodedInstruction::Xor {
                        source,
                        destination,
                    }
                } else {
                    DecodedInstruction::BitwiseAnd {
                        source,
                        destination,
                    }
                }
            };
            if registers[2] == registers[0] {
                decoded == [operation(registers[1], registers[2])]
            } else if registers[2] == registers[1] {
                decoded == [operation(registers[0], registers[2])]
            } else {
                decoded
                    == [
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
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
        SelectedInstructionKind::ExactSubtractI64 { .. } => match alternative.variant {
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
                        DecodedInstruction::Move {
                            source: registers[0],
                            destination: registers[2],
                        },
                        DecodedInstruction::Subtract {
                            source: registers[1],
                            destination: registers[2],
                        },
                    ]
            }
            _ => false,
        },
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => decoded == [DecodedInstruction::Return],
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Float32ToBits
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
        | SelectedInstructionKind::CallScalar { .. } => false,
    };
    if valid {
        Ok(())
    } else {
        Err(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

pub(crate) fn footprint(
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let (reads, writes, writes_rflags) = match kind {
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            (vec![], vec![operands[0]], false)
        }
        SelectedInstructionKind::MaterializeI64 { .. } => (vec![], vec![operands[0]], false),
        SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::ZeroExtendU32 => (vec![operands[0]], vec![operands[1]], false),
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CompareI64Immediate { .. } => (vec![operands[0]], vec![], true),
        SelectedInstructionKind::ExactDivideU64 { .. }
        | SelectedInstructionKind::SaturatingDivideI32 { .. } => (
            vec![operands[0], operands[1], operands[3]],
            vec![operands[2]],
            true,
        ),
        SelectedInstructionKind::WrappingRemainderI64 { .. }
        | SelectedInstructionKind::SaturatingAddI32
        | SelectedInstructionKind::SaturatingSubtractI32 => (
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
        SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } if alternative.variant == 0 => {
            (vec![], vec![operands[2]], true)
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::SaturatingSubtractU64
        | SelectedInstructionKind::SaturatingAddU64
        | SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseXorI64 => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => (vec![], vec![], false),
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Float32ToBits
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
        | SelectedInstructionKind::CallScalar { .. } => (vec![], vec![], false),
    };
    let physical = x86_64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
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
                | SelectedInstructionKind::MaterializeI64 { .. } => vec![],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::CompareI64Zero
                | SelectedInstructionKind::CompareI64Immediate { .. }
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![0],
                SelectedInstructionKind::CompareI64 => vec![0, 1],
                SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::SaturatingDivideI32 { .. } => vec![0, 1, 3],
                SelectedInstructionKind::WrappingRemainderI64 { .. }
                | SelectedInstructionKind::SaturatingAddI32
                | SelectedInstructionKind::SaturatingSubtractI32 => vec![0, 1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. } => vec![0, 1],
                SelectedInstructionKind::ExactSubtractI64 { .. } if alternative.variant == 0 => {
                    vec![]
                }
                SelectedInstructionKind::ExactSubtractI64 { .. }
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::SaturatingSubtractU64
                | SelectedInstructionKind::SaturatingAddU64 => vec![0, 1],
                _ => unreachable!("control forms handled separately"),
            },
            match kind {
                SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                | SelectedInstructionKind::MaterializeI64 { .. } => vec![0],
                SelectedInstructionKind::CopyI64
                | SelectedInstructionKind::ZeroExtendU8
                | SelectedInstructionKind::ZeroExtendU16
                | SelectedInstructionKind::SignExtendI8
                | SelectedInstructionKind::SignExtendI16
                | SelectedInstructionKind::SignExtendI32
                | SelectedInstructionKind::ZeroExtendU32
                | SelectedInstructionKind::ExactAddI64Immediate { .. }
                | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => vec![1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                SelectedInstructionKind::SaturatingSubtractU64
                | SelectedInstructionKind::SaturatingAddU64
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseXorI64 => {
                    vec![2]
                }
                SelectedInstructionKind::CompareI64Zero => vec![],
                SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::SaturatingDivideI32 { .. } => vec![2],
                SelectedInstructionKind::WrappingRemainderI64 { .. }
                | SelectedInstructionKind::SaturatingAddI32
                | SelectedInstructionKind::SaturatingSubtractI32 => vec![2, 3],
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
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::SaturatingSubtractU64
                | SelectedInstructionKind::SaturatingAddU64
                | SelectedInstructionKind::SaturatingAddI32
                | SelectedInstructionKind::SaturatingSubtractI32
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
        if matches!(
            kind,
            SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::SaturatingDivideI32 { .. }
        ) {
            effects.implicit_unit_clobbers = units("rdx");
            effects.implicit_unit_clobbers.extend(units("rflags"));
            effects.implicit_unit_clobbers.sort_unstable();
            effects.implicit_unit_clobbers.dedup();
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        }
        if matches!(kind, SelectedInstructionKind::WrappingRemainderI64 { .. }) {
            effects.implicit_unit_clobbers = units("rflags");
            effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
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
