//! Selected 64-bit immediate materialization forms: the exact `mov r32, imm32`,
//! sign-extended `mov r64, imm32`, and `xor` zeroing encodings a selection
//! rule may substitute for a full 64-bit immediate load, each with its
//! decoder and independent validation.

pub(crate) mod mov_r32_imm32;
pub(crate) mod mov_r64_imm32_sign_extended;
pub(crate) mod xor_zero;
