/*
We replay and validate generated x86-64 entry and frame-transport instruction
bytes (`decode_x86_64_generated_entry`), reconstructing exact general-purpose
and vector register reads and writes, stack touches, flag writes, and terminal
control transfers directly from the emitted byte stream.
*/

use crate::selected_form_encoding::decoding::{DecodedInstruction, decode_one};
use calling_conventions::{
    GeneratedEntryControl as Control, GeneratedEntryEvidence, GeneratedEntryInstruction,
    MachineRegister as Register, MachineState as State, MachineStateSet, RegisterSet,
    StateEffectsEvidence,
};

#[derive(Default)]
struct Effects {
    reads: Vec<Register>,
    writes: Vec<Register>,
    used: Vec<State>,
    clobbered: Vec<State>,
}
impl Effects {
    fn read(&mut self, code: u8, vector: bool) {
        self.reads.push(register(code, vector));
        if !vector && code == 4 {
            self.used.push(State::StackPointer);
        }
    }
    fn write(&mut self, code: u8, vector: bool) {
        self.writes.push(register(code, vector));
        if !vector && code == 4 {
            self.clobbered.push(State::StackPointer);
        }
    }
    fn state(&mut self, state: State, write: bool) {
        self.used.push(state);
        if write {
            self.clobbered.push(state);
        }
    }
    fn flags(&mut self) {
        self.state(State::Flags, true);
    }
    fn finish(self) -> StateEffectsEvidence {
        StateEffectsEvidence::new(
            RegisterSet::new(self.reads),
            RegisterSet::new(self.writes),
            MachineStateSet::new(self.used),
            MachineStateSet::new(self.clobbered),
        )
    }
}
fn register(code: u8, vector: bool) -> Register {
    if vector {
        return Register::X86Xmm(code);
    }
    [
        Register::X86Rax,
        Register::X86Rcx,
        Register::X86Rdx,
        Register::X86Rbx,
        Register::X86Rsp,
        Register::X86Rbp,
        Register::X86Rsi,
        Register::X86Rdi,
        Register::X86R8,
        Register::X86R9,
        Register::X86R10,
        Register::X86R11,
        Register::X86R12,
        Register::X86R13,
        Register::X86R14,
        Register::X86R15,
    ][usize::from(code)]
}

