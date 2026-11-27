/*
We provide canonical x86-64 `RSP`-relative `STMXCSR`/`LDMXCSR` encodings for
floating control-state custody (`OMEGA_CANONICAL_MXCSR`).
*/

use diagnostics::Diagnostic;

pub const OMEGA_CANONICAL_MXCSR: u32 = 0x0000_1f80;

pub fn encode_stmxcsr_rsp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_mxcsr_rsp_displacement(0x18, byte_offset)
}

pub fn encode_ldmxcsr_rsp_displacement(byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_mxcsr_rsp_displacement(0x10, byte_offset)
}

fn encode_mxcsr_rsp_displacement(reg_field: u8, byte_offset: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(2 + 2 + 4);
    bytes.extend_from_slice(&[0x0f, 0xae]);
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
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        OMEGA_CANONICAL_MXCSR, encode_ldmxcsr_rsp_displacement, encode_stmxcsr_rsp_displacement,
    };

    #[test]
    fn mxcsr_sequences_pin_nearest_gradual_masked_controls() {
        assert_eq!(OMEGA_CANONICAL_MXCSR, 0x1f80);
        assert_eq!(
            encode_stmxcsr_rsp_displacement(4).unwrap(),
            [0x0f, 0xae, 0x5c, 0x24, 4]
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
