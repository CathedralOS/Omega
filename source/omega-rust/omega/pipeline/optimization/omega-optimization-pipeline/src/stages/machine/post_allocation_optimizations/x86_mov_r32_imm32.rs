//! Pipeline custody for zero-extended `MOV r32, imm32` materialization.
//!
//! [`model`] owns the typed stage carrier; [`stage`] joins direct and
//! selected-lowering inputs to the independently validated machine rule.

#[path = "x86_mov_r32_imm32/model.rs"]
mod model;
#[path = "x86_mov_r32_imm32/stage.rs"]
mod stage;

pub use model::*;
pub use stage::*;