fn common(bytes: &[u8], offset: usize) -> Option<(usize, Effects, Control)> {
    let (instruction, length) = decode_one(bytes).ok()?;
    let mut effects = Effects::default();
    let mut control = Control::FallThrough;
    match instruction {
        DecodedInstruction::Move {
            source,
            destination,
        }
        | DecodedInstruction::ZeroExtendU8 {
            source,
            destination,
        }
        | DecodedInstruction::ZeroExtendU16 {
            source,
            destination,
        }
        | DecodedInstruction::ZeroExtendU32 {
            source,
            destination,
        }
        | DecodedInstruction::SignExtendI8 {
            source,
            destination,
        }
        | DecodedInstruction::SignExtendI16 {
            source,
            destination,
        }
        | DecodedInstruction::SignExtendI32 {
            source,
            destination,
        } => {
            effects.read(source, false);
            effects.write(destination, false);
        }
        DecodedInstruction::Materialize { destination, .. } => effects.write(destination, false),
        DecodedInstruction::Compare { left, right } => {
            effects.read(left, false);
            effects.read(right, false);
            effects.flags();
        }
        DecodedInstruction::CompareImmediate { register, .. }
        | DecodedInstruction::CompareSignedImmediate8 { register, .. }
        | DecodedInstruction::Test { register } => {
            effects.read(register, false);
            effects.flags();
        }
        DecodedInstruction::CompareRaxImmediate32 { .. } => {
            effects.read(0, false);
            effects.flags();
        }
        DecodedInstruction::Or {
            source,
            destination,
        }
        | DecodedInstruction::Xor {
            source,
            destination,
        }
        | DecodedInstruction::BitwiseAnd {
            source,
            destination,
        }
        | DecodedInstruction::Add {
            source,
            destination,
        }
        | DecodedInstruction::Subtract {
            source,
            destination,
        } => {
            effects.read(source, false);
            effects.read(destination, false);
            effects.write(destination, false);
            effects.flags();
        }
        DecodedInstruction::ShiftRightLogicalImmediate { destination, .. } => {
            effects.read(destination, false);
            effects.write(destination, false);
            effects.flags();
        }
        DecodedInstruction::Lea {
            destination,
            base,
            index,
            ..
        } => {
            effects.read(base, false);
            if let Some(index) = index {
                effects.read(index, false);
            }
            effects.write(destination, false);
        }
        DecodedInstruction::AtomicLoad {
            destination, base, ..
        } => {
            effects.read(base, false);
            effects.write(destination, false);
        }
        DecodedInstruction::AtomicStore { source, base, .. } => {
            effects.read(source, false);
            effects.read(base, false);
        }
        DecodedInstruction::JumpEqualShort { displacement }
        | DecodedInstruction::JumpNotEqualShort { displacement }
        | DecodedInstruction::JumpNoOverflowShort { displacement }
        | DecodedInstruction::JumpBelowShort { displacement }
        | DecodedInstruction::JumpAboveOrEqualShort { displacement } => {
            effects.state(State::Flags, false);
            control = Control::Branch {
                target_byte_offset: offset as i64 + length as i64 + i64::from(displacement),
                conditional: true,
            };
        }
        DecodedInstruction::JumpShort { displacement } => {
            control = Control::Branch {
                target_byte_offset: offset as i64 + length as i64 + i64::from(displacement),
                conditional: false,
            }
        }
        DecodedInstruction::PrivilegedStop => {
            effects.flags();
            control = Control::Stop;
        }
        DecodedInstruction::Crash => control = Control::Stop,
        DecodedInstruction::Return => {
            effects.read(4, false);
            effects.write(4, false);
            control = Control::Return;
        }
        _ => return None,
    }
    if control != Control::FallThrough {
        effects.state(State::InstructionPointer, true);
    }
    Some((length, effects, control))
}

struct Operand {
    register: Option<u8>,
    address: Vec<u8>,
    relative: bool,
    end: usize,
}
fn operand(bytes: &[u8], position: usize, rex: u8) -> Result<Operand, String> {
    let modrm = *bytes.get(position).ok_or("truncated generated x86 ModRM")?;
    let mode = modrm >> 6;
    let rm = modrm & 7;
    if mode == 3 {
        return Ok(Operand {
            register: Some(rm | ((rex & 1) << 3)),
            address: Vec::new(),
            relative: false,
            end: position + 1,
        });
    }
    let mut end = position + 1;
    let mut address = Vec::new();
    let mut relative = false;
    let mut displacement = match mode {
        1 => 1,
        2 => 4,
        _ => 0,
    };
    if rm == 4 {
        let sib = *bytes.get(end).ok_or("truncated generated x86 SIB")?;
        end += 1;
        let index = (sib >> 3) & 7;
        if index != 4 || rex & 2 != 0 {
            address.push(index | (((rex >> 1) & 1) << 3));
        }
        let base = sib & 7;
        if mode == 0 && base == 5 {
            displacement = 4;
        } else {
            address.push(base | ((rex & 1) << 3));
        }
    } else if mode == 0 && rm == 5 {
        relative = true;
        displacement = 4;
    } else {
        address.push(rm | ((rex & 1) << 3));
    }
    end += displacement;
    if end > bytes.len() {
        return Err("truncated generated x86 displacement".into());
    }
    Ok(Operand {
        register: None,
        address,
        relative,
        end,
    })
}
fn read(effects: &mut Effects, operand: &Operand) {
    if let Some(register) = operand.register {
        effects.read(register, false);
    }
    address(effects, operand);
}
fn write(effects: &mut Effects, operand: &Operand) {
    if let Some(register) = operand.register {
        effects.write(register, false);
    }
    address(effects, operand);
}
fn address(effects: &mut Effects, operand: &Operand) {
    for &register in &operand.address {
        effects.read(register, false);
    }
    if operand.relative {
        effects.state(State::InstructionPointer, false);
    }
}

