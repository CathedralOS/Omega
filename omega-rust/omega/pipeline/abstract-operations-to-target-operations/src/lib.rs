#![forbid(unsafe_code)]
//! Optimizer module role: crate map. Enter `lowering/mod.rs`, then descend by result family and semantic responsibility.
//!
//! `lower_to_target_operations` turns validated abstract operations into target
//! operations with call placement; `model` carries the admitted settlements the
//! lowering publishes, `optimized` the selected-optimization carriers,
//! `placed_view_inputs` the placed-view plans, and `validation` the
//! independent replay of every result.

mod lowering;
mod model;
mod optimized;
mod placed_view_inputs;
mod validation;

pub use lowering::{
    TargetLoweringRequest, lower_to_target_operations,
    lower_to_target_operations_and_native_callbacks,
};
pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedIeeeFloatFmaSettlement,
    AdmittedNativeCallbackArgument, LoweringError, PlacedViewInputTranslationError,
    SelectedPlacedViewInputPlan,
};
pub use optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use placed_view_inputs::{
    lower_to_target_operations_with_placed_view_inputs, validate_placed_view_input_translation,
};
pub use validation::*;

#[cfg(test)]
mod tests;
