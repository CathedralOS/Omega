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

pub fn encode_stmxcsr_rsp_disp8(byte_offset: u8) -> [u8; 5] {
    [0x0f, 0xae, 0x5c, 0x24, byte_offset]
}

pub fn encode_ldmxcsr_rsp_disp8(byte_offset: u8) -> [u8; 5] {
    [0x0f, 0xae, 0x54, 0x24, byte_offset]
}

pub fn encode_store_mxcsr_constant_rsp_disp8(byte_offset: u8, value: u32) -> [u8; 8] {
    let value = value.to_le_bytes();
    [
        0xc7,
        0x44,
        0x24,
        byte_offset,
        value[0],
        value[1],
        value[2],
        value[3],
    ]
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
        assert_eq!(encode_stmxcsr_rsp_disp8(4), [0x0f, 0xae, 0x5c, 0x24, 4]);
        assert_eq!(
            encode_store_mxcsr_constant_rsp_disp8(0, OMEGA_CANONICAL_MXCSR),
            [0xc7, 0x44, 0x24, 0, 0x80, 0x1f, 0, 0]
        );
        assert_eq!(encode_ldmxcsr_rsp_disp8(0), [0x0f, 0xae, 0x54, 0x24, 0]);
    }
}
