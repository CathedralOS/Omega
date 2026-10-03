//! AArch64 realization of `CompareByteSequences`: an equality scan over two
//! measured byte spans. Operand 0/1 hold the left base pointer and byte count,
//! 2/3 the right pair; operands 4-6 are early-clobber loop scratch (cursor and
//! the two loaded bytes); operand 7 is the defined 0/1 verdict. Mismatched
//! counts produce 0 and read no bytes; equal zero counts produce 1 likewise.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use crate::selected_form_encoding::selected_forms::resolve_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedStackEffect;
use selected_instructions::MachineEncodedTrapBehavior;

const COND_NE: u8 = 0x1;

/// Internal labels within the emitted sequence; addresses are word indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Label {
    Loop,
    Equal,
    Done,
}

/// One fixed-width instruction slot in the emitted plan.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Instruction {
    /// `mov xR, #0`.
    Zero(u8),
    /// `mov xR, #1`.
    One(u8),
    /// `cbz xR, target`.
    Cbz(u8, Label),
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
    /// `b.cond target`.
    BranchConditional { condition: u8, target: Label },
}

fn layout(registers: [u8; 8]) -> Vec<(Option<Label>, Instruction)> {
    let [
        left_base,
        left_count,
        right_base,
        right_count,
        cursor,
        left_byte,
        right_byte,
        result,
    ] = registers;
    // The uniform prologue defines every written operand before any input is
    // read, matching the early-clobber contract on operands 4-7.
    vec![
        (None, Instruction::Zero(cursor)),
        (None, Instruction::Zero(left_byte)),
        (None, Instruction::Zero(right_byte)),
        (None, Instruction::Zero(result)),
        (
            None,
            Instruction::CompareRegister {
                left: left_count,
                right: right_count,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_NE,
                target: Label::Done,
            },
        ),
        (None, Instruction::Cbz(left_count, Label::Equal)),
        (
            Some(Label::Loop),
            Instruction::LdrbRegister {
                destination: left_byte,
                base: left_base,
                index: cursor,
            },
        ),
        (
            None,
            Instruction::LdrbRegister {
                destination: right_byte,
                base: right_base,
                index: cursor,
            },
        ),
        (
            None,
            Instruction::CompareRegister {
                left: left_byte,
                right: right_byte,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_NE,
                target: Label::Done,
            },
        ),
        (
            None,
            Instruction::AddImmediate {
                destination: cursor,
                source: cursor,
                immediate: 1,
            },
        ),
        (
            None,
            Instruction::CompareRegister {
                left: cursor,
                right: left_count,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_NE,
                target: Label::Loop,
            },
        ),
        (Some(Label::Equal), Instruction::One(result)),
        (Some(Label::Done), Instruction::Zero(cursor)),
    ]
}

/// One decoded instruction slot in the expected stream.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Decoded {
    Zero(u8),
    One(u8),
    Cbz(u8, i32),
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
    BranchConditional {
        condition: u8,
        target: i32,
    },
}

