#![forbid(unsafe_code)]

#[macro_use]
extern crate selected_instruction_analyses;

mod selected_rewrites;

pub use selected_rewrites::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationOutput,
    SelectedOptimizationReplay, StructuralRewriteError, StructuralRewriteValidationError,
    optimize_selected_instructions,
};
