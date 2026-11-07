//! Closed target replay of the forms emitted by native entry/frame transport.
use crate::selected_form_encoding::decoding::{DecodedWord, decode_words};
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
    fn read(&mut self, register: u8, stack: bool, vector: bool) {
        if vector {
            self.reads.push(Register::Aarch64V(register));
        } else if register != 31 {
            self.reads.push(Register::Aarch64X(register));
        } else if stack {
            self.used.push(State::StackPointer);
        }
    }
    fn write(&mut self, register: u8, stack: bool, vector: bool) {
        if vector {
            self.writes.push(Register::Aarch64V(register));
        } else if register != 31 {
            self.writes.push(Register::Aarch64X(register));
        } else if stack {
            self.clobbered.push(State::StackPointer);
        }
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

fn instruction(word: u32, offset: u32) -> Result<(Effects, Control), String> {
    let mut effects = Effects::default();
    let mut control = Control::FallThrough;
    let source = ((word >> 5) & 31) as u8;
    let destination = (word & 31) as u8;
    if decode_words(&word.to_le_bytes()).is_ok_and(|rows| rows == [DecodedWord::Return]) {
        effects.read(30, false, false);
        effects.clobbered.push(State::InstructionPointer);
        control = Control::Return;
    } else if word & 0xfc00_0000 == 0x9400_0000 {
        effects.write(30, false, false);
        effects.clobbered.push(State::InstructionPointer);
        control = Control::Call {
            field_byte_offset: offset,
        };
    } else if word == 0xd503_201f {
        // NOP is a real covered instruction, not an unclassified byte gap.
    } else if word >> 31 == 1 && word & 0x1f80_0000 == 0x1100_0000 {
        let sets_flags = word & (1 << 29) != 0;
        effects.read(source, true, false);
        effects.write(destination, !sets_flags, false);
        if sets_flags {
            effects.clobbered.push(State::Flags);
        }
    } else if matches!(
        word & 0xffc0_0000,
        0x3900_0000
            | 0x3940_0000
            | 0x7900_0000
            | 0x7940_0000
            | 0xb900_0000
            | 0xb940_0000
            | 0xf900_0000
            | 0xf940_0000
            | 0xbd00_0000
            | 0xbd40_0000
            | 0xfd00_0000
            | 0xfd40_0000
            | 0x3d80_0000
            | 0x3dc0_0000
    ) {
        let vector = word & (1 << 26) != 0;
        let load = word & (1 << 22) != 0;
        effects.read(source, true, false);
        if load {
            effects.write(destination, false, vector);
        } else {
            effects.read(destination, false, vector);
        }
    } else if word >> 31 == 1
        && matches!(word & 0x7f80_0000, 0x1280_0000 | 0x5280_0000 | 0x7280_0000)
    {
        if word & 0x7f80_0000 == 0x7280_0000 {
            effects.read(destination, false, false);
        }
        effects.write(destination, false, false);
    } else if word >> 31 == 1
        && word & 0x7f80_0000 == 0x1200_0000
        && source != 31
        && destination != 31
        && valid_logical_immediate(word)
    {
        // AND immediate used by absolute aligned homes; S is clear.
        effects.read(source, false, false);
        effects.write(destination, false, false);
    } else if word & 0xffe0_ffe0 == 0xaa00_03e0 {
        effects.read(((word >> 16) & 31) as u8, false, false);
        effects.write(destination, false, false);
    } else {
        return Err(format!(
            "unsupported generated AArch64 instruction {word:08x}"
        ));
    }
    Ok((effects, control))
}

fn valid_logical_immediate(word: u32) -> bool {
    // DecodeBitMasks rejects an absent element size and an all-ones element.
    // The encoded rotation cannot change validity once that size is known.
    let imms = (word >> 10) & 63;
    let size_encoding = ((word >> 16) & 64) | (!imms & 63);
    if size_encoding < 2 {
        return false;
    }
    let length = 31 - size_encoding.leading_zeros();
    let levels = (1 << length) - 1;
    imms & levels != levels
}

pub fn decode_aarch64_generated_entry(
    bytes: &[u8],
    body_call_field: Option<u32>,
) -> Result<GeneratedEntryEvidence, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("generated AArch64 entry has a partial instruction word".into());
    }
    let mut rows = Vec::with_capacity(bytes.len() / 4);
    for (index, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let offset = u32::try_from(index * 4).map_err(|_| "generated AArch64 entry exceeds u32")?;
        let (effects, mut control) =
            instruction(u32::from_le_bytes(*word), offset).map_err(|reason| {
                format!("generated AArch64 instruction at byte {offset}: {reason}")
            })?;
        if let Control::Call { field_byte_offset } = control
            && Some(field_byte_offset) != body_call_field
        {
            let word = u32::from_le_bytes(*word);
            let displacement = ((word & 0x03ff_ffff) << 6) as i32 >> 4;
            control = Control::LocalCall {
                target_byte_offset: i64::from(offset) + i64::from(displacement),
            };
        }
        rows.push(GeneratedEntryInstruction {
            byte_offset: offset,
            byte_count: 4,
            effects: effects.finish(),
            control,
        });
    }
    GeneratedEntryEvidence::from_target_instructions(bytes.len(), rows, body_call_field)
        .map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_link_uses_link_register_and_ip_without_inventing_stack_use() {
        let evidence =
            decode_aarch64_generated_entry(&0x9400_0000u32.to_le_bytes(), Some(0)).unwrap();
        assert!(
            evidence
                .effects
                .registers()
                .as_slice()
                .contains(&Register::Aarch64X(30))
        );
        assert!(
            evidence
                .effects
                .machine_state()
                .contains_all(MachineStateSet::new([State::InstructionPointer]))
        );
        assert!(
            !evidence
                .effects
                .machine_state()
                .contains_all(MachineStateSet::new([State::StackPointer]))
        );
    }

    #[test]
    fn reserved_and_other_instruction_classes_do_not_acquire_entry_authority() {
        for word in [0x9180_0000u32, 0x9200_fc00] {
            assert!(decode_aarch64_generated_entry(&word.to_le_bytes(), None).is_err());
        }
        // Valid AND X0,X0,#-16; ordinary stack arithmetic remains separately covered.
        assert!(decode_aarch64_generated_entry(&0x927c_ec00u32.to_le_bytes(), None).is_ok());
        let stack = decode_aarch64_generated_entry(&0xd100_43ffu32.to_le_bytes(), None).unwrap();
        assert!(
            stack
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::StackPointer]))
        );
        assert!(
            !stack
                .effects
                .clobbered_state()
                .contains_all(MachineStateSet::new([State::Flags]))
        );
        assert!(decode_aarch64_generated_entry(&[0, 0, 0], None).is_err());
    }
}
