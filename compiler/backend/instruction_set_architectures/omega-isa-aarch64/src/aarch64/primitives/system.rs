use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_branch_link_placeholder() -> [u8; 4] {
    encode_instruction(0x94000000)
}

pub(in crate::aarch64) fn encode_svc(immediate: u16) -> [u8; 4] {
    encode_instruction(0xD4000001 | (u32::from(immediate) << 5))
}
