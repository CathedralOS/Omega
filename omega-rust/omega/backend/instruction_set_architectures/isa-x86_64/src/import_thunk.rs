//! The closed import-thunk form x86-64 image emission produces:
//! `jmp qword ptr [rip+disp32]` — six bytes whose whole machine effect is a
//! single instruction-pointer write through an absolute slot the displacement
//! addresses. Image emission owns production; the PCC evidence layer replays
//! the emitted bytes against this specification rather than trusting a
//! producer-claimed form or extent.

use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence};

use crate::selected_form_encoding::decoding::{DecodedInstruction, decode_all};

/// The fixed byte extent of the closed import-thunk form.
pub const X86_64_IMPORT_THUNK_BYTE_COUNT: usize = 6;

/// The footprint the emitted thunk sequence claims: no register writes, the
/// instruction pointer alone.
pub fn x86_64_import_thunk_footprint() -> StateFootprintEvidence {
    StateFootprintEvidence::new(
        RegisterSet::default(),
        MachineStateSet::new([MachineState::InstructionPointer]),
    )
}

/// Decode one complete import thunk: `Some(displacement)` exactly when the
/// byte run is the single closed form, end to end. A byte run carrying the
/// opcode prefix plus trailing bytes, or any other emitted form, is not a
/// thunk.
pub fn decode_x86_64_import_thunk(bytes: &[u8]) -> Option<i32> {
    match decode_all(bytes) {
        Ok(rows) => match rows.as_slice() {
            [DecodedInstruction::JumpIndirectRip { displacement }] => Some(*displacement),
            _ => None,
        },
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64_IMPORT_THUNK_BYTE_COUNT, decode_x86_64_import_thunk, x86_64_import_thunk_footprint,
    };
    use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence};

    #[test]
    fn decode_returns_the_emitted_displacement() {
        let bytes = [0xff, 0x25, 0x78, 0x56, 0x34, 0x12];
        assert_eq!(bytes.len(), X86_64_IMPORT_THUNK_BYTE_COUNT);
        assert_eq!(decode_x86_64_import_thunk(&bytes), Some(0x1234_5678));
        assert_eq!(
            decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0, 0]),
            Some(0)
        );
    }

    #[test]
    fn decode_rejects_everything_but_the_closed_form() {
        // A different ModRM target is still an indirect jump but not the
        // closed thunk form.
        assert_eq!(
            decode_x86_64_import_thunk(&[0xff, 0x24, 0x24, 0, 0, 0]),
            None
        );
        assert_eq!(decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0]), None);
        assert_eq!(
            decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0, 0, 0x90]),
            None
        );
        assert_eq!(decode_x86_64_import_thunk(&[]), None);
        assert_eq!(decode_x86_64_import_thunk(&[0xc3, 0, 0, 0, 0, 0]), None);
    }

    #[test]
    fn footprint_claims_the_instruction_pointer_alone() {
        assert_eq!(
            x86_64_import_thunk_footprint(),
            StateFootprintEvidence::new(
                RegisterSet::default(),
                MachineStateSet::new([MachineState::InstructionPointer]),
            )
        );
    }
}
