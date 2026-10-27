//! A fixed local loop answers a byte-sequence predicate without source CFG
//! edges, mirroring the CopyBytes contract: inputs survive; early-clobber
//! cursor, byte and result temporaries own every write.
//!
//! The emitted form is a typed instruction plan shared by `assemble` and the
//! expected-stream oracle; `validate` re-decodes the emitted words through an
//! independent decoder so a single flipped bit either fails decode or
//! mismatches the plan. All branches are AArch64 `B.cond`/`B`/TBNZ/CBZ forms
//! with word-granularity immediates far inside their encodable ranges.

use super::plan_words::{self, COND_EQ, COND_HI, COND_HS, COND_LO, Instruction};
use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding, decode_all};
use crate::selected_form_encoding::selected_forms::request_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::ByteSequencePredicate;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedMemoryEffect;

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

fn layout(
    predicate: ByteSequencePredicate,
    registers: [u8; 5],
) -> Vec<(Option<Label>, Instruction<Label>)> {
    let [base, count, cursor, byte, result] = registers;
    let mut out: Vec<(Option<Label>, Instruction<Label>)> = Vec::new();
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
    out: &mut Vec<(Option<Label>, Instruction<Label>)>,
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

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<[u8; 5], Aarch64SelectedFormEncodingError> {
    // Operands 2-4 are early-clobber scratch.
    request_registers(
        physical,
        alternative,
        MachineAlternativeFamily::ScanByteSequence,
        operands,
        2,
    )
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    predicate: ByteSequencePredicate,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    let bytes = plan_words::assemble(&layout(predicate, registers))?;
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
    if decode_all(bytes, plan_words::decode_word)?
        != plan_words::expected_stream(&layout(predicate, registers))?
    {
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
    use crate::test_support::reject_every_bit_flip;
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
            reject_every_bit_flip(&encoded.bytes, &format!("{predicate:?}"), |changed| {
                validate(&physical, predicate, alternative, &operands, &changed).is_err()
            });
        }
    }
}
