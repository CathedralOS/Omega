//! The word-level realization of a byte-sequence plan: one instruction
//! vocabulary, the encoder that resolves each branch label to a relative word
//! displacement, and the independent decoder `validate` replays the emitted
//! stream through. `compare_byte_sequences` and `scan_byte_sequence` differ
//! only in the plan they lay out, so both emit and re-decode their words here.

use super::Aarch64SelectedFormEncodingError;

/// AArch64 condition codes the byte-sequence plans branch on.
pub(super) const COND_EQ: u8 = 0x0;
pub(super) const COND_NE: u8 = 0x1;
pub(super) const COND_HS: u8 = 0x2;
pub(super) const COND_LO: u8 = 0x3;
pub(super) const COND_HI: u8 = 0x8;

/// One fixed-width instruction slot in an emitted plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Instruction<Label> {
    /// `mov xR, #0`.
    Zero(u8),
    /// `mov xR, #1`.
    One(u8),
    /// `cbz xR, target`.
    Cbz(u8, Label),
    /// `tbnz wR, #7, target` — ASCII high-bit test on the byte scratch.
    Tbnz7(u8, Label),
    /// `ldrb wD, [xB, xM]`.
    LdrbRegister {
        destination: u8,
        base: u8,
        index: u8,
    },
    /// `add xD, xS, #imm`.
    AddImmediate {
        destination: u8,
        source: u8,
        immediate: u16,
    },
    /// `cmp xL, xR` — subtracts right from left for flags only.
    CompareRegister { left: u8, right: u8 },
    /// `cmp xR, #imm`.
    CompareImmediate { register: u8, immediate: u16 },
    /// `b.cond target`.
    BranchConditional { condition: u8, target: Label },
    /// `b target`.
    Branch(Label),
}

/// One decoded instruction slot in the expected stream: the same instructions
/// with branch targets resolved to absolute word indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Decoded {
    Zero(u8),
    One(u8),
    Cbz(u8, i32),
    Tbnz7(u8, i32),
    LdrbRegister {
        destination: u8,
        base: u8,
        index: u8,
    },
    AddImmediate {
        destination: u8,
        source: u8,
        immediate: u16,
    },
    CompareRegister {
        left: u8,
        right: u8,
    },
    CompareImmediate {
        register: u8,
        immediate: u16,
    },
    BranchConditional {
        condition: u8,
        target: i32,
    },
    Branch(i32),
}

pub(super) fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

/// Emit `plan` as words in slot order, resolving every branch label to the
/// word index it marks.
pub(super) fn words<Label: Copy + Eq>(
    plan: &[(Option<Label>, Instruction<Label>)],
) -> Result<Vec<u32>, Aarch64SelectedFormEncodingError> {
    let labels = plan
        .iter()
        .enumerate()
        .filter_map(|(index, (label, _))| label.map(|label| (label, index)))
        .collect::<Vec<_>>();
    plan.iter()
        .enumerate()
        .map(|(position, (_, instruction))| emit_word(*instruction, position, &labels))
        .collect()
}

