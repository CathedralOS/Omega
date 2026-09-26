#![forbid(unsafe_code)]

//! Lowered Psi to the canonical Terminal Psi artifact.
//!
//! The stage operation is `finalize_terminal_artifact` (`publish_artifact`).
//! It seals source-free semantics with their proof and debug companions, and
//! accepts only the output of the explicit pre-Terminal optimization stage.
//! `boundary_operator_custody` supplies the checked boundary-operator scope
//! that publication rejoins.

mod boundary_operator_custody;
mod publish_artifact;

pub use boundary_operator_custody::{
    CheckedBoundaryOperatorApplicationOccurrence, CheckedBoundaryOperatorApplicationScope,
    CheckedDynamicCallLane, CheckedDynamicCallOccurrence, checked_boundary_operator_scope,
};
pub use publish_artifact::finalize_terminal_artifact;
