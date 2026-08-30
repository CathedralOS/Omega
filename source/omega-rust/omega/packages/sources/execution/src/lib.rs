//! Native process enforcement for compiler-owned package-source resolution.
//!
//! The public surface is deliberately narrow: callers select a closed
//! resolution phase, provide already-verified executable and custody paths,
//! and receive opaque policy observations. Implementation lives behind named
//! modules for command construction, confinement, and lifecycle.

#![deny(unsafe_op_in_unsafe_fn)]

mod backend;
mod confinement;
mod model;
mod prepared;
mod process;
mod request;

pub use backend::ResolverExecutionBackend;
pub use model::{
    ResolverExecutionBackendIdentity, ResolverExecutionGuarantee,
    ResolverExecutionGuaranteeDisposition, ResolverExecutionGuaranteeRow, ResolverExecutionPhase,
    ResolverExecutionPolicyObservation, ResolverExecutionResourceCeilings,
};
pub use prepared::{ResolverExecutionCommandIdentity, ResolverPreparedExecution};
pub use process::{
    ResolverExecutionChild, ResolverExecutionCompletionObservation, ResolverExecutionExitStatus,
    ResolverExecutionTerminationDisposition,
};
