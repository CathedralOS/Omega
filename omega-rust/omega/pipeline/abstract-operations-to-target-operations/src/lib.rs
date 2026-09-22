#![forbid(unsafe_code)]
//! Optimizer module role: crate map. Start at `lowering.rs`, then descend by result family and semantic responsibility.
//!
//! `lower_to_target_operations` turns validated abstract operations into target
//! operations with call placement; beneath it `model` carries the admitted
//! settlements the lowering publishes, `optimized` the selected-optimization
//! carriers and `placed_view_inputs` the placed-view plans. `validation` is
//! the independent replay of every result.

mod lowering;
mod validation;

pub use lowering::model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedIeeeFloatFmaSettlement,
    AdmittedNativeCallbackArgument, LoweringError, PlacedViewInputTranslationError,
    SelectedPlacedViewInputPlan,
};
pub use lowering::optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use lowering::placed_view_inputs::{
    lower_to_target_operations_with_placed_view_inputs, validate_placed_view_input_translation,
};
pub use lowering::{TargetLoweringRequest, lower_to_target_operations};
pub use validation::*;

#[cfg(test)]
mod tests;
