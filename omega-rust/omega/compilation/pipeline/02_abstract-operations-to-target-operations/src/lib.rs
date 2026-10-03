#![forbid(unsafe_code)]
//! Abstract operations to target operations.
//!
//! Production enters through `lower_optimized_to_target_operations`
//! (`lowering/optimized.rs`). It takes a validated optimized abstract plan and
//! its admitted settlements, lowers it, joins the provider installation, and
//! seals the result with the settlement-aware translation validation, which
//! replays every target row independently (`validation`;
//! `validate_abstract_to_target_translation` is its settlement-free form).
//! `lower_to_target_operations` (`lowering.rs`) is the unvalidated lowering
//! core that entrance calls; tests use it directly. Beneath it,
//! `lowering/coordination.rs` binds settlements and lowers each function by
//! result family. Stage 00 owns the admitted settlement facts. The
//! `placed_view_inputs` route is the in-progress direct-entry placed-view
//! slice (TASKS.md PLAN-LAID-VIEWS).

mod lowering;
mod validation;

pub use lowering::model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError, PlacedViewInputTranslationError, SelectedPlacedViewInputPlan,
};
pub use lowering::optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use lowering::placed_view_inputs::{
    lower_to_target_operations_with_placed_view_inputs, validate_placed_view_input_translation,
};
pub use lowering::{TargetLoweringRequest, lower_to_target_operations};
pub use validation::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt, validate_abstract_to_target_translation,
};

// Receiving stages reconstruct ABI classes from their own declaration inputs,
// using the same boundary classification rather than a second type roster.
pub use validation::structural_shapes::{InvalidStructuralShape, classified_boundary_shape};

#[cfg(test)]
mod tests;
