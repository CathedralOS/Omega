pub(in crate::aarch64) fn encode_instruction(instruction: u32) -> [u8; 4] {
    instruction.to_le_bytes()
}
