/*
We provide canonical x86-64 instruction encodings and structural decoders for
exact IEEE 754 binary32/binary64 bit materialization into `XMM0`..`XMM15`,
`RSP`-relative `STMXCSR`/`LDMXCSR` control-state custody (`OMEGA_CANONICAL_MXCSR`),
and register-only scalar FMA3 (`VFMADD132SS`/`VFMADD132SD`) verification.
*/

use calling_conventions::MachineRegister;
use diagnostics::Diagnostic;

pub const OMEGA_CANONICAL_MXCSR: u32 = 0x0000_1f80;

pub fn encode_binary32_bits_to_xmm(
    bits: u32,
    destination: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let destination = xmm_index(destination)?;
    let mut bytes = Vec::with_capacity(if destination < 8 { 9 } else { 10 });
    bytes.push(0xb8);
    crate::codec_bytes::push_encoded(&mut bytes, bits.to_le_bytes());
    bytes.push(0x66);
    if destination >= 8 {
        bytes.push(0x44);
    }
    bytes.extend_from_slice(&[0x0f, 0x6e, 0xc0 | ((destination & 7) << 3)]);
    Ok(bytes)
}

pub fn encode_binary64_bits_to_xmm(
    bits: u64,
    destination: MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let destination = xmm_index(destination)?;
    let mut bytes = Vec::with_capacity(15);
    bytes.extend_from_slice(&[0x48, 0xb8]);
    crate::codec_bytes::push_encoded(&mut bytes, bits.to_le_bytes());
    bytes.push(0x66);
    bytes.push(if destination < 8 { 0x48 } else { 0x4c });
    bytes.extend_from_slice(&[0x0f, 0x6e, 0xc0 | ((destination & 7) << 3)]);
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
            bytes.push(reg_field | 0x04);
            bytes.push(0x24);
        }
        1..=127 => {
            bytes.push(reg_field | 0x44);
            bytes.push(0x24);
            bytes.push(byte_offset as u8);
        }
        _ => {
            let displacement = i32::try_from(byte_offset).map_err(|_| {
                Diagnostic::error("RSP-relative MXCSR frame offset exceeds positive disp32")
            })?;
            bytes.push(reg_field | 0x84);
            bytes.push(0x24);
            crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
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
    use super::{
        MachineRegister, OMEGA_CANONICAL_MXCSR, encode_binary32_bits_to_xmm,
        encode_binary64_bits_to_xmm, encode_ldmxcsr_rsp_displacement,
        encode_stmxcsr_rsp_displacement, encode_store_mxcsr_constant_rsp_displacement,
    };

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodedScalarFmaFormat {
    Binary32,
    Binary64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedVfmadd132Scalar {
    pub format: DecodedScalarFmaFormat,
    pub destination: MachineRegister,
    pub addend: MachineRegister,
    pub multiplicand: MachineRegister,
}

pub fn decode_vfmadd132_scalar(bytes: &[u8]) -> Result<DecodedVfmadd132Scalar, Diagnostic> {
    let [vex, vex_second, vex_third, opcode, mod_rm] = bytes else {
        return Err(Diagnostic::error(format!(
            "canonical scalar VFMADD132 encoding must contain exactly 5 bytes, got {}",
            bytes.len()
        )));
    };
    if *vex != 0xc4
        || (*vex_second & 0x5f) != 0x42
        || (*vex_third & 0x07) != 0x01
        || *opcode != 0x99
        || (*mod_rm & 0xc0) != 0xc0
    {
        return Err(Diagnostic::error(
            "bytes are not a canonical register-only scalar VFMADD132SS/SD encoding",
        ));
    }

    let format = if (*vex_third & 0x80) == 0 {
        DecodedScalarFmaFormat::Binary32
    } else {
        DecodedScalarFmaFormat::Binary64
    };
    let destination = ((*mod_rm >> 3) & 7) | (u8::from((*vex_second & 0x80) == 0) << 3);
    let multiplicand = (*mod_rm & 7) | (u8::from((*vex_second & 0x20) == 0) << 3);
    let addend = !(*vex_third >> 3) & 0x0f;
    Ok(DecodedVfmadd132Scalar {
        format,
        destination: MachineRegister::X86Xmm(destination),
        addend: MachineRegister::X86Xmm(addend),
        multiplicand: MachineRegister::X86Xmm(multiplicand),
    })
}

#[cfg(test)]
mod fma_tests {
    use super::{
        DecodedScalarFmaFormat, DecodedVfmadd132Scalar, MachineRegister, decode_vfmadd132_scalar,
    };

    #[test]
    fn independently_decodes_both_formats_and_extended_registers() {
        for (bytes, expected) in [
            (
                [0xc4, 0xe2, 0x71, 0x99, 0xc2],
                DecodedVfmadd132Scalar {
                    format: DecodedScalarFmaFormat::Binary32,
                    destination: MachineRegister::X86Xmm(0),
                    addend: MachineRegister::X86Xmm(1),
                    multiplicand: MachineRegister::X86Xmm(2),
                },
            ),
            (
                [0xc4, 0x42, 0xa9, 0x99, 0xcf],
                DecodedVfmadd132Scalar {
                    format: DecodedScalarFmaFormat::Binary64,
                    destination: MachineRegister::X86Xmm(9),
                    addend: MachineRegister::X86Xmm(10),
                    multiplicand: MachineRegister::X86Xmm(15),
                },
            ),
        ] {
            assert_eq!(decode_vfmadd132_scalar(&bytes).unwrap(), expected);
        }
    }

    #[test]
    fn decoder_rejects_noncanonical_or_non_132_scalar_forms() {
        let canonical = [0xc4, 0xe2, 0x71, 0x99, 0xc2];
        for (index, replacement) in [(0, 0xc5), (1, 0xa2), (2, 0x75), (3, 0x98), (4, 0x02)] {
            let mut mutated = canonical;
            mutated[index] = replacement;
            assert!(decode_vfmadd132_scalar(&mutated).is_err());
        }
        assert!(decode_vfmadd132_scalar(&canonical[..4]).is_err());
        let mut trailing = canonical.to_vec();
        trailing.push(0x90);
        assert!(decode_vfmadd132_scalar(&trailing).is_err());
    }
}
