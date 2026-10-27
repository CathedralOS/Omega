//! AArch64 realization of `CompareByteSequences`: an equality scan over two
//! measured byte spans. Operand 0/1 hold the left base pointer and byte count,
//! 2/3 the right pair; operands 4-6 are early-clobber loop scratch (cursor and
//! the two loaded bytes); operand 7 is the defined 0/1 verdict. Mismatched
//! counts produce 0 and read no bytes; equal zero counts produce 1 likewise.

use super::plan_words::{self, COND_NE, Instruction};
use super::{Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding, decode_all};
use crate::selected_form_encoding::selected_forms::resolve_registers;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineAlternativeKey;
use selected_instructions::MachineEncodedMemoryEffect;

/// Internal labels within the emitted sequence; addresses are word indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Label {
    Loop,
    Equal,
    Done,
}

fn layout(registers: [u8; 8]) -> Vec<(Option<Label>, Instruction<Label>)> {
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
    let bytes = plan_words::assemble(&layout(registers))?;
    validate(physical, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands)?;
    if decode_all(bytes, plan_words::decode_word)?
        != plan_words::expected_stream(&layout(registers))?
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::nzcv_effects_encoding(
        physical,
        operands,
        bytes,
        4,
        MachineEncodedMemoryEffect::CompareByteSequencesV1 {
            left_base_operand: 0,
            left_count_operand: 1,
            right_base_operand: 2,
            right_count_operand: 3,
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
            .constraint(crate::AARCH64_COMPARE_BYTE_SEQUENCES)
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
        reject_every_bit_flip(&encoded.bytes, "compare word stream", |changed| {
            validate(&physical, alternative, &operands, &changed).is_err()
        });
    }
}