fn instruction(bytes: &[u8], offset: usize) -> Result<(usize, Effects, Control), String> {
    let mut effects = Effects::default();
    let mut control = Control::FallThrough;
    let mut cursor = 0;
    let first = *bytes.first().ok_or("empty generated x86 instruction")?;
    let legacy = if matches!(first, 0x66 | 0xf2 | 0xf3) {
        cursor += 1;
        Some(first)
    } else {
        None
    };
    let rex = if bytes
        .get(cursor)
        .is_some_and(|byte| (0x40..=0x4f).contains(byte))
    {
        let rex = bytes[cursor];
        cursor += 1;
        rex
    } else {
        0
    };
    let opcode = *bytes.get(cursor).ok_or("truncated generated x86 opcode")?;
    cursor += 1;
    if rex == 0 {
        let modrm = bytes.get(cursor).copied().unwrap_or(0);
        let high_source = opcode == 0x88 && (modrm >> 3) & 7 >= 4;
        let high_destination = matches!(opcode, 0x88 | 0x80) && modrm >= 0xc4 && modrm & 7 >= 4;
        let high_extended_source = opcode == 0x0f
            && bytes.get(cursor) == Some(&0xb6)
            && bytes
                .get(cursor + 1)
                .is_some_and(|modrm| *modrm >= 0xc0 && modrm & 7 >= 4);
        if high_source || high_destination || high_extended_source {
            return Err("unsupported generated x86 high-byte register".into());
        }
    }
    if let Some(row) = common(bytes, offset) {
        return Ok(row);
    }
    if matches!(legacy, Some(0xf2 | 0xf3))
        && !(opcode == 0x0f
            && bytes
                .get(cursor)
                .is_some_and(|byte| matches!(byte, 0x10 | 0x11)))
    {
        return Err("unsupported generated x86 mandatory prefix".into());
    }
    let immediate_bytes = if legacy == Some(0x66) && rex & 8 == 0 {
        2
    } else {
        4
    };
    match opcode {
        0x90 if legacy.is_none() && rex == 0 => {}
        0xfc if legacy.is_none() && rex == 0 => effects.flags(),
        0xcf if legacy.is_none() && rex == 0x48 => {
            for state in [
                State::Flags,
                State::InstructionPointer,
                State::StackPointer,
                State::SegmentState,
            ] {
                effects.state(state, true);
            }
            control = Control::InterruptReturn;
        }
        0x50..=0x5f if legacy.is_none() && (rex == 0 || rex == 0x41) => {
            let register = (opcode & 7) | ((rex & 1) << 3);
            effects.read(4, false);
            effects.write(4, false);
            if opcode < 0x58 {
                effects.read(register, false);
            } else {
                effects.write(register, false);
            }
        }
        0xe8 if legacy.is_none() && rex == 0 => {
            let field = offset
                .checked_add(cursor)
                .ok_or("generated x86 call coordinate overflows")?;
            cursor += 4;
            effects.read(4, false);
            effects.write(4, false);
            effects.state(State::InstructionPointer, true);
            control = Control::Call {
                field_byte_offset: u32::try_from(field)
                    .map_err(|_| "generated x86 call exceeds u32")?,
            };
        }
        0x70..=0x7f if legacy.is_none() && rex == 0 => {
            let displacement = *bytes.get(cursor).ok_or("truncated generated x86 branch")? as i8;
            cursor += 1;
            effects.state(State::Flags, false);
            effects.state(State::InstructionPointer, true);
            control = Control::Branch {
                target_byte_offset: offset as i64 + cursor as i64 + i64::from(displacement),
                conditional: true,
            };
        }
        0xb8..=0xbf if legacy.is_none() => {
            effects.write((opcode & 7) | ((rex & 1) << 3), false);
            cursor += if rex & 8 != 0 { 8 } else { 4 };
        }
        0x25 | 0x3d | 0xa9 if legacy.is_none() => {
            effects.read(0, false);
            if opcode == 0x25 {
                effects.write(0, false);
            }
            effects.flags();
            cursor += 4;
        }
        0x0f => {
            let second = *bytes
                .get(cursor)
                .ok_or("truncated generated x86 extended opcode")?;
            cursor += 1;
            let modrm = *bytes
                .get(cursor)
                .ok_or("truncated generated x86 extended ModRM")?;
            let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
            let decoded = operand(bytes, cursor, rex)?;
            cursor = decoded.end;
            match second {
                0x01 if decoded.register.is_none() && ((modrm >> 3) & 7) <= 3 => {
                    address(&mut effects, &decoded);
                    effects.state(State::ControlState, ((modrm >> 3) & 7) >= 2);
                }
                0x00 if decoded.register.is_some() && matches!((modrm >> 3) & 7, 1 | 3) => {
                    if (modrm >> 3) & 7 == 1 {
                        write(&mut effects, &decoded);
                        effects.state(State::ControlState, false);
                    } else {
                        read(&mut effects, &decoded);
                        effects.state(State::ControlState, true);
                    }
                }
                0x10 | 0x11
                    if decoded.register.is_none() && matches!(legacy, None | Some(0xf2 | 0xf3)) =>
                {
                    address(&mut effects, &decoded);
                    if second == 0x10 {
                        effects.write(register, true);
                    } else {
                        effects.read(register, true);
                    }
                }
                0xb6 | 0xb7 => {
                    if second == 0xb6 && rex == 0 && decoded.register.is_some_and(|code| code >= 4)
                    {
                        return Err("unsupported generated x86 high-byte register".into());
                    }
                    read(&mut effects, &decoded);
                    effects.write(register, false);
                }
                _ => return Err("unsupported generated x86 extended form".into()),
            }
        }
        0x01 | 0x09 | 0x39 | 0x85 | 0x89 | 0x88 | 0x8b | 0x8d | 0x80 | 0x81 | 0x83 | 0xc1
        | 0xc7 | 0xf7 => {
            let modrm = *bytes.get(cursor).ok_or("truncated generated x86 ModRM")?;
            let register = ((modrm >> 3) & 7) | (((rex >> 2) & 1) << 3);
            let group = (modrm >> 3) & 7;
            let decoded = operand(bytes, cursor, rex)?;
            cursor = decoded.end;
            if rex == 0
                && ((opcode == 0x88 && register >= 4)
                    || (matches!(opcode, 0x88 | 0x80)
                        && decoded.register.is_some_and(|code| code >= 4)))
            {
                return Err("unsupported generated x86 high-byte register".into());
            }
            match opcode {
                0x89 | 0x88 => {
                    effects.read(register, false);
                    write(&mut effects, &decoded);
                }
                0x8b => {
                    read(&mut effects, &decoded);
                    effects.write(register, false);
                }
                0x8d if decoded.register.is_none() => {
                    address(&mut effects, &decoded);
                    effects.write(register, false);
                }
                0x01 | 0x09 | 0x39 | 0x85 => {
                    effects.read(register, false);
                    read(&mut effects, &decoded);
                    effects.flags();
                    if matches!(opcode, 0x01 | 0x09) {
                        write(&mut effects, &decoded);
                    }
                }
                0x80 | 0x81 | 0x83 if matches!(group, 0 | 4 | 5 | 7) => {
                    read(&mut effects, &decoded);
                    effects.flags();
                    if group != 7 {
                        write(&mut effects, &decoded);
                    }
                    cursor += if opcode == 0x81 { immediate_bytes } else { 1 };
                }
                0xc1 if matches!(group, 4 | 5 | 7) => {
                    read(&mut effects, &decoded);
                    write(&mut effects, &decoded);
                    effects.flags();
                    cursor += 1;
                }
                0xc7 if group == 0 => {
                    write(&mut effects, &decoded);
                    cursor += immediate_bytes;
                }
                0xf7 if group == 0 => {
                    read(&mut effects, &decoded);
                    effects.flags();
                    cursor += immediate_bytes;
                }
                _ => return Err("unsupported generated x86 operand form".into()),
            }
        }
        _ => return Err(format!("unsupported generated x86 opcode {opcode:02x}")),
    }
    if cursor > bytes.len() {
        return Err("truncated generated x86 immediate".into());
    }
    Ok((cursor, effects, control))
}

