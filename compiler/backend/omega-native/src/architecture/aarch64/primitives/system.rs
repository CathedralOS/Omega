use super::instruction::encode_instruction;

pub(in crate::architecture::aarch64) fn encode_branch_link_placeholder() -> Vec<u8> {
    encode_instruction(0x94000000)
}

pub(in crate::architecture::aarch64) fn encode_svc(immediate: u16) -> Vec<u8> {
    encode_instruction(0xD4000001 | (u32::from(immediate) << 5))
}
