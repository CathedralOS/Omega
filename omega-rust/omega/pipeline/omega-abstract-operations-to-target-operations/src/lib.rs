#![forbid(unsafe_code)]
//! Optimizer module role: crate map. Enter `lowering/mod.rs`, then descend by result family and semantic responsibility.
mod lowering;
mod model;
mod placed_view_inputs;
mod validation;

pub use lowering::{
    lower_ranked_to_target_operations, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma,
    lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks,
};
pub use model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedIeeeFloatFmaSettlement,
    AdmittedNativeCallbackArgument, LoweringError, PlacedViewInputTranslationError,
    SelectedPlacedViewInputPlan,
};
pub use placed_view_inputs::{
    lower_to_target_operations_with_placed_view_inputs, validate_placed_view_input_translation,
};
pub use validation::*;

#[cfg(test)]
mod tests;
