//! The exact page-relative indirect branch used by AArch64 import stubs.
//!
//! Mach-O and PE images both reach an imported function through a pointer slot
//! the loader fills: `adrp x16, slot@page; ldr x16, [x16, slot@pageoff]; br x16`.
//! Each image format places the slot; this module owns the instruction bytes.
use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};

pub const AARCH64_IMPORT_THUNK_BYTE_COUNT: usize = 12;

const ADRP_X16: u32 = 0x9000_0010;
const ADRP_IMMEDIATE_MASK: u32 = 0x60ff_ffe0;
const LDR_X16_FROM_X16: u32 = 0xf940_0210;
const LDR_IMMEDIATE_MASK: u32 = 0x003f_fc00;
const BR_X16: u32 = 0xd61f_0200;

/// The thunk writes x16 and transfers control.
pub fn aarch64_import_thunk_footprint() -> StateFootprintEvidence {
    StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::Aarch64X(16)]),
        MachineStateSet::new([MachineState::InstructionPointer]),
    )
}

fn words(bytes: &[u8]) -> Option<[u32; 3]> {
    if bytes.len() != AARCH64_IMPORT_THUNK_BYTE_COUNT {
        return None;
    }
    let word = |index: usize| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("four-byte instruction"),
        )
    };
    Some([word(0), word(1), word(2)])
}

/// True when `bytes` is the thunk with any page and page-offset immediates.
pub fn decode_aarch64_import_thunk(bytes: &[u8]) -> bool {
    words(bytes).is_some_and(|[adrp, ldr, br]| {
        adrp & !ADRP_IMMEDIATE_MASK == ADRP_X16
            && ldr & !LDR_IMMEDIATE_MASK == LDR_X16_FROM_X16
            && br == BR_X16
    })
}

/// Fill the zero immediates of an encoded thunk at `thunk_address` so it loads
/// the eight-byte slot at `slot_address`.
pub fn patch_aarch64_import_thunk(
    bytes: &mut [u8],
    thunk_address: u64,
    slot_address: u64,
) -> Result<(), &'static str> {
    let [adrp, ldr, br] = words(bytes).ok_or("AArch64 import thunk has the wrong length")?;
    if adrp != ADRP_X16 || ldr != LDR_X16_FROM_X16 || br != BR_X16 {
        return Err("AArch64 import thunk is not an encoded zero placeholder");
    }
    let page_delta = ((slot_address & !0xfff) as i64 - (thunk_address & !0xfff) as i64) / 4096;
    if !(-(1 << 20)..(1 << 20)).contains(&page_delta) {
        return Err("AArch64 import thunk ADRP is out of range");
    }
    let page_offset = slot_address & 0xfff;
    if !page_offset.is_multiple_of(8) {
        return Err("AArch64 import thunk slot is not 8-byte aligned");
    }
    let immediate = (page_delta as u32) & 0x1f_ffff;
    let adrp = adrp | ((immediate & 0b11) << 29) | (((immediate >> 2) & 0x7ffff) << 5);
    let ldr = ldr | (((page_offset / 8) as u32) << 10);
    bytes[0..4].copy_from_slice(&adrp.to_le_bytes());
    bytes[4..8].copy_from_slice(&ldr.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholder() -> Vec<u8> {
        [ADRP_X16, LDR_X16_FROM_X16, BR_X16]
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect()
    }

    #[test]
    fn patch_encodes_the_slot_page_and_scaled_offset() {
        let mut bytes = placeholder();
        // Thunk at 0x1_4000_1ff0, slot two pages later at offset 0x38.
        patch_aarch64_import_thunk(&mut bytes, 0x1_4000_1ff0, 0x1_4000_3038).unwrap();
        let adrp = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let ldr = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        // adrp x16, #2 pages: immlo = 2, immhi = 0.
        assert_eq!(adrp, 0xd000_0010);
        // ldr x16, [x16, #0x38]: imm12 = 7.
        assert_eq!(ldr, 0xf940_1e10);
        assert!(decode_aarch64_import_thunk(&bytes));
    }

    #[test]
    fn patch_rejects_misaligned_slots_and_patched_input() {
        let mut bytes = placeholder();
        assert!(patch_aarch64_import_thunk(&mut bytes, 0x1000, 0x2004).is_err());
        patch_aarch64_import_thunk(&mut bytes, 0x1000, 0x3008).unwrap();
        assert!(patch_aarch64_import_thunk(&mut bytes, 0x1000, 0x3008).is_err());
    }

    #[test]
    fn decode_rejects_other_instructions() {
        assert!(decode_aarch64_import_thunk(&placeholder()));
        let mut other = placeholder();
        other[8..12].copy_from_slice(&0xd65f_03c0u32.to_le_bytes());
        assert!(!decode_aarch64_import_thunk(&other));
        assert!(!decode_aarch64_import_thunk(&other[..8]));
    }
}
