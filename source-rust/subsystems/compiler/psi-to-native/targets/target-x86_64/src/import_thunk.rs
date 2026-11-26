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

/// Fill the zero displacement of an encoded thunk at `thunk_address` so it
/// loads the eight-byte slot at `slot_address`.
pub fn patch_x86_64_import_thunk(
    bytes: &mut [u8],
    thunk_address: u64,
    slot_address: u64,
) -> Result<(), &'static str> {
    if decode_x86_64_import_thunk(bytes) != Some(0) {
        return Err("x86-64 import thunk is not an encoded zero placeholder");
    }
    let displacement = thunk_address
        .checked_add(X86_64_IMPORT_THUNK_BYTE_COUNT as u64)
        .and_then(|next| i64::try_from(next).ok())
        .zip(i64::try_from(slot_address).ok())
        .and_then(|(next, slot)| slot.checked_sub(next))
        .and_then(|delta| i32::try_from(delta).ok())
        .ok_or("x86-64 import thunk slot is out of disp32 range")?;
    bytes[2..].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64_IMPORT_THUNK_BYTE_COUNT, decode_x86_64_import_thunk, patch_x86_64_import_thunk,
        x86_64_import_thunk_footprint,
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
    fn patch_fills_only_a_zero_placeholder_within_disp32() {
        let mut bytes = [0xff, 0x25, 0, 0, 0, 0];
        patch_x86_64_import_thunk(&mut bytes, 0x1000, 0x3008).unwrap();
        assert_eq!(decode_x86_64_import_thunk(&bytes), Some(0x2002));
        assert!(patch_x86_64_import_thunk(&mut bytes, 0x1000, 0x3008).is_err());
        let mut bytes = [0xff, 0x25, 0, 0, 0, 0];
        assert!(patch_x86_64_import_thunk(&mut bytes, 0, 1 << 32).is_err());
        assert!(patch_x86_64_import_thunk(&mut bytes, u64::MAX, 0).is_err());
        assert_eq!(bytes, [0xff, 0x25, 0, 0, 0, 0]);
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