fn emit_word(
    instruction: Instruction,
    position: usize,
    labels: &[(Label, usize)],
) -> Result<u32, Aarch64SelectedFormEncodingError> {
    let target_word = |label: Label| -> usize {
        labels
            .iter()
            .find(|(candidate, _)| *candidate == label)
            .map(|(_, index)| *index)
            .expect("compare_byte_sequences layout defines every branch label")
    };
    let relative = |label: Label, bits: u32| -> Result<u32, Aarch64SelectedFormEncodingError> {
        let displacement = target_word(label) as i64 - position as i64;
        let limit = 1i64 << (bits - 1);
        if !(-limit..limit).contains(&displacement) {
            return Err(match bits {
                19 => Aarch64SelectedFormEncodingError::BranchDisplacementOutsideImm19,
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
        Instruction::BranchConditional { condition, target } => {
            0x5400_0000 | (relative(target, 19)? << 5) | condition as u32
        }
    })
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn decode_word(word: u32, position: usize) -> Result<Decoded, Aarch64SelectedFormEncodingError> {
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
    if word & 0xff00_0010 == 0x5400_0000 {
        let displacement = sign_extend((word >> 5) & 0x7ffff, 19);
        return Ok(Decoded::BranchConditional {
            condition: register & 15,
            target: position as i32 + displacement,
        });
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}

fn expected_stream(registers: [u8; 8]) -> Result<Vec<Decoded>, Aarch64SelectedFormEncodingError> {
    let plan = layout(registers);
    let mut label_index = 0usize;
    let mut labels = Vec::new();
    for (label, _) in &plan {
        if let Some(label) = label {
            labels.push((*label, label_index));
        }
        label_index += 1;
    }
    let mut expected = Vec::with_capacity(plan.len());
    for (position, (_, instruction)) in plan.iter().enumerate() {
        expected.push(decode_word(
            emit_word(*instruction, position, &labels)?,
            position,
        )?);
    }
    Ok(expected)
}

fn assemble(registers: [u8; 8]) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let plan = layout(registers);
    let mut label_index = 0usize;
    let mut labels = Vec::new();
    for (label, _) in &plan {
        if let Some(label) = label {
            labels.push((*label, label_index));
        }
        label_index += 1;
    }
    let mut bytes = Vec::with_capacity(plan.len() * 4);
    for (position, (_, instruction)) in plan.iter().enumerate() {
        bytes.extend_from_slice(&emit_word(*instruction, position, &labels)?.to_le_bytes());
    }
    Ok(bytes)
}

fn decode_all(bytes: &[u8]) -> Result<Vec<Decoded>, Aarch64SelectedFormEncodingError> {
    let (words, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    words
        .iter()
        .enumerate()
        .map(|(position, word)| decode_word(u32::from_le_bytes(*word), position))
        .collect()
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 8], Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::CompareByteSequences,
            variant: 0,
        })
    {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let registers: [u8; 8] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::OperandCountMismatch)?;
    // Early-clobber writes may never land on a preserved input or on a
    // scratch written earlier in the sequence.
    if registers[..4].contains(&registers[4])
        || registers[..5].contains(&registers[5])
        || registers[..6].contains(&registers[6])
        || registers[..7].contains(&registers[7])
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    let bytes = assemble(registers)?;
    validate(physical, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    if decode_all(bytes)? != expected_stream(registers)? {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let flags = physical
        .model()
        .view_named("nzcv")
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?
        .units
        .clone();
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands[..4].to_vec(),
            register_writes: operands[4..].to_vec(),
            writes_nzcv: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0, 1, 2, 3],
                external_operand_writes: vec![4, 5, 6, 7],
                implicit_unit_uses: vec![],
                implicit_unit_defs: vec![],
                implicit_unit_clobbers: flags,
                memory: MachineEncodedMemoryEffect::CompareByteSequencesV1 {
                    left_base_operand: 0,
                    left_count_operand: 1,
                    right_base_operand: 2,
                    right_count_operand: 3,
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use crate::aarch64_physical_register_model;
    use crate::selected_form_encoding::selected_forms::{
        encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
    };
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::SelectedInstructionKind;

    fn kind() -> SelectedInstructionKind {
        SelectedInstructionKind::CompareByteSequences
    }

    fn alternative() -> MachineAlternativeKey {
        MachineAlternativeKey {
            family: MachineAlternativeFamily::CompareByteSequences,
            variant: 0,
        }
    }

    #[test]
    fn constraint_catalog_declares_the_compare_register_contract() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let catalog = crate::aarch64_register_constraint_catalog(&physical);
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == crate::AARCH64_COMPARE_BYTE_SEQUENCES)
            .expect("catalog declares the compare constraint");
        assert_eq!(row.operands.len(), 8);
        for position in 0..4 {
            assert!(row.operands[position].access == register_model::RegisterOperandAccess::Use);
        }
        for scratch in 4..8 {
            assert!(row.operands[scratch].early_clobber);
        }
    }

    #[test]
    fn encode_and_validate_replay_every_compare_word_for_word() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        let alternative = alternative();
        let encoded = encode(&physical, alternative, &operands).unwrap();
        let through_selected_form =
            encode_aarch64_selected_form(&physical, kind(), alternative, &operands).unwrap();
        assert_eq!(encoded.bytes, through_selected_form.bytes);
        validate_aarch64_selected_form_encoding(
            &physical,
            kind(),
            alternative,
            &operands,
            &encoded.bytes,
        )
        .unwrap();
        for position in 0..encoded.bytes.len() * 8 {
            let mut changed = encoded.bytes.clone();
            changed[position / 8] ^= 1 << (position % 8);
            assert!(
                validate(&physical, alternative, &operands, &changed).is_err(),
                "bit {position}"
            );
        }
    }
}
