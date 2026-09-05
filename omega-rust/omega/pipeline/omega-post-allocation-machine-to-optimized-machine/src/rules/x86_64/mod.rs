//! Optimizer module role: stage group. x86-64-only symbolic machine transformations.

pub mod materialize_i64_mov_r32_imm32;
pub mod materialize_i64_mov_r64_imm32_sign_extended;
pub mod materialize_i64_xor_zero;
