#![forbid(unsafe_code)]

//! Checked trees to unsealed, target-neutral Psi.
//! Output retains source custody, proof and debug companions for later stages.
//! Unsupported constructs fail closed; this stage does not optimize or publish.
//! Start at [`psi_lowering`] for selection, lowering and evidence completion.

pub mod psi_lowering;
pub use psi_lowering::*;
