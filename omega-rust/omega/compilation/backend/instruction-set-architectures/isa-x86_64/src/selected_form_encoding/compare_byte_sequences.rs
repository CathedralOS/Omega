//! Byte-sequence equality compare: evaluate two measured spans
//! (base + byte count each) and define a 0/1 verdict. One compound form
//! with local control flow, the same encoding style as the `CopyBytes`
//! and `ScanByteSequence` loops.
//!
//! Operand roles: `[left_base, left_count, right_base, right_count, cursor,
//! left_byte, right_byte, result]`. The four inputs are preserved; cursor
//! and the two byte loads are early-clobber loop scratch; result is the
//! defined verdict. Mismatched counts produce 0 and read no bytes; equal
//! zero counts produce 1 likewise.

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect, RegisterViewId,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError,
};
use crate::selected_form_encoding::instruction_bytes::{assemble_labeled_stream, modrm, rex};
use crate::selected_form_encoding::request_validation::resolve_registers;

/// Branch-condition suffix of the near `0F 8x` conditional-jump form.
const CC_E: u8 = 0x84;
const CC_NE: u8 = 0x85;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Label {
    Loop,
    Equal,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Instruction {
    /// `xor xR, xR` — 64-bit self-clear.
    XorSelf(u8),
    /// `inc xR`.
    Inc(u8),
    /// `test xR, xR` — 64-bit self-test setting ZF.
    TestSelf64(u8),
    /// `cmp xOperand, xSource` — 64-bit register compare for flags only.
    CompareRegister64 { source: u8, operand: u8 },
    /// `movzx xValue, byte [xBase + xIndex + offset]` — SIB byte load.
    LoadByte {
        value: u8,
        base: u8,
        index: u8,
        offset: u32,
    },
    /// `0F 8c rel32`.
    JumpConditional { condition: u8, target: Label },
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
    vec![
        // The uniform prologue defines every written operand before any
        // input is read, matching the early-clobber contract on 4-7.
        (None, Instruction::XorSelf(cursor)),
        (None, Instruction::XorSelf(left_byte)),
        (None, Instruction::XorSelf(right_byte)),
        (None, Instruction::XorSelf(result)),
        (
            None,
            Instruction::CompareRegister64 {
                source: right_count,
                operand: left_count,
            },
        ),
        (
            None,
            Instruction::JumpConditional {
                condition: CC_NE,
                target: Label::Done,
            },
        ),
        (None, Instruction::TestSelf64(left_count)),
        (
            None,
            Instruction::JumpConditional {
                condition: CC_E,
                target: Label::Equal,
            },
        ),
        (
            Some(Label::Loop),
            Instruction::LoadByte {
                value: left_byte,
                base: left_base,
                index: cursor,
                offset: 0,
            },
        ),
        (
            None,
            Instruction::LoadByte {
                value: right_byte,
                base: right_base,
                index: cursor,
                offset: 0,
            },
        ),
        (
            None,
            Instruction::CompareRegister64 {
                source: right_byte,
                operand: left_byte,
            },
        ),
        (
            None,
            Instruction::JumpConditional {
                condition: CC_NE,
                target: Label::Done,
            },
        ),
        (None, Instruction::Inc(cursor)),
        (
            None,
            Instruction::CompareRegister64 {
                source: left_count,
                operand: cursor,
            },
        ),
        (
            None,
            Instruction::JumpConditional {
                condition: CC_NE,
                target: Label::Loop,
            },
        ),
        (Some(Label::Equal), Instruction::Inc(result)),
        (Some(Label::Done), Instruction::XorSelf(cursor)),
    ]
}

fn emit_instruction(
    bytes: &mut Vec<u8>,
    instruction: Instruction,
    patches: &mut Vec<(usize, Label)>,
) {
    match instruction {
        Instruction::XorSelf(register) => bytes.extend([
            rex(register, 0, register),
            0x31,
            modrm(3, register, register),
        ]),
        Instruction::Inc(register) => {
            bytes.extend([rex(0, 0, register), 0xff, modrm(3, 0, register)])
        }
        Instruction::TestSelf64(register) => bytes.extend([
            rex(register, 0, register),
            0x85,
            modrm(3, register, register),
        ]),
        Instruction::CompareRegister64 { source, operand } => {
            bytes.extend([rex(source, 0, operand), 0x39, modrm(3, source, operand)])
        }
        Instruction::LoadByte {
            value,
            base,
            index,
            offset,
        } => {
            bytes.extend([
                rex(value, index, base),
                0x0f,
                0xb6,
                modrm(2, value, 4),
                ((index & 7) << 3) | (base & 7),
            ]);
            bytes.extend(offset.to_le_bytes());
        }
        Instruction::JumpConditional { condition, target } => {
            bytes.extend([0x0f, condition]);
            patches.push((bytes.len(), target));
            bytes.extend([0; 4]);
        }
    }
}

/// The decoded form of one emitted instruction, with absolute branch targets
/// resolved from the rel32 fields — the independent shape `validate`
/// compares against the construction-side layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Decoded {
    XorSelf(u8),
    Inc(u8),
    TestSelf64(u8),
    CompareRegister64 {
        source: u8,
        operand: u8,
    },
    LoadByte {
        value: u8,
        base: u8,
        index: u8,
        offset: u32,
    },
    JumpConditional {
        condition: u8,
        target: usize,
    },
}

