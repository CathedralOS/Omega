//! A fixed local loop answers a byte-sequence predicate without source CFG
//! edges, mirroring the CopyBytes contract: inputs survive; early-clobber
//! cursor, byte and result temporaries own every write.
//!
//! The emitted form is a typed instruction plan shared by `assemble` and the
//! expected-stream oracle; `validate` re-decodes the emitted words through an
//! independent decoder so a single flipped bit either fails decode or
//! mismatches the plan. All branches are AArch64 `B.cond`/`B`/TBNZ/CBZ forms
//! with word-granularity immediates far inside their encodable ranges.

use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding, decode_all};
use crate::selected_form_encoding::selected_forms::resolve_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::ByteSequencePredicate;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedMemoryEffect;

/// AArch64 condition codes used by the class dispatch.
const COND_EQ: u8 = 0x0;
const COND_HS: u8 = 0x2;
const COND_LO: u8 = 0x3;
const COND_HI: u8 = 0x8;

/// Internal labels within the emitted sequence; addresses are word indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Label {
    Loop,
    Done,
    Fail,
    Advance1,
    Cont1,
    Cont2Any,
    Lead3,
    E0Path,
    EdPath,
    Cont3Any,
    F0Path,
    F4Path,
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

fn layout(
    predicate: ByteSequencePredicate,
    registers: [u8; 5],
) -> Vec<(Option<Label>, Instruction)> {
    let [base, count, cursor, byte, result] = registers;
    let mut out: Vec<(Option<Label>, Instruction)> = Vec::new();
    // The uniform prologue defines every written operand before any input is
    // read, matching the early-clobber contract on operands 2/3/4.
    out.extend([
        (None, Instruction::Zero(cursor)),
        (None, Instruction::Zero(byte)),
        (None, Instruction::Zero(result)),
    ]);
    if predicate == ByteSequencePredicate::NonEmpty {
        out.extend([
            (None, Instruction::Cbz(count, Label::Done)),
            (None, Instruction::One(result)),
            (Some(Label::Done), Instruction::Zero(cursor)),
        ]);
        return out;
    }
    out.push((None, Instruction::One(result)));
    match predicate {
        ByteSequencePredicate::NoNul | ByteSequencePredicate::AsciiOnly => {
            out.push((None, Instruction::Cbz(count, Label::Done)));
            out.push((
                Some(Label::Loop),
                Instruction::LdrbRegister {
                    destination: byte,
                    base,
                    index: cursor,
                },
            ));
            out.push((
                None,
                if predicate == ByteSequencePredicate::NoNul {
                    Instruction::Cbz(byte, Label::Fail)
                } else {
                    Instruction::Tbnz7(byte, Label::Fail)
                },
            ));
            out.extend([
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
                        right: count,
                    },
                ),
                (
                    None,
                    Instruction::BranchConditional {
                        condition: COND_LO,
                        target: Label::Loop,
                    },
                ),
                (None, Instruction::Branch(Label::Done)),
                (Some(Label::Fail), Instruction::Zero(result)),
                (Some(Label::Done), Instruction::Zero(cursor)),
            ]);
            return out;
        }
        _ => {}
    }
    // ValidUtf8: lead-byte class dispatch onto per-width continuation tails.
    out.extend([
        (None, Instruction::Cbz(count, Label::Done)),
        (
            Some(Label::Loop),
            Instruction::CompareRegister {
                left: cursor,
                right: count,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_HS,
                target: Label::Done,
            },
        ),
        (
            None,
            Instruction::LdrbRegister {
                destination: byte,
                base,
                index: cursor,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0x80,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_LO,
                target: Label::Advance1,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xC2,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_LO,
                target: Label::Fail,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xE0,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_LO,
                target: Label::Cont1,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xF0,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_HS,
                target: Label::Lead3,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xE0,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_EQ,
                target: Label::E0Path,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xED,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_EQ,
                target: Label::EdPath,
            },
        ),
        (None, Instruction::Branch(Label::Cont2Any)),
        (
            Some(Label::Lead3),
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xF5,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_HS,
                target: Label::Fail,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xF0,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_EQ,
                target: Label::F0Path,
            },
        ),
        (
            None,
            Instruction::CompareImmediate {
                register: byte,
                immediate: 0xF4,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_EQ,
                target: Label::F4Path,
            },
        ),
        (None, Instruction::Branch(Label::Cont3Any)),
    ]);
    for (label, width, low, high) in [
        (Label::Cont1, 1u16, 0x80u16, 0xBFu16),
        (Label::Cont2Any, 2, 0x80, 0xBF),
        (Label::E0Path, 2, 0xA0, 0xBF),
        (Label::EdPath, 2, 0x80, 0x9F),
        (Label::Cont3Any, 3, 0x80, 0xBF),
        (Label::F0Path, 3, 0x90, 0xBF),
        (Label::F4Path, 3, 0x80, 0x8F),
    ] {
        emit_continuation_tail(&mut out, label, width, low, high, registers);
    }
    out.extend([
        (
            Some(Label::Advance1),
            Instruction::AddImmediate {
                destination: cursor,
                source: cursor,
                immediate: 1,
            },
        ),
        (None, Instruction::Branch(Label::Loop)),
        (Some(Label::Fail), Instruction::Zero(result)),
        (Some(Label::Done), Instruction::Zero(cursor)),
    ]);
    out
}