/// The plan as the little-endian byte stream its validator replays.
pub(super) fn assemble<Label: Copy + Eq>(
    plan: &[(Option<Label>, Instruction<Label>)],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    Ok(words(plan)?
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

/// The plan as the expected decoded stream.
pub(super) fn expected_stream<Label: Copy + Eq>(
    plan: &[(Option<Label>, Instruction<Label>)],
) -> Result<Vec<Decoded>, Aarch64SelectedFormEncodingError> {
    words(plan)?
        .into_iter()
        .enumerate()
        .map(|(position, word)| decode_word(word, position))
        .collect()
}

fn emit_word<Label: Copy + Eq>(
    instruction: Instruction<Label>,
    position: usize,
    labels: &[(Label, usize)],
) -> Result<u32, Aarch64SelectedFormEncodingError> {
    let target_word = |label: Label| -> usize {
        labels
            .iter()
            .find(|(candidate, _)| *candidate == label)
            .map(|(_, index)| *index)
            .expect("the plan defines every branch label")
    };
    let relative = |label: Label, bits: u32| -> Result<u32, Aarch64SelectedFormEncodingError> {
        let displacement = target_word(label) as i64 - position as i64;
        let limit = 1i64 << (bits - 1);
        if !(-limit..limit).contains(&displacement) {
            return Err(match bits {
                14 | 19 => Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19,
                _ => Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm26,
            });
        }
        Ok((displacement as i32) as u32 & ((1u32 << bits) - 1))
    };
    Ok(match instruction {
        Instruction::Zero(register) => 0xd280_0000 | register as u32,
        Instruction::One(register) => 0xd280_0020 | register as u32,
        Instruction::Cbz(register, target) => {
            0xb400_0000 | (relative(target, 19)? << 5) | register as u32
        }
        Instruction::Tbnz7(register, target) => {
            0x3738_0000 | (relative(target, 14)? << 5) | register as u32
        }
        Instruction::LdrbRegister {
            destination,
            base,
            index,
        } => 0x3860_6800 | ((index as u32) << 16) | ((base as u32) << 5) | destination as u32,
        Instruction::AddImmediate {
            destination,
            source,
            immediate,
        } => 0x9100_0000 | ((immediate as u32) << 10) | ((source as u32) << 5) | destination as u32,
        Instruction::CompareRegister { left, right } => {
            0xeb00_001f | ((right as u32) << 16) | ((left as u32) << 5)
        }
        Instruction::CompareImmediate {
            register,
            immediate,
        } => 0xf100_0000 | ((immediate as u32) << 10) | ((register as u32) << 5) | 0x1f,
        Instruction::BranchConditional { condition, target } => {
            0x5400_0000 | (relative(target, 19)? << 5) | condition as u32
        }
        Instruction::Branch(target) => 0x1400_0000 | relative(target, 26)?,
    })
}

pub(super) fn decode_word(
    word: u32,
    position: usize,
) -> Result<Decoded, Aarch64SelectedFormEncodingError> {
    let register = (word & 31) as u8;
    if word & 0xffe0_0000 == 0xd280_0000 {
        return Ok(match (word >> 5) & 0xffff {
            0 => Decoded::Zero(register),
            1 => Decoded::One(register),
            _ => return Err(Aarch64SelectedFormEncodingError::MalformedEncoding),
        });
    }
    if word & 0xff00_0000 == 0xb400_0000 {
        let displacement = sign_extend((word >> 5) & 0x7ffff, 19);
        return Ok(Decoded::Cbz(register, position as i32 + displacement));
    }
    if word & 0xfff8_0000 == 0x3738_0000 {
        let displacement = sign_extend((word >> 5) & 0x3fff, 14);
        return Ok(Decoded::Tbnz7(register, position as i32 + displacement));
    }
    if word & 0xffe0_fc00 == 0x3860_6800 {
        return Ok(Decoded::LdrbRegister {
            destination: register,
            base: ((word >> 5) & 31) as u8,
            index: ((word >> 16) & 31) as u8,
        });
    }
    if word & 0xffc0_0000 == 0x9100_0000 {
        return Ok(Decoded::AddImmediate {
            destination: register,
            source: ((word >> 5) & 31) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
        });
    }
    if word & 0xffe0_fc1f == 0xeb00_001f {
        return Ok(Decoded::CompareRegister {
            left: ((word >> 5) & 31) as u8,
            right: ((word >> 16) & 31) as u8,
        });
    }
    if word & 0xffc0_0000 == 0xf100_0000 && register == 0x1f {
        return Ok(Decoded::CompareImmediate {
            register: ((word >> 5) & 31) as u8,
            immediate: ((word >> 10) & 0xfff) as u16,
        });
    }
    if word & 0xff00_0010 == 0x5400_0000 {
        let displacement = sign_extend((word >> 5) & 0x7ffff, 19);
        return Ok(Decoded::BranchConditional {
            condition: register & 15,
            target: position as i32 + displacement,
        });
    }
    if word & 0xfc00_0000 == 0x1400_0000 {
        let displacement = sign_extend(word & 0x03ff_ffff, 26);
        return Ok(Decoded::Branch(position as i32 + displacement));
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}
