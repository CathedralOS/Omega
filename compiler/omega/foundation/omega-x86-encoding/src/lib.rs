#![forbid(unsafe_code)]

//! Dependency-clean x86 encodings shared by the legacy and terminal-Psi
//! machine-emission lanes.

pub const IMMEDIATE_PORT_WRITE_WIDTH: usize = 27;
pub const IMMEDIATE_PORT_READ_U8_WIDTH: usize = 16;

/// Encode one immediate `out dx, al` operation.
///
/// R10/R11 are caller-volatile scratch registers in both supported x86-64
/// native ABIs. The semantic operands remain exactly `u16` and `u8`; the
/// zero-extended `movabs` forms keep this encoding independent of compiler
/// value-storage representations.
pub fn encode_immediate_port_write(port: u16, value: u8) -> [u8; IMMEDIATE_PORT_WRITE_WIDTH] {
    let mut bytes = [0_u8; IMMEDIATE_PORT_WRITE_WIDTH];
    bytes[0..2].copy_from_slice(&[0x49, 0xba]);
    bytes[2..10].copy_from_slice(&u64::from(port).to_le_bytes());
    bytes[10..13].copy_from_slice(&[0x44, 0x89, 0xd2]);
    bytes[13..15].copy_from_slice(&[0x49, 0xbb]);
    bytes[15..23].copy_from_slice(&u64::from(value).to_le_bytes());
    bytes[23..26].copy_from_slice(&[0x44, 0x89, 0xd8]);
    bytes[26] = 0xee;
    bytes
}

/// Encode one immediate `in al, dx` whose zero-extended byte remains in the
/// native integer result register.
pub fn encode_immediate_port_read_u8(port: u16) -> [u8; IMMEDIATE_PORT_READ_U8_WIDTH] {
    let mut bytes = [0_u8; IMMEDIATE_PORT_READ_U8_WIDTH];
    bytes[0..2].copy_from_slice(&[0x49, 0xba]);
    bytes[2..10].copy_from_slice(&u64::from(port).to_le_bytes());
    bytes[10..13].copy_from_slice(&[0x44, 0x89, 0xd2]);
    bytes[13..15].copy_from_slice(&[0x31, 0xc0]);
    bytes[15] = 0xec;
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pic_eoi_has_one_exact_out_instruction() {
        assert_eq!(
            encode_immediate_port_write(0x20, 0x20),
            [
                0x49, 0xba, 0x20, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x89, 0xd2, 0x49, 0xbb, 0x20, 0, 0, 0,
                0, 0, 0, 0, 0x44, 0x89, 0xd8, 0xee,
            ]
        );
    }

    #[test]
    fn immediate_u8_read_zero_extends_the_result_register() {
        assert_eq!(
            encode_immediate_port_read_u8(0x64),
            [
                0x49, 0xba, 0x64, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x89, 0xd2, 0x31, 0xc0, 0xec,
            ]
        );
    }
}
