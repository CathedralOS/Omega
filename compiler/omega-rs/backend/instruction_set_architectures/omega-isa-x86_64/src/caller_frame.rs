//! Compiler-private Microsoft x64 caller-frame address mechanics.
//!
//! These encoders only compute an address relative to the current RSP. They do
//! not reserve or write stack storage and do not perform a call.

use crate::x86_gpr_number;
use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

pub const fn outgoing_stack_address_load_width() -> usize {
    8
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
}
