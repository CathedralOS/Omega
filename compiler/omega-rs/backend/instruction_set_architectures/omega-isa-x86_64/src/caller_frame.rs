//! Compiler-private Microsoft x64 caller-frame address mechanics.
//!
//! These encoders cover compiler-private balanced RSP adjustment and address
//! computation. They do not write stack storage or perform a call.

use crate::x86_gpr_number;
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

pub const fn outgoing_stack_address_load_width() -> usize {
    8
}

pub const fn outgoing_stack_u64_write_width() -> usize {
    18
}

fn validate_outgoing_stack_frame_byte_count(byte_count: u32) -> Result<(), Diagnostic> {
    if byte_count < 32 {
        return Err(Diagnostic::error(
            "Microsoft x64 outgoing stack frame must retain at least 32 shadow bytes",
        ));
    }
    if byte_count > i32::MAX as u32 {
        return Err(Diagnostic::error(
            "Microsoft x64 outgoing stack frame exceeds positive disp32",
        ));
    }
    if byte_count % 16 != 8 {
        return Err(Diagnostic::error(
            "Microsoft x64 outgoing stack frame must be 8 modulo 16 bytes",
        ));
    }
    Ok(())
}

/// `sub/add rsp, imm` width: imm8 through 127, otherwise imm32.
pub fn outgoing_stack_frame_adjust_width(byte_count: u32) -> Result<usize, Diagnostic> {
    validate_outgoing_stack_frame_byte_count(byte_count)?;
    Ok(rsp_adjust_width(byte_count as usize))
}

pub fn encode_outgoing_stack_frame_reserve_bytes(byte_count: u32) -> Result<Vec<u8>, Diagnostic> {
    validate_outgoing_stack_frame_byte_count(byte_count)?;
    let mut bytes = Vec::with_capacity(rsp_adjust_width(byte_count as usize));
    append_sub_rsp(&mut bytes, byte_count as usize);
    Ok(bytes)
}

pub fn encode_outgoing_stack_frame_release_bytes(byte_count: u32) -> Result<Vec<u8>, Diagnostic> {
    validate_outgoing_stack_frame_byte_count(byte_count)?;
    let mut bytes = Vec::with_capacity(rsp_adjust_width(byte_count as usize));
    append_add_rsp(&mut bytes, byte_count as usize);
    Ok(bytes)
}

pub fn outgoing_stack_frame_adjust_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rsp])
}

pub fn outgoing_stack_frame_adjust_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags, MachineState::StackPointer])
}

pub(crate) fn rsp_adjust_width(reserve: usize) -> usize {
    if reserve <= 127 { 4 } else { 7 }
}

pub(crate) fn append_sub_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xec, reserve as u8]);
    } else {
        bytes.extend([0x48, 0x81, 0xec]);
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

pub(crate) fn append_add_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xc4, reserve as u8]);
    } else {
        bytes.extend([0x48, 0x81, 0xc4]);
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

pub(crate) fn append_store_rax_to_rsp_disp32(
    bytes: &mut Vec<u8>,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(stack_byte_offset)
        .map_err(|_| Diagnostic::error("Microsoft x64 stack offset exceeds positive disp32"))?;
    bytes.extend([0x48, 0x89, 0x84, 0x24]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

pub fn encode_outgoing_stack_u64_write_bytes(
    stack_byte_offset: u32,
    value: u64,
) -> Result<[u8; 18], Diagnostic> {
    if stack_byte_offset < 32 || stack_byte_offset % 8 != 0 {
        return Err(Diagnostic::error(
            "Microsoft x64 outgoing u64 write must be aligned beyond shadow space",
        ));
    }
    let mut bytes = Vec::with_capacity(outgoing_stack_u64_write_width());
    crate::append_mov_rax_imm64(&mut bytes, value);
    append_store_rax_to_rsp_disp32(&mut bytes, stack_byte_offset)?;
    bytes
        .try_into()
        .map_err(|_| Diagnostic::error("Microsoft x64 outgoing u64 write lost its canonical width"))
}

pub fn outgoing_stack_u64_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax])
}

pub fn outgoing_stack_u64_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::StackPointer])
}

/// Encode `lea register, [rsp + disp32]` for one Microsoft x64 positional
/// integer register.
pub fn encode_outgoing_stack_address_load_bytes(
    register: MachineRegister,
    stack_byte_offset: u32,
) -> Result<[u8; 8], Diagnostic> {
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "Microsoft x64 caller-frame address uses unsupported register {register:?}"
        ))
    })?;
    if !matches!(
        register,
        MachineRegister::X86Rax
            | MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9
    ) {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 caller-frame address uses non-positional register {register:?}"
        )));
    }
    let displacement = i32::try_from(stack_byte_offset).map_err(|_| {
        Diagnostic::error("Microsoft x64 caller-frame offset exceeds positive disp32")
    })?;
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&[
        0x48 | if register_number >= 8 { 0x04 } else { 0 },
        0x8d,
        0x84 | ((register_number & 7) << 3),
        0x24,
    ]);
    bytes[4..].copy_from_slice(&displacement.to_le_bytes());
    Ok(bytes)
}

pub fn outgoing_stack_address_load_register_writes(register: MachineRegister) -> RegisterSet {
    RegisterSet::new([register])
}

pub fn outgoing_stack_address_load_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::StackPointer])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_exact_rcx_and_rdx_caller_copy_addresses() {
        assert_eq!(
            encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rcx, 32).unwrap(),
            [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]
        );
        assert_eq!(
            encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rdx, 48).unwrap(),
            [0x48, 0x8d, 0x94, 0x24, 48, 0, 0, 0]
        );
    }

    #[test]
    fn rejects_non_positional_register_and_out_of_range_offset() {
        assert!(encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rbx, 32).is_err());
        assert!(
            encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rcx, u32::MAX).is_err()
        );
    }

    #[test]
    fn encodes_exact_balanced_seventy_two_byte_frame() {
        assert_eq!(
            encode_outgoing_stack_frame_reserve_bytes(72).unwrap(),
            [0x48, 0x83, 0xec, 0x48]
        );
        assert_eq!(
            encode_outgoing_stack_frame_release_bytes(72).unwrap(),
            [0x48, 0x83, 0xc4, 0x48]
        );
        assert_eq!(outgoing_stack_frame_adjust_width(72).unwrap(), 4);
    }

    #[test]
    fn rejects_invalid_outgoing_frame_sizes() {
        for byte_count in [0, 24, 32, 64, i32::MAX as u32 + 1] {
            assert!(outgoing_stack_frame_adjust_width(byte_count).is_err());
            assert!(encode_outgoing_stack_frame_reserve_bytes(byte_count).is_err());
            assert!(encode_outgoing_stack_frame_release_bytes(byte_count).is_err());
        }
    }

    #[test]
    fn encodes_exact_full_width_outgoing_word() {
        assert_eq!(
            encode_outgoing_stack_u64_write_bytes(32, 0xfedc_ba98_7654_3210).unwrap(),
            [
                0x48, 0xb8, 0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe, 0x48, 0x89, 0x84, 0x24,
                0x20, 0, 0, 0,
            ]
        );
        assert!(encode_outgoing_stack_u64_write_bytes(24, 1).is_err());
        assert!(encode_outgoing_stack_u64_write_bytes(33, 1).is_err());
        assert!(encode_outgoing_stack_u64_write_bytes(u32::MAX - 7, 1).is_err());
    }
}
