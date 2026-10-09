//! Selected 64-bit immediate materialization error surface: the failure
//! vocabulary a selection rule reports when substituting a `mov r32, imm32`
//! or sign-extended `mov r64, imm32` encoding for a full 64-bit immediate
//! load.

pub(crate) mod mov_r32_imm32;
pub(crate) mod mov_r64_imm32_sign_extended;
