use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_move_x_register(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xAA0003E0 | (u32::from(source_register) << 16) | u32::from(destination_register),
    )
}
