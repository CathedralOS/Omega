#![forbid(unsafe_code)]

//! Canonical publication of validated, optimized Psi.
//!
//! This stage seals source-free semantics and their proof and debug companions.
//! It accepts only the output of the explicit pre-Terminal optimization stage.

#[path = "boundary_operator_custody/replay_scope.rs"]
mod boundary_operator_custody;
mod publish_artifact;

pub use boundary_operator_custody::{
    CheckedBoundaryOperatorApplicationOccurrence, CheckedBoundaryOperatorApplicationScope,
    CheckedDynamicCallLane, CheckedDynamicCallOccurrence, checked_boundary_operator_scope,
};
pub use publish_artifact::finalize_terminal_artifact;
