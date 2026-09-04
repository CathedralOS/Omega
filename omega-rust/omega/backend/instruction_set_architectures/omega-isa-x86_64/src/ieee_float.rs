//! Canonical x86-64 support encodings for exact IEEE scalar materialization
//! and MXCSR custody.
//!
//! These are instruction mechanics only. Feature admission and the semantic
//! reason for installing Omega's floating-control state remain upstream.

use omega_calling_conventions::MachineRegister;
use psi_diagnostics::Diagnostic;

pub const OMEGA_CANONICAL_MXCSR: u32 = 0x0000_1f80;

pub fn encode_binary32_bits_to_xmm(
    bits: u32,
    destination: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let destination = xmm_index(destination)?;
    let mut bytes = Vec::with_capacity(if destination < 8 { 9 } else { 10 });
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.push(0x66);
    if destination >= 8 {
        bytes.push(0x44); // REX.R
    }
    bytes.extend_from_slice(&[0x0f, 0x6e, 0xc0 | ((destination & 7) << 3)]); // movd xmm, eax
    Ok(bytes)
}

pub fn encode_binary64_bits_to_xmm(
    bits: u64,
    destination: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let destination = xmm_index(destination)?;
    let mut bytes = Vec::with_capacity(15);
    bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.push(0x66);
    bytes.push(if destination < 8 { 0x48 } else { 0x4c }); // REX.W[+R]
    bytes.extend_from_slice(&[0x0f, 0x6e, 0xc0 | ((destination & 7) << 3)]); // movq xmm, rax
    Ok(bytes)
}

pub fn encode_stmxcsr_rsp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_mxcsr_rsp_displacement(0x18, byte_offset, &[])
}

pub fn encode_ldmxcsr_rsp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_mxcsr_rsp_displacement(0x10, byte_offset, &[])
}

pub fn encode_store_mxcsr_constant_rsp_displacement(
    byte_offset: u32,
    value: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_rsp_displacement(&[0xc7], 0x00, byte_offset, &value.to_le_bytes())
}

fn encode_mxcsr_rsp_displacement(
    reg_field: u8,
    byte_offset: u32,
    suffix: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    encode_rsp_displacement(&[0x0f, 0xae], reg_field, byte_offset, suffix)
}

/// Encode one RSP-relative memory operand using the shortest canonical
/// nonnegative displacement. x86 `disp8` is signed, so offsets 128 and above
/// must use `disp32`; treating a `u8` offset as raw `disp8` would address below
/// RSP for half of its range.
fn encode_rsp_displacement(
    opcode: &[u8],
    reg_field: u8,
    byte_offset: u32,
    suffix: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(opcode.len() + 2 + 4 + suffix.len());
    bytes.extend_from_slice(opcode);
    match byte_offset {
        0 => {
            bytes.push(reg_field | 0x04); // mod=00, r/m=SIB
            bytes.push(0x24); // scale=1, no index, base=RSP
        }
        1..=127 => {
            bytes.push(reg_field | 0x44); // mod=01, r/m=SIB
            bytes.push(0x24);
            bytes.push(byte_offset as u8);
        }
        _ => {
            let displacement = i32::try_from(byte_offset).map_err(|_| {
                Diagnostic::error("RSP-relative MXCSR frame offset exceeds positive disp32")
            })?;
            bytes.push(reg_field | 0x84); // mod=10, r/m=SIB
            bytes.push(0x24);
            bytes.extend_from_slice(&displacement.to_le_bytes());
        }
    }
    bytes.extend_from_slice(suffix);
    Ok(bytes)
}

fn xmm_index(register: MachineRegister) -> Result<u8, Diagnostic> {
    match register {
        MachineRegister::X86Xmm(index @ 0..=15) => Ok(index),
        _ => Err(Diagnostic::error(
            "exact IEEE scalar materialization requires XMM0..XMM15",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_raw_bits_materialize_without_host_float_conversion() {
        assert_eq!(
            encode_binary32_bits_to_xmm(0x8000_0000, MachineRegister::X86Xmm(0)).unwrap(),
            [0xb8, 0, 0, 0, 0x80, 0x66, 0x0f, 0x6e, 0xc0]
        );
        assert_eq!(
            encode_binary64_bits_to_xmm(0x7ff8_0000_0000_0042, MachineRegister::X86Xmm(2),)
                .unwrap(),
            [
                0x48, 0xb8, 0x42, 0, 0, 0, 0, 0, 0xf8, 0x7f, 0x66, 0x48, 0x0f, 0x6e, 0xd0,
            ]
        );
    }

    #[test]
    fn mxcsr_sequences_pin_nearest_gradual_masked_controls() {
        assert_eq!(OMEGA_CANONICAL_MXCSR, 0x1f80);
        assert_eq!(
            encode_stmxcsr_rsp_displacement(4).unwrap(),
            [0x0f, 0xae, 0x5c, 0x24, 4]
        );
        assert_eq!(
            encode_store_mxcsr_constant_rsp_displacement(0, OMEGA_CANONICAL_MXCSR).unwrap(),
            [0xc7, 0x04, 0x24, 0x80, 0x1f, 0, 0]
        );
        assert_eq!(
            encode_ldmxcsr_rsp_displacement(0).unwrap(),
            [0x0f, 0xae, 0x14, 0x24]
        );
    }

    #[test]
    fn mxcsr_rsp_offsets_use_signed_disp8_then_disp32() {
        assert_eq!(
            encode_stmxcsr_rsp_displacement(127).unwrap(),
            [0x0f, 0xae, 0x5c, 0x24, 127]
        );
        assert_eq!(
            encode_stmxcsr_rsp_displacement(128).unwrap(),
            [0x0f, 0xae, 0x9c, 0x24, 128, 0, 0, 0]
        );
        assert_eq!(
            encode_ldmxcsr_rsp_displacement(0x1234).unwrap(),
            [0x0f, 0xae, 0x94, 0x24, 0x34, 0x12, 0, 0]
        );
        assert!(encode_stmxcsr_rsp_displacement(0x8000_0000).is_err());
    }
}
