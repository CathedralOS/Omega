//! Byte-sequence predicate scan: evaluate a `ByteSequencePredicate` over
//! `count` bytes at `base`, defining a 0/1 verdict. One compound form with
//! local control flow, the same encoding style as the `CopyBytes` loop.
//!
//! Operand roles: `[base, count, cursor, byte, result]`. Base and count are
//! preserved inputs; cursor and byte are early-clobber loop scratch; result
//! is the defined verdict. `NonEmpty` needs no loop, `NoNul`/`AsciiOnly`
//! are early-exit scans, and `ValidUtf8` dispatches the lead byte into
//! per-class continuation checks (the multi-byte classes differ only in the
//! second byte's admitted range, so each range is an inline immediate
//! compare rather than a table).

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect, RegisterViewId,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError,
};
use crate::selected_form_encoding::instruction_bytes::{assemble_labeled_stream, modrm, rex};
use crate::selected_form_encoding::request_validation::resolve_registers;
use selected_instructions::ByteSequencePredicate;

/// Branch-condition suffix of the near `0F 8x` conditional-jump form.
const CC_B: u8 = 0x82;
const CC_AE: u8 = 0x83;
const CC_E: u8 = 0x84;
const CC_NE: u8 = 0x85;
const CC_A: u8 = 0x87;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Label {
    Loop,
    Done,
    Fail,
    Advance1,
    Cont1,
    Cont2Any,
    E0Path,
    EdPath,
    Lead3,
    Cont3Any,
    F0Path,
    F4Path,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Instruction {
    XorSelf(u8),
    Inc(u8),
    AddImmediate(u8, u8),
    TestSelf64(u8),
    TestSelf8(u8),
    TestImmediate8(u8, u8),
    CompareImmediate8(u8, u8),
    /// `cmp operand, source`: flags describe `operand - source`.
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
    SetNotEqual8(u8),
    LoadAddress {
        value: u8,
        base: u8,
        offset: u8,
    },
    JumpConditional {
        condition: u8,
        target: Label,
    },
    Jump {
        target: Label,
    },
}

fn layout(
    predicate: ByteSequencePredicate,
    base: u8,
    count: u8,
    cursor: u8,
    byte: u8,
    result: u8,
) -> Vec<(Option<Label>, Instruction)> {
    use Instruction::*;
    let mut stream: Vec<(Option<Label>, Instruction)> = Vec::new();
    let mut emit = |label: Option<Label>, instruction: Instruction| {
        stream.push((label, instruction));
    };
    emit(None, XorSelf(cursor));
    emit(None, XorSelf(byte));
    emit(None, XorSelf(result));
    if predicate == ByteSequencePredicate::NonEmpty {
        emit(None, TestSelf64(count));
        emit(None, SetNotEqual8(result));
        return stream;
    }
    emit(None, Inc(result));
    emit(None, TestSelf64(count));
    emit(
        None,
        JumpConditional {
            condition: CC_E,
            target: Label::Done,
        },
    );
    if predicate == ByteSequencePredicate::ValidUtf8 {
        emit(
            Some(Label::Loop),
            CompareRegister64 {
                source: count,
                operand: cursor,
            },
        );
        emit(
            None,
            JumpConditional {
                condition: CC_AE,
                target: Label::Done,
            },
        );
        emit(
            None,
            LoadByte {
                value: byte,
                base,
                index: cursor,
                offset: 0,
            },
        );
        emit(None, CompareImmediate8(byte, 0x80));
        emit(
            None,
            JumpConditional {
                condition: CC_B,
                target: Label::Advance1,
            },
        );
        // 0x80..0xC2 are stray continuations or the overlong leads C0/C1.
        emit(None, CompareImmediate8(byte, 0xC2));
        emit(
            None,
            JumpConditional {
                condition: CC_B,
                target: Label::Fail,
            },
        );
        emit(None, CompareImmediate8(byte, 0xE0));
        emit(
            None,
            JumpConditional {
                condition: CC_B,
                target: Label::Cont1,
            },
        );
        emit(None, CompareImmediate8(byte, 0xF0));
        emit(
            None,
            JumpConditional {
                condition: CC_AE,
                target: Label::Lead3,
            },
        );
        emit(None, CompareImmediate8(byte, 0xE0));
        emit(
            None,
            JumpConditional {
                condition: CC_E,
                target: Label::E0Path,
            },
        );
        emit(None, CompareImmediate8(byte, 0xED));
        emit(
            None,
            JumpConditional {
                condition: CC_E,
                target: Label::EdPath,
            },
        );
        emit(
            None,
            Jump {
                target: Label::Cont2Any,
            },
        );
        emit(Some(Label::Lead3), CompareImmediate8(byte, 0xF5));
        emit(
            None,
            JumpConditional {
                condition: CC_AE,
                target: Label::Fail,
            },
        );
        emit(None, CompareImmediate8(byte, 0xF0));
        emit(
            None,
            JumpConditional {
                condition: CC_E,
                target: Label::F0Path,
            },
        );
        emit(None, CompareImmediate8(byte, 0xF4));
        emit(
            None,
            JumpConditional {
                condition: CC_E,
                target: Label::F4Path,
            },
        );
        emit(
            None,
            Jump {
                target: Label::Cont3Any,
            },
        );
        // One continuation: the byte admits 0x80..=0xBF.
        emit_cont_tail(
            &mut emit,
            Label::Cont1,
            1,
            0x80,
            0xBF,
            base,
            count,
            cursor,
            byte,
        );
        // Two continuations: ordinary lead bytes admit 0x80..=0xBF twice;
        // E0 forbids the overlong second bytes below 0xA0 and ED forbids
        // the surrogate halves above 0x9F.
        emit_cont_tail(
            &mut emit,
            Label::Cont2Any,
            2,
            0x80,
            0xBF,
            base,
            count,
            cursor,
            byte,
        );
        emit_cont_tail(
            &mut emit,
            Label::E0Path,
            2,
            0xA0,
            0xBF,
            base,
            count,
            cursor,
            byte,
        );
        emit_cont_tail(
            &mut emit,
            Label::EdPath,
            2,
            0x80,
            0x9F,
            base,
            count,
            cursor,
            byte,
        );
        // Three continuations: F0 needs second >= 0x90, F4 needs second
        // <= 0x8F; the rest admit 0x80..=0xBF.
        emit_cont_tail(
            &mut emit,
            Label::Cont3Any,
            3,
            0x80,
            0xBF,
            base,
            count,
            cursor,
            byte,
        );
        emit_cont_tail(
            &mut emit,
            Label::F0Path,
            3,
            0x90,
            0xBF,
            base,
            count,
            cursor,
            byte,
        );
        emit_cont_tail(
            &mut emit,
            Label::F4Path,
            3,
            0x80,
            0x8F,
            base,
            count,
            cursor,
            byte,
        );
        emit(Some(Label::Advance1), Inc(cursor));
        emit(
            None,
            Jump {
                target: Label::Loop,
            },
        );
    } else {
        // NoNul and AsciiOnly share the early-exit shape: a failure test on
        // the loaded byte zeroes the verdict, otherwise the loop runs to
        // `count`.
        emit(
            Some(Label::Loop),
            LoadByte {
                value: byte,
                base,
                index: cursor,
                offset: 0,
            },
        );
        match predicate {
            ByteSequencePredicate::NoNul => {
                emit(None, TestSelf8(byte));
                emit(
                    None,
                    JumpConditional {
                        condition: CC_E,
                        target: Label::Fail,
                    },
                );
            }
            ByteSequencePredicate::AsciiOnly => {
                emit(None, TestImmediate8(byte, 0x80));
                emit(
                    None,
                    JumpConditional {
                        condition: CC_NE,
                        target: Label::Fail,
                    },
                );
            }
            _ => unreachable!(),
        }
        emit(None, Inc(cursor));
        emit(
            None,
            CompareRegister64 {
                source: count,
                operand: cursor,
            },
        );
        emit(
            None,
            JumpConditional {
                condition: CC_B,
                target: Label::Loop,
            },
        );
        emit(
            None,
            Jump {
                target: Label::Done,
            },
        );
    }
    emit(Some(Label::Fail), XorSelf(result));
    emit(Some(Label::Done), XorSelf(cursor));
    stream
}

/// `tail` for a lead byte expecting `continuations` continuation bytes whose
/// first admits `lower..=upper` and whose rest admit 0x80..=0xBF.
fn emit_cont_tail(
    emit: &mut dyn FnMut(Option<Label>, Instruction),
    label: Label,
    continuations: u8,
    lower: u8,
    upper: u8,
    base: u8,
    count: u8,
    cursor: u8,
    byte: u8,
) {
    use Instruction::*;
    emit(
        Some(label),
        LoadAddress {
            value: byte,
            base: cursor,
            offset: continuations,
        },
    );
    emit(
        None,
        CompareRegister64 {
            source: count,
            operand: byte,
        },
    );
    emit(
        None,
        JumpConditional {
            condition: CC_AE,
            target: Label::Fail,
        },
    );
    for offset in 1..=u32::from(continuations) {
        emit(
            None,
            LoadByte {
                value: byte,
                base,
                index: cursor,
                offset,
            },
        );
        let (cont_lower, cont_upper) = if offset == 1 {
            (lower, upper)
        } else {
            (0x80, 0xBF)
        };
        emit(None, CompareImmediate8(byte, cont_lower));
        emit(
            None,
            JumpConditional {
                condition: CC_B,
                target: Label::Fail,
            },
        );
        emit(None, CompareImmediate8(byte, cont_upper));
        emit(
            None,
            JumpConditional {
                condition: CC_A,
                target: Label::Fail,
            },
        );
    }
    emit(None, AddImmediate(cursor, continuations + 1));
    emit(
        None,
        Jump {
            target: Label::Loop,
        },
    );
}

fn rex8(register: u8) -> u8 {
    0x40 | (register >> 3)
}

/// The byte-register self-test carries the register in both modrm fields,
/// so a register at or past 8 needs REX.R beside REX.B — not just the
/// extension-form prefix `rex8` emits.
fn rex8_pair(register: u8) -> u8 {
    0x40 | ((register >> 3) << 2) | (register >> 3)
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
        Instruction::AddImmediate(register, immediate) => {
            bytes.extend([rex(0, 0, register), 0x83, modrm(3, 0, register), immediate])
        }
        Instruction::TestSelf64(register) => bytes.extend([
            rex(register, 0, register),
            0x85,
            modrm(3, register, register),
        ]),
        Instruction::TestSelf8(register) => {
            bytes.extend([rex8_pair(register), 0x84, modrm(3, register, register)])
        }
        Instruction::TestImmediate8(register, immediate) => {
            bytes.extend([rex8(register), 0xf6, modrm(3, 0, register), immediate])
        }
        Instruction::CompareImmediate8(register, immediate) => {
            bytes.extend([rex8(register), 0x80, modrm(3, 7, register), immediate])
        }
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
        Instruction::SetNotEqual8(register) => {
            bytes.extend([rex8(register), 0x0f, 0x95, modrm(3, 0, register)])
        }
        Instruction::LoadAddress {
            value,
            base,
            offset,
        } => {
            // `modrm.rm == 4` asks for a SIB byte: rsp and r12 bases take the
            // SIB form with the index field's 'none' code.
            bytes.extend([rex(value, 0, base), 0x8d]);
            if base & 7 == 4 {
                bytes.extend([modrm(1, value, 4), 0x24 | (base & 7), offset]);
            } else {
                bytes.extend([modrm(1, value, base), offset]);
            }
        }
        Instruction::JumpConditional { condition, target } => {
            bytes.extend([0x0f, condition]);
            patches.push((bytes.len(), target));
            bytes.extend([0; 4]);
        }
        Instruction::Jump { target } => {
            bytes.push(0xe9);
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
    AddImmediate(u8, u8),
    TestSelf64(u8),
    TestSelf8(u8),
    TestImmediate8(u8, u8),
    CompareImmediate8(u8, u8),
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
    SetNotEqual8(u8),
    LoadAddress {
        value: u8,
        base: u8,
        offset: u8,
    },
    JumpConditional {
        condition: u8,
        target: usize,
    },
    Jump {
        target: usize,
    },
}

fn decode_one(
    bytes: &[u8],
    position: usize,
) -> Result<(Decoded, usize), X86_64SelectedFormEncodingError> {
    let instruction = &bytes[position..];
    let fail = X86_64SelectedFormEncodingError::EncodedFormMismatch;
    let rex_fields = |prefix: u8, mode: u8, want_64: bool| -> Option<(u8, u8, u8)> {
        if want_64 {
            if prefix & 0xf8 != 0x48 {
                return None;
            }
        } else if prefix & 0xf8 != 0x40 {
            return None;
        }
        Some((
            ((mode >> 3) & 7) | ((prefix & 4) << 1),
            (mode & 7) | ((prefix & 1) << 3),
            ((prefix & 2) << 2) | ((mode >> 3) & 7),
        ))
    };
    match *instruction {
        [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 => {
            let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
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
        [prefix, 0x83, mode, immediate, ..]
            if prefix & 0xfe == 0x48 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
        {
            Ok((
                Decoded::AddImmediate((mode & 7) | ((prefix & 1) << 3), immediate),
                4,
            ))
        }
        [prefix, 0x85, mode, ..] if prefix & 0xfa == 0x48 => {
            let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
            if mode >> 6 == 3 && register == rm {
                Ok((Decoded::TestSelf64(register), 3))
            } else {
                Err(fail)
            }
        }
        [prefix, 0x84, mode, ..] if prefix & 0xfa == 0x40 => {
            let (register, rm, _) = rex_fields(prefix, mode, false).unwrap();
            if mode >> 6 == 3 && register == rm {
                Ok((Decoded::TestSelf8(register), 3))
            } else {
                Err(fail)
            }
        }
        [prefix, 0xf6, mode, immediate, ..]
            if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
        {
            let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
            Ok((Decoded::TestImmediate8(rm, immediate), 4))
        }
        [prefix, 0x80, mode, immediate, ..]
            if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 7 =>
        {
            let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
            Ok((Decoded::CompareImmediate8(rm, immediate), 4))
        }
        [prefix, 0x39, mode, ..] if prefix & 0xfa == 0x48 => {
            let (register, rm, _) = rex_fields(prefix, mode, true).unwrap();
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
        [prefix, 0x0f, 0x95, mode, ..]
            if prefix & 0xfe == 0x40 && mode >> 6 == 3 && ((mode >> 3) & 7) == 0 =>
        {
            let (_, rm, _) = rex_fields(prefix, mode, false).unwrap();
            Ok((Decoded::SetNotEqual8(rm), 4))
        }
        [prefix, 0x8d, mode, offset, ..]
            if prefix & 0xfa == 0x48 && mode >> 6 == 1 && mode & 7 != 4 =>
        {
            let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let base = (mode & 7) | ((prefix & 1) << 3);
            Ok((
                Decoded::LoadAddress {
                    value,
                    base,
                    offset,
                },
                4,
            ))
        }
        [prefix, 0x8d, mode, sib, offset, ..]
            if prefix & 0xfa == 0x48 && mode >> 6 == 1 && mode & 7 == 4 && sib & 0xf8 == 0x20 =>
        {
            let value = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let base = (sib & 7) | ((prefix & 1) << 3);
            Ok((
                Decoded::LoadAddress {
                    value,
                    base,
                    offset,
                },
                5,
            ))
        }
        [
            0x0f,
            condition @ (0x82 | 0x83 | 0x84 | 0x85 | 0x87),
            d0,
            d1,
            d2,
            d3,
            ..,
        ] => {
            let relative = i32::from_le_bytes([d0, d1, d2, d3]);
            let target = position as i64 + 6 + i64::from(relative);
            let target = usize::try_from(target)
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            if target > bytes.len() {
                return Err(fail);
            }
            Ok((Decoded::JumpConditional { condition, target }, 6))
        }
        [0xe9, d0, d1, d2, d3, ..] => {
            let relative = i32::from_le_bytes([d0, d1, d2, d3]);
            let target = position as i64 + 5 + i64::from(relative);
            let target = usize::try_from(target)
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
            if target > bytes.len() {
                return Err(fail);
            }
            Ok((Decoded::Jump { target }, 5))
        }
        _ => Err(fail),
    }
}

fn decode_all_stream(bytes: &[u8]) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
    if bytes.is_empty() {
        return Err(X86_64SelectedFormEncodingError::MalformedEncoding);
    }
    let mut decoded = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let (instruction, length) = decode_one(bytes, position)?;
        decoded.push(instruction);
        position += length;
    }
    Ok(decoded)
}

/// The label targets, resolved to the byte positions `assemble` produces —
/// the validator names a branch by where it lands, not by the layout label.
fn expected_stream(
    stream: &[(Option<Label>, Instruction)],
) -> Result<Vec<Decoded>, X86_64SelectedFormEncodingError> {
    let mut offsets: Vec<(Label, usize)> = Vec::new();
    let mut position = 0;
    for (label, instruction) in stream {
        if let Some(label) = label {
            offsets.push((*label, position));
        }
        position += instruction_size(*instruction);
    }
    stream
        .iter()
        .map(|(_, instruction)| match *instruction {
            Instruction::XorSelf(r) => Ok(Decoded::XorSelf(r)),
            Instruction::Inc(r) => Ok(Decoded::Inc(r)),
            Instruction::AddImmediate(r, i) => Ok(Decoded::AddImmediate(r, i)),
            Instruction::TestSelf64(r) => Ok(Decoded::TestSelf64(r)),
            Instruction::TestSelf8(r) => Ok(Decoded::TestSelf8(r)),
            Instruction::TestImmediate8(r, i) => Ok(Decoded::TestImmediate8(r, i)),
            Instruction::CompareImmediate8(r, i) => Ok(Decoded::CompareImmediate8(r, i)),
            Instruction::CompareRegister64 { source, operand } => {
                Ok(Decoded::CompareRegister64 { source, operand })
            }
            Instruction::LoadByte {
                value,
                base,
                index,
                offset,
            } => Ok(Decoded::LoadByte {
                value,
                base,
                index,
                offset,
            }),
            Instruction::SetNotEqual8(r) => Ok(Decoded::SetNotEqual8(r)),
            Instruction::LoadAddress {
                value,
                base,
                offset,
            } => Ok(Decoded::LoadAddress {
                value,
                base,
                offset,
            }),
            Instruction::JumpConditional { condition, target } => {
                let target = offsets
                    .iter()
                    .find(|(label, _)| *label == target)
                    .map(|(_, position)| *position)
                    .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                Ok(Decoded::JumpConditional { condition, target })
            }
            Instruction::Jump { target } => {
                let target = offsets
                    .iter()
                    .find(|(label, _)| *label == target)
                    .map(|(_, position)| *position)
                    .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
                Ok(Decoded::Jump { target })
            }
        })
        .collect()
}

fn instruction_size(instruction: Instruction) -> usize {
    match instruction {
        Instruction::XorSelf(_)
        | Instruction::Inc(_)
        | Instruction::TestSelf64(_)
        | Instruction::TestSelf8(_)
        | Instruction::CompareRegister64 { .. } => 3,
        Instruction::AddImmediate(_, _)
        | Instruction::TestImmediate8(_, _)
        | Instruction::CompareImmediate8(_, _)
        | Instruction::SetNotEqual8(_)
        | Instruction::LoadAddress { .. } => 4,
        Instruction::JumpConditional { .. } => 6,
        Instruction::Jump { .. } => 5,
        Instruction::LoadByte { .. } => 9,
    }
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    predicate: ByteSequencePredicate,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [base, count, cursor, byte, result] =
        request(physical, alternative, operands, displacement)?;
    let bytes = assemble_labeled_stream(
        &layout(predicate, base, count, cursor, byte, result),
        emit_instruction,
    )?;
    validate(
        physical,
        predicate,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    predicate: ByteSequencePredicate,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [base, count, cursor, byte, result] =
        request(physical, alternative, operands, displacement)?;
    let expected = expected_stream(&layout(predicate, base, count, cursor, byte, result))?;
    if decode_all_stream(bytes)? != expected {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    super::canonical_effects_encoding(
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

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<[u8; 5], X86_64SelectedFormEncodingError> {
    super::request_validation::guard_canonical(physical)?;
    if displacement != 0
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::ScanByteSequence,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; 5] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    // `rsp` cannot encode the SIB index or the disp8 LEA base the loop
    // uses, so the cursor and byte scratch may never land there.
    if registers[2] == 4 || registers[3] == 4 {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if registers[..2].contains(&registers[2])
        || registers[..3].contains(&registers[3])
        || registers[..4].contains(&registers[4])
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

#[cfg(test)]
mod tests {
    use super::super::{
        SelectedInstructionKind, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };
    use super::{MachineAlternativeFamily, MachineAlternativeKey, encode, validate};
    use selected_instructions::ByteSequencePredicate;

    fn kind(predicate: ByteSequencePredicate) -> SelectedInstructionKind {
        SelectedInstructionKind::ScanByteSequence { predicate }
    }

    #[test]
    fn scan_byte_sequence_catalog_requires_early_clobbers_and_memory_effect() {
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
            crate::X86_64_SCAN_BYTE_SEQUENCE,
            &[2, 3],
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
                    row.semantic == selected_instructions::MachineSemanticKind::ScanByteSequence
                })
                .unwrap();
            assert_eq!(
                row.memory,
                selected_instructions::MachineMemoryEffect::ScanByteSequenceV1
            );
            assert_eq!(row.alternatives[0].encoded.external_operand_reads, [0, 1]);
            assert_eq!(
                row.alternatives[0].encoded.external_operand_writes,
                [2, 3, 4]
            );
        }
    }

    #[test]
    fn scan_byte_sequence_round_trips_and_rejects_every_changed_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ScanByteSequence,
            variant: 0,
        };
        for predicate in ByteSequencePredicate::ALL {
            for names in [
                ["r13", "r14", "r15", "r12", "rbp"],
                ["rax", "rcx", "rdx", "rsi", "rdi"],
            ] {
                let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode(&physical, predicate, alternative, &operands, 0).unwrap();
                assert_eq!(
                    encode_x86_64_selected_form(&physical, kind(predicate), alternative, &operands)
                        .unwrap(),
                    encoded
                );
                validate_x86_64_selected_form_encoding(
                    &physical,
                    kind(predicate),
                    alternative,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate(&physical, predicate, alternative, &operands, 0, &changed)
                            .is_err(),
                        "predicate {predicate:?} bit {bit}"
                    );
                }
                super::super::assert_byte_fragility(encoded.bytes(), |bytes| {
                    validate(&physical, predicate, alternative, &operands, 0, bytes).is_err()
                });
                assert!(encode(&physical, predicate, alternative, &operands, 1).is_err());
            }
        }
    }
}
