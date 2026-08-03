use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_branch_link_placeholder() -> [u8; 4] {
    encode_instruction(0x94000000)
}

pub(in crate::aarch64) fn encode_svc(immediate: u16) -> [u8; 4] {
    encode_instruction(0xD4000001 | (u32::from(immediate) << 5))
}

/// `BRK #imm16` — software breakpoint. Generates a synchronous exception that the
/// OS reports as a fatal signal (SIGTRAP), aborting the process. This is the
/// aarch64 counterpart of the x86_64 `ud2` used for Trapping-domain overflow.
pub(in crate::aarch64) fn encode_brk(immediate: u16) -> [u8; 4] {
    encode_instruction(0xD4200000 | (u32::from(immediate) << 5))
}

/// `MRS Xd, FPCR` / `MSR FPCR, Xn` for compiler-balanced explicit-rounding
/// envelopes. These are not exposed as source-level machine-control
/// operations.
pub(in crate::aarch64) fn encode_read_fpcr(register: u8) -> [u8; 4] {
    encode_instruction(0xD53B4400 | u32::from(register))
}

pub(in crate::aarch64) fn encode_write_fpcr(register: u8) -> [u8; 4] {
    encode_instruction(0xD51B4400 | u32::from(register))
}