pub fn decode_x86_64_generated_entry(
    bytes: &[u8],
    body_call_field: Option<u32>,
) -> Result<GeneratedEntryEvidence, String> {
    let mut rows = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (length, effects, mut control) = instruction(&bytes[offset..], offset)
            .map_err(|reason| format!("generated x86 instruction at byte {offset}: {reason}"))?;
        if let Control::Call { field_byte_offset } = control
            && Some(field_byte_offset) != body_call_field
        {
            let field = field_byte_offset as usize;
            let displacement = i32::from_le_bytes(
                bytes
                    .get(field..field + 4)
                    .ok_or("truncated generated local call")?
                    .try_into()
                    .map_err(|_| "truncated generated local call")?,
            );
            control = Control::LocalCall {
                target_byte_offset: i64::from(field_byte_offset) + 4 + i64::from(displacement),
            };
        }
        rows.push(GeneratedEntryInstruction {
            byte_offset: u32::try_from(offset).map_err(|_| "generated x86 entry exceeds u32")?,
            byte_count: u32::try_from(length).map_err(|_| "generated x86 form exceeds u32")?,
            effects: effects.finish(),
            control,
        });
        offset += length;
    }
    GeneratedEntryEvidence::from_target_instructions(bytes.len(), rows, body_call_field)
        .map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_register_observation_is_use_without_a_preservation_write() {
        let evidence =
            decode_x86_64_generated_entry(&[0x0f, 0x01, 0x04, 0x24, 0x0f, 0x00, 0xc8, 0xc3], None)
                .unwrap();
        assert!(
            evidence
                .effects
                .machine_state()
                .contains_all(MachineStateSet::new([State::ControlState]))
        );
        assert!(
            !evidence
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::ControlState]))
        );
        assert!(
            evidence
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::StackPointer]))
        );
        assert!(
            evidence
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::InstructionPointer]))
        );
    }

    #[test]
    fn unsupported_high_bytes_do_not_become_stack_registers() {
        for bytes in [
            &[0x88, 0xe0][..],
            &[0x88, 0xc4],
            &[0x0f, 0xb6, 0xc4],
            &[0x80, 0xc4, 0x01],
            &[0x88, 0xa4, 0x24, 0, 0, 0, 0],
        ] {
            assert!(decode_x86_64_generated_entry(bytes, None).is_err());
        }
        let rex = decode_x86_64_generated_entry(&[0x40, 0x88, 0xe0], None).unwrap();
        assert!(
            rex.effects
                .registers_read()
                .as_slice()
                .contains(&Register::X86Rsp)
        );
    }

    #[test]
    fn immediate_width_keeps_following_return_in_its_own_row() {
        let evidence =
            decode_x86_64_generated_entry(&[0x66, 0xf7, 0xc0, 0, 0, 0x90, 0xc3], None).unwrap();
        assert_eq!(
            evidence
                .instructions
                .iter()
                .map(|row| row.byte_count)
                .collect::<Vec<_>>(),
            [5, 1, 1]
        );
        assert!(
            evidence
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::InstructionPointer]))
        );
        let rex = decode_x86_64_generated_entry(&[0x66, 0x48, 0xc7, 0xc0, 0, 0, 0x90, 0x90], None)
            .unwrap();
        assert_eq!(rex.instructions.len(), 1);
        assert_eq!(rex.instructions[0].byte_count, 8);
        assert!(decode_x86_64_generated_entry(&[0x66, 0x48, 0xc7, 0xc0, 0, 0], None).is_err());
    }

    #[test]
    fn local_prefix_call_and_body_relocation_have_distinct_exact_custody() {
        let bytes = [0xe8, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0, 0xc3];
        let evidence = decode_x86_64_generated_entry(&bytes, Some(6)).unwrap();
        assert_eq!(
            evidence.instructions[0].control,
            Control::LocalCall {
                target_byte_offset: 5
            }
        );
        assert_eq!(
            evidence.instructions[1].control,
            Control::Call {
                field_byte_offset: 6
            }
        );
        for displacement in [1, 127] {
            let mut changed = bytes;
            changed[1] = displacement;
            assert!(decode_x86_64_generated_entry(&changed, Some(6)).is_err());
        }
        assert!(decode_x86_64_generated_entry(&bytes, None).is_err());
        assert!(decode_x86_64_generated_entry(&bytes, Some(7)).is_err());
    }
}
