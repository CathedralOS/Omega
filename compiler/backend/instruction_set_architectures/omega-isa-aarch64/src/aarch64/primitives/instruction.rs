pub(in crate::aarch64) fn encode_instruction(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}
