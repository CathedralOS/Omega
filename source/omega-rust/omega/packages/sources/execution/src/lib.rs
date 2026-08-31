//! Process custody for compiler-owned package-source resolution.
//!
//! A backend freezes one absolute executable path outside package-controlled
//! roots. Prepared commands retain structured arguments, environment changes,
//! working directory, standard-stream custody, resource limits, and
//! platform-owned process-container cleanup. Nothing in this crate is canonical package
//! evidence.

#![deny(unsafe_op_in_unsafe_fn)]

mod backend;
mod phase;
mod prepared;
mod process;
mod request;

pub use backend::ResolverExecutionBackend;
pub use phase::ResolverExecutionPhase;
pub use prepared::ResolverPreparedExecution;
pub use process::{
    ResolverExecutionChild, ResolverExecutionCompletion, ResolverExecutionExitStatus,
};
