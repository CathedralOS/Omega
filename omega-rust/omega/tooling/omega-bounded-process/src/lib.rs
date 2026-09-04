//! Resource-bounded native child execution with owned process-tree cleanup.
//!
//! `preparation` owns opaque structured command setup, `lifecycle` owns native
//! limits and process-container closure, and `capture` owns bounded duplex I/O
//! under one wall-clock deadline. These controls bound concrete resources and
//! cleanup; they do not claim filesystem, executable, credential, or network
//! isolation.

#![deny(unsafe_op_in_unsafe_fn)]

mod capture;
mod lifecycle;
mod preparation;

pub use capture::{
    BoundedCaptureBudget, BoundedCaptureBudgetExceeded, BoundedCaptureLimits, BoundedProcessInput,
    BoundedProcessOutput, BoundedProcessRunError, BoundedProcessStream, run_bounded_process,
};
pub use lifecycle::{BoundedProcessChild, BoundedProcessCompletion, BoundedProcessExitStatus};
pub use preparation::{BoundedProcessLimits, BoundedProcessPrepared};
