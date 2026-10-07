//! Canonical AArch64 FPCR custody sequences.
//!
//! Returning foreign calls use target-owned call-clobbered scratch registers
//! to move the complete FPCR through an aligned function-frame slot: `x9`
//! carries the FPCR value. A slot inside the scaled twelve-bit reach of
//! `str`/`ldr` (32760 bytes) is addressed from `sp` directly. Past it the
//! address is formed in shifted page steps and the access carries the low
//! twelve bits: a restore forms the address in `x9` itself before loading
//! into it, while a save, whose value already occupies `x9`, forms it in IP0
//! (`x16`). These helpers do not establish Omega's canonical controls; they
//! preserve the caller's exact control state around an opaque foreign leaf.

use diagnostics::Diagnostic;

use crate::immediate_offset::{ADD_IMMEDIATE, address_word_count, append_page_steps};

const SCRATCH_REGISTER: u32 = 9;
/// IP0, the save's address register past the direct reach.
pub(crate) const SAVE_ADDRESS_REGISTER: u8 = 16;
const MRS_FPCR_X9: u32 = 0xd53b_4400 | SCRATCH_REGISTER;
const MSR_FPCR_X9: u32 = 0xd51b_4400 | SCRATCH_REGISTER;
const STR_X9: u32 = 0xf900_0000 | SCRATCH_REGISTER;
const LDR_X9: u32 = 0xf940_0000 | SCRATCH_REGISTER;

/// Largest byte size of either sequence over any `u32` slot offset: the
/// page steps of the last page-aligned offset, the access, and the system
/// register move.
pub(crate) const MAX_FPCR_SEQUENCE_BYTES: u16 =
    ((address_word_count(u32::MAX as u64 & !4095) + 2) * 4) as u16;

/// Save the complete FPCR into an eight-byte stack slot.
pub fn encode_save_fpcr_to_sp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut words = vec![MRS_FPCR_X9];
    words.extend(stack_access(STR_X9, SAVE_ADDRESS_REGISTER, byte_offset)?);
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// Restore the complete FPCR from an eight-byte stack slot.
pub fn encode_restore_fpcr_from_sp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut words = stack_access(LDR_X9, SCRATCH_REGISTER as u8, byte_offset)?;
    words.push(MSR_FPCR_X9);
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// The `x9` access to `[sp + byte_offset]`, preceded by the page steps into
/// `address` when the offset lies past the scaled immediate reach.
fn stack_access(access: u32, address: u8, byte_offset: u32) -> Result<Vec<u32>, Diagnostic> {
    if !byte_offset.is_multiple_of(8) {
        return Err(Diagnostic::error(format!(
            "AArch64 FPCR stack displacement `{byte_offset}` is not eight-byte aligned"
        )));
    }
    if byte_offset / 8 <= 0xfff {
        return Ok(vec![access | ((byte_offset / 8) << 10) | (31 << 5)]);
    }
    let mut words = Vec::new();
    append_page_steps(
        &mut words,
        ADD_IMMEDIATE,
        address,
        31,
        u64::from(byte_offset >> 12),
    );
    words.push(access | (((byte_offset & 4095) / 8) << 10) | (u32::from(address) << 5));
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::{encode_restore_fpcr_from_sp_displacement, encode_save_fpcr_to_sp_displacement};

    #[test]
    fn fpcr_save_and_restore_use_exact_system_register_and_stack_words() {
        assert_eq!(
            encode_save_fpcr_to_sp_displacement(24).unwrap(),
            [0x09, 0x44, 0x3b, 0xd5, 0xe9, 0x0f, 0x00, 0xf9]
        );
        assert_eq!(
            encode_restore_fpcr_from_sp_displacement(24).unwrap(),
            [0xe9, 0x0f, 0x40, 0xf9, 0x09, 0x44, 0x1b, 0xd5]
        );
    }

    #[test]
    fn fpcr_stack_displacement_is_aligned_and_reaches_past_the_immediate() {
        assert!(encode_save_fpcr_to_sp_displacement(0).is_ok());
        assert!(encode_restore_fpcr_from_sp_displacement(32_760).is_ok());
        assert!(encode_save_fpcr_to_sp_displacement(4).is_err());
        // 65544 = 16 pages + 8: the save addresses through x16 and keeps the
        // FPCR in x9; the restore addresses through x9 before loading it.
        let words = |bytes: Vec<u8>| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            words(encode_save_fpcr_to_sp_displacement(65_544).unwrap()),
            [0xd53b_4409, 0x9140_43f0, 0xf900_0609]
        );
        assert_eq!(
            words(encode_restore_fpcr_from_sp_displacement(65_544).unwrap()),
            [0x9140_43e9, 0xf940_0529, 0xd51b_4409]
        );
        assert_eq!(
            encode_restore_fpcr_from_sp_displacement(u32::MAX - 7)
                .unwrap()
                .len(),
            usize::from(super::MAX_FPCR_SEQUENCE_BYTES)
        );
    }
}
