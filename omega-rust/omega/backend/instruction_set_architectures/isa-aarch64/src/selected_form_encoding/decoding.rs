//! Decoding encoded words back into forms and checking them against the
//! selected instruction they claim to realize.

use crate::aarch64_physical_register_model;
use crate::selected_form_encoding::movn_materialization::append_canonical_materialization;
use crate::selected_form_encoding::selected_forms::{integer_bits, u12};
use crate::selected_form_encoding::{
    Aarch64MovkPatch, Aarch64MovnSeed, Aarch64SelectedFormEncodingError,
    Aarch64SelectedFormFootprint, Aarch64ShortestMovnMaterializationRecipe,
};
use register_model::RegisterViewId;
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstructionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodedWord {
    SignedDivide {
        dividend: u8,
        divisor: u8,
        destination: u8,
    },
    MultiplySubtract {
        left: u8,
        right: u8,
        minuend: u8,
        destination: u8,
    },
    UnsignedDivide {
        dividend: u8,
        divisor: u8,
        destination: u8,
    },
    AddWithFlags {
        left: u8,
        right: u8,
        destination: u8,
    },
    SelectMaximumOnCarry {
        source: u8,
        destination: u8,
    },
    SubtractWithFlags {
        left: u8,
        right: u8,
        destination: u8,
    },
    SelectZeroOnBorrow {
        source: u8,
        destination: u8,
    },
    /// `mov destination, #0x7fffffff` as one ORR bitmask immediate.
    MaterializeI32Maximum {
        destination: u8,
    },
    /// `mov destination, #-0x80000000` as one ORR bitmask immediate.
    MaterializeI32Minimum {
        destination: u8,
    },
    /// `csel destination, source, destination, gt`: keep the value unless the
    /// preceding compare found it greater than the bound in `source`.
    SelectOnGreater {
        source: u8,
        destination: u8,
    },
    /// `csel destination, source, destination, lt`.
    SelectOnLess {
        source: u8,
        destination: u8,
    },
    BitwiseXor {
        left: u8,
        right: u8,
        destination: u8,
    },
    BitwiseAnd {
        left: u8,
        right: u8,
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
    MovN {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    MovZ {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    MovK {
        register: u8,
        shift: u8,
        immediate: u16,
    },
    Copy {
        source: u8,
        destination: u8,
    },
    CompareZero {
        source: u8,
    },
    CompareImmediate {
        source: u8,
        immediate: u16,
    },
    Compare {
        left: u8,
        right: u8,
    },
    Add {
        left: u8,
        right: u8,
        destination: u8,
    },
    AddImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Subtract {
        left: u8,
        right: u8,
        destination: u8,
    },
    SubtractImmediate {
        source: u8,
        immediate: u16,
        destination: u8,
    },
    Return,
}

pub(crate) fn decode_words(
    bytes: &[u8],
) -> Result<Vec<DecodedWord>, Aarch64SelectedFormEncodingError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| decode_word(u32::from_le_bytes(*bytes)))
        .collect()
}

fn decode_word(word: u32) -> Result<DecodedWord, Aarch64SelectedFormEncodingError> {
    if word & 0xffe0_fc00 == 0x9ac0_0c00 {
        return Ok(DecodedWord::SignedDivide {
            dividend: ((word >> 5) & 31) as u8,
            divisor: ((word >> 16) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_8000 == 0x9b00_8000 {
        return Ok(DecodedWord::MultiplySubtract {
            left: ((word >> 5) & 31) as u8,
            right: ((word >> 16) & 31) as u8,
            minuend: ((word >> 10) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x9ac0_0800 {
        return Ok(DecodedWord::UnsignedDivide {
            dividend: ((word >> 5) & 31) as u8,
            divisor: ((word >> 16) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0xab00_0000 && word & 31 != 31 {
        return Ok(DecodedWord::AddWithFlags {
            left: ((word >> 5) & 31) as u8,
            right: ((word >> 16) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0xda9f_3000 {
        return Ok(DecodedWord::SelectMaximumOnCarry {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0xeb00_0000 && word & 31 != 31 {
        return Ok(DecodedWord::SubtractWithFlags {
            left: ((word >> 5) & 31) as u8,
            right: ((word >> 16) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0x9a9f_2000 {
        return Ok(DecodedWord::SelectZeroOnBorrow {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_ffe0 == 0xb240_7be0 {
        return Ok(DecodedWord::MaterializeI32Maximum {
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_ffe0 == 0xb261_83e0 {
        return Ok(DecodedWord::MaterializeI32Minimum {
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x9a80_c000 && (word >> 16) & 31 == word & 31 {
        return Ok(DecodedWord::SelectOnGreater {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x9a80_b000 && (word >> 16) & 31 == word & 31 {
        return Ok(DecodedWord::SelectOnLess {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_0fe0 == 0x9a9f_07e0 {
        let condition = (((word >> 12) & 15) ^ 1) as u8;
        if matches!(condition, 0 | 3 | 9 | 11 | 13) {
            return Ok(DecodedWord::SetBoolean {
                condition,
                destination: (word & 31) as u8,
            });
        }
    }

    if word & 0xffff_fc00 == 0xd340_1c00 {
        return Ok(DecodedWord::ZeroExtendU8 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0xd340_3c00 {
        return Ok(DecodedWord::ZeroExtendU16 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0x9340_1c00 {
        return Ok(DecodedWord::SignExtendI8 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0x9340_3c00 {
        return Ok(DecodedWord::SignExtendI16 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0x9340_7c00 {
        return Ok(DecodedWord::SignExtendI32 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    if word & 0xffff_fc00 == 0xd340_7c00 {
        return Ok(DecodedWord::ZeroExtendU32 {
            source: ((word >> 5) & 31) as u8,
            destination: (word & 31) as u8,
        });
    }
    let register = (word & 0x1f) as u8;
    let shift = ((word >> 21) & 0x3) as u8;
    let immediate = ((word >> 5) & 0xffff) as u16;
    if word & 0xff80_0000 == 0x9280_0000 {
        return Ok(DecodedWord::MovN {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xffe0_0000 == 0xd280_0000 {
        return Ok(DecodedWord::MovZ {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xff80_0000 == 0xf280_0000 {
        return Ok(DecodedWord::MovK {
            register,
            shift,
            immediate,
        });
    }
    if word & 0xffe0_ffe0 == 0xaa00_03e0 {
        return Ok(DecodedWord::Copy {
            source: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffff_fc1f == 0xf100_001f {
        return Ok(DecodedWord::CompareZero {
            source: ((word >> 5) & 0x1f) as u8,
        });
    }
    if word & 0xffc0_001f == 0xf100_001f {
        return Ok(DecodedWord::CompareImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
        });
    }
    if word & 0xffe0_fc1f == 0xeb00_001f {
        return Ok(DecodedWord::Compare {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
        });
    }
    if word & 0xffe0_fc00 == 0x8a00_0000 {
        return Ok(DecodedWord::BitwiseAnd {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffe0_fc00 == 0xca00_0000 {
        return Ok(DecodedWord::BitwiseXor {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffe0_fc00 == 0x8b00_0000 {
        return Ok(DecodedWord::Add {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0x9100_0000 {
        return Ok(DecodedWord::AddImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word & 0xffe0_fc00 == 0xcb00_0000 {
        return Ok(DecodedWord::Subtract {
            left: ((word >> 5) & 0x1f) as u8,
            right: ((word >> 16) & 0x1f) as u8,
            destination: register,
        });
    }
    if word & 0xffc0_0000 == 0xd100_0000 {
        return Ok(DecodedWord::SubtractImmediate {
            source: ((word >> 5) & 0x1f) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
            destination: register,
        });
    }
    if word == 0xd65f_03c0 {
        return Ok(DecodedWord::Return);
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}

pub(crate) fn validate_decoded(
    kind: SelectedInstructionKind,
    registers: &[u8],
    decoded: &[DecodedWord],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let valid = match kind {
        SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            let condition = match kind {
                SelectedInstructionKind::MaterializeBooleanEqual => 0,
                SelectedInstructionKind::MaterializeBooleanU64LessThan => 3,
                SelectedInstructionKind::MaterializeBooleanI64LessThan => 11,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 9,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 13,
                _ => unreachable!("Boolean condition arm"),
            };
            decoded
                == [DecodedWord::SetBoolean {
                    condition,
                    destination: registers[0],
                }]
        }
        SelectedInstructionKind::ZeroExtendU8 => {
            decoded
                == [DecodedWord::ZeroExtendU8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU16 => {
            decoded
                == [DecodedWord::ZeroExtendU16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI8 => {
            decoded
                == [DecodedWord::SignExtendI8 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI16 => {
            decoded
                == [DecodedWord::SignExtendI16 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::SignExtendI32 => {
            decoded
                == [DecodedWord::SignExtendI32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ZeroExtendU32 => {
            decoded
                == [DecodedWord::ZeroExtendU32 {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::MaterializeI64 { value } => {
            decode_materialization(decoded, registers[0]) == integer_bits(value).ok()
        }
        SelectedInstructionKind::CopyI64 => {
            decoded
                == [DecodedWord::Copy {
                    source: registers[0],
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::CompareI64Zero => {
            decoded
                == [DecodedWord::CompareZero {
                    source: registers[0],
                }]
        }
        SelectedInstructionKind::CompareI64Immediate { immediate } => {
            let immediate = u12(immediate)?;
            // `subs xzr, xN, #0` is the shared machine form: the decoder
            // reports it as `CompareZero`, so a zero immediate validates
            // through that canonical word.
            decoded
                == [if immediate == 0 {
                    DecodedWord::CompareZero {
                        source: registers[0],
                    }
                } else {
                    DecodedWord::CompareImmediate {
                        source: registers[0],
                        immediate,
                    }
                }]
        }
        SelectedInstructionKind::CompareI64 => {
            decoded
                == [DecodedWord::Compare {
                    left: registers[0],
                    right: registers[1],
                }]
        }
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. } => {
            decoded
                == [DecodedWord::Add {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::BitwiseAndI64 => {
            decoded
                == [DecodedWord::BitwiseAnd {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::SaturatingSubtractU64 => {
            decoded
                == [
                    DecodedWord::SubtractWithFlags {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    },
                    DecodedWord::SelectZeroOnBorrow {
                        source: registers[2],
                        destination: registers[2],
                    },
                ]
        }
        SelectedInstructionKind::SaturatingAddU64 => {
            decoded
                == [
                    DecodedWord::AddWithFlags {
                        left: registers[0],
                        right: registers[1],
                        destination: registers[2],
                    },
                    DecodedWord::SelectMaximumOnCarry {
                        source: registers[2],
                        destination: registers[2],
                    },
                ]
        }
        SelectedInstructionKind::ExactDivideU64 { .. } => {
            decoded
                == [DecodedWord::UnsignedDivide {
                    dividend: registers[0],
                    divisor: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            registers[2] != registers[0]
                && registers[2] != registers[1]
                && decoded
                    == [
                        DecodedWord::SignedDivide {
                            dividend: registers[0],
                            divisor: registers[1],
                            destination: registers[2],
                        },
                        DecodedWord::MultiplySubtract {
                            left: registers[2],
                            right: registers[1],
                            minuend: registers[0],
                            destination: registers[2],
                        },
                    ]
        }
        SelectedInstructionKind::SaturatingAddI32
        | SelectedInstructionKind::SaturatingSubtractI32
        | SelectedInstructionKind::SaturatingDivideI32 { .. } => {
            let (value, scratch) = (registers[2], registers[3]);
            let arithmetic = match kind {
                SelectedInstructionKind::SaturatingAddI32 => DecodedWord::Add {
                    left: registers[0],
                    right: registers[1],
                    destination: value,
                },
                SelectedInstructionKind::SaturatingSubtractI32 => DecodedWord::Subtract {
                    left: registers[0],
                    right: registers[1],
                    destination: value,
                },
                _ => DecodedWord::SignedDivide {
                    dividend: registers[0],
                    divisor: registers[1],
                    destination: value,
                },
            };
            let mut expected = vec![
                arithmetic,
                DecodedWord::MaterializeI32Maximum {
                    destination: scratch,
                },
                DecodedWord::Compare {
                    left: value,
                    right: scratch,
                },
                DecodedWord::SelectOnGreater {
                    source: scratch,
                    destination: value,
                },
            ];
            // A quotient of sign-normalized i32 carriers only exceeds the
            // carrier upward (i32::MIN / -1); add and subtract clamp both ends.
            if !matches!(kind, SelectedInstructionKind::SaturatingDivideI32 { .. }) {
                expected.extend([
                    DecodedWord::MaterializeI32Minimum {
                        destination: scratch,
                    },
                    DecodedWord::Compare {
                        left: value,
                        right: scratch,
                    },
                    DecodedWord::SelectOnLess {
                        source: scratch,
                        destination: value,
                    },
                ]);
            }
            !registers[..2].contains(&value)
                && !registers[..3].contains(&scratch)
                && decoded == expected
        }
        SelectedInstructionKind::BitwiseXorI64 => {
            decoded
                == [DecodedWord::BitwiseXor {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::ExactAddI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::AddImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => {
            decoded
                == [DecodedWord::Subtract {
                    left: registers[0],
                    right: registers[1],
                    destination: registers[2],
                }]
        }
        SelectedInstructionKind::ExactSubtractI64Immediate { immediate, .. } => {
            decoded
                == [DecodedWord::SubtractImmediate {
                    source: registers[0],
                    immediate: u12(immediate)?,
                    destination: registers[1],
                }]
        }
        SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::ReturnAggregate { .. }
        | SelectedInstructionKind::ReturnUnit => decoded == [DecodedWord::Return],
        SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::Jump
        | SelectedInstructionKind::Float32ToBits
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
        | SelectedInstructionKind::CallScalar { .. } => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

fn decode_materialization(decoded: &[DecodedWord], register: u8) -> Option<u64> {
    let (mut value, start) = match decoded.first()? {
        DecodedWord::MovZ {
            register: actual,
            shift: 0,
            immediate,
        } if *actual == register => (u64::from(*immediate), 1),
        _ => return None,
    };
    let mut previous_shift = 0;
    for word in &decoded[start..] {
        let DecodedWord::MovK {
            register: actual,
            shift,
            immediate,
        } = word
        else {
            return None;
        };
        if *actual != register || *shift <= previous_shift || *shift > 3 || *immediate == 0 {
            return None;
        }
        previous_shift = *shift;
        let shift = u64::from(*shift) * 16;
        value = (value & !(0xffff_u64 << shift)) | (u64::from(*immediate) << shift);
    }
    Some(value)
}

pub(crate) fn decode_movn_materialization(
    decoded: &[DecodedWord],
    register: u8,
) -> Option<(u64, Aarch64ShortestMovnMaterializationRecipe)> {
    let DecodedWord::MovN {
        register: actual,
        shift,
        immediate,
    } = *decoded.first()?
    else {
        return None;
    };
    if actual != register || shift > 3 {
        return None;
    }
    let seed_shift = u64::from(shift) * 16;
    let mut value = u64::MAX;
    value = (value & !(0xffff_u64 << seed_shift)) | (u64::from(!immediate) << seed_shift);
    let mut patches = Vec::with_capacity(decoded.len().saturating_sub(1));
    let mut previous_halfword = None;
    for word in &decoded[1..] {
        let DecodedWord::MovK {
            register: actual,
            shift,
            immediate,
        } = *word
        else {
            return None;
        };
        if actual != register
            || shift > 3
            || previous_halfword.is_some_and(|previous| shift <= previous)
        {
            return None;
        }
        previous_halfword = Some(shift);
        let patch_shift = u64::from(shift) * 16;
        value = (value & !(0xffff_u64 << patch_shift)) | (u64::from(immediate) << patch_shift);
        patches.push(Aarch64MovkPatch {
            halfword: shift,
            immediate,
        });
    }
    Some((
        value,
        Aarch64ShortestMovnMaterializationRecipe {
            seed: Aarch64MovnSeed {
                halfword: shift,
                immediate,
            },
            patches,
            baseline_byte_count: {
                let mut baseline = Vec::new();
                append_canonical_materialization(&mut baseline, register, value);
                baseline.len() * 4
            },
        },
    ))
}

pub(crate) fn footprint(
    kind: SelectedInstructionKind,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    let (reads, writes, writes_nzcv) = match kind {
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
        | SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        SelectedInstructionKind::CompareI64 => (vec![operands[0], operands[1]], vec![], true),
        SelectedInstructionKind::SaturatingSubtractU64
        | SelectedInstructionKind::SaturatingAddU64 => {
            (vec![operands[0], operands[1]], vec![operands[2]], true)
        }
        SelectedInstructionKind::SaturatingAddI32
        | SelectedInstructionKind::SaturatingSubtractI32
        | SelectedInstructionKind::SaturatingDivideI32 { .. } => (
            vec![operands[0], operands[1]],
            vec![operands[2], operands[3]],
            true,
        ),
        SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::BitwiseAndI64
        | SelectedInstructionKind::BitwiseXorI64
        | SelectedInstructionKind::WrappingAddI64
        | SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactSubtractI64 { .. } => {
            (vec![operands[0], operands[1]], vec![operands[2]], false)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            (vec![operands[0]], vec![operands[1]], false)
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
        | SelectedInstructionKind::CallScalar { .. } => (vec![], vec![], false),
    };
    let physical = aarch64_physical_register_model();
    let units = |name: &str| physical.view_named(name).unwrap().units.clone();
    let encoded = if matches!(
        kind,
        SelectedInstructionKind::ReturnScalar
            | SelectedInstructionKind::ReturnAggregate { .. }
            | SelectedInstructionKind::ReturnUnit
    ) {
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: units("x30"),
            implicit_unit_defs: units("pc"),
            implicit_unit_clobbers: vec![],
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                target: physical.view_named("x30").unwrap().id,
            },
        }
    } else if matches!(
        kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    ) {
        let mut uses = units("nzcv");
        uses.extend(units("pc"));
        uses.sort_unstable();
        uses.dedup();
        MachineEncodedEffects {
            external_operand_reads: vec![],
            external_operand_writes: vec![],
            implicit_unit_uses: uses,
            implicit_unit_defs: units("pc"),
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
                SelectedInstructionKind::CompareI64
                | SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::WrappingRemainderI64 { .. }
                | SelectedInstructionKind::SaturatingAddI32
                | SelectedInstructionKind::SaturatingSubtractI32
                | SelectedInstructionKind::SaturatingDivideI32 { .. } => vec![0, 1],
                SelectedInstructionKind::ByteViewAddress
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::SaturatingSubtractU64
                | SelectedInstructionKind::SaturatingAddU64
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![0, 1],
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
                | SelectedInstructionKind::BitwiseAndI64
                | SelectedInstructionKind::BitwiseXorI64
                | SelectedInstructionKind::SaturatingSubtractU64
                | SelectedInstructionKind::SaturatingAddU64
                | SelectedInstructionKind::WrappingAddI64
                | SelectedInstructionKind::ExactAddI64 { .. }
                | SelectedInstructionKind::ExactSubtractI64 { .. } => vec![2],
                SelectedInstructionKind::CompareI64Zero => vec![],
                SelectedInstructionKind::CompareI64Immediate { .. } => vec![],
                SelectedInstructionKind::ExactDivideU64 { .. }
                | SelectedInstructionKind::WrappingRemainderI64 { .. } => vec![2],
                SelectedInstructionKind::SaturatingAddI32
                | SelectedInstructionKind::SaturatingSubtractI32
                | SelectedInstructionKind::SaturatingDivideI32 { .. } => vec![2, 3],
                SelectedInstructionKind::CompareI64 => vec![],
                _ => unreachable!("control forms handled separately"),
            },
        );
        if writes_nzcv {
            effects.implicit_unit_defs = units("nzcv");
        }
        if matches!(
            kind,
            SelectedInstructionKind::MaterializeBooleanEqual
                | SelectedInstructionKind::MaterializeBooleanU64LessThan
                | SelectedInstructionKind::MaterializeBooleanI64LessThan
                | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
        ) {
            effects.implicit_unit_uses = units("nzcv");
        }
        effects
    };
    Aarch64SelectedFormFootprint {
        register_reads: reads,
        register_writes: writes,
        writes_nzcv,
        encoded,
    }
}