fn decode_one(
    bytes: &[u8],
    position: usize,
) -> Result<(Decoded, usize), X86_64SelectedFormEncodingError> {
    let instruction = &bytes[position..];
    let fail = X86_64SelectedFormEncodingError::EncodedFormMismatch;
    let rex_fields = |prefix: u8, mode: u8| -> Option<(u8, u8)> {
        if prefix & 0xf8 != 0x48 {
            return None;
        }
        Some((
            ((mode >> 3) & 7) | ((prefix & 4) << 1),
            (mode & 7) | ((prefix & 1) << 3),
        ))
    };
    match *instruction {
        [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 => {
            let Some((register, rm)) = rex_fields(prefix, mode) else {
                return Err(fail);
            };
            if mode >> 6 == 3 && register == rm {
                Ok((Decoded::XorSelf(register), 3))
            } else {
                Err(fail)
            }
        }
        [prefix, 0xff, mode, ..]
            if prefix & 0xfe == 0x48 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
        {
            Ok((Decoded::Inc((mode & 7) | ((prefix & 1) << 3)), 3))
        }
        [prefix, 0x85, mode, ..] if prefix & 0xfa == 0x48 => {
            let Some((register, rm)) = rex_fields(prefix, mode) else {
                return Err(fail);
            };
            if mode >> 6 == 3 && register == rm {
                Ok((Decoded::TestSelf64(register), 3))
            } else {
                Err(fail)
            }
        }
        [prefix, 0x39, mode, ..] if prefix & 0xfa == 0x48 => {
            let Some((register, rm)) = rex_fields(prefix, mode) else {
                return Err(fail);
            };
            if mode >> 6 == 3 {
                Ok((
                    Decoded::CompareRegister64 {
                        source: register,
                        operand: rm,
                    },
                    3,
                ))
            } else {
                Err(fail)
            }
        }
        [prefix, 0x0f, 0xb6, mode, sib, d0, d1, d2, d3, ..]
            if prefix & 0xf8 == 0x48 && mode & 0xc7 == 0x84 && sib & 0xc0 == 0 =>
        {
            let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let base = (sib & 7) | ((prefix & 1) << 3);
            let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
            if index == 4 {
                return Err(fail);
            }
            Ok((
                Decoded::LoadByte {
                    value,
                    base,
                    index,
                    offset: u32::from_le_bytes([d0, d1, d2, d3]),
                },
                9,
            ))
        }
        [0x0f, condition, d0, d1, d2, d3, ..] if condition & 0xf0 == 0x80 => {
            let relative = i32::from_le_bytes([d0, d1, d2, d3]) as i64;
            let target = position as i64 + 6 + relative;
            if target < 0 || target > bytes.len() as i64 {
                return Err(fail);
            }
            Ok((
                Decoded::JumpConditional {
                    condition,
                    target: target as usize,
                },
                6,
            ))
        }
        _ => Err(fail),
    }
}

fn decode_all_stream(bytes: &[u8]) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
    let mut decoded = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let (instruction, length) = decode_one(bytes, position)?;
        decoded.push(instruction);
        position += length;
    }
    Ok(decoded)
}

