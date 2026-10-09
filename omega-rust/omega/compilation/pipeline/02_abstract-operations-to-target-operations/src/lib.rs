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
//!
//! Nonescaping scalar observations consume Psi's admitted access; they do not
//! form or transfer shared loans. Native lowering joins their current storage
//! and ordered reference crossings, without replaying source compatibility as
//! a loan-formation rule. New loans, moves and writes retain their distinct
//! custody requirements. This follows pipeline.md's verification placement:
//! source permission is judged by Psi, not re-proved by a native stage.

mod lowering;
mod validation;

pub use lowering::model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError,
};
pub(crate) use lowering::model::PlacedViewInputTranslationError;
pub use lowering::optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use lowering::placed_view_inputs;
pub(crate) use lowering::{TargetLoweringRequest, lower_to_target_operations};
pub(crate) use validation::{  AbstractToTargetTranslationValidationError};

// Receiving stages reconstruct ABI classes from their own declaration inputs,
// using the same boundary classification rather than a second type roster.
pub use validation::structural_shapes::{ classified_boundary_shape};
