#![forbid(unsafe_code)]

/*
We provide dependency-free fixed-width x86-64 machine-code sequences for
immediate port I/O (`encode_immediate_port_write` for `out dx, al` and
`encode_immediate_port_read_u8` for `in al, dx`). Both encoders materialize the
16-bit port and 8-bit payload through caller-volatile scratch registers (`r10`
and `r11`) using zero-extended `movabs` immediates before moving into `edx` and
`eax`, keeping the emitted bytewidths (`IMMEDIATE_PORT_WRITE_WIDTH = 27` and
`IMMEDIATE_PORT_READ_U8_WIDTH = 16`) constant across all operand values.
*/

/*
We isolate these immediate port-I/O byte encoders in a zero-dependency leaf
crate so that both the selected-form x86-64 encoder (`isa-x86_64`) and backend
machine-emission consumers can reference the exact same byte sequences and width
constants without coupling to higher-level compiler IRs or value-storage
representations.
*/

/*
`isa-x86_64` consumes `IMMEDIATE_PORT_WRITE_WIDTH` and
`encode_immediate_port_write` when declaring machine effects and encoding
selected `PortWrite` instructions, and our unit tests verify exact byte output
for representative port-write (PIC EOI `0x20`) and zero-extended port-read
(`0x64`) operations.
*/

pub const IMMEDIATE_PORT_WRITE_WIDTH: usize = 27;
pub const IMMEDIATE_PORT_READ_U8_WIDTH: usize = 16;

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
    use crate::{encode_immediate_port_read_u8, encode_immediate_port_write};

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
