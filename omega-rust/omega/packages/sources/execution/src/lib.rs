//! Process custody for compiler-owned package-source resolution.
//!
//! A backend freezes one absolute executable path outside package-controlled
//! roots. Prepared commands retain structured arguments, environment changes,
//! working directory, standard-stream custody, resource limits, and
//! shared platform-owned process-container cleanup. Nothing in this crate is
//! canonical package evidence.

#![deny(unsafe_op_in_unsafe_fn)]

mod backend;
mod phase;
mod request;

pub use backend::ResolverExecutionBackend;
pub use bounded_process::{
    BoundedProcessChild as ResolverExecutionChild,
    BoundedProcessCompletion as ResolverExecutionCompletion,
    BoundedProcessExitStatus as ResolverExecutionExitStatus,
    BoundedProcessPrepared as ResolverPreparedExecution,
};
pub use phase::ResolverExecutionPhase;
