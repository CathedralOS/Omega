//! Load TR only after the current GDT and selected descriptor match the borrowed
//! storage. Refusal leaves TR and the descriptor unchanged; architectural faults
//! remain faults. The caller retains the table, TSS and stacks after success.

use super::{Error, Operation};
use crate::selected_form_encoding::instruction_bytes::assemble_labeled_stream;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Label {
    Refused,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    CodeSelector,
    Flags,
    StoreGdtr,
    Selector(u8),
    Load {
        bits: u8,
        base: u8,
        offset: u8,
        result: u8,
    },
    Move(u8, u8),
    Immediate(u8, u32),
    Binary {
        opcode: u8,
        source: u8,
        target: u8,
    },
    ImmediateOperation {
        extension: u8,
        register: u8,
        value: u32,
    },
    Shift {
        extension: u8,
        register: u8,
        count: u8,
    },
    LoadTask,
    ReadTask(u8),
    Branch(u8, Label),
    End,
}

fn activation() -> Vec<(Option<Label>, Step)> {
    use Step::*;
    let mut steps = Vec::new();
    let mut push = |step| steps.push((None, step));
    let fail = Branch(0x85, Label::Refused);
    let imm = |extension, register, value| ImmediateOperation {
        extension,
        register,
        value,
    };
    let binary = |opcode, source, target| Binary {
        opcode,
        source,
        target,
    };
    let load = |bits, base, offset, result| Load {
        bits,
        base,
        offset,
        result,
    };
    // No privileged observation or borrowed-memory access precedes the CPL/IF checks.
    push(CodeSelector);
    push(imm(4, 0, 3));
    push(imm(7, 0, 0));
    push(fail);
    push(Flags);
    push(imm(4, 0, 0x200));
    push(imm(7, 0, 0));
    push(fail);
    push(StoreGdtr);
    push(load(16, 1, 0, 0));
    push(imm(0, 0, 1));
    push(binary(0x39, 8, 0));
    push(fail);
    push(load(64, 1, 2, 0));
    push(binary(0x39, 7, 0));
    push(fail);
    push(Selector(9));
    push(Move(9, 0));
    push(imm(4, 0, 4));
    push(imm(7, 0, 0));
    push(fail);
    push(imm(4, 9, 0xffff_fff8));
    push(imm(7, 9, 0));
    push(Branch(0x84, Label::Refused));
    push(Move(9, 0));
    push(imm(0, 0, 16));
    push(binary(0x39, 8, 0));
    push(Branch(0x87, Label::Refused));
    push(binary(0x01, 7, 9));
    // Available 64-bit TSS, present, byte granularity, exactly 104 bytes.
    // DPL and AVL do not affect this CPL0 load; reserved bits must be zero.
    push(load(8, 9, 5, 0));
    push(imm(4, 0, 0x9f));
    push(imm(7, 0, 0x89));
    push(fail);
    push(load(8, 9, 6, 0));
    push(imm(4, 0, 0xef));
    push(imm(7, 0, 0));
    push(fail);
    push(load(16, 9, 0, 0));
    push(imm(7, 0, 103));
    push(fail);
    push(load(32, 9, 12, 0));
    push(imm(7, 0, 0));
    push(fail);
    // Reassemble all 64 base bits before comparing the actual borrowed TSS.
    push(load(32, 9, 2, 10));
    push(imm(4, 10, 0x00ff_ffff));
    push(load(8, 9, 7, 11));
    push(Shift {
        extension: 4,
        register: 11,
        count: 24,
    });
    push(binary(0x09, 11, 10));
    push(load(32, 9, 8, 11));
    push(Shift {
        extension: 4,
        register: 11,
        count: 32,
    });
    push(binary(0x09, 11, 10));
    push(binary(0x39, 6, 10));
    push(fail);
    push(LoadTask);
    push(Immediate(0, 1));
    push(Branch(0xe9, Label::Done));
    steps.push((Some(Label::Refused), Immediate(0, 0)));
    steps.push((Some(Label::Done), End));
    steps
}