fn expected_stream(registers: [u8; 8]) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
    let plan = layout(registers);
    let bytes = assemble_labeled_stream(&plan, emit_instruction)?;
    decode_all_stream(&bytes)
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<[u8; 8], X86_64SelectedFormEncodingError> {
    super::request_validation::guard_canonical(physical)?;
    if displacement != 0
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::CompareByteSequences,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; 8] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    // `rsp` cannot encode the SIB index the loop's byte loads use, so the
    // cursor may never land there.
    if registers[4] == 4 {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    // Early-clobber writes may never land on a preserved input or on a
    // scratch written earlier in the sequence.
    if registers[..4].contains(&registers[4])
        || registers[..5].contains(&registers[5])
        || registers[..6].contains(&registers[6])
        || registers[..7].contains(&registers[7])
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands, displacement)?;
    let bytes = assemble_labeled_stream(&layout(registers), emit_instruction)?;
    validate(physical, alternative, operands, displacement, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let registers = request(physical, alternative, operands, displacement)?;
    if decode_all_stream(bytes)? != expected_stream(registers)? {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::canonical_effects_encoding(
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
    use super::super::{
        SelectedInstructionKind, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };
    use super::{MachineAlternativeFamily, MachineAlternativeKey, encode, validate};

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
    fn compare_byte_sequences_catalog_requires_early_clobbers_and_memory_effect() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let catalog = crate::x86_64_register_constraint_catalog(&physical);
        let constraints =
            crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical).unwrap();
        crate::selected_form_encoding::tests::assert_early_clobber_required(
            &catalog,
            &physical,
            crate::X86_64_COMPARE_BYTE_SEQUENCES,
            &[4, 5, 6, 7],
        );
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::windows_x64(),
        ] {
            let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, effects.clone())
                .unwrap();
            let row = effects
                .declarations
                .iter()
                .find(|row| {
                    row.semantic == selected_instructions::MachineSemanticKind::CompareByteSequences
                })
                .unwrap();
            assert_eq!(
                row.memory,
                selected_instructions::MachineMemoryEffect::CompareByteSequencesV1
            );
            assert_eq!(
                row.alternatives[0].encoded.external_operand_reads,
                [0, 1, 2, 3]
            );
            assert_eq!(
                row.alternatives[0].encoded.external_operand_writes,
                [4, 5, 6, 7]
            );
        }
    }

    #[test]
    fn compare_byte_sequences_round_trips_and_rejects_every_changed_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = alternative();
        for names in [
            ["r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"],
            ["rax", "rcx", "rdx", "rsi", "rdi", "rbx", "rbp", "r8"],
        ] {
            let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
            let encoded = encode(&physical, alternative, &operands, 0).unwrap();
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind(), alternative, &operands).unwrap(),
                encoded
            );
            validate_x86_64_selected_form_encoding(
                &physical,
                kind(),
                alternative,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate(&physical, alternative, &operands, 0, &changed).is_err(),
                    "bit {bit}"
                );
            }
            super::super::assert_byte_fragility(encoded.bytes(), |bytes| {
                validate(&physical, alternative, &operands, 0, bytes).is_err()
            });
            assert!(encode(&physical, alternative, &operands, 1).is_err());
        }
    }
}
