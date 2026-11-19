//! The exact RIP-relative indirect branch used by x86-64 import stubs.
use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence};

pub const X86_64_IMPORT_THUNK_BYTE_COUNT: usize = 6;

pub fn x86_64_import_thunk_footprint() -> StateFootprintEvidence {
    StateFootprintEvidence::new(
        RegisterSet::default(),
        MachineStateSet::new([MachineState::InstructionPointer]),
    )
}

pub fn decode_x86_64_import_thunk(bytes: &[u8]) -> Option<i32> {
    let &[0xff, 0x25, a, b, c, d] = bytes else {
        return None;
    };
    Some(i32::from_le_bytes([a, b, c, d]))
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