fn emit(bytes: &mut Vec<u8>, step: Step, patches: &mut Vec<(usize, Label)>) {
    let rex = |wide: bool, reg: u8, base: u8| {
        0x40 | (u8::from(wide) << 3) | ((reg >> 3) << 2) | (base >> 3)
    };
    let rr = |reg: u8, base: u8| 0xc0 | ((reg & 7) << 3) | (base & 7);
    match step {
        Step::CodeSelector => bytes.extend([0x8c, 0xc8]), // MOV EAX, CS zero-extends.
        Step::Flags => bytes.extend([0x9c, 0x58]),
        Step::StoreGdtr => bytes.extend([0x0f, 0x01, 0x01]),
        Step::Selector(result) => bytes.extend([rex(false, result, 2), 0x0f, 0xb7, rr(result, 2)]),
        Step::Load {
            bits,
            base,
            offset,
            result,
        } => {
            bytes.push(rex(bits == 64, result, base));
            match bits {
                8 => bytes.extend([0x0f, 0xb6]),
                16 => bytes.extend([0x0f, 0xb7]),
                _ => bytes.push(0x8b),
            }
            bytes.extend([0x40 | ((result & 7) << 3) | (base & 7), offset]);
        }
        Step::Move(source, target) => {
            bytes.extend([rex(true, source, target), 0x89, rr(source, target)])
        }
        Step::Immediate(register, value) => {
            bytes.extend([rex(false, 0, register), 0xb8 | (register & 7)]);
            bytes.extend(value.to_le_bytes());
        }
        Step::Binary {
            opcode,
            source,
            target,
        } => bytes.extend([rex(true, source, target), opcode, rr(source, target)]),
        Step::ImmediateOperation {
            extension,
            register,
            value,
        } => {
            bytes.extend([rex(true, 0, register), 0x81, rr(extension, register)]);
            bytes.extend(value.to_le_bytes());
        }
        Step::Shift {
            extension,
            register,
            count,
        } => bytes.extend([rex(true, 0, register), 0xc1, rr(extension, register), count]),
        Step::LoadTask => bytes.extend([0x0f, 0x00, 0xda]),
        Step::ReadTask(result) => {
            bytes.extend([rex(false, 0, result), 0x0f, 0x00, rr(1, result)]);
            bytes.extend([rex(false, result, result), 0x0f, 0xb7, rr(result, result)]);
        }
        Step::Branch(condition, label) => {
            if condition == 0xe9 {
                bytes.push(0xe9);
            } else {
                bytes.extend([0x0f, condition]);
            }
            patches.push((bytes.len(), label));
            bytes.extend([0; 4]);
        }
        Step::End => {}
    }
}

pub(super) fn encode(operation: Operation, registers: &[u8]) -> Result<Vec<u8>, Error> {
    match (operation, registers) {
        (Operation::TaskRegisterActivate, [7, 6, 2, 1, 8, 0]) => {
            assemble_labeled_stream(&activation(), emit)
        }
        (Operation::TaskRegisterRead, [result]) if *result < 16 && *result != 4 => {
            let mut bytes = Vec::new();
            emit(&mut bytes, Step::ReadTask(*result), &mut Vec::new());
            Ok(bytes)
        }
        _ => Err(Error::EncodedFormMismatch),
    }
}

// Decode operand roles and immediates without calling the encoder. Relative
// branches must land on the actual refusal/done instruction boundaries.
fn decode(bytes: &[u8], cursor: &mut usize) -> Option<(Step, Option<usize>)> {
    let start = *cursor;
    let tail = bytes.get(start..)?;
    let (step, length, target) = if tail.starts_with(&[0x8c, 0xc8]) {
        (Step::CodeSelector, 2, None)
    } else if tail.starts_with(&[0x9c, 0x58]) {
        (Step::Flags, 2, None)
    } else if tail.starts_with(&[0x0f, 0x01, 0x01]) {
        (Step::StoreGdtr, 3, None)
    } else if tail.starts_with(&[0x0f, 0x00, 0xda]) {
        (Step::LoadTask, 3, None)
    } else if tail.first() == Some(&0xe9)
        || (tail.first() == Some(&0x0f) && matches!(tail.get(1), Some(0x84 | 0x85 | 0x87)))
    {
        let offset = if tail[0] == 0xe9 { 1 } else { 2 };
        let displacement = i32::from_le_bytes(tail.get(offset..offset + 4)?.try_into().ok()?);
        let target = usize::try_from((start + offset + 4) as i64 + i64::from(displacement)).ok()?;
        (
            Step::Branch(if offset == 1 { 0xe9 } else { tail[1] }, Label::Refused),
            offset + 4,
            Some(target),
        )
    } else {
        let prefix = *tail.first()?;
        if !(0x40..=0x4f).contains(&prefix) || prefix & 2 != 0 {
            return None;
        }
        let wide = prefix & 8 != 0;
        let opcode = *tail.get(1)?;
        let modrm = *tail.get(2)?;
        let reg = ((modrm >> 3) & 7) | ((prefix & 4) << 1);
        let base = (modrm & 7) | ((prefix & 1) << 3);
        match opcode {
            0xb8..=0xbf if !wide && prefix & 4 == 0 => (
                Step::Immediate(
                    (opcode & 7) | ((prefix & 1) << 3),
                    u32::from_le_bytes(tail.get(2..6)?.try_into().ok()?),
                ),
                6,
                None,
            ),
            0x89 if wide && modrm >> 6 == 3 => (Step::Move(reg, base), 3, None),
            0x01 | 0x09 | 0x39 if wide && modrm >> 6 == 3 => (
                Step::Binary {
                    opcode,
                    source: reg,
                    target: base,
                },
                3,
                None,
            ),
            0x81 if wide && prefix & 4 == 0 && modrm >> 6 == 3 => (
                Step::ImmediateOperation {
                    extension: reg,
                    register: base,
                    value: u32::from_le_bytes(tail.get(3..7)?.try_into().ok()?),
                },
                7,
                None,
            ),
            0xc1 if wide && prefix & 4 == 0 && modrm >> 6 == 3 => (
                Step::Shift {
                    extension: reg,
                    register: base,
                    count: *tail.get(3)?,
                },
                4,
                None,
            ),
            0x8b if modrm >> 6 == 1 && base & 7 != 4 => (
                Step::Load {
                    bits: if wide { 64 } else { 32 },
                    base,
                    offset: *tail.get(3)?,
                    result: reg,
                },
                4,
                None,
            ),
            0x0f if !wide => {
                let second = modrm;
                let address = *tail.get(3)?;
                let reg = ((address >> 3) & 7) | ((prefix & 4) << 1);
                let base = (address & 7) | ((prefix & 1) << 3);
                match (second, address >> 6) {
                    (0xb7, 3) if base == 2 => (Step::Selector(reg), 4, None),
                    (0xb6 | 0xb7, 1) if base & 7 != 4 => (
                        Step::Load {
                            bits: if second == 0xb6 { 8 } else { 16 },
                            base,
                            offset: *tail.get(4)?,
                            result: reg,
                        },
                        5,
                        None,
                    ),
                    _ => return None,
                }
            }
            _ => return None,
        }
    };
    *cursor += length;
    Some((step, target))
}

