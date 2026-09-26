//! Resource-bounded native child execution with owned process-tree cleanup.
//!
//! Start at `bounded_process.rs` for the complete run loop. `preparation` owns
//! opaque structured command setup, `process_child` owns native limits and
//! process-container closure, and `bounded_process` owns bounded duplex I/O
//! under one wall-clock deadline. These controls bound concrete resources and
//! cleanup; they do not claim filesystem, executable, credential, or network
//! isolation.

#![deny(unsafe_op_in_unsafe_fn)]

mod bounded_process;
mod preparation;
mod process_child;

pub use self::bounded_process::{
    BoundedCaptureBudget, BoundedCaptureBudgetExceeded, BoundedCaptureLimits, BoundedProcessInput,
    BoundedProcessOutput, BoundedProcessRunError, BoundedProcessStream, run_bounded_process,
};
pub use preparation::{BoundedProcessLimits, BoundedProcessPrepared};
pub use process_child::{BoundedProcessChild, BoundedProcessCompletion, BoundedProcessExitStatus};
