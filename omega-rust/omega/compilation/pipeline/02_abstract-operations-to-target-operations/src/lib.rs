#![forbid(unsafe_code)]
//! Abstract operations to target operations.
//!
//! Production enters through `lower_optimized_to_target_operations`
//! (`lowering/optimized.rs`). It takes a validated optimized abstract plan and
//! its admitted settlements, lowers it, joins the provider installation, and
//! retains the lowered program beside those inputs. Provider installation
//! identity is joined before publication; translation replay belongs to the
//! independent audit, not this producer's default route.
//! `lower_to_target_operations` (`lowering.rs`) is the unvalidated lowering
//! core that entrance calls; tests use it directly. Beneath it,
//! `lowering/coordination.rs` binds settlements and lowers each function by
//! result family. Stage 00 owns the admitted settlement facts. The
//! `placed_view_inputs` route is the in-progress direct-entry placed-view
//! slice (TASKS.md PLAN-LAID-VIEWS).
//!
//! Operation dispatch does not preflight borrow legality, and calls transport
//! the existing descriptor leaves of owned operands rather than reconstructing
//! their permissions from types. Storage owners still join current homes,
//! ordered pointer crossings and ABI placements; the stage 03 corpus audit
//! independently exercises custody. Reference formation, release, edge and
//! return lowering still contain local custody restrictions, so this is not
//! yet a claim that every native borrow replay has been removed. Source
//! permission belongs to Psi, not a second source checker in native lowering.

mod lowering;
mod validation;

pub use lowering::model::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    LoweringError,
};
pub use lowering::optimized::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    lower_optimized_to_target_operations,
};
pub use lowering::placed_view_inputs;
pub(crate) use lowering::{TargetLoweringRequest, lower_to_target_operations};
pub(crate) use validation::AbstractToTargetTranslationValidationError;

// Receiving stages reconstruct ABI classes from their own declaration inputs,
// using the same boundary classification rather than a second type roster.
pub use validation::structural_shapes::classified_boundary_shape;