pub(super) fn validate(operation: Operation, registers: &[u8], bytes: &[u8]) -> Option<()> {
    if operation == Operation::TaskRegisterRead {
        let [result] = registers else {
            return None;
        };
        if *result >= 16 || *result == 4 || bytes.len() != 8 {
            return None;
        }
        let [rex, 0x0f, 0x00, modrm, mov_rex, 0x0f, 0xb7, mov_modrm] = bytes else {
            return None;
        };
        let observed = (modrm & 7) | ((rex & 1) << 3);
        let source = (mov_modrm & 7) | ((mov_rex & 1) << 3);
        let target = ((mov_modrm >> 3) & 7) | ((mov_rex & 4) << 1);
        return (*rex == 0x40 | (*result >> 3)
            && modrm & 0xf8 == 0xc8
            && *mov_rex == 0x40 | ((*result >> 3) << 2) | (*result >> 3)
            && mov_modrm >> 6 == 3
            && observed == *result
            && source == *result
            && target == *result)
            .then_some(());
    }
    if operation != Operation::TaskRegisterActivate || registers != [7, 6, 2, 1, 8, 0] {
        return None;
    }
    let mut cursor = 0;
    let mut labels = Vec::new();
    let mut branches = Vec::new();
    for (label, expected) in activation() {
        if let Some(label) = label {
            labels.push((label, cursor));
        }
        if expected == Step::End {
            continue;
        }
        let (actual, target) = decode(bytes, &mut cursor)?;
        match (expected, actual, target) {
            (Step::Branch(condition, label), Step::Branch(actual, _), Some(target))
                if condition == actual =>
            {
                branches.push((label, target))
            }
            (expected, actual, None) if expected == actual => {}
            _ => return None,
        }
    }
    (cursor == bytes.len()
        && branches
            .iter()
            .all(|(label, target)| labels.contains(&(*label, *target))))
    .then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_checks_every_operand_and_refusal_branch_before_ltr() {
        let registers = [7, 6, 2, 1, 8, 0];
        let bytes = encode(Operation::TaskRegisterActivate, &registers).unwrap();
        assert!(validate(Operation::TaskRegisterActivate, &registers, &bytes).is_some());
        assert!(bytes.len() <= 512);
        for index in 0..bytes.len() {
            assert!(
                validate(Operation::TaskRegisterActivate, &registers, &bytes[..index]).is_none()
            );
            for bit in 0..8 {
                let mut changed = bytes.clone();
                changed[index] ^= 1 << bit;
                assert!(
                    validate(Operation::TaskRegisterActivate, &registers, &changed).is_none(),
                    "changed byte {index} bit {bit}"
                );
            }
        }
        for index in 0..registers.len() {
            let mut changed = registers;
            changed[index] ^= 1;
            assert!(encode(Operation::TaskRegisterActivate, &changed).is_err());
            assert!(validate(Operation::TaskRegisterActivate, &changed, &bytes).is_none());
        }
    }

    #[test]
    fn selector_read_defines_all_result_bits_without_changing_flags() {
        for result in 0..16 {
            if result == 4 {
                continue;
            }
            let bytes = encode(Operation::TaskRegisterRead, &[result]).unwrap();
            assert!(validate(Operation::TaskRegisterRead, &[result], &bytes).is_some());
            for index in 0..bytes.len() {
                for bit in 0..8 {
                    let mut changed = bytes.clone();
                    changed[index] ^= 1 << bit;
                    assert!(validate(Operation::TaskRegisterRead, &[result], &changed).is_none());
                }
            }
        }
    }
}
