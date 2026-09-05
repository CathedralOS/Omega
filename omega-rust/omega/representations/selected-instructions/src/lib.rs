#![forbid(unsafe_code)]

//! Optimizer module role: crate map. The selected-instruction representation.
//!
//! Start at [`selected_instructions::SelectedInstructionPlan`]. Its subordinate
//! areas own control flow, values, instructions, calls, and target effects.

pub mod selected_instructions;
pub use selected_instructions::*;
