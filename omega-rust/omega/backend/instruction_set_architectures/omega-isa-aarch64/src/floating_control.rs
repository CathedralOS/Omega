//! Canonical AArch64 FPCR custody sequences.
//!
//! Returning foreign calls use one target-owned call-clobbered scratch
//! register (`x9`) to move the complete FPCR through an aligned function-frame
//! slot. These helpers do not establish Omega's canonical controls; they
//! preserve the caller's exact control state around an opaque foreign leaf.

use psi_diagnostics::Diagnostic;

const SCRATCH_REGISTER: u32 = 9;
const MRS_FPCR_X9: u32 = 0xd53b_4400 | SCRATCH_REGISTER;
const MSR_FPCR_X9: u32 = 0xd51b_4400 | SCRATCH_REGISTER;
const STR_X9_SP: u32 = 0xf900_03e0 | SCRATCH_REGISTER;
const LDR_X9_SP: u32 = 0xf940_03e0 | SCRATCH_REGISTER;

/// Save the complete FPCR into an eight-byte stack slot.
pub fn encode_save_fpcr_to_sp_displacement(byte_offset: u32) -> Result<[u8; 8], Diagnostic> {
    let store = stack_access(STR_X9_SP, byte_offset)?;
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&MRS_FPCR_X9.to_le_bytes());
    bytes[4..].copy_from_slice(&store.to_le_bytes());
    Ok(bytes)
}

/// Restore the complete FPCR from an eight-byte stack slot.
pub fn encode_restore_fpcr_from_sp_displacement(byte_offset: u32) -> Result<[u8; 8], Diagnostic> {
    let load = stack_access(LDR_X9_SP, byte_offset)?;
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&load.to_le_bytes());
    bytes[4..].copy_from_slice(&MSR_FPCR_X9.to_le_bytes());
    Ok(bytes)
}

fn stack_access(base: u32, byte_offset: u32) -> Result<u32, Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 0xfff {
        return Err(Diagnostic::error(format!(
            "AArch64 FPCR stack displacement `{byte_offset}` is not an encodable aligned uimm12"
        )));
    }
    Ok(base | ((byte_offset / 8) << 10))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn fpcr_stack_displacement_is_aligned_and_bounded() {
        assert!(encode_save_fpcr_to_sp_displacement(0).is_ok());
        assert!(encode_restore_fpcr_from_sp_displacement(32_760).is_ok());
        assert!(encode_save_fpcr_to_sp_displacement(4).is_err());
        assert!(encode_restore_fpcr_from_sp_displacement(32_768).is_err());
    }
}
