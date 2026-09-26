use super::{encode_immediate_port_read_u8, encode_immediate_port_write};

#[test]
fn pic_eoi_has_one_exact_out_instruction() {
    assert_eq!(
        encode_immediate_port_write(0x20, 0x20),
        [
            0x49, 0xba, 0x20, 0, 0, 0, 0, 0, 0, 0, 0x44, 0x89, 0xd2, 0x49, 0xbb, 0x20, 0, 0, 0, 0,
            0, 0, 0, 0x44, 0x89, 0xd8, 0xee,
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