/// Emit `n` continuation bytes for a lead already consumed, each constrained
/// to `[low, high]` for the first and `[0x80, 0xBF]` afterwards. `byte` is the
/// address scratch and then the loaded value.
fn emit_continuation_tail(
    out: &mut Vec<(Option<Label>, Instruction)>,
    label: Label,
    width: u16,
    low: u16,
    high: u16,
    registers: [u8; 5],
) {
    let [base, count, cursor, byte, _result] = registers;
    // The byte scratch doubles as the limit index: cursor + width reaching
    // count means the tail would read past the sequence.
    out.push((
        Some(label),
        Instruction::AddImmediate {
            destination: byte,
            source: cursor,
            immediate: width,
        },
    ));
    out.extend([
        (
            None,
            Instruction::CompareRegister {
                left: byte,
                right: count,
            },
        ),
        (
            None,
            Instruction::BranchConditional {
                condition: COND_HS,
                target: Label::Fail,
            },
        ),
    ]);
    for offset in 1..=width {
        out.extend([
            (
                None,
                Instruction::AddImmediate {
                    destination: byte,
                    source: cursor,
                    immediate: offset,
                },
            ),
            (
                None,
                Instruction::LdrbRegister {
                    destination: byte,
                    base,
                    index: byte,
                },
            ),
            (
                None,
                Instruction::CompareImmediate {
                    register: byte,
                    immediate: if offset == 1 { low } else { 0x80 },
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_LO,
                    target: Label::Fail,
                },
            ),
            (
                None,
                Instruction::CompareImmediate {
                    register: byte,
                    immediate: if offset == 1 { high } else { 0xBF },
                },
            ),
            (
                None,
                Instruction::BranchConditional {
                    condition: COND_HI,
                    target: Label::Fail,
                },
            ),
        ]);
    }
    out.extend([
        (
            None,
            Instruction::AddImmediate {
                destination: cursor,
                source: cursor,
                immediate: width + 1,
            },
        ),
        (None, Instruction::Branch(Label::Loop)),
    ]);
}

/// The word-level shape `validate` reconstructs from raw bytes: the same
/// instructions but with branch targets resolved to absolute word indices.
#[derive(PartialEq, Eq, Debug)]
enum Decoded {
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
            .expect("scan_byte_sequence layout defines every branch label")
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

use super::compare_byte_sequences::sign_extend;

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
            condition: register as u8 & 15,
            target: position as i32 + displacement,
        });
    }
    if word & 0xfc00_0000 == 0x1400_0000 {
        let displacement = sign_extend(word & 0x03ff_ffff, 26);
        return Ok(Decoded::Branch(position as i32 + displacement));
    }
    Err(Aarch64SelectedFormEncodingError::MalformedEncoding)
}

fn expected_stream(
    predicate: ByteSequencePredicate,
    registers: [u8; 5],
) -> Result<Vec<Decoded>, Aarch64SelectedFormEncodingError> {
    let plan = layout(predicate, registers);
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

fn assemble(
    predicate: ByteSequencePredicate,
    registers: [u8; 5],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let plan = layout(predicate, registers);
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

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 5], Aarch64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::ScanByteSequence,
            variant: 0,
        })
    {
        return Err(Aarch64SelectedFormEncodingError::AlternativeMismatch);
    }
    let registers: [u8; 5] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Aarch64SelectedFormEncodingError::OperandCountMismatch)?;
    // Early-clobber writes may never land on a preserved input.
    if registers[..2].contains(&registers[2])
        || registers[..3].contains(&registers[3])
        || registers[..4].contains(&registers[4])
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    predicate: ByteSequencePredicate,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    let bytes = assemble(predicate, registers)?;
    validate(physical, predicate, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    predicate: ByteSequencePredicate,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    if decode_all(bytes, decode_word)? != expected_stream(predicate, registers)? {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::nzcv_effects_encoding(
        physical,
        operands,
        bytes,
        2,
        MachineEncodedMemoryEffect::ScanByteSequenceV1 {
            base_pointer_operand: 0,
            count_operand: 1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use crate::aarch64_physical_register_model;
    use crate::selected_form_encoding::selected_forms::{
        encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
    };
    use selected_instructions::ByteSequencePredicate;
    use selected_instructions::MachineAlternativeFamily;
    use selected_instructions::MachineAlternativeKey;
    use selected_instructions::SelectedInstructionKind;

    fn kind(predicate: ByteSequencePredicate) -> SelectedInstructionKind {
        SelectedInstructionKind::ScanByteSequence { predicate }
    }

    #[test]
    fn constraint_catalog_declares_the_scan_register_contract() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let catalog = crate::aarch64_register_constraint_catalog(&physical);
        let row = catalog
            .constraint(crate::AARCH64_SCAN_BYTE_SEQUENCE)
            .expect("catalog declares the scan constraint");
        assert_eq!(row.operands.len(), 5);
        assert!(row.operands[0].access == register_model::RegisterOperandAccess::Use);
        assert!(row.operands[1].access == register_model::RegisterOperandAccess::Use);
        for scratch in 2..5 {
            assert!(row.operands[scratch].early_clobber);
        }
    }

    #[test]
    fn encode_and_validate_replay_every_predicate_word_for_word() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let operands = ["x0", "x1", "x2", "x3", "x4"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ScanByteSequence,
            variant: 0,
        };
        for predicate in ByteSequencePredicate::ALL {
            let encoded = encode(&physical, predicate, alternative, &operands).unwrap();
            let through_selected_form =
                encode_aarch64_selected_form(&physical, kind(predicate), alternative, &operands)
                    .unwrap();
            assert_eq!(encoded.bytes, through_selected_form.bytes);
            validate_aarch64_selected_form_encoding(
                &physical,
                kind(predicate),
                alternative,
                &operands,
                &encoded.bytes,
            )
            .unwrap();
            for position in 0..encoded.bytes.len() * 8 {
                let mut changed = encoded.bytes.clone();
                changed[position / 8] ^= 1 << (position % 8);
                assert!(
                    validate(&physical, predicate, alternative, &operands, &changed).is_err(),
                    "{predicate:?} bit {position}"
                );
            }
        }
    }
}
