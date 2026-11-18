//! Target semantics, register effects and size declarations for selected forms.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingRegister;

pub const fn address_word_count(amount: u64) -> u64 {
    let pages = (amount >> 12).div_ceil(4095);
    if amount & 4095 != 0 || amount == 0 {
        pages + 1
    } else {
        pages
    }
}
pub const MAX_ADDRESS_BYTES: u16 = (address_word_count(u32::MAX as u64) * 4) as u16;
pub const MAX_FPCR_SEQUENCE_BYTES: u16 =
    ((address_word_count(u32::MAX as u64 & !4095) + 2) * 4) as u16;
pub mod atomic;
pub mod atomic_fence;
pub mod crash;
pub mod float_result_policy;
pub mod floating_control;
pub mod hosted_exit_process;
pub mod hosted_read_byte;
pub mod hosted_write_byte;
pub mod ieee_float_arithmetic;
pub mod memory_fence;
pub mod register_access;
pub mod table_call;
pub mod trapping_forms;

pub const MAX_STORE64_BYTES: u16 = ((address_word_count(u32::MAX as u64 & !4095) + 1) * 4) as u16;
